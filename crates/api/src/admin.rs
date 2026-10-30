//! Admin console API: overview metrics, job queue control, schedules, catalogue reload, live SSE stream.
//! Access: users.is_admin. `ADMIN_USERNAMES` (comma-separated) sets exactly who is admin at startup.

use std::convert::Infallible;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post, put};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::PgPool;

use crate::{ApiError, AppState, account};

const KINDS: &[&str] = &["incremental", "details", "refresh", "enrich", "sweep", "localize"];

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

pub async fn apply_admin_env(pool: &PgPool) -> anyhow::Result<()> {
    if let Ok(list) = std::env::var("ADMIN_USERNAMES") {
        let names: Vec<String> = list.split(',').map(|s| s.trim().to_lowercase()).filter(|s| !s.is_empty()).collect();
        sqlx::query("update users set is_admin = lower(username) = any($1)").bind(&names).execute(pool).await?;
    }
    Ok(())
}

fn err(status: StatusCode, msg: &str) -> Response {
    (status, Json(json!({ "ok": false, "error": msg }))).into_response()
}

/// Ok(Ok(username)) for admins; Ok(Err(response)) otherwise.
async fn require_admin(s: &AppState, headers: &HeaderMap) -> Result<Result<String, Response>, ApiError> {
    let Some(u) = account::current_user(&s.pool, headers).await? else { return Ok(Err(err(StatusCode::UNAUTHORIZED, "Sign in required"))) };
    let row: (bool, String) = sqlx::query_as("select is_admin, username from users where id = $1").bind(u.id).fetch_one(&s.pool).await?;
    Ok(if row.0 { Ok(row.1) } else { Err(err(StatusCode::FORBIDDEN, "Admins only")) })
}

macro_rules! admin {
    ($s:expr, $h:expr) => {
        match require_admin(&$s, &$h).await? {
            Ok(name) => name,
            Err(r) => return Ok(r),
        }
    };
}

async fn overview_json(s: &AppState) -> Result<Value, ApiError> {
    let data: Value = sqlx::query_scalar(
        "select jsonb_build_object(
            'items', count(*),
            'listed', count(*) filter (where status = 'ok'),
            'delisted', count(*) filter (where status = 'delisted'),
            'needs_enrichment', count(*) filter (where needs_enrichment),
            'published_24h', count(*) filter (where published_at > now() - interval '24 hours'),
            'refreshed_24h', count(*) filter (where scraped_at > now() - interval '24 hours'),
            'stalest_scrape', min(scraped_at) filter (where status = 'ok'),
            'missing_images', count(*) filter (where status = 'ok' and not exists (select 1 from booth_images b where b.item_id = items.id)),
            'users', (select count(*) from users),
            'favourites', (select count(*) from favourites),
            'saved_searches', (select count(*) from saved_searches),
            'db_bytes', pg_database_size(current_database()))
         from items",
    )
    .fetch_one(&s.pool)
    .await?;
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
    admin!(s, headers);
    Ok(Json(overview_json(&s).await?).into_response())
}

/// Server-sent events: the overview every 2 s (the worker's progress writes are throttled to the same cadence).
async fn stream(State(s): State<Arc<AppState>>, headers: HeaderMap) -> Result<Response, ApiError> {
    admin!(s, headers);
    let events = futures_util::stream::unfold((s, true), |(s, first)| async move {
        if !first {
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
        let ev = match overview_json(&s).await {
            Ok(v) => Event::default().event("overview").data(v.to_string()),
            Err(e) => Event::default().event("error").data(format!("{:#}", e.0)),
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
    admin!(s, headers);
    let jobs: Value = sqlx::query_scalar(
        "select coalesce(jsonb_agg(to_jsonb(j) order by j.id desc), '[]') from (select id, kind, status, params, done, total, message, error, created_by, created_at, started_at, finished_at from jobs order by id desc limit 100) j",
    )
    .fetch_one(&s.pool)
    .await?;
    let schedules: Value = sqlx::query_scalar("select coalesce(jsonb_agg(to_jsonb(s) order by s.kind), '[]') from job_schedules s").fetch_one(&s.pool).await?;
    Ok(Json(json!({ "jobs": jobs, "schedules": schedules })).into_response())
}

async fn get_job(State(s): State<Arc<AppState>>, headers: HeaderMap, Path(id): Path<i64>) -> Result<Response, ApiError> {
    admin!(s, headers);
    let job: Option<Value> = sqlx::query_scalar("select to_jsonb(j) from jobs j where id = $1").bind(id).fetch_optional(&s.pool).await?;
    Ok(match job {
        Some(j) => Json(j).into_response(),
        None => err(StatusCode::NOT_FOUND, "No such job"),
    })
}

#[derive(Deserialize)]
struct NewJob {
    kind: String,
    #[serde(default)]
    params: Value,
}

fn valid_params(p: &Value) -> bool {
    (p.is_null() || p.is_object()) && p.to_string().len() <= 2000
}

async fn create_job(State(s): State<Arc<AppState>>, headers: HeaderMap, Json(b): Json<NewJob>) -> Result<Response, ApiError> {
    let who = admin!(s, headers);
    if !KINDS.contains(&b.kind.as_str()) || !valid_params(&b.params) {
        return Ok(err(StatusCode::BAD_REQUEST, "Unknown kind or invalid params"));
    }
    let params = if b.params.is_null() { json!({}) } else { b.params };
    let id: Option<i64> = sqlx::query_scalar("insert into jobs (kind, params, created_by) values ($1, $2, $3) on conflict do nothing returning id")
        .bind(&b.kind)
        .bind(&params)
        .bind(format!("admin:{who}"))
        .fetch_optional(&s.pool)
        .await?;
    Ok(match id {
        Some(id) => Json(json!({ "id": id })).into_response(),
        None => err(StatusCode::CONFLICT, "A job of this kind is already queued or running"),
    })
}

async fn cancel_job(State(s): State<Arc<AppState>>, headers: HeaderMap, Path(id): Path<i64>) -> Result<Response, ApiError> {
    admin!(s, headers);
    // queued jobs are dropped at once; running ones stop at their next progress check
    let status: Option<String> = sqlx::query_scalar(
        "update jobs set status = case when status = 'queued' then 'cancelled' else 'cancelling' end,
                         finished_at = case when status = 'queued' then now() else finished_at end
         where id = $1 and status in ('queued', 'running') returning status",
    )
    .bind(id)
    .fetch_optional(&s.pool)
    .await?;
    Ok(match status {
        Some(st) => Json(json!({ "status": st })).into_response(),
        None => err(StatusCode::CONFLICT, "Job is not queued or running"),
    })
}

#[derive(Deserialize)]
struct ScheduleUpdate {
    enabled: Option<bool>,
    interval_minutes: Option<i32>,
    params: Option<Value>,
    /// run as soon as possible
    run_now: Option<bool>,
}

async fn update_schedule(State(s): State<Arc<AppState>>, headers: HeaderMap, Path(kind): Path<String>, Json(b): Json<ScheduleUpdate>) -> Result<Response, ApiError> {
    admin!(s, headers);
    if b.interval_minutes.is_some_and(|m| !(5..=10080).contains(&m)) || b.params.as_ref().is_some_and(|p| !p.is_object() || !valid_params(p)) {
        return Ok(err(StatusCode::BAD_REQUEST, "Interval must be 5–10080 minutes; params must be an object"));
    }
    let row: Option<Value> = sqlx::query_scalar(
        "update job_schedules set enabled = coalesce($2, enabled), interval_minutes = coalesce($3, interval_minutes),
                params = coalesce($4, params), next_run_at = case when $5 then now() else next_run_at end
         where kind = $1 returning to_jsonb(job_schedules)",
    )
    .bind(&kind)
    .bind(b.enabled)
    .bind(b.interval_minutes)
    .bind(&b.params)
    .bind(b.run_now.unwrap_or(false))
    .fetch_optional(&s.pool)
    .await?;
    Ok(match row {
        Some(r) => Json(r).into_response(),
        None => err(StatusCode::NOT_FOUND, "No such schedule"),
    })
}

async fn reload(State(s): State<Arc<AppState>>, headers: HeaderMap) -> Result<Response, ApiError> {
    admin!(s, headers);
    // bump so the reloader rebuilds even if nothing changed
    sqlx::query("update catalog_meta set version = version + 1, changed_at = now()").execute(&s.pool).await?;
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
}

fn key_hint(k: &str) -> String {
    let n = k.chars().count();
    if n <= 8 { "••••".into() } else { format!("{}…{}", k.chars().take(3).collect::<String>(), k.chars().skip(n - 4).collect::<String>()) }
}

use catalogue_core::settings::{self, AiConfig};

fn public_ai(c: &AiConfig) -> Value {
    json!({
        "base_url": c.base_url, "model": c.model,
        "api_key_set": c.api_key.is_some(), "api_key_hint": c.api_key.as_deref().map(key_hint),
        // fields set by AI_* environment variables can't be edited in the console
        "from_env": { "base_url": c.base_url_from_env, "model": c.model_from_env, "api_key": c.api_key_from_env },
    })
}

async fn get_settings(State(s): State<Arc<AppState>>, headers: HeaderMap) -> Result<Response, ApiError> {
    admin!(s, headers);
    Ok(Json(json!({ "ai": public_ai(&settings::ai(&s.pool).await?) })).into_response())
}

async fn put_ai(State(s): State<Arc<AppState>>, headers: HeaderMap, Json(b): Json<AiSettings>) -> Result<Response, ApiError> {
    admin!(s, headers);
    let url_ok = b.base_url.as_deref().is_none_or(|u| u.len() <= 300 && (u.starts_with("https://") || u.starts_with("http://")));
    if !url_ok || b.model.as_deref().is_some_and(|m| m.len() > 100) || b.api_key.as_deref().is_some_and(|k| k.len() > 500) {
        return Ok(err(StatusCode::BAD_REQUEST, "Base URL must start with http(s)://; model ≤ 100 chars; key ≤ 500 chars"));
    }
    let mut v = settings::stored_ai(&s.pool).await?;
    if let Some(u) = b.base_url {
        v["base_url"] = json!(u.trim().trim_end_matches('/'));
    }
    if let Some(m) = b.model {
        v["model"] = json!(m.trim());
    }
    if let Some(k) = b.api_key.filter(|k| !k.trim().is_empty()) {
        v["api_key"] = json!(k.trim());
    }
    sqlx::query("insert into app_settings (key, value) values ('ai', $1) on conflict (key) do update set value = excluded.value, updated_at = now()")
        .bind(&v)
        .execute(&s.pool)
        .await?;
    let config = settings::resolve(&v, |k| std::env::var(k).ok());
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

/// Calls `GET {base_url}/models` with the stored key, so the admin can check the endpoint before enabling enrichment.
async fn test_ai(State(s): State<Arc<AppState>>, headers: HeaderMap) -> Result<Response, ApiError> {
    admin!(s, headers);
    let c = settings::ai(&s.pool).await?;
    let (Some(base), Some(key)) = (c.base_url.as_deref(), c.api_key.as_deref()) else {
        return Ok(err(StatusCode::BAD_REQUEST, "Set the base URL and API key first"));
    };
    let res = s.http.get(format!("{base}/models")).bearer_auth(key).timeout(Duration::from_secs(10)).send().await;
    Ok(match res {
        Ok(r) if r.status().is_success() => {
            let body: Value = r.json().await.unwrap_or(Value::Null);
            let models: Vec<&str> = body["data"].as_array().into_iter().flatten().filter_map(|m| m["id"].as_str()).take(50).collect();
            let model = c.model.as_deref().unwrap_or_default();
            Json(json!({ "ok": true, "models": models, "model_found": model.is_empty() || models.is_empty() || models.contains(&model) })).into_response()
        }
        Ok(r) => err(StatusCode::BAD_GATEWAY, &format!("Provider answered {}", r.status())),
        Err(e) => err(StatusCode::BAD_GATEWAY, &format!("Could not reach provider: {e}")),
    })
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

/// Sets a random password for a user and signs them out everywhere. The password is returned once, to the admin,
/// who passes it on; it is never stored in plain text.
async fn reset_password(State(s): State<Arc<AppState>>, headers: HeaderMap, Json(b): Json<ResetPassword>) -> Result<Response, ApiError> {
    let who = admin!(s, headers);
    let password = random_password()?;
    let hash = account::hash_password(password.clone()).await?;
    let mut tx = s.pool.begin().await?;
    let user: Option<(i64, String)> = sqlx::query_as("update users set password_hash = $2 where lower(username) = lower($1) returning id, username")
        .bind(b.username.trim())
        .bind(hash)
        .fetch_optional(&mut *tx)
        .await?;
    let Some((id, username)) = user else { return Ok(err(StatusCode::NOT_FOUND, "No such user")) };
    let sessions = sqlx::query("delete from sessions where user_id = $1").bind(id).execute(&mut *tx).await?.rows_affected();
    tx.commit().await?;
    println!("admin {who} reset the password of {username}");
    Ok(Json(json!({ "username": username, "password": password, "sessions_revoked": sessions })).into_response())
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/admin/users/reset-password", post(reset_password))
        .route("/api/admin/settings", get(get_settings))
        .route("/api/admin/settings/ai", put(put_ai))
        .route("/api/admin/settings/ai/test", post(test_ai))
        .route("/api/admin/overview", get(overview))
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

    #[test]
    fn random_passwords_are_long_and_distinct() {
        let a = random_password().unwrap();
        let b = random_password().unwrap();
        assert_eq!(a.chars().count(), 16);
        assert_ne!(a, b);
        assert!(a.chars().all(|c| c.is_ascii_alphanumeric() && !"0O1lI".contains(c)));
    }
}
