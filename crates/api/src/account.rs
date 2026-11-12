//! Password accounts, cookie sessions (each listed and signed out on its own), preferences, favourites and saved
//! searches. An admin can disable an account (admin/users.rs) or close registration (app_settings 'registration').
//! Not yet: Discord/X OAuth (needs app credentials) and password-reset email (needs a mail service).
//!
//! CSRF: the session cookie is SameSite=Lax and CORS never allows credentials, so cross-site
//! requests can't carry it; body-carrying mutations additionally require JSON.

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use argon2::password_hash::{PasswordHasher, PasswordVerifier};
use argon2::Argon2;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post, put};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::PgPool;

use crate::{ApiError, AppState};

const COOKIE: &str = "session";
const KEEP_DAYS: i64 = 30;
const SHORT_HOURS: i64 = 12;
const MAX_FAILURES: u32 = 10;
const LOCKOUT: Duration = Duration::from_secs(15 * 60);
const MAX_SAVED_SEARCHES: i64 = 100;
/// viewed items kept per user (the web keeps the same number in a signed-out browser)
const MAX_VIEWS: i64 = 500;
/// One item open in this many also trims the account to `MAX_VIEWS`, as every import does. In between an account can
/// hold a few more, which `list_views` never returns.
const TRIM_VIEWS_ONE_IN: u32 = 20;

/// Failed logins per lowercased login name: (count, window start)
#[derive(Default)]
pub struct LoginThrottle(Mutex<HashMap<String, (u32, Instant)>>);

impl LoginThrottle {
    fn blocked(&self, key: &str) -> bool {
        self.blocked_at(key, Instant::now())
    }
    fn blocked_at(&self, key: &str, now: Instant) -> bool {
        let map = self.0.lock().unwrap();
        map.get(key).is_some_and(|(n, t)| *n >= MAX_FAILURES && now.duration_since(*t) < LOCKOUT)
    }
    fn fail(&self, key: &str) {
        self.fail_at(key, Instant::now());
    }
    fn fail_at(&self, key: &str, now: Instant) {
        let mut map = self.0.lock().unwrap();
        // Keep every active window: evicting one would reset its login lockout.
        map.retain(|_, (_, start)| now.duration_since(*start) < LOCKOUT);
        let e = map.entry(key.to_owned()).or_insert((0, now));
        e.0 += 1;
    }
    fn clear(&self, key: &str) {
        self.0.lock().unwrap().remove(key);
    }
}

pub struct User {
    pub id: i64,
}

fn err(status: StatusCode, msg: &str) -> Response {
    (status, Json(json!({ "ok": false, "error": msg }))).into_response()
}

pub(crate) fn token_hash(token: &str) -> Vec<u8> {
    Sha256::digest(token.as_bytes()).to_vec()
}

fn cookie_token(headers: &HeaderMap) -> Option<String> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .find_map(|kv| kv.trim().strip_prefix("session=").map(str::to_owned))
}

fn secure_flag() -> &'static str {
    if std::env::var("COOKIE_SECURE").as_deref() == Ok("0") { "" } else { "; Secure" }
}

pub async fn current_user(pool: &PgPool, headers: &HeaderMap) -> Result<Option<User>, ApiError> {
    let Some(token) = cookie_token(headers) else { return Ok(None) };
    let id: Option<i64> = sqlx::query_scalar("select user_id from sessions where token_hash = $1 and expires_at > now()")
        .bind(token_hash(&token))
        .fetch_optional(pool)
        .await?;
    Ok(id.map(|id| User { id }))
}

async fn require_user(s: &AppState, headers: &HeaderMap) -> Result<Result<User, Response>, ApiError> {
    Ok(current_user(&s.pool, headers).await?.ok_or_else(|| err(StatusCode::UNAUTHORIZED, "Sign in required")))
}

static ARGON2_SEMAPHORE: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(4);

async fn argon2<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> Result<T, ApiError> {
    let permit = ARGON2_SEMAPHORE.acquire().await.map_err(|e| anyhow::anyhow!("{e}"))?;
    Ok(tokio::task::spawn_blocking(move || {
        let _permit = permit;
        f()
    })
    .await?)
}

pub async fn hash_password(password: String) -> Result<String, ApiError> {
    argon2(move || Argon2::default().hash_password(password.as_bytes()).map(|h| h.to_string()))
        .await?
        .map_err(|e| anyhow::anyhow!("hash: {e}").into())
}

async fn verify_password(password: String, hash: String) -> Result<bool, ApiError> {
    argon2(move || Argon2::default().verify_password(password.as_bytes(), hash.as_str()).is_ok()).await
}

/// Inserts the session ($1-$5) and stamps the account's last sign-in.
const INSERT_SESSION: &str = "with s as (insert into sessions (token_hash, user_id, expires_at, user_agent, ip) values ($1, $2, now() + $3, $4, $5) returning user_id)
     update users set last_sign_in_at = now() from s where users.id = s.user_id";

/// Creates a session row and returns the Set-Cookie value.
/// If `verified_hash` is Some, the session is only inserted if the user's current password hash matches it and the
/// account isn't disabled, preventing an in-flight login from creating a session after a concurrent reset or disable.
async fn start_session(
    pool: &PgPool,
    user_id: i64,
    keep: bool,
    headers: &HeaderMap,
    verified_hash: Option<&str>,
) -> Result<Option<String>, ApiError> {
    let mut raw = [0u8; 32];
    getrandom::fill(&mut raw).map_err(|e| anyhow::anyhow!("rng: {e}"))?;
    let token: String = raw.iter().map(|b| format!("{b:02x}")).collect();
    let ttl = if keep { chrono::Duration::days(KEEP_DAYS) } else { chrono::Duration::hours(SHORT_HOURS) };
    let ua = headers.get(header::USER_AGENT).and_then(|v| v.to_str().ok()).map(|s| s.chars().take(300).collect::<String>());
    let ip = headers.get("cf-connecting-ip").and_then(|v| v.to_str().ok()).map(str::to_owned);

    match verified_hash {
        Some(expected_hash) => {
            // Row lock on `users` serializes with admin `reset_password` and disable (both `UPDATE users`, then delete sessions):
            // if the admin commits first, `FOR UPDATE` sees the new hash or disabled_at and rejects;
            // if login locks first, the admin's UPDATE blocks until login commits, and then its DELETE removes this session.
            let mut tx = pool.begin().await?;
            let current_hash: Option<String> = sqlx::query_scalar(
                "select password_hash from users where id = $1 and disabled_at is null for update",
            )
            .bind(user_id)
            .fetch_optional(&mut *tx)
            .await?;

            if current_hash.as_deref() != Some(expected_hash) {
                return Ok(None);
            }

            sqlx::query(INSERT_SESSION)
                .bind(token_hash(&token))
                .bind(user_id)
                .bind(ttl)
                .bind(ua)
                .bind(ip)
                .execute(&mut *tx)
                .await?;

            tx.commit().await?;
        }
        None => {
            sqlx::query(INSERT_SESSION)
                .bind(token_hash(&token))
                .bind(user_id)
                .bind(ttl)
                .bind(ua)
                .bind(ip)
                .execute(pool)
                .await?;
        }
    }

    let max_age = if keep { format!("; Max-Age={}", KEEP_DAYS * 86_400) } else { String::new() };
    Ok(Some(format!("{COOKIE}={token}; Path=/; HttpOnly; SameSite=Lax{}{max_age}", secure_flag())))
}

fn clear_cookie() -> String {
    format!("{COOKIE}=; Path=/; HttpOnly; SameSite=Lax{}; Max-Age=0", secure_flag())
}

fn valid_username(u: &str) -> bool {
    (3..=24).contains(&u.chars().count()) && u.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn valid_email(e: &str) -> bool {
    e.len() <= 254 && e.split_once('@').is_some_and(|(a, d)| !a.is_empty() && d.contains('.') && !e.contains(char::is_whitespace))
}

fn valid_login(l: &str) -> bool {
    l.len() <= 254
}

fn valid_password(p: &str) -> bool {
    (10..=256).contains(&p.chars().count())
}

fn valid_display_name(n: &str) -> bool {
    (1..=50).contains(&n.trim().chars().count())
}

/// Open unless an admin closed it.
pub async fn registration_open(pool: &PgPool) -> Result<bool, sqlx::Error> {
    let open: Option<Option<bool>> =
        sqlx::query_scalar("select (value->>'open')::boolean from app_settings where key = 'registration'").fetch_optional(pool).await?;
    Ok(open.flatten().unwrap_or(true))
}

/// The account's live sessions: the one making the request first (`current`), then newest first.
pub async fn sessions_json(pool: &PgPool, user_id: i64, current: Option<Vec<u8>>) -> Result<Value, sqlx::Error> {
    sqlx::query_scalar(
        "select coalesce(jsonb_agg(jsonb_build_object('id', id, 'created_at', created_at, 'expires_at', expires_at,
                  'user_agent', user_agent, 'ip', ip, 'current', coalesce(token_hash = $2, false))
                  order by coalesce(token_hash = $2, false) desc, created_at desc, id desc), '[]')
         from sessions where user_id = $1 and expires_at > now()",
    )
    .bind(user_id)
    .bind(current)
    .fetch_one(pool)
    .await
}

fn is_unique_violation(e: &sqlx::Error) -> bool {
    e.as_database_error().and_then(|d| d.code()).as_deref() == Some("23505")
}

/// Only known keys with bounded values may be stored.
pub fn validate_preferences(v: &Value) -> Result<(), &'static str> {
    let obj = v.as_object().ok_or("preferences must be an object")?;
    for (k, val) in obj {
        let s = val.as_str().ok_or("preference values must be strings")?;
        // "" removes a known key
        let ok = s.is_empty() && ["ui", "language", "currency", "theme", "style", "black", "filters", "avatar", "adult"].contains(&k.as_str())
            || match k.as_str() {
            // interface language
            "ui" => ["zh", "en", "ja", "ko"].contains(&s),
            // item text language, or "ui": follow the interface language
            "language" => ["ui", "original", "zh", "en", "ja", "ko"].contains(&s),
            "currency" => s.len() == 3 && s.chars().all(|c| c.is_ascii_uppercase()),
            "theme" => (1..=20).contains(&s.len()) && s.chars().all(|c| c.is_ascii_lowercase()),
            // the Astryx theme (web lib/settings-core.ts STYLES); the site ignores a name it does not know
            "style" => (1..=20).contains(&s.len()) && s.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit()),
            // pure black in dark mode (web lib/settings-core.ts BLACK)
            "black" => ["on", "off"].contains(&s),
            // the filters in a rail at the side or the toolbar's row (web lib/settings-core.ts FILTER_PLACES); "top",
            // the row's old name, from a page still open on the earlier site, which now reads it as the rail
            "filters" => ["side", "row", "top"].contains(&s),
            // "my avatar" for the avatar-fit view; empty clears it
            "avatar" => s.len() <= 100 && s.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'),
            "adult" => ["safe", "suggestive", "include", "adult_only"].contains(&s),
            _ => return Err("unknown preference"),
            };
        if !ok {
            return Err("invalid preference value");
        }
    }
    Ok(())
}

#[derive(Deserialize)]
pub(crate) struct Register {
    pub(crate) display_name: String,
    pub(crate) username: String,
    pub(crate) email: String,
    pub(crate) password: String,
    #[serde(default)]
    pub(crate) keep: bool,
}

pub(crate) async fn register(
    State(s): State<Arc<AppState>>,
    crate::limit::ClientIp(ip): crate::limit::ClientIp,
    headers: HeaderMap,
    Json(b): Json<Register>,
) -> Result<Response, ApiError> {
    if let Err(r) = s.limits.auth.check(ip) {
        return Ok(r.into_response());
    }
    if !registration_open(&s.pool).await? {
        return Ok(err(StatusCode::FORBIDDEN, "Registration is closed."));
    }
    if !valid_display_name(&b.display_name) {
        return Ok(err(StatusCode::BAD_REQUEST, "Display name is required."));
    }
    if !valid_username(&b.username) {
        return Ok(err(StatusCode::BAD_REQUEST, "Username must be 3-24 letters, digits or underscores."));
    }
    if !valid_email(&b.email) {
        return Ok(err(StatusCode::BAD_REQUEST, "Invalid email."));
    }
    if !valid_password(&b.password) {
        return Ok(err(StatusCode::BAD_REQUEST, "Your password must be at least 10 characters."));
    }
    let hash = hash_password(b.password).await?;
    // listed in ADMIN_USERNAMES: admin right away, not only after the next api restart
    let is_admin = crate::admin::admin_names().is_some_and(|n| n.contains(&b.username.to_lowercase()));
    let id: i64 = match sqlx::query_scalar("insert into users (username, email, display_name, password_hash, is_admin) values ($1, $2, $3, $4, $5) returning id")
        .bind(&b.username)
        .bind(b.email.trim())
        .bind(b.display_name.trim())
        .bind(hash)
        .bind(is_admin)
        .fetch_one(&s.pool)
        .await
    {
        Ok(id) => id,
        Err(e) if is_unique_violation(&e) => return Ok(err(StatusCode::CONFLICT, "Username or email is already taken.")),
        Err(e) => return Err(e.into()),
    };
    let cookie = start_session(&s.pool, id, b.keep, &headers, None).await?.expect("session insert on register");
    Ok(([(header::SET_COOKIE, cookie)], Json(json!({ "ok": true }))).into_response())
}

#[derive(Deserialize)]
pub(crate) struct Login {
    pub(crate) login: String,
    pub(crate) password: String,
    #[serde(default)]
    pub(crate) keep: bool,
}

pub(crate) async fn login(
    State(s): State<Arc<AppState>>,
    crate::limit::ClientIp(ip): crate::limit::ClientIp,
    headers: HeaderMap,
    Json(b): Json<Login>,
) -> Result<Response, ApiError> {
    if let Err(r) = s.limits.auth.check(ip) {
        return Ok(r.into_response());
    }
    if !valid_login(&b.login) {
        return Ok(err(StatusCode::UNAUTHORIZED, "Incorrect username or password."));
    }
    let key = b.login.trim().to_lowercase();
    if s.throttle.blocked(&key) {
        return Ok(err(StatusCode::TOO_MANY_REQUESTS, "Too many attempts. Try again later."));
    }
    let row: Option<(i64, String, bool)> =
        sqlx::query_as("select id, password_hash, disabled_at is not null from users where lower(username) = $1 or lower(email) = $1")
            .bind(&key)
            .fetch_optional(&s.pool)
            .await?;
    // verify against a dummy hash when the user is unknown, so timing doesn't reveal which logins exist
    let (id, hash, disabled) = row.unwrap_or((0, s.dummy_hash.clone(), false));
    if !verify_password(b.password, hash.clone()).await? || id == 0 {
        s.throttle.fail(&key);
        return Ok(err(StatusCode::UNAUTHORIZED, "Incorrect username or password."));
    }
    s.throttle.clear(&key);
    // said only to someone who knows the password
    if disabled {
        return Ok(err(StatusCode::FORBIDDEN, "This account is disabled."));
    }
    let cookie = match start_session(&s.pool, id, b.keep, &headers, Some(&hash)).await? {
        Some(c) => c,
        None => {
            s.throttle.fail(&key);
            return Ok(err(StatusCode::UNAUTHORIZED, "Incorrect username or password."));
        }
    };
    Ok(([(header::SET_COOKIE, cookie)], Json(json!({ "ok": true }))).into_response())
}

async fn logout(State(s): State<Arc<AppState>>, headers: HeaderMap) -> Result<Response, ApiError> {
    if let Some(token) = cookie_token(&headers) {
        sqlx::query("delete from sessions where token_hash = $1").bind(token_hash(&token)).execute(&s.pool).await?;
    }
    Ok(([(header::SET_COOKIE, clear_cookie())], Json(json!({ "ok": true }))).into_response())
}

async fn registration(State(s): State<Arc<AppState>>) -> Result<Response, ApiError> {
    Ok(Json(json!({ "open": registration_open(&s.pool).await? })).into_response())
}

async fn list_sessions(State(s): State<Arc<AppState>>, headers: HeaderMap) -> Result<Response, ApiError> {
    let u = match require_user(&s, &headers).await? {
        Ok(u) => u,
        Err(r) => return Ok(r),
    };
    let current = cookie_token(&headers).map(|t| token_hash(&t));
    Ok(Json(json!({ "sessions": sessions_json(&s.pool, u.id, current).await? })).into_response())
}

async fn remove_session(State(s): State<Arc<AppState>>, headers: HeaderMap, Path(id): Path<i64>) -> Result<Response, ApiError> {
    let u = match require_user(&s, &headers).await? {
        Ok(u) => u,
        Err(r) => return Ok(r),
    };
    sqlx::query("delete from sessions where id = $1 and user_id = $2").bind(id).bind(u.id).execute(&s.pool).await?;
    Ok(Json(json!({ "ok": true })).into_response())
}

/// The signed-in account, or `null` without a session: every page asks on load, and a 401 would log a failed
/// request in each signed-out visitor's console.
async fn me(State(s): State<Arc<AppState>>, headers: HeaderMap) -> Result<Response, ApiError> {
    let Some(u) = current_user(&s.pool, &headers).await? else {
        return Ok(Json(Value::Null).into_response());
    };
    let (username, email, display_name, preferences, is_admin): (String, String, String, Value, bool) =
        sqlx::query_as("select username, email, display_name, preferences, is_admin from users where id = $1").bind(u.id).fetch_one(&s.pool).await?;
    Ok(Json(json!({ "id": u.id, "username": username, "email": email, "display_name": display_name, "preferences": preferences, "is_admin": is_admin })).into_response())
}

/// The username stays as registered: ADMIN_USERNAMES makes an account admin by its name, so taking over a listed name
/// must not be possible. A body that still sends `username` is refused, not half applied.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Profile {
    display_name: Option<String>,
}

async fn update_profile(State(s): State<Arc<AppState>>, headers: HeaderMap, Json(b): Json<Profile>) -> Result<Response, ApiError> {
    let u = match require_user(&s, &headers).await? {
        Ok(u) => u,
        Err(r) => return Ok(r),
    };
    if b.display_name.as_deref().is_some_and(|n| !valid_display_name(n)) {
        return Ok(err(StatusCode::BAD_REQUEST, "Display name is required."));
    }
    sqlx::query("update users set display_name = coalesce($2, display_name) where id = $1")
        .bind(u.id)
        .bind(b.display_name.as_deref().map(str::trim))
        .execute(&s.pool)
        .await?;
    Ok(Json(json!({ "ok": true })).into_response())
}

#[derive(Deserialize)]
struct ChangePassword {
    current: String,
    new: String,
}

/// The signed-in account's own password, checked under the same limits as a sign-in: each check is an Argon2 hash
/// drawn from the IP's `auth` bucket, and failures count against the username, so a session cookie alone can't be
/// used to guess the password and take the account over.
async fn check_own_password(s: &AppState, ip: IpAddr, user_id: i64, password: String, wrong: &str) -> Result<Result<(), Response>, ApiError> {
    if let Err(r) = s.limits.auth.check(ip) {
        return Ok(Err(r.into_response()));
    }
    let (username, hash): (String, String) =
        sqlx::query_as("select username, password_hash from users where id = $1").bind(user_id).fetch_one(&s.pool).await?;
    let key = username.to_lowercase();
    if s.throttle.blocked(&key) {
        return Ok(Err(err(StatusCode::TOO_MANY_REQUESTS, "Too many attempts. Try again later.")));
    }
    if !verify_password(password, hash).await? {
        s.throttle.fail(&key);
        return Ok(Err(err(StatusCode::FORBIDDEN, wrong)));
    }
    s.throttle.clear(&key);
    Ok(Ok(()))
}

async fn change_password(
    State(s): State<Arc<AppState>>,
    crate::limit::ClientIp(ip): crate::limit::ClientIp,
    headers: HeaderMap,
    Json(b): Json<ChangePassword>,
) -> Result<Response, ApiError> {
    let u = match require_user(&s, &headers).await? {
        Ok(u) => u,
        Err(r) => return Ok(r),
    };
    if !valid_password(&b.new) {
        return Ok(err(StatusCode::BAD_REQUEST, "Your password must be at least 10 characters."));
    }
    if b.new == b.current {
        return Ok(err(StatusCode::BAD_REQUEST, "Your new password must be different from your current password."));
    }
    if let Err(r) = check_own_password(&s, ip, u.id, b.current, "Current password is incorrect.").await? {
        return Ok(r);
    }
    let new_hash = hash_password(b.new).await?;
    let mut tx = s.pool.begin().await?;
    sqlx::query("update users set password_hash = $2 where id = $1").bind(u.id).bind(new_hash).execute(&mut *tx).await?;
    // sign out every other session
    let current = cookie_token(&headers).map(|t| token_hash(&t)).unwrap_or_default();
    sqlx::query("delete from sessions where user_id = $1 and token_hash <> $2").bind(u.id).bind(current).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(Json(json!({ "ok": true })).into_response())
}

#[derive(Deserialize)]
struct DeleteAccount {
    password: String,
}

async fn delete_account(
    State(s): State<Arc<AppState>>,
    crate::limit::ClientIp(ip): crate::limit::ClientIp,
    headers: HeaderMap,
    Json(b): Json<DeleteAccount>,
) -> Result<Response, ApiError> {
    let u = match require_user(&s, &headers).await? {
        Ok(u) => u,
        Err(r) => return Ok(r),
    };
    // ADMIN_USERNAMES makes an account admin by its name: once deleted, whoever registered the name next would be one
    let is_admin: bool = sqlx::query_scalar("select is_admin from users where id = $1").bind(u.id).fetch_one(&s.pool).await?;
    if is_admin {
        return Ok(err(StatusCode::CONFLICT, "Admins can't delete their account."));
    }
    if let Err(r) = check_own_password(&s, ip, u.id, b.password, "Password is incorrect.").await? {
        return Ok(r);
    }
    // sessions, favourites, viewed items and saved searches cascade
    sqlx::query("delete from users where id = $1").bind(u.id).execute(&s.pool).await?;
    Ok(([(header::SET_COOKIE, clear_cookie())], Json(json!({ "ok": true }))).into_response())
}

async fn get_preferences(State(s): State<Arc<AppState>>, headers: HeaderMap) -> Result<Response, ApiError> {
    let u = match require_user(&s, &headers).await? {
        Ok(u) => u,
        Err(r) => return Ok(r),
    };
    let prefs: Value = sqlx::query_scalar("select preferences from users where id = $1").bind(u.id).fetch_one(&s.pool).await?;
    Ok(Json(prefs).into_response())
}

/// Merges the given keys; an empty string removes a key.
async fn put_preferences(State(s): State<Arc<AppState>>, headers: HeaderMap, Json(b): Json<Value>) -> Result<Response, ApiError> {
    let u = match require_user(&s, &headers).await? {
        Ok(u) => u,
        Err(r) => return Ok(r),
    };
    if let Err(msg) = validate_preferences(&b) {
        return Ok(err(StatusCode::BAD_REQUEST, msg));
    }
    let prefs: Value = sqlx::query_scalar(
        "update users set preferences = jsonb_strip_nulls(preferences || (select coalesce(jsonb_object_agg(key, case when value = '\"\"' then 'null'::jsonb else value end), '{}')
                                                                        from jsonb_each($2)))
         where id = $1 returning preferences",
    )
    .bind(u.id)
    .bind(&b)
    .fetch_one(&s.pool)
    .await?;
    Ok(Json(prefs).into_response())
}

/// Each account's favourite item ids, newest first, so paging through the favourites view does not read them every
/// time. A change made through this API drops them at once; one made through another API instance shows after a
/// minute.
pub struct FavouriteCache {
    ids: moka::sync::Cache<i64, Arc<[i64]>>,
    /// changes so far: ids read while one came in are not kept, they may predate it
    writes: Mutex<u64>,
}

impl FavouriteCache {
    pub fn new() -> Self {
        Self { ids: moka::sync::Cache::builder().max_capacity(10_000).time_to_live(Duration::from_secs(60)).build(), writes: Mutex::new(0) }
    }

    fn writes(&self) -> u64 {
        *self.writes.lock().unwrap()
    }

    /// After a change to the account's favourites.
    fn changed(&self, user_id: i64) {
        let mut w = self.writes.lock().unwrap();
        *w += 1;
        self.ids.invalidate(&user_id);
    }

    /// Keeps `ids`, read when `writes()` was `writes`, unless a change came in since.
    fn keep(&self, user_id: i64, ids: Arc<[i64]>, writes: u64) {
        let w = self.writes.lock().unwrap();
        if *w == writes {
            self.ids.insert(user_id, ids);
        }
    }
}

pub async fn favourite_ids(s: &AppState, user_id: i64) -> Result<Arc<[i64]>, ApiError> {
    if let Some(ids) = s.favourites.ids.get(&user_id) {
        return Ok(ids);
    }
    let writes = s.favourites.writes();
    let ids: Arc<[i64]> = sqlx::query_scalar::<_, i64>("select item_id from favourites where user_id = $1 order by created_at desc")
        .bind(user_id)
        .fetch_all(&s.pool)
        .await?
        .into();
    s.favourites.keep(user_id, ids.clone(), writes);
    Ok(ids)
}

async fn list_favourites(State(s): State<Arc<AppState>>, headers: HeaderMap) -> Result<Response, ApiError> {
    let u = match require_user(&s, &headers).await? {
        Ok(u) => u,
        Err(r) => return Ok(r),
    };
    Ok(Json(json!({ "ids": &*favourite_ids(&s, u.id).await? })).into_response())
}

async fn add_favourite(State(s): State<Arc<AppState>>, headers: HeaderMap, Path(item): Path<i64>) -> Result<Response, ApiError> {
    let u = match require_user(&s, &headers).await? {
        Ok(u) => u,
        Err(r) => return Ok(r),
    };
    match sqlx::query("insert into favourites (user_id, item_id) values ($1, $2) on conflict do nothing").bind(u.id).bind(item).execute(&s.pool).await {
        Ok(_) => {
            s.favourites.changed(u.id);
            Ok(Json(json!({ "ok": true })).into_response())
        }
        Err(e) if e.as_database_error().and_then(|d| d.code()).as_deref() == Some("23503") => Ok(err(StatusCode::NOT_FOUND, "Unknown item.")),
        Err(e) => Err(e.into()),
    }
}

async fn remove_favourite(State(s): State<Arc<AppState>>, headers: HeaderMap, Path(item): Path<i64>) -> Result<Response, ApiError> {
    let u = match require_user(&s, &headers).await? {
        Ok(u) => u,
        Err(r) => return Ok(r),
    };
    sqlx::query("delete from favourites where user_id = $1 and item_id = $2").bind(u.id).bind(item).execute(&s.pool).await?;
    s.favourites.changed(u.id);
    Ok(Json(json!({ "ok": true })).into_response())
}

async fn list_views(State(s): State<Arc<AppState>>, headers: HeaderMap) -> Result<Response, ApiError> {
    let u = match require_user(&s, &headers).await? {
        Ok(u) => u,
        Err(r) => return Ok(r),
    };
    let ids: Vec<i64> = sqlx::query_scalar("select item_id from item_views where user_id = $1 order by viewed_at desc limit $2")
        .bind(u.id)
        .bind(MAX_VIEWS)
        .fetch_all(&s.pool)
        .await?;
    Ok(Json(json!({ "ids": ids })).into_response())
}

#[derive(Deserialize)]
struct ViewEntry {
    id: i64,
    /// view timestamp in epoch milliseconds (optional, missing defaults to server now())
    #[serde(default)]
    at: Option<i64>,
}

#[derive(Deserialize)]
struct Views {
    /// newest first: one item just opened, or a signed-out browser's history merged in at sign-in
    views: Vec<ViewEntry>,
}

fn dedupe_views(entries: Vec<ViewEntry>) -> (Vec<i64>, Vec<Option<f64>>) {
    let mut seen = HashMap::new();
    let mut deduped = Vec::new();
    for entry in entries {
        let at = entry.at.filter(|&ms| ms > 0 && ms < 4_102_444_800_000);
        if let Some(&idx) = seen.get(&entry.id) {
            let (_, existing_at): &mut (i64, Option<i64>) = &mut deduped[idx];
            *existing_at = match (*existing_at, at) {
                (None, _) | (_, None) => None,
                (Some(a), Some(b)) => Some(a.max(b)),
            };
        } else {
            seen.insert(entry.id, deduped.len());
            deduped.push((entry.id, at));
        }
    }
    deduped.into_iter().map(|(id, at)| (id, at.map(|ms| ms as f64 / 1000.0))).unzip()
}

async fn add_views(State(s): State<Arc<AppState>>, headers: HeaderMap, Json(b): Json<Views>) -> Result<Response, ApiError> {
    let u = match require_user(&s, &headers).await? {
        Ok(u) => u,
        Err(r) => return Ok(r),
    };
    let entries: Vec<ViewEntry> = b.views.into_iter().take(MAX_VIEWS as usize).collect();
    if entries.is_empty() {
        return Ok(Json(json!({ "ok": true })).into_response());
    }
    let (ids, ats) = dedupe_views(entries);

    // no transaction or lock, so an item open is one statement; concurrent writes can at worst leave a few rows
    // over the cap until the next trim
    // missing at defaults to now(); future timestamps are clamped to server now()
    sqlx::query(
        "insert into item_views (user_id, item_id, viewed_at)
         select $1, v.id, least(coalesce(to_timestamp(v.at_sec), now()), now())
         from unnest($2::bigint[], $3::double precision[]) as v(id, at_sec) join items i on i.id = v.id
         on conflict (user_id, item_id) do update set viewed_at = greatest(item_views.viewed_at, excluded.viewed_at)",
    )
    .bind(u.id)
    .bind(&ids)
    .bind(&ats)
    .execute(&s.pool)
    .await?;
    if ids.len() > 1 || getrandom::u32().is_ok_and(|r| r % TRIM_VIEWS_ONE_IN == 0) {
        trim_views(&s.pool, u.id).await?;
    }
    Ok(Json(json!({ "ok": true })).into_response())
}

/// Deletes the views older than the account's newest `MAX_VIEWS`. Compared by time, not by id: a view a concurrent
/// open moves up is checked again with its new time and kept, so the cap can only be exceeded, never undercut.
async fn trim_views(pool: &PgPool, user_id: i64) -> Result<(), sqlx::Error> {
    sqlx::query(
        "delete from item_views where user_id = $1 and viewed_at <
         (select viewed_at from item_views where user_id = $1 order by viewed_at desc offset $2 limit 1)",
    )
    .bind(user_id)
    .bind(MAX_VIEWS - 1)
    .execute(pool)
    .await?;
    Ok(())
}

async fn clear_views(State(s): State<Arc<AppState>>, headers: HeaderMap) -> Result<Response, ApiError> {
    let u = match require_user(&s, &headers).await? {
        Ok(u) => u,
        Err(r) => return Ok(r),
    };
    sqlx::query("delete from item_views where user_id = $1").bind(u.id).execute(&s.pool).await?;
    Ok(Json(json!({ "ok": true })).into_response())
}

async fn list_searches(State(s): State<Arc<AppState>>, headers: HeaderMap) -> Result<Response, ApiError> {
    let u = match require_user(&s, &headers).await? {
        Ok(u) => u,
        Err(r) => return Ok(r),
    };
    let rows: Vec<(i64, String, String)> =
        sqlx::query_as("select id, name, query from saved_searches where user_id = $1 order by created_at desc").bind(u.id).fetch_all(&s.pool).await?;
    let items: Vec<Value> = rows
        .into_iter()
        .map(|(id, name, query)| json!({ "id": id, "name": name, "query": query, "rss": format!("{}/api/rss?{query}", s.site_url) }))
        .collect();
    Ok(Json(json!({ "items": items })).into_response())
}

/// A saved search's name: 1 to 100 characters once trimmed.
fn valid_search_name(name: &str) -> bool {
    (1..=100).contains(&name.trim().chars().count())
}

#[derive(Deserialize)]
struct NewSearch {
    name: String,
    query: String,
}

async fn add_search(State(s): State<Arc<AppState>>, headers: HeaderMap, Json(b): Json<NewSearch>) -> Result<Response, ApiError> {
    let u = match require_user(&s, &headers).await? {
        Ok(u) => u,
        Err(r) => return Ok(r),
    };
    let query = b.query.trim().trim_start_matches('?');
    if !valid_search_name(&b.name) || query.len() > 2000 || query.contains(char::is_whitespace) {
        return Ok(err(StatusCode::BAD_REQUEST, "Invalid name or query."));
    }
    let id: Option<i64> = sqlx::query_scalar(
        "insert into saved_searches (user_id, name, query) select $1, $2, $3
         where (select count(*) from saved_searches where user_id = $1) < $4 returning id",
    )
    .bind(u.id)
    .bind(b.name.trim())
    .bind(query)
    .bind(MAX_SAVED_SEARCHES)
    .fetch_optional(&s.pool)
    .await?;
    match id {
        Some(id) => Ok(Json(json!({ "id": id, "rss": format!("{}/api/rss?{query}", s.site_url) })).into_response()),
        None => Ok(err(StatusCode::CONFLICT, "Saved search limit reached.")),
    }
}

async fn remove_search(State(s): State<Arc<AppState>>, headers: HeaderMap, Path(id): Path<i64>) -> Result<Response, ApiError> {
    let u = match require_user(&s, &headers).await? {
        Ok(u) => u,
        Err(r) => return Ok(r),
    };
    sqlx::query("delete from saved_searches where id = $1 and user_id = $2").bind(id).bind(u.id).execute(&s.pool).await?;
    Ok(Json(json!({ "ok": true })).into_response())
}

#[derive(Deserialize)]
struct SearchName {
    name: String,
}

async fn rename_search(State(s): State<Arc<AppState>>, headers: HeaderMap, Path(id): Path<i64>, Json(b): Json<SearchName>) -> Result<Response, ApiError> {
    let u = match require_user(&s, &headers).await? {
        Ok(u) => u,
        Err(r) => return Ok(r),
    };
    if !valid_search_name(&b.name) {
        return Ok(err(StatusCode::BAD_REQUEST, "Invalid name or query."));
    }
    sqlx::query("update saved_searches set name = $1 where id = $2 and user_id = $3").bind(b.name.trim()).bind(id).bind(u.id).execute(&s.pool).await?;
    Ok(Json(json!({ "ok": true })).into_response())
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        // public: signing up, in and out, and whether sign-up is open; every later handler reads the session itself
        .route("/api/account/register", post(register))
        .route("/api/account/login", post(login))
        .route("/api/account/logout", post(logout))
        .route("/api/account/registration", get(registration))
        .route("/api/account/sessions", get(list_sessions))
        .route("/api/account/sessions/{id}", axum::routing::delete(remove_session))
        .route("/api/account", get(me).delete(delete_account))
        .route("/api/account/profile", axum::routing::patch(update_profile))
        .route("/api/account/password", post(change_password))
        .route("/api/account/preferences", get(get_preferences).put(put_preferences))
        .route("/api/account/favourites", get(list_favourites))
        .route("/api/account/favourites/{id}", put(add_favourite).delete(remove_favourite))
        .route("/api/account/views", get(list_views).post(add_views).delete(clear_views))
        .route("/api/account/searches", get(list_searches).post(add_search))
        .route("/api/account/searches/{id}", axum::routing::patch(rename_search).delete(remove_search))
}

/// Hash used to equalise login timing for unknown users.
pub fn dummy_hash() -> String {
    Argon2::default().hash_password(b"timing-equaliser-not-a-password").expect("argon2").to_string()
}

/// An API over `pool` with an empty catalogue, for handler tests.
#[cfg(test)]
pub(crate) fn test_state(pool: PgPool) -> Arc<AppState> {
    crate::install_tls();
    Arc::new(
        AppState::new(
            pool,
            crate::Snapshot {
                catalog: crate::catalog::Catalog::build(Vec::new(), 0, None).unwrap(),
                category_phrases: Default::default(),
                avatars: crate::Lookup::new(Vec::new()),
                authors: crate::Lookup::new(Vec::new()),
                tags: crate::Lookup::new(Vec::new()),
                booth_tags: crate::Lookup::new(Vec::new()),
                categories: json!({ "categories": [] }),
                version: 1,
                loaded_at: chrono::Utc::now(),
                build_secs: 0.0,
            },
            Default::default(),
            "http://127.0.0.1:3000".into(),
            None,
            dummy_hash(),
            Some(Duration::from_secs(2)),
        )
        .unwrap(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validators() {
        assert!(valid_username("abc_123") && !valid_username("ab") && !valid_username("has space") && !valid_username(&"a".repeat(25)));
        assert!(valid_email("a@b.co") && !valid_email("nope") && !valid_email("a @b.co") && !valid_email("@b.co"));
        assert!(valid_login("user@example.com") && valid_login(&"a".repeat(254)) && !valid_login(&"a".repeat(255)));
        assert!(valid_password("0123456789") && !valid_password("short"));
    }

    #[test]
    fn profile_cannot_change_the_username() {
        assert!(serde_json::from_value::<Profile>(json!({ "display_name": "Ann" })).is_ok());
        assert!(serde_json::from_value::<Profile>(json!({ "username": "admin" })).is_err());
    }

    #[test]
    fn preferences_whitelist() {
        assert!(validate_preferences(&json!({"language": "ja", "currency": "USD", "theme": "charcoal", "avatar": "manuka"})).is_ok());
        assert!(validate_preferences(&json!({"avatar": "", "language": ""})).is_ok());
        assert!(validate_preferences(&json!({"style": "y2k"})).is_ok());
        assert!(validate_preferences(&json!({"style": ""})).is_ok());
        assert!(validate_preferences(&json!({"style": "Y2K"})).is_err());
        assert!(validate_preferences(&json!({"black": "on"})).is_ok());
        assert!(validate_preferences(&json!({"black": "yes"})).is_err());
        assert!(validate_preferences(&json!({"filters": "side"})).is_ok());
        assert!(validate_preferences(&json!({"filters": "row"})).is_ok());
        assert!(validate_preferences(&json!({"filters": "left"})).is_err());
        assert!(validate_preferences(&json!({"evil": ""})).is_err());
        assert!(validate_preferences(&json!({"language": "fr"})).is_err());
        assert!(validate_preferences(&json!({"language": "zh"})).is_ok());
        assert!(validate_preferences(&json!({"language": "ui"})).is_ok());
        assert!(validate_preferences(&json!({"ui": "ko"})).is_ok());
        assert!(validate_preferences(&json!({"ui": "original"})).is_err());
        assert!(validate_preferences(&json!({"evil": "x"})).is_err());
        assert!(validate_preferences(&json!({"currency": 5})).is_err());
        assert!(validate_preferences(&json!(["x"])).is_err());
    }

    #[test]
    fn cookie_parsing() {
        let mut h = HeaderMap::new();
        h.insert(header::COOKIE, "a=1; session=abc123; b=2".parse().unwrap());
        assert_eq!(cookie_token(&h).as_deref(), Some("abc123"));
    }

    #[test]
    fn throttle_blocks_after_limit() {
        let t = LoginThrottle::default();
        for _ in 0..MAX_FAILURES {
            assert!(!t.blocked("x"));
            t.fail("x");
        }
        assert!(t.blocked("x"));
        t.clear("x");
        assert!(!t.blocked("x"));
    }

    #[test]
    fn throttle_prunes_expired_names_without_resetting_active_windows() {
        let t = LoginThrottle::default();
        let expired = Instant::now();
        let start = expired + LOCKOUT / 2;
        let now = expired + LOCKOUT;
        {
            let mut map = t.0.lock().unwrap();
            map.insert("expired".into(), (MAX_FAILURES, expired));
            map.insert("locked".into(), (MAX_FAILURES, start));
            map.insert("active".into(), (MAX_FAILURES - 1, start));
        }
        t.fail_at("active", now);
        t.fail_at("new", now);
        let map = t.0.lock().unwrap();
        assert_eq!(map.len(), 3);
        assert!(!map.contains_key("expired"));
        assert_eq!(map["locked"], (MAX_FAILURES, start));
        assert_eq!(map["active"], (MAX_FAILURES, start));
        assert_eq!(map["new"].0, 1);
        drop(map);
        assert!(t.blocked_at("locked", now));
        assert!(t.blocked_at("active", now));
    }

    #[test]
    fn throttle_restarts_an_expired_names_window() {
        let t = LoginThrottle::default();
        let start = Instant::now();
        let now = start + LOCKOUT;
        t.0.lock().unwrap().insert("expired".into(), (MAX_FAILURES, start));
        assert!(t.blocked_at("expired", now - Duration::from_nanos(1)));
        assert!(!t.blocked_at("expired", now));
        t.fail_at("expired", now);
        assert_eq!(t.0.lock().unwrap()["expired"], (1, now));
        assert!(!t.blocked_at("expired", now));
    }

    #[test]
    fn views_deduplication_and_timestamps() {
        let json = r#"{"views": [{"id": 1, "at": 1000}, {"id": 2}, {"id": 1, "at": 2000}, {"id": 1, "at": 500}]}"#;
        let b: Views = serde_json::from_str(json).unwrap();
        let (ids, ats) = dedupe_views(b.views);
        assert_eq!(ids, vec![1, 2]);
        assert_eq!(ats, vec![Some(2.0), None]);
    }

    /// Account 'a', signed in with the cookie `headers()` sends.
    async fn signed_in(pool: &PgPool) -> i64 {
        let user = sqlx::query_scalar("insert into users (username, email, display_name, password_hash) values ('a', 'a@example.test', 'A', 'x') returning id")
            .fetch_one(pool)
            .await
            .unwrap();
        sqlx::query("insert into sessions (token_hash, user_id, expires_at) values ($1, $2, now() + interval '1 day')")
            .bind(token_hash("tok-a"))
            .bind(user)
            .execute(pool)
            .await
            .unwrap();
        user
    }

    fn headers() -> HeaderMap {
        let mut h = HeaderMap::new();
        h.insert(header::COOKIE, "session=tok-a".parse().unwrap());
        h
    }

    /// Listed items 1 to n.
    async fn items(pool: &PgPool, n: i64) {
        sqlx::query(
            "insert into items (id, title, author_name, author_uri, is_adult, status, scraped_at)
             select g, 't', 'c', 'https://example.test', false, 'ok', now() from generate_series(1, $1) g",
        )
        .bind(n)
        .execute(pool)
        .await
        .unwrap();
    }

    /// The `ids` of a list answer.
    async fn ids(r: Response) -> Vec<i64> {
        let body: Value = serde_json::from_slice(&axum::body::to_bytes(r.into_body(), usize::MAX).await.unwrap()).unwrap();
        serde_json::from_value(body["ids"].clone()).unwrap()
    }

    #[test]
    fn favourites_read_before_a_change_are_not_cached() {
        let c = FavouriteCache::new();
        let read_at = c.writes();
        c.changed(1);
        c.keep(1, Arc::from([5]), read_at);
        assert!(c.ids.get(&1).is_none());
        c.keep(1, Arc::from([5]), c.writes());
        assert_eq!(c.ids.get(&1).as_deref(), Some(&[5][..]));
        c.changed(1);
        assert!(c.ids.get(&1).is_none());
    }

    #[sqlx::test(migrations = "../core/migrations")]
    #[ignore = "requires PostgreSQL with pgvector"]
    async fn favourites_list_follows_changes_at_once(pool: PgPool) {
        let user = signed_in(&pool).await;
        items(&pool, 3).await;
        let state = test_state(pool.clone());
        let list = || async { ids(list_favourites(State(state.clone()), headers()).await.ok().unwrap()).await };

        add_favourite(State(state.clone()), headers(), Path(1)).await.ok().unwrap();
        assert_eq!(list().await, [1]);
        add_favourite(State(state.clone()), headers(), Path(2)).await.ok().unwrap();
        assert_eq!(list().await, [2, 1]);
        remove_favourite(State(state.clone()), headers(), Path(1)).await.ok().unwrap();
        assert_eq!(list().await, [2]);
        // a change that does not pass through this API is not seen until the cache expires
        sqlx::query("insert into favourites (user_id, item_id) values ($1, 3)").bind(user).execute(&pool).await.unwrap();
        assert_eq!(list().await, [2]);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn argon2_permit_stays_held_when_awaiting_future_is_dropped() {
        static TEST_MUTEX: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
        let _guard = TEST_MUTEX.lock().await;

        let (entered_tx, entered_rx) = tokio::sync::oneshot::channel();
        let (release_tx, release_rx) = std::sync::mpsc::sync_channel(1);

        tokio::time::timeout(std::time::Duration::from_secs(2), async {
            while ARGON2_SEMAPHORE.available_permits() < 4 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("baseline permits available");

        assert_eq!(ARGON2_SEMAPHORE.available_permits(), 4);

        let task = tokio::spawn(async move {
            argon2(move || {
                let _ = entered_tx.send(());
                let _ = release_rx.recv();
            })
            .await
        });

        entered_rx.await.expect("closure entered");
        assert_eq!(ARGON2_SEMAPHORE.available_permits(), 3);

        task.abort();
        let _ = task.await;

        assert_eq!(
            ARGON2_SEMAPHORE.available_permits(),
            3,
            "permit must not be released on future drop while closure is running"
        );

        release_tx.send(()).expect("release closure");

        let returned = tokio::time::timeout(std::time::Duration::from_secs(2), async {
            while ARGON2_SEMAPHORE.available_permits() < 4 {
                tokio::task::yield_now().await;
            }
        })
        .await;
        assert!(returned.is_ok(), "permit must return after closure finishes");
        assert_eq!(ARGON2_SEMAPHORE.available_permits(), 4);
    }

    #[sqlx::test(migrations = "../core/migrations")]
    #[ignore = "requires PostgreSQL with pgvector"]
    async fn renaming_a_saved_search_needs_its_owner_and_a_valid_name(pool: PgPool) {
        let users: Vec<i64> = sqlx::query_scalar(
            "insert into users (username, email, display_name, password_hash)
             values ('a', 'a@example.test', 'A', 'x'), ('b', 'b@example.test', 'B', 'x') returning id",
        )
        .fetch_all(&pool)
        .await
        .unwrap();
        sqlx::query("insert into sessions (token_hash, user_id, expires_at) values ($1, $2, now() + interval '1 day')")
            .bind(token_hash("tok-a"))
            .bind(users[0])
            .execute(&pool)
            .await
            .unwrap();
        let ids: Vec<i64> = sqlx::query_scalar("insert into saved_searches (user_id, name, query) values ($1, 'mine', 'q=a'), ($2, 'theirs', 'q=b') returning id")
            .bind(users[0])
            .bind(users[1])
            .fetch_all(&pool)
            .await
            .unwrap();
        let state = test_state(pool.clone());
        let rename = |cookie: Option<&str>, id: i64, name: &str| {
            let mut h = HeaderMap::new();
            if let Some(c) = cookie {
                h.insert(header::COOKIE, c.parse().unwrap());
            }
            rename_search(State(state.clone()), h, Path(id), Json(SearchName { name: name.into() }))
        };
        let status = |r: Result<Response, ApiError>| r.into_response().status();
        let names = || sqlx::query_scalar::<_, String>("select name from saved_searches order by id").fetch_all(&pool);

        assert_eq!(status(rename(None, ids[0], "x").await), StatusCode::UNAUTHORIZED);
        assert_eq!(status(rename(Some("session=tok-a"), ids[0], "  ").await), StatusCode::BAD_REQUEST);
        assert_eq!(status(rename(Some("session=tok-a"), ids[0], &"n".repeat(101)).await), StatusCode::BAD_REQUEST);
        assert_eq!(status(rename(Some("session=tok-a"), ids[0], "  renamed ").await), StatusCode::OK);
        // another account's search is left as it was
        assert_eq!(status(rename(Some("session=tok-a"), ids[1], "stolen").await), StatusCode::OK);
        assert_eq!(names().await.unwrap(), ["renamed", "theirs"]);
    }

    #[sqlx::test(migrations = "../core/migrations")]
    #[ignore = "requires PostgreSQL with pgvector"]
    async fn the_account_is_null_without_a_session(pool: PgPool) {
        let user = signed_in(&pool).await;
        let state = test_state(pool);
        let account = |h: HeaderMap| {
            let state = state.clone();
            async move {
                let r = me(State(state), h).await.ok().unwrap();
                assert_eq!(r.status(), StatusCode::OK);
                serde_json::from_slice::<Value>(&axum::body::to_bytes(r.into_body(), usize::MAX).await.unwrap()).unwrap()
            }
        };
        let mut unknown = HeaderMap::new();
        unknown.insert(header::COOKIE, "session=gone".parse().unwrap());

        assert_eq!(account(HeaderMap::new()).await, Value::Null);
        assert_eq!(account(unknown).await, Value::Null);
        assert_eq!(account(headers()).await["id"], user);
    }

    #[sqlx::test(migrations = "../core/migrations")]
    #[ignore = "requires PostgreSQL with pgvector"]
    async fn imports_trim_viewed_items_and_the_list_keeps_the_newest(pool: PgPool) {
        let user = signed_in(&pool).await;
        items(&pool, 501).await;
        let state = test_state(pool.clone());
        // newest first, item n viewed at n ms past a fixed time
        let add = |ids: Vec<i64>| {
            let views = ids.into_iter().map(|id| ViewEntry { id, at: Some(1_700_000_000_000 + id) }).collect();
            add_views(State(state.clone()), headers(), Json(Views { views }))
        };
        let count = || sqlx::query_scalar::<_, i64>("select count(*) from item_views where user_id = $1").bind(user).fetch_one(&pool);

        add((1..=300).rev().collect()).await.ok().unwrap();
        add((301..=501).rev().collect()).await.ok().unwrap();
        assert_eq!(count().await.unwrap(), 500, "an import trims to the newest MAX_VIEWS");

        // a single open is one upsert; the account may stay a row over until the next trim
        add_views(State(state.clone()), headers(), Json(Views { views: vec![ViewEntry { id: 1, at: None }] })).await.ok().unwrap();
        assert!((500..=501).contains(&count().await.unwrap()));
        let listed = ids(list_views(State(state.clone()), headers()).await.ok().unwrap()).await;
        assert_eq!(listed.len(), 500);
        assert_eq!(listed[..2], [1, 501]);
    }

    #[sqlx::test(migrations = "../core/migrations")]
    #[ignore = "requires PostgreSQL with pgvector"]
    async fn item_stats_count_favourites_through_adds_removes_and_deleted_accounts(pool: PgPool) {
        let users: Vec<i64> = sqlx::query_scalar(
            "insert into users (username, email, display_name, password_hash)
             values ('a', 'a@example.test', 'A', 'x'), ('b', 'b@example.test', 'B', 'x') returning id",
        )
        .fetch_all(&pool)
        .await
        .unwrap();
        items(&pool, 2).await;
        let fav = |user: i64, item: i64| sqlx::query("insert into favourites (user_id, item_id) values ($1, $2) on conflict do nothing").bind(user).bind(item).execute(&pool);
        fav(users[0], 1).await.unwrap();
        fav(users[1], 1).await.unwrap();
        fav(users[0], 2).await.unwrap();
        // already a favourite: nothing inserted, nothing counted
        fav(users[0], 1).await.unwrap();
        sqlx::query("delete from favourites where user_id = $1 and item_id = 2").bind(users[0]).execute(&pool).await.unwrap();
        // the account's favourites go with it
        sqlx::query("delete from users where id = $1").bind(users[1]).execute(&pool).await.unwrap();

        let counts: Vec<(i64, i64)> = sqlx::query_as("select item_id, favourites from item_stats order by item_id").fetch_all(&pool).await.unwrap();
        assert_eq!(counts, [(1, 1), (2, 0)]);
        let ranked: i64 = sqlx::query_scalar("select count(*) from refresh_priority").fetch_one(&pool).await.unwrap();
        assert_eq!(ranked, 2);
    }

    #[sqlx::test(migrations = "../core/migrations")]
    #[ignore = "requires PostgreSQL with pgvector"]
    async fn a_view_moved_up_while_trimming_is_kept(pool: PgPool) {
        let user = signed_in(&pool).await;
        items(&pool, 501).await;
        // item n viewed n seconds past a fixed time: item 1 is the one past the cap
        sqlx::query("insert into item_views (user_id, item_id, viewed_at) select $1, g, to_timestamp(1700000000 + g) from generate_series(1, 501) g")
            .bind(user)
        .execute(&pool)
        .await
        .unwrap();

        // another device opens item 1 again, on its own connection; its write is not committed yet when the trim starts
        let mut conn = <sqlx::PgConnection as sqlx::Connection>::connect_with(&pool.connect_options()).await.unwrap();
        let mut open = sqlx::Connection::begin(&mut conn).await.unwrap();
        sqlx::query("update item_views set viewed_at = now() where user_id = $1 and item_id = 1").bind(user).execute(&mut *open).await.unwrap();
        let trim = tokio::spawn({
            let pool = pool.clone();
            async move { trim_views(&pool, user).await }
        });
        // the trim waits for item 1's row
        for _ in 0..250 {
            if sqlx::query_scalar("select exists (select 1 from pg_locks where not granted)").fetch_one(&mut *open).await.unwrap() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        assert!(!trim.is_finished(), "the trim must have waited for the open");
        open.commit().await.unwrap();
        trim.await.unwrap().unwrap();

        let newest: i64 = sqlx::query_scalar("select item_id from item_views where user_id = $1 order by viewed_at desc limit 1")
            .bind(user)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(newest, 1, "the item just opened again must not be trimmed");
        let count: i64 = sqlx::query_scalar("select count(*) from item_views where user_id = $1").bind(user).fetch_one(&pool).await.unwrap();
        assert_eq!(count, 501, "the cap is only ever exceeded until the next trim");
    }

    #[sqlx::test(migrations = "../core/migrations")]
    #[ignore = "requires PostgreSQL with pgvector"]
    async fn disabled_accounts_and_closed_registration_keep_people_out(pool: PgPool) {
        let state = test_state(pool.clone());
        let ip = std::net::IpAddr::from([127, 0, 0, 1]);
        let sign_up = |username: &str| {
            register(State(state.clone()), crate::limit::ClientIp(ip), HeaderMap::new(), Json(Register {
                display_name: "A".into(),
                username: username.into(),
                email: format!("{username}@example.test"),
                password: "0123456789".into(),
                keep: false,
            }))
        };
        let sign_in = || {
            login(State(state.clone()), crate::limit::ClientIp(ip), HeaderMap::new(), Json(Login { login: "ann".into(), password: "0123456789".into(), keep: false }))
        };
        let status = |r: Result<Response, ApiError>| r.into_response().status();

        assert_eq!(status(sign_up("ann").await), StatusCode::OK);
        assert!(sqlx::query_scalar::<_, bool>("select last_sign_in_at is not null from users").fetch_one(&pool).await.unwrap());
        assert_eq!(status(sign_in().await), StatusCode::OK);
        sqlx::query("update users set disabled_at = now()").execute(&pool).await.unwrap();
        assert_eq!(status(sign_in().await), StatusCode::FORBIDDEN);

        sqlx::query("insert into app_settings (key, value) values ('registration', '{\"open\": false}')").execute(&pool).await.unwrap();
        assert!(!registration_open(&pool).await.unwrap());
        assert_eq!(status(sign_up("bob").await), StatusCode::FORBIDDEN);
        assert_eq!(sqlx::query_scalar::<_, i64>("select count(*) from users").fetch_one(&pool).await.unwrap(), 1);
    }

    #[sqlx::test(migrations = "../core/migrations")]
    #[ignore = "requires PostgreSQL with pgvector"]
    async fn password_checks_are_limited_like_sign_ins(pool: PgPool) {
        signed_in(&pool).await;
        let state = test_state(pool.clone());
        let change = |ip: [u8; 4]| {
            change_password(State(state.clone()), crate::limit::ClientIp(ip.into()), headers(), Json(ChangePassword {
                current: "not-the-password".into(),
                new: "0123456789".into(),
            }))
        };
        let status = |r: Result<Response, ApiError>| r.into_response().status();

        // an IP out of sign-in attempts can't check passwords either
        let busy = [203, 0, 113, 1];
        while state.limits.auth.check(busy.into()).is_ok() {}
        assert_eq!(status(change(busy).await), StatusCode::TOO_MANY_REQUESTS);
        let delete = delete_account(State(state.clone()), crate::limit::ClientIp(busy.into()), headers(), Json(DeleteAccount { password: "x".into() }));
        assert_eq!(status(delete.await), StatusCode::TOO_MANY_REQUESTS);

        // wrong guesses from many IPs lock the account's name, as failed sign-ins do
        for i in 0..MAX_FAILURES as u8 {
            assert_eq!(status(change([198, 51, 100, i]).await), StatusCode::FORBIDDEN);
        }
        assert_eq!(status(change([198, 51, 100, 200]).await), StatusCode::TOO_MANY_REQUESTS);
        assert!(state.throttle.blocked("a"));
    }

    #[sqlx::test(migrations = "../core/migrations")]
    #[ignore = "requires PostgreSQL with pgvector"]
    async fn a_new_password_must_differ_from_the_current_one(pool: PgPool) {
        signed_in(&pool).await;
        let old_hash = hash_password("0123456789".into()).await.ok().unwrap();
        sqlx::query("update users set password_hash = $1").bind(&old_hash).execute(&pool).await.unwrap();
        let state = test_state(pool.clone());
        let change = |new: &str| {
            change_password(State(state.clone()), crate::limit::ClientIp([203, 0, 113, 7].into()), headers(), Json(ChangePassword {
                current: "0123456789".into(),
                new: new.into(),
            }))
        };
        let status = |r: Result<Response, ApiError>| r.into_response().status();
        let hash = || sqlx::query_scalar::<_, String>("select password_hash from users").fetch_one(&pool);

        assert_eq!(status(change("0123456789").await), StatusCode::BAD_REQUEST);
        assert_eq!(hash().await.unwrap(), old_hash);
        assert_eq!(status(change("9876543210").await), StatusCode::OK);
        assert_ne!(hash().await.unwrap(), old_hash);
    }

    #[sqlx::test(migrations = "../core/migrations")]
    #[ignore = "requires PostgreSQL with pgvector"]
    async fn admins_cannot_delete_their_account(pool: PgPool) {
        signed_in(&pool).await;
        sqlx::query("update users set is_admin = true").execute(&pool).await.unwrap();
        let state = test_state(pool.clone());
        let ip = crate::limit::ClientIp([203, 0, 113, 9].into());
        let r = delete_account(State(state), ip, headers(), Json(DeleteAccount { password: "x".into() })).await.into_response();
        assert_eq!(r.status(), StatusCode::CONFLICT);
        assert_eq!(sqlx::query_scalar::<_, i64>("select count(*) from users").fetch_one(&pool).await.unwrap(), 1);
    }

    #[sqlx::test(migrations = "../core/migrations")]
    #[ignore = "requires PostgreSQL with pgvector"]
    async fn people_see_and_sign_out_only_their_own_sessions(pool: PgPool) {
        let users: Vec<i64> = sqlx::query_scalar(
            "insert into users (username, email, display_name, password_hash)
             values ('a', 'a@example.test', 'A', 'x'), ('b', 'b@example.test', 'B', 'x') returning id",
        )
        .fetch_all(&pool)
        .await
        .unwrap();
        let ids: Vec<i64> = sqlx::query_scalar(
            "insert into sessions (token_hash, user_id, expires_at) values ($1, $3, now() + interval '1 day'), ($2, $3, now() + interval '1 day'),
               ($4, $5, now() + interval '1 day') returning id",
        )
        .bind(token_hash("tok-a"))
        .bind(token_hash("tok-a2"))
        .bind(users[0])
        .bind(token_hash("tok-b"))
        .bind(users[1])
        .fetch_all(&pool)
        .await
        .unwrap();
        // this device signed in before the other one, and is still listed first
        sqlx::query("update sessions set created_at = now() - interval '1 hour' where id = $1").bind(ids[0]).execute(&pool).await.unwrap();
        let state = test_state(pool.clone());
        let mut a = HeaderMap::new();
        a.insert(header::COOKIE, "session=tok-a".parse().unwrap());

        let res = list_sessions(State(state.clone()), a.clone()).await.ok().unwrap();
        let body: Value = serde_json::from_slice(&axum::body::to_bytes(res.into_body(), 1 << 20).await.unwrap()).unwrap();
        let current: Vec<bool> = body["sessions"].as_array().unwrap().iter().map(|s| s["current"].as_bool().unwrap()).collect();
        assert_eq!(current, [true, false]);

        // b's session is left alone; a's other one goes
        remove_session(State(state.clone()), a.clone(), Path(ids[2])).await.ok().unwrap();
        remove_session(State(state.clone()), a.clone(), Path(ids[1])).await.ok().unwrap();
        let left: Vec<i64> = sqlx::query_scalar("select id from sessions order by id").fetch_all(&pool).await.unwrap();
        assert_eq!(left, [ids[0], ids[2]]);
    }

    /// An answer as a client reads it: its status and its JSON body.
    async fn answer(r: Result<Response, ApiError>) -> (StatusCode, Value) {
        let r = r.into_response();
        let status = r.status();
        (status, serde_json::from_slice(&axum::body::to_bytes(r.into_body(), 1 << 20).await.unwrap()).unwrap())
    }

    /// The site and other clients read a refusal's status and its `error` text (web lib/messages.ts matches the text).
    #[sqlx::test(migrations = "../core/migrations")]
    #[ignore = "requires PostgreSQL with pgvector"]
    async fn refusals_keep_their_status_and_message(pool: PgPool) {
        signed_in(&pool).await;
        let state = test_state(pool.clone());
        let ip = crate::limit::ClientIp([203, 0, 113, 20].into());
        let refused = |status: StatusCode, message: &str| (status, json!({ "ok": false, "error": message }));
        let sign_in = |name: &str| login(State(state.clone()), ip, HeaderMap::new(), Json(Login { login: name.into(), password: "0123456789".into(), keep: false }));

        assert_eq!(answer(sign_in("nobody").await).await, refused(StatusCode::UNAUTHORIZED, "Incorrect username or password."));
        for _ in 0..MAX_FAILURES {
            state.throttle.fail("locked");
        }
        assert_eq!(answer(sign_in("locked").await).await, refused(StatusCode::TOO_MANY_REQUESTS, "Too many attempts. Try again later."));

        // a sign-up form, from an address of its own: one address has five attempts
        let sign_up = |display_name: &str, username: &str, email: &str, password: &str| {
            let form = Register { display_name: display_name.into(), username: username.into(), email: email.into(), password: password.into(), keep: false };
            register(State(state.clone()), crate::limit::ClientIp([203, 0, 113, 21].into()), HeaderMap::new(), Json(form))
        };
        let bad = |message: &str| refused(StatusCode::BAD_REQUEST, message);
        assert_eq!(answer(sign_up(" ", "ann", "ann@example.test", "0123456789").await).await, bad("Display name is required."));
        assert_eq!(answer(sign_up("Ann", "an", "ann@example.test", "0123456789").await).await, bad("Username must be 3-24 letters, digits or underscores."));
        assert_eq!(answer(sign_up("Ann", "ann", "ann", "0123456789").await).await, bad("Invalid email."));
        assert_eq!(answer(sign_up("Ann", "ann", "ann@example.test", "short").await).await, bad("Your password must be at least 10 characters."));
        // account 'a' has this email
        assert_eq!(
            answer(sign_up("Ann", "ann", "a@example.test", "0123456789").await).await,
            refused(StatusCode::CONFLICT, "Username or email is already taken.")
        );

        let delete = delete_account(State(state.clone()), crate::limit::ClientIp([203, 0, 113, 22].into()), headers(), Json(DeleteAccount { password: "x".into() }));
        assert_eq!(answer(delete.await).await, refused(StatusCode::FORBIDDEN, "Password is incorrect."));

        sqlx::query("insert into saved_searches (user_id, name, query) select u.id, 'n', 'q=' || g from users u, generate_series(1, $1) g")
            .bind(MAX_SAVED_SEARCHES)
            .execute(&pool)
            .await
            .unwrap();
        let one_more = add_search(State(state.clone()), headers(), Json(NewSearch { name: "n".into(), query: "q=x".into() }));
        assert_eq!(answer(one_more.await).await, refused(StatusCode::CONFLICT, "Saved search limit reached."));

        assert_eq!(answer(list_favourites(State(state.clone()), HeaderMap::new()).await).await, refused(StatusCode::UNAUTHORIZED, "Sign in required"));
        assert_eq!(answer(add_favourite(State(state.clone()), headers(), Path(999)).await).await, refused(StatusCode::NOT_FOUND, "Unknown item."));
        assert_eq!(
            answer(put_preferences(State(state.clone()), headers(), Json(json!({ "evil": "x" }))).await).await,
            refused(StatusCode::BAD_REQUEST, "unknown preference")
        );
    }
}
