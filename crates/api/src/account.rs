//! Password accounts, cookie sessions, preferences, favourites and saved searches.
//! Not yet: Discord/X OAuth (needs app credentials) and password-reset email (needs a mail service).
//!
//! CSRF: the session cookie is SameSite=Lax and CORS never allows credentials, so cross-site
//! requests can't carry it; body-carrying mutations additionally require JSON.

use std::collections::HashMap;
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

/// Failed logins per lowercased login name: (count, window start)
#[derive(Default)]
pub struct LoginThrottle(Mutex<HashMap<String, (u32, Instant)>>);

impl LoginThrottle {
    fn blocked(&self, key: &str) -> bool {
        let map = self.0.lock().unwrap();
        map.get(key).is_some_and(|(n, t)| *n >= MAX_FAILURES && t.elapsed() < LOCKOUT)
    }
    fn fail(&self, key: &str) {
        let mut map = self.0.lock().unwrap();
        let e = map.entry(key.to_owned()).or_insert((0, Instant::now()));
        if e.1.elapsed() >= LOCKOUT {
            *e = (0, Instant::now());
        }
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

fn token_hash(token: &str) -> Vec<u8> {
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

pub async fn hash_password(password: String) -> Result<String, ApiError> {
    Ok(tokio::task::spawn_blocking(move || Argon2::default().hash_password(password.as_bytes()).map(|h| h.to_string()))
        .await?
        .map_err(|e| anyhow::anyhow!("hash: {e}"))?)
}

async fn verify_password(password: String, hash: String) -> Result<bool, ApiError> {
    Ok(tokio::task::spawn_blocking(move || Argon2::default().verify_password(password.as_bytes(), hash.as_str()).is_ok()).await?)
}

/// Creates a session row and returns the Set-Cookie value.
/// If `verified_hash` is Some, the session is only inserted if the user's current password hash matches it,
/// preventing an in-flight login from creating a session after a concurrent password reset.
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
            // Row lock on `users` serializes with admin `reset_password` (which runs `UPDATE users SET password_hash`):
            // if reset commits first, `FOR UPDATE` sees the new hash and rejects;
            // if login locks first, reset's UPDATE blocks until login commits, and then reset's DELETE removes this session.
            let mut tx = pool.begin().await?;
            let current_hash: Option<String> = sqlx::query_scalar(
                "select password_hash from users where id = $1 for update",
            )
            .bind(user_id)
            .fetch_optional(&mut *tx)
            .await?;

            if current_hash.as_deref() != Some(expected_hash) {
                return Ok(None);
            }

            sqlx::query(
                "insert into sessions (token_hash, user_id, expires_at, user_agent, ip) values ($1, $2, now() + $3, $4, $5)",
            )
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
            sqlx::query(
                "insert into sessions (token_hash, user_id, expires_at, user_agent, ip) values ($1, $2, now() + $3, $4, $5)",
            )
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

fn valid_password(p: &str) -> bool {
    (10..=256).contains(&p.chars().count())
}

fn valid_display_name(n: &str) -> bool {
    (1..=50).contains(&n.trim().chars().count())
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
        let ok = s.is_empty() && ["ui", "language", "currency", "theme", "avatar", "adult"].contains(&k.as_str())
            || match k.as_str() {
            // interface language
            "ui" => ["zh", "en", "ja", "ko"].contains(&s),
            // item text language
            "language" => ["original", "zh", "en", "ja", "ko"].contains(&s),
            "currency" => s.len() == 3 && s.chars().all(|c| c.is_ascii_uppercase()),
            "theme" => (1..=20).contains(&s.len()) && s.chars().all(|c| c.is_ascii_lowercase()),
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
struct Register {
    display_name: String,
    username: String,
    email: String,
    password: String,
    #[serde(default)]
    keep: bool,
}

async fn register(State(s): State<Arc<AppState>>, headers: HeaderMap, Json(b): Json<Register>) -> Result<Response, ApiError> {
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
struct Login {
    login: String,
    password: String,
    #[serde(default)]
    keep: bool,
}

async fn login(State(s): State<Arc<AppState>>, headers: HeaderMap, Json(b): Json<Login>) -> Result<Response, ApiError> {
    let key = b.login.trim().to_lowercase();
    if s.throttle.blocked(&key) {
        return Ok(err(StatusCode::TOO_MANY_REQUESTS, "Too many attempts. Try again later."));
    }
    let row: Option<(i64, String)> = sqlx::query_as("select id, password_hash from users where lower(username) = $1 or lower(email) = $1")
        .bind(&key)
        .fetch_optional(&s.pool)
        .await?;
    // verify against a dummy hash when the user is unknown, so timing doesn't reveal which logins exist
    let (id, hash) = row.unwrap_or((0, s.dummy_hash.clone()));
    if !verify_password(b.password, hash.clone()).await? || id == 0 {
        s.throttle.fail(&key);
        return Ok(err(StatusCode::UNAUTHORIZED, "Incorrect username or password."));
    }
    s.throttle.clear(&key);
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

async fn me(State(s): State<Arc<AppState>>, headers: HeaderMap) -> Result<Response, ApiError> {
    let u = match require_user(&s, &headers).await? {
        Ok(u) => u,
        Err(r) => return Ok(r),
    };
    let (username, email, display_name, preferences, is_admin): (String, String, String, Value, bool) =
        sqlx::query_as("select username, email, display_name, preferences, is_admin from users where id = $1").bind(u.id).fetch_one(&s.pool).await?;
    Ok(Json(json!({ "id": u.id, "username": username, "email": email, "display_name": display_name, "preferences": preferences, "is_admin": is_admin })).into_response())
}

#[derive(Deserialize)]
struct Profile {
    display_name: Option<String>,
    username: Option<String>,
}

async fn update_profile(State(s): State<Arc<AppState>>, headers: HeaderMap, Json(b): Json<Profile>) -> Result<Response, ApiError> {
    let u = match require_user(&s, &headers).await? {
        Ok(u) => u,
        Err(r) => return Ok(r),
    };
    if b.display_name.as_deref().is_some_and(|n| !valid_display_name(n)) {
        return Ok(err(StatusCode::BAD_REQUEST, "Display name is required."));
    }
    if b.username.as_deref().is_some_and(|n| !valid_username(n)) {
        return Ok(err(StatusCode::BAD_REQUEST, "Username must be 3-24 letters, digits or underscores."));
    }
    match sqlx::query("update users set display_name = coalesce($2, display_name), username = coalesce($3, username) where id = $1")
        .bind(u.id)
        .bind(b.display_name.as_deref().map(str::trim))
        .bind(&b.username)
        .execute(&s.pool)
        .await
    {
        Ok(_) => Ok(Json(json!({ "ok": true })).into_response()),
        Err(e) if is_unique_violation(&e) => Ok(err(StatusCode::CONFLICT, "Username is already taken.")),
        Err(e) => Err(e.into()),
    }
}

#[derive(Deserialize)]
struct ChangePassword {
    current: String,
    new: String,
}

async fn change_password(State(s): State<Arc<AppState>>, headers: HeaderMap, Json(b): Json<ChangePassword>) -> Result<Response, ApiError> {
    let u = match require_user(&s, &headers).await? {
        Ok(u) => u,
        Err(r) => return Ok(r),
    };
    if !valid_password(&b.new) {
        return Ok(err(StatusCode::BAD_REQUEST, "Your password must be at least 10 characters."));
    }
    let hash: String = sqlx::query_scalar("select password_hash from users where id = $1").bind(u.id).fetch_one(&s.pool).await?;
    if !verify_password(b.current, hash).await? {
        return Ok(err(StatusCode::FORBIDDEN, "Current password is incorrect."));
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

async fn delete_account(State(s): State<Arc<AppState>>, headers: HeaderMap, Json(b): Json<DeleteAccount>) -> Result<Response, ApiError> {
    let u = match require_user(&s, &headers).await? {
        Ok(u) => u,
        Err(r) => return Ok(r),
    };
    let hash: String = sqlx::query_scalar("select password_hash from users where id = $1").bind(u.id).fetch_one(&s.pool).await?;
    if !verify_password(b.password, hash).await? {
        return Ok(err(StatusCode::FORBIDDEN, "Password is incorrect."));
    }
    // sessions, favourites and saved searches cascade
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

pub async fn favourite_ids(pool: &PgPool, user_id: i64) -> Result<Vec<i64>, ApiError> {
    Ok(sqlx::query_scalar("select item_id from favourites where user_id = $1 order by created_at desc").bind(user_id).fetch_all(pool).await?)
}

async fn list_favourites(State(s): State<Arc<AppState>>, headers: HeaderMap) -> Result<Response, ApiError> {
    let u = match require_user(&s, &headers).await? {
        Ok(u) => u,
        Err(r) => return Ok(r),
    };
    Ok(Json(json!({ "ids": favourite_ids(&s.pool, u.id).await? })).into_response())
}

async fn add_favourite(State(s): State<Arc<AppState>>, headers: HeaderMap, Path(item): Path<i64>) -> Result<Response, ApiError> {
    let u = match require_user(&s, &headers).await? {
        Ok(u) => u,
        Err(r) => return Ok(r),
    };
    match sqlx::query("insert into favourites (user_id, item_id) values ($1, $2) on conflict do nothing").bind(u.id).bind(item).execute(&s.pool).await {
        Ok(_) => Ok(Json(json!({ "ok": true })).into_response()),
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
    if !(1..=100).contains(&b.name.trim().chars().count()) || query.len() > 2000 || query.contains(char::is_whitespace) {
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

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/account/register", post(register))
        .route("/api/account/login", post(login))
        .route("/api/account/logout", post(logout))
        .route("/api/account", get(me).delete(delete_account))
        .route("/api/account/profile", axum::routing::patch(update_profile))
        .route("/api/account/password", post(change_password))
        .route("/api/account/preferences", get(get_preferences).put(put_preferences))
        .route("/api/account/favourites", get(list_favourites))
        .route("/api/account/favourites/{id}", put(add_favourite).delete(remove_favourite))
        .route("/api/account/searches", get(list_searches).post(add_search))
        .route("/api/account/searches/{id}", axum::routing::delete(remove_search))
}

/// Hash used to equalise login timing for unknown users.
pub fn dummy_hash() -> String {
    Argon2::default().hash_password(b"timing-equaliser-not-a-password").expect("argon2").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validators() {
        assert!(valid_username("abc_123") && !valid_username("ab") && !valid_username("has space") && !valid_username(&"a".repeat(25)));
        assert!(valid_email("a@b.co") && !valid_email("nope") && !valid_email("a @b.co") && !valid_email("@b.co"));
        assert!(valid_password("0123456789") && !valid_password("short"));
    }

    #[test]
    fn preferences_whitelist() {
        assert!(validate_preferences(&json!({"language": "ja", "currency": "USD", "theme": "charcoal", "avatar": "manuka"})).is_ok());
        assert!(validate_preferences(&json!({"avatar": "", "language": ""})).is_ok());
        assert!(validate_preferences(&json!({"evil": ""})).is_err());
        assert!(validate_preferences(&json!({"language": "fr"})).is_err());
        assert!(validate_preferences(&json!({"language": "zh"})).is_ok());
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
}
