//! Admin console API: overview metrics and daily trends, job queue control, schedules, catalogue reload, live SSE stream,
//! user management (admin/users.rs) and site settings (AI provider, open registration).
//! Access: users.is_admin. `ADMIN_USERNAMES` (comma-separated) sets exactly who is admin at startup.

use std::convert::Infallible;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::extract::{Path, State};
use axum::http::HeaderMap;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post, put};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::PgPool;

use crate::error::Refusal;
use crate::{ApiError, AppState, account};

/// sysinfo needs state between refreshes to compute CPU usage
pub struct SysMetrics(Mutex<Option<(sysinfo::System, sysinfo::Disks)>>);

impl SysMetrics {
    pub fn new() -> Self {
        Self(Mutex::new(sysinfo::System::new().ok().zip(sysinfo::Disks::new_with_refreshed_list().ok())))
    }

    fn snapshot(&self) -> Value {
        let mut guard = self.0.lock().unwrap();
        let Some((sys, disks)) = guard.as_mut() else { return Value::Null };
        sys.refresh_memory();
        sys.refresh_cpu_usage();
        let pid = sysinfo::get_current_pid().ok();
        if let Some(pid) = pid {
            sys.refresh_processes_specifics(sysinfo::ProcessesToUpdate::Some(&[pid]), true, sysinfo::ProcessRefreshKind::nothing().with_memory().with_cpu());
        }
        let proc = pid.and_then(|p| sys.process(p));
        disks.refresh(true);
        let cwd = std::env::current_dir().unwrap_or_default();
        // the disk holding the working directory (longest matching mount point)
        let disk = disks.iter().filter(|d| cwd.starts_with(d.mount_point())).max_by_key(|d| d.mount_point().as_os_str().len());
        json!({
            "cpu_percent": sys.global_cpu_usage(),
            "cpu_count": sys.cpus().len(),
            "mem_total": sys.total_memory(),
            "mem_used": sys.used_memory(),
            "load_avg": sysinfo::System::load_average().ok().map(|l| l.one),
            "api_rss": proc.map(|p| p.memory()),
            "api_cpu_percent": proc.map(|p| p.cpu_usage()),
            "disk_total": disk.map(|d| d.total_space()),
            "disk_free": disk.map(|d| d.available_space()),
            "disk_mount": disk.map(|d| d.mount_point().display().to_string()),
        })
    }
}

/// Lowercased `ADMIN_USERNAMES`; None when the variable isn't set (admin flags are then left as they are).
pub fn admin_names() -> Option<Vec<String>> {
    let list = std::env::var("ADMIN_USERNAMES").ok()?;
    Some(list.split(',').map(|s| s.trim().to_lowercase()).filter(|s| !s.is_empty()).collect())
}

/// Makes exactly the accounts named in `names` admin, and returns the listed names that no account has.
pub async fn apply_admin_names(pool: &PgPool, names: Option<&[String]>) -> anyhow::Result<Vec<String>> {
    let Some(names) = names else { return Ok(Vec::new()) };
    sqlx::query("update users set is_admin = lower(username) = any($1)").bind(names).execute(pool).await?;
    let missing = sqlx::query_scalar("select n from unnest($1::text[]) n where not exists (select 1 from users where lower(username) = n)")
        .bind(names)
        .fetch_all(pool)
        .await?;
    Ok(missing)
}

/// The admin's username; every handler of `routes()` starts here.
async fn require_admin(s: &AppState, headers: &HeaderMap) -> Result<String, ApiError> {
    let Some(u) = account::current_user(&s.pool, headers).await? else { return Err(Refusal::SignInRequired.into()) };
    let row: (bool, String) = sqlx::query_as("select is_admin, username from users where id = $1").bind(u.id).fetch_one(&s.pool).await?;
    if row.0 { Ok(row.1) } else { Err(Refusal::AdminsOnly.into()) }
}

#[cfg(test)]
mod authz;
mod trends;
mod users;

async fn data_json(pool: &PgPool) -> Result<Value, sqlx::Error> {
    // booth_images.item_id is unique: one join checks image coverage without per-item index probes.
    sqlx::query_scalar(
        "select jsonb_build_object(
            'items', count(*),
            'listed', count(*) filter (where status = 'ok'),
            'delisted', count(*) filter (where status = 'delisted'),
            'needs_enrichment', count(*) filter (where needs_enrichment),
            'published_24h', count(*) filter (where published_at > now() - interval '24 hours'),
            'refreshed_24h', count(*) filter (where scraped_at > now() - interval '24 hours'),
            'stalest_scrape', min(scraped_at) filter (where status = 'ok'),
            'missing_images', count(*) filter (where status = 'ok' and b.item_id is null),
            'users', (select count(*) from users),
            'favourites', (select count(*) from favourites),
            'saved_searches', (select count(*) from saved_searches),
            'db_bytes', pg_database_size(current_database()))
         from items left join booth_images b on b.item_id = items.id",
    )
    .fetch_one(pool)
    .await
}

async fn overview_json(s: &AppState) -> anyhow::Result<Value> {
    let data = data_json(&s.pool).await?;
    let worker: Value = sqlx::query_scalar(
        "select to_jsonb(w) || jsonb_build_object('alive', coalesce(heartbeat_at > now() - interval '20 seconds', false)) from worker_status w",
    )
    .fetch_one(&s.pool)
    .await?;
    let live: Value = sqlx::query_scalar(
        "select coalesce(jsonb_agg(to_jsonb(j) order by j.created_at), '[]') from (select id, kind, status, params, done, total, message, error, created_by, created_at, started_at, finished_at from jobs where status in ('queued', 'running', 'cancelling')) j",
    )
    .fetch_one(&s.pool)
    .await?;
    let db_version: i64 = sqlx::query_scalar("select version from catalog_meta").fetch_one(&s.pool).await?;
    let snap = s.snap();
    let imgproxy = match &s.imgproxy_url {
        Some(url) => match s.http.get(format!("{url}/stats")).timeout(Duration::from_secs(1)).send().await {
            Ok(r) => r.json::<Value>().await.unwrap_or(Value::Null),
            Err(_) => json!({ "reachable": false }),
        },
        None => Value::Null,
    };
    Ok(json!({
        "now": chrono::Utc::now(),
        "data": data,
        "catalog": {
            "version": snap.version, "db_version": db_version, "docs": snap.catalog.len(),
            "bitmap_bytes": snap.catalog.bitmap_bytes(), "loaded_at": snap.loaded_at, "build_secs": snap.build_secs,
        },
        "worker": worker,
        "live_jobs": live,
        "system": s.sys.snapshot(),
        "imgproxy": imgproxy,
    }))
}

async fn overview(State(s): State<Arc<AppState>>, headers: HeaderMap) -> Result<Response, ApiError> {
    require_admin(&s, &headers).await?;
    Ok(Json(overview_json(&s).await?).into_response())
}

/// Server-sent events: the overview every 2 s (the worker's progress writes are throttled to the same cadence).
async fn stream(State(s): State<Arc<AppState>>, headers: HeaderMap) -> Result<Response, ApiError> {
    require_admin(&s, &headers).await?;
    let events = futures_util::stream::unfold((s, true), |(s, first)| async move {
        if !first {
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
        let ev = match overview_json(&s).await {
            Ok(v) => Event::default().event("overview").data(v.to_string()),
            Err(e) => Event::default().event("error").data(format!("{e:#}")),
        };
        Some((Ok::<_, Infallible>(ev), (s, false)))
    });
    // no-transform stops intermediaries (Next.js dev proxy, CDNs) from gzipping and therefore buffering the stream
    Ok((
        [(axum::http::header::CACHE_CONTROL, "no-cache, no-transform"), (axum::http::HeaderName::from_static("x-accel-buffering"), "no")],
        Sse::new(events).keep_alive(KeepAlive::default()),
    )
        .into_response())
}

async fn list_jobs(State(s): State<Arc<AppState>>, headers: HeaderMap) -> Result<Response, ApiError> {
    require_admin(&s, &headers).await?;
    let jobs: Value = sqlx::query_scalar(
        "select coalesce(jsonb_agg(to_jsonb(j) order by j.id desc), '[]') from (select id, kind, status, params, done, total, message, error, created_by, created_at, started_at, finished_at from jobs order by id desc limit 100) j",
    )
    .fetch_one(&s.pool)
    .await?;
    let schedules: Value = sqlx::query_scalar("select coalesce(jsonb_agg(to_jsonb(s) order by s.kind), '[]') from job_schedules s").fetch_one(&s.pool).await?;
    Ok(Json(json!({ "jobs": jobs, "schedules": schedules })).into_response())
}

async fn get_job(State(s): State<Arc<AppState>>, headers: HeaderMap, Path(id): Path<i64>) -> Result<Response, ApiError> {
    require_admin(&s, &headers).await?;
    let job: Option<Value> = sqlx::query_scalar("select to_jsonb(j) from jobs j where id = $1").bind(id).fetch_optional(&s.pool).await?;
    match job {
        Some(j) => Ok(Json(j).into_response()),
        None => Err(Refusal::NoSuchJob.into()),
    }
}

#[derive(Deserialize)]
struct NewJob {
    kind: String,
    #[serde(default)]
    params: Value,
}

fn valid_params(p: &Value) -> bool {
    (p.is_null() || p.is_object()) && p.to_string().len() <= 2000 && json_storable(p)
}

fn json_storable(v: &Value) -> bool {
    match v {
        Value::String(s) => crate::error::storable(s),
        Value::Array(a) => a.iter().all(json_storable),
        Value::Object(o) => o.iter().all(|(k, v)| crate::error::storable(k) && json_storable(v)),
        _ => true,
    }
}

async fn create_job(State(s): State<Arc<AppState>>, headers: HeaderMap, Json(b): Json<NewJob>) -> Result<Response, ApiError> {
    let who = require_admin(&s, &headers).await?;
    if boothdex_core::jobs::JobKind::parse(&b.kind).is_none() || !valid_params(&b.params) {
        return Err(Refusal::BadJob.into());
    }
    let params = if b.params.is_null() { json!({}) } else { b.params };
    let id: Option<i64> = sqlx::query_scalar("insert into jobs (kind, params, created_by) values ($1, $2, $3) on conflict do nothing returning id")
        .bind(&b.kind)
        .bind(&params)
        .bind(format!("admin:{who}"))
        .fetch_optional(&s.pool)
        .await?;
    match id {
        Some(id) => Ok(Json(json!({ "id": id })).into_response()),
        None => Err(Refusal::JobAlreadyActive.into()),
    }
}

async fn cancel_job(State(s): State<Arc<AppState>>, headers: HeaderMap, Path(id): Path<i64>) -> Result<Response, ApiError> {
    require_admin(&s, &headers).await?;
    // queued jobs are dropped at once; running ones stop at their next progress check
    let status: Option<String> = sqlx::query_scalar(
        "update jobs set status = case when status = 'queued' then 'cancelled' else 'cancelling' end,
                         finished_at = case when status = 'queued' then now() else finished_at end
         where id = $1 and status in ('queued', 'running') returning status",
    )
    .bind(id)
    .fetch_optional(&s.pool)
    .await?;
    match status {
        Some(st) => Ok(Json(json!({ "status": st })).into_response()),
        None => Err(Refusal::JobNotActive.into()),
    }
}

#[derive(Deserialize)]
struct ScheduleUpdate {
    enabled: Option<bool>,
    interval_minutes: Option<i32>,
    params: Option<Value>,
}

async fn update_schedule(State(s): State<Arc<AppState>>, headers: HeaderMap, Path(kind): Path<String>, Json(b): Json<ScheduleUpdate>) -> Result<Response, ApiError> {
    require_admin(&s, &headers).await?;
    if boothdex_core::jobs::JobKind::parse(&kind).is_none() {
        return Err(Refusal::NoSuchSchedule.into());
    }
    if b.interval_minutes.is_some_and(|m| !(5..=10080).contains(&m)) || b.params.as_ref().is_some_and(|p| !p.is_object() || !valid_params(p)) {
        return Err(Refusal::BadSchedule.into());
    }
    let row: Option<Value> = sqlx::query_scalar(
        "update job_schedules set enabled = coalesce($2, enabled), interval_minutes = coalesce($3, interval_minutes),
                params = coalesce($4, params)
         where kind = $1 returning to_jsonb(job_schedules)",
    )
    .bind(&kind)
    .bind(b.enabled)
    .bind(b.interval_minutes)
    .bind(&b.params)
    .fetch_optional(&s.pool)
    .await?;
    match row {
        Some(r) => Ok(Json(r).into_response()),
        None => Err(Refusal::NoSuchSchedule.into()),
    }
}

async fn reload(State(s): State<Arc<AppState>>, headers: HeaderMap) -> Result<Response, ApiError> {
    require_admin(&s, &headers).await?;
    // bump so the reloader rebuilds even if nothing changed
    boothdex_core::bump_catalog_version(&s.pool).await?;
    s.reload.notify_one();
    Ok(Json(json!({ "ok": true })).into_response())
}

/// AI provider for enrichment (OpenAI-compatible). Stored in app_settings; the key is write-only from the UI.
#[derive(Deserialize)]
struct AiSettings {
    base_url: Option<String>,
    model: Option<String>,
    /// empty or missing keeps the stored key
    api_key: Option<String>,
    reasoning_effort: Option<String>,
    /// empty clears it (semantic search off)
    embed_model: Option<String>,
}

fn key_hint(k: &str) -> String {
    let n = k.chars().count();
    if n <= 8 { "••••".into() } else { format!("{}…{}", k.chars().take(3).collect::<String>(), k.chars().skip(n - 4).collect::<String>()) }
}

use boothdex_core::settings::{self, AiConfig};

fn public_ai(c: &AiConfig) -> Value {
    json!({
        "base_url": c.base_url, "model": c.model, "reasoning_effort": c.reasoning_effort, "embed_model": c.embed_model,
        "api_key_set": c.api_key.is_some(), "api_key_hint": c.api_key.as_deref().map(key_hint),
        // fields set by AI_* environment variables can't be edited in the console
        "from_env": { "base_url": c.base_url_from_env, "model": c.model_from_env, "api_key": c.api_key_from_env,
                      "reasoning_effort": c.reasoning_effort_from_env, "embed_model": c.embed_model_from_env },
    })
}

async fn get_settings(State(s): State<Arc<AppState>>, headers: HeaderMap) -> Result<Response, ApiError> {
    require_admin(&s, &headers).await?;
    Ok(Json(json!({
        "ai": public_ai(&settings::ai(&s.pool).await?),
        "registration": { "open": account::registration_open(&s.pool).await? },
    }))
    .into_response())
}

#[derive(Deserialize)]
struct Registration {
    open: bool,
}

async fn put_registration(State(s): State<Arc<AppState>>, headers: HeaderMap, Json(b): Json<Registration>) -> Result<Response, ApiError> {
    let who = require_admin(&s, &headers).await?;
    sqlx::query("insert into app_settings (key, value) values ('registration', $1) on conflict (key) do update set value = excluded.value, updated_at = now()")
        .bind(json!({ "open": b.open }))
        .execute(&s.pool)
        .await?;
    println!("admin {who} {} registration", if b.open { "opened" } else { "closed" });
    Ok(Json(json!({ "open": b.open })).into_response())
}

async fn put_ai(State(s): State<Arc<AppState>>, headers: HeaderMap, Json(b): Json<AiSettings>) -> Result<Response, ApiError> {
    require_admin(&s, &headers).await?;
    let url_ok = b.base_url.as_deref().is_none_or(|u| u.len() <= 300 && (u.starts_with("https://") || u.starts_with("http://")));
    let effort = b.reasoning_effort.as_deref().map(settings::reasoning_effort);
    let storable = [&b.base_url, &b.model, &b.embed_model, &b.api_key].iter().all(|f| f.as_deref().is_none_or(crate::error::storable));
    if !url_ok || !storable || [&b.model, &b.embed_model].iter().any(|m| m.as_deref().is_some_and(|m| m.len() > 100)) || b.api_key.as_deref().is_some_and(|k| k.len() > 500) || effort == Some(None) {
        return Err(Refusal::BadAiSettings.into());
    }
    let _guard = s.ai_save.lock().await;
    let mut v = settings::stored_ai(&s.pool).await?;
    if let Some(u) = b.base_url {
        v["base_url"] = json!(u.trim().trim_end_matches('/'));
    }
    if let Some(m) = b.model {
        v["model"] = json!(m.trim());
    }
    if let Some(m) = b.embed_model {
        v["embed_model"] = json!(m.trim());
    }
    if let Some(Some(e)) = effort {
        v["reasoning_effort"] = json!(e);
    }
    if let Some(k) = b.api_key.filter(|k| !k.trim().is_empty()) {
        v["api_key"] = json!(k.trim());
    }
    sqlx::query("insert into app_settings (key, value) values ('ai', $1) on conflict (key) do update set value = excluded.value, updated_at = now()")
        .bind(&v)
        .execute(&s.pool)
        .await?;
    let config = settings::resolve(&v, |k| std::env::var(k).ok());
    s.ai.store(Some(Arc::new(config.clone())));
    queue_localize_if_needed(&s.pool, &config).await?;
    Ok(Json(public_ai(&config)).into_response())
}

/// Once the provider is fully configured, translate the taxonomy into Chinese if that hasn't happened yet.
/// The worker picks the job up within seconds; it is cheap (a few hundred short labels).
pub async fn queue_localize_if_needed(pool: &PgPool, c: &AiConfig) -> Result<(), sqlx::Error> {
    if c.base_url.is_none() || c.model.is_none() || c.api_key.is_none() {
        return Ok(());
    }
    sqlx::query(
        "insert into jobs (kind, params, created_by) select 'localize', '{}', 'ai-settings'
         where exists (
           select 1 from smart_categories where name_en is not null and name_zh is null
           union all
           select 1 from smart_categories where group_en is not null and group_zh is null
           union all
           select 1 from smart_tags where display is not null and display_zh is null
         )
         on conflict do nothing",
    )
    .execute(pool)
    .await?;
    Ok(())
}

/// Calls `GET {base_url}/models` with the stored key, so the admin can check the endpoint before enabling enrichment;
/// with an embeddings model set, also embeds one word to check that model (access, 1024-dimension support).
async fn test_ai(State(s): State<Arc<AppState>>, headers: HeaderMap) -> Result<Response, ApiError> {
    require_admin(&s, &headers).await?;
    let c = settings::ai(&s.pool).await?;
    let (Some(base), Some(key)) = (c.base_url.as_deref(), c.api_key.as_deref()) else {
        return Err(Refusal::AiNotConfigured.into());
    };
    let res = s.http.get(format!("{base}/models")).bearer_auth(key).timeout(Duration::from_secs(10)).send().await;
    match res {
        Ok(r) if r.status().is_success() => {
            let body: Value = r.json().await.unwrap_or(Value::Null);
            let models: Vec<&str> = body["data"].as_array().into_iter().flatten().filter_map(|m| m["id"].as_str()).take(50).collect();
            let model = c.model.as_deref().unwrap_or_default();
            let embed_error = match c.embed_model.as_deref() {
                Some(m) => boothdex_core::embed::embed(&s.http, base, key, m, &["test".into()], 1).await.err().map(|e| format!("{e:#}")),
                None => None,
            };
            Ok(Json(json!({ "ok": true, "models": models, "model_found": model.is_empty() || models.is_empty() || models.contains(&model), "embed_error": embed_error }))
                .into_response())
        }
        Ok(r) => Err(Refusal::ProviderFailed.with(format!("Provider answered {}", r.status()))),
        Err(e) => Err(Refusal::ProviderFailed.with(format!("Could not reach provider: {e}"))),
    }
}

#[derive(Deserialize)]
struct ResetPassword {
    username: String,
}

/// 16 characters from an alphabet without look-alikes (no 0/O, 1/l/I), ~92 bits of entropy.
fn random_password() -> anyhow::Result<String> {
    const ALPHABET: &[u8] = b"abcdefghijkmnpqrstuvwxyzABCDEFGHJKLMNPQRSTUVWXYZ23456789";
    let mut raw = [0u8; 16];
    getrandom::fill(&mut raw).map_err(|e| anyhow::anyhow!("rng: {e}"))?;
    // modulo bias over 56 symbols from a byte is < 0.5% per symbol; acceptable for a one-time password
    Ok(raw.iter().map(|b| ALPHABET[*b as usize % ALPHABET.len()] as char).collect())
}

/// Sets a random password for a user, signs them out everywhere and lifts their sign-in lockout. The password is
/// returned once, to the admin, who passes it on; it is never stored in plain text.
async fn reset_password(State(s): State<Arc<AppState>>, headers: HeaderMap, Json(b): Json<ResetPassword>) -> Result<Response, ApiError> {
    let who = require_admin(&s, &headers).await?;
    // no stored name holds a NUL
    if !crate::error::storable(&b.username) {
        return Err(Refusal::NoSuchUser.into());
    }
    let password = random_password()?;
    let hash = account::hash_password(password.clone()).await?;
    let mut tx = s.pool.begin().await?;
    let user: Option<(i64, String)> = sqlx::query_as("update users set password_hash = $2 where lower(username) = lower($1) returning id, username")
        .bind(b.username.trim())
        .bind(hash)
        .fetch_optional(&mut *tx)
        .await?;
    let Some((id, username)) = user else { return Err(Refusal::NoSuchUser.into()) };
    let sessions = sqlx::query("delete from sessions where user_id = $1").bind(id).execute(&mut *tx).await?.rows_affected();
    tx.commit().await?;
    s.throttle.clear(&username.to_lowercase());
    println!("admin {who} reset the password of {username}");
    Ok(Json(json!({ "username": username, "password": password, "sessions_revoked": sessions })).into_response())
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/admin/users", get(users::list))
        .route("/api/admin/users/{id}", get(users::get).patch(users::patch))
        .route("/api/admin/users/{id}/sessions/{session}", axum::routing::delete(users::remove_session))
        .route("/api/admin/users/reset-password", post(reset_password))
        .route("/api/admin/settings", get(get_settings))
        .route("/api/admin/settings/registration", put(put_registration))
        .route("/api/admin/settings/ai", put(put_ai))
        .route("/api/admin/settings/ai/test", post(test_ai))
        .route("/api/admin/overview", get(overview))
        .route("/api/admin/trends", get(trends::trends))
        .route("/api/admin/stream", get(stream))
        .route("/api/admin/jobs", get(list_jobs).post(create_job))
        .route("/api/admin/jobs/{id}", get(get_job))
        .route("/api/admin/jobs/{id}/cancel", post(cancel_job))
        .route("/api/admin/schedules/{kind}", put(update_schedule))
        .route("/api/admin/reload", post(reload))
}

#[cfg(test)]
mod tests {
    use super::random_password;

    /// PostgreSQL text and jsonb cannot hold a NUL: each admin input refuses one before the statement would fail.
    #[sqlx::test(migrations = "../core/migrations")]
    #[ignore = "requires PostgreSQL with pgvector"]
    async fn a_nul_in_admin_input_is_refused_not_failed(pool: sqlx::PgPool) {
        use axum::extract::{Path, Query, State};
        use axum::http::{HeaderMap, StatusCode, header};
        use axum::response::{IntoResponse, Response};
        use axum::Json;
        use serde_json::json;

        use crate::{ApiError, account};

        let boss: i64 = sqlx::query_scalar("insert into users (username, email, display_name, password_hash, is_admin) values ('boss', 'boss@example.test', 'Boss', 'x', true) returning id")
            .fetch_one(&pool)
            .await
            .unwrap();
        sqlx::query("insert into sessions (token_hash, user_id, expires_at) values ($1, $2, now() + interval '1 day')")
            .bind(account::token_hash("tok-boss"))
            .bind(boss)
            .execute(&pool)
            .await
            .unwrap();
        let state = account::test_state(pool.clone());
        let mut h = HeaderMap::new();
        h.insert(header::COOKIE, "session=tok-boss".parse().unwrap());
        let status = |r: Result<Response, ApiError>| r.into_response().status();

        let found = super::users::list(State(state.clone()), h.clone(), Query(serde_json::from_value(json!({ "q": "bo\0" })).unwrap())).await.ok().unwrap();
        let body: serde_json::Value = serde_json::from_slice(&axum::body::to_bytes(found.into_body(), 1 << 20).await.unwrap()).unwrap();
        assert_eq!(body, json!({ "total": 0, "users": [] }));
        let job = super::NewJob { kind: "embed".into(), params: json!({ "note": "a\0" }) };
        assert_eq!(status(super::create_job(State(state.clone()), h.clone(), Json(job)).await), StatusCode::BAD_REQUEST);
        let schedule = |kind: &str, params: serde_json::Value| {
            let update = super::ScheduleUpdate { enabled: None, interval_minutes: None, params: Some(params) };
            super::update_schedule(State(state.clone()), h.clone(), Path(kind.into()), Json(update))
        };
        assert_eq!(status(schedule("refresh\0", json!({})).await), StatusCode::NOT_FOUND);
        assert_eq!(status(schedule("refresh", json!({ "a\0": 1 })).await), StatusCode::BAD_REQUEST);
        let reset = super::reset_password(State(state.clone()), h.clone(), Json(super::ResetPassword { username: "bo\0ss".into() }));
        assert_eq!(status(reset.await), StatusCode::NOT_FOUND);
        let ai = serde_json::from_value(json!({ "api_key": "sk-secret\0" })).unwrap();
        assert_eq!(status(super::put_ai(State(state.clone()), h.clone(), Json(ai)).await), StatusCode::BAD_REQUEST);
        assert_eq!(sqlx::query_scalar::<_, i64>("select count(*) from jobs").fetch_one(&pool).await.unwrap(), 0);
    }

    #[sqlx::test(migrations = "../core/migrations")]
    #[ignore = "requires PostgreSQL with pgvector"]
    async fn overview_counts_items_once_and_only_listed_items_missing_images(pool: sqlx::PgPool) {
        let empty = super::data_json(&pool).await.unwrap();
        for key in ["items", "listed", "delisted", "needs_enrichment", "published_24h", "refreshed_24h", "missing_images"] {
            assert_eq!(empty[key], 0, "empty {key}");
        }
        assert!(empty["stalest_scrape"].is_null());
        sqlx::raw_sql(
            "insert into items (id, title, author_name, author_uri, is_adult, status, scraped_at, published_at, needs_enrichment) values
               (1, 'with images', 'c', 'https://example.test', false, 'ok', now(), now(), true),
               (2, 'without images', 'c', 'https://example.test', false, 'ok', now() - interval '2 days', now() - interval '2 days', false),
               (3, 'empty gallery', 'c', 'https://example.test', false, 'ok', now(), null, false),
               (4, 'delisted without images', 'c', 'https://example.test', false, 'delisted', now(), null, true),
               (5, 'delisted with images', 'c', 'https://example.test', false, 'delisted', now(), null, false);
             insert into booth_images (item_id, images, complete) values (1, '{a,b,c}', true), (3, '{}', false), (5, '{d}', true), (999, '{orphan}', true);",
        ).execute(&pool).await.unwrap();
        let data = super::data_json(&pool).await.unwrap();
        for (key, count) in [
            ("items", 5), ("listed", 3), ("delisted", 2), ("needs_enrichment", 2),
            ("published_24h", 1), ("refreshed_24h", 4), ("missing_images", 1),
            ("users", 0), ("favourites", 0), ("saved_searches", 0),
        ] {
            assert_eq!(data[key], count, "{key}");
        }
        let stalest: chrono::DateTime<chrono::Utc> = sqlx::query_scalar("select min(scraped_at) from items where status = 'ok'")
            .fetch_one(&pool).await.unwrap();
        assert_eq!(data["stalest_scrape"].as_str().unwrap().parse::<chrono::DateTime<chrono::Utc>>().unwrap(), stalest);
    }

    #[test]
    fn random_passwords_are_long_and_distinct() {
        let a = random_password().unwrap();
        let b = random_password().unwrap();
        assert_eq!(a.chars().count(), 16);
        assert_ne!(a, b);
        assert!(a.chars().all(|c| c.is_ascii_alphanumeric() && !"0O1lI".contains(c)));
    }

    /// What the console refuses a visitor, and its job buttons and forms, with: the status and the message.
    #[sqlx::test(migrations = "../core/migrations")]
    #[ignore = "requires PostgreSQL with pgvector"]
    async fn console_refusals_keep_their_status_and_message(pool: sqlx::PgPool) {
        use super::*;

        async fn refusal(r: Result<Response, ApiError>) -> (u16, String, String) {
            let r = r.into_response();
            let status = r.status().as_u16();
            let body: Value = serde_json::from_slice(&axum::body::to_bytes(r.into_body(), 1 << 20).await.unwrap()).unwrap();
            (status, body["code"].as_str().unwrap().to_owned(), body["error"].as_str().unwrap().to_owned())
        }

        let boss: i64 = sqlx::query_scalar("insert into users (username, email, display_name, password_hash, is_admin) values ('boss', 'boss@example.test', 'Boss', 'x', true) returning id")
            .fetch_one(&pool)
            .await
            .unwrap();
        sqlx::query("insert into sessions (token_hash, user_id, expires_at) values ($1, $2, now() + interval '1 day')")
            .bind(account::token_hash("tok-boss"))
            .bind(boss)
            .execute(&pool)
            .await
            .unwrap();
        let state = account::test_state(pool);
        let mut headers = HeaderMap::new();
        headers.insert(axum::http::header::COOKIE, "session=tok-boss".parse().unwrap());
        let job = |kind: &str| create_job(State(state.clone()), headers.clone(), Json(NewJob { kind: kind.into(), params: Value::Null }));
        let schedule = |kind: &str, interval_minutes: Option<i32>| {
            update_schedule(State(state.clone()), headers.clone(), Path(kind.into()), Json(ScheduleUpdate { enabled: None, interval_minutes, params: None }))
        };

        // without a session, in the JSON shape of every refusal
        let visitor = overview(State(state.clone()), HeaderMap::new()).await.into_response();
        assert_eq!(visitor.status().as_u16(), 401);
        let body: Value = serde_json::from_slice(&axum::body::to_bytes(visitor.into_body(), 1 << 20).await.unwrap()).unwrap();
        assert_eq!(body, json!({ "ok": false, "code": "sign_in_required", "error": "Sign in required" }));

        assert_eq!(refusal(get_job(State(state.clone()), headers.clone(), Path(0)).await).await, (404, "no_such_job".into(), "No such job".into()));
        assert_eq!(refusal(job("x").await).await, (400, "bad_job".into(), "Unknown kind or invalid params".into()));
        assert!(job("refresh").await.is_ok());
        assert_eq!(refusal(job("refresh").await).await, (409, "job_already_active".into(), "A job of this kind is already queued or running".into()));
        assert_eq!(refusal(cancel_job(State(state.clone()), headers.clone(), Path(0)).await).await, (409, "job_not_active".into(), "Job is not queued or running".into()));
        assert_eq!(refusal(schedule("nope", None).await).await, (404, "no_such_schedule".into(), "No such schedule".into()));
        assert_eq!(refusal(schedule("refresh", Some(1)).await).await, (400, "bad_schedule".into(), "Interval must be 5–10080 minutes; params must be an object".into()));
        let ai = AiSettings { base_url: Some("ftp://example.test".into()), model: None, api_key: None, reasoning_effort: None, embed_model: None };
        assert_eq!(
            refusal(put_ai(State(state.clone()), headers.clone(), Json(ai)).await).await,
            (400, "bad_ai_settings".into(), "Base URL must start with http(s)://; model ≤ 100 chars; key ≤ 500 chars; reasoning effort: none, minimal, low, medium, high or xhigh".into())
        );
    }
}
