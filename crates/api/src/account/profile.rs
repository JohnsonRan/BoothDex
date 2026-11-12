use std::sync::Arc;

use axum::extract::State;
use axum::http::{HeaderMap, header};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Deserialize;
use serde_json::{Value, json};

use super::password::{check_own_password, hash_password, valid_password};
use super::session::clear_cookie;
use super::sign_in::valid_display_name;
use super::{cookie_token, current_user, require_user, token_hash};
use crate::error::Refusal;
use crate::{ApiError, AppState};

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

/// The signed-in account, or `null` without a session: every page asks on load, and a 401 would log a failed
/// request in each signed-out visitor's console.
pub(super) async fn me(State(s): State<Arc<AppState>>, headers: HeaderMap) -> Result<Response, ApiError> {
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
pub(super) struct Profile {
    display_name: Option<String>,
}

pub(super) async fn update_profile(State(s): State<Arc<AppState>>, headers: HeaderMap, Json(b): Json<Profile>) -> Result<Response, ApiError> {
    let u = require_user(&s, &headers).await?;
    if b.display_name.as_deref().is_some_and(|n| !valid_display_name(n)) {
        return Err(Refusal::DisplayNameRequired.into());
    }
    sqlx::query("update users set display_name = coalesce($2, display_name) where id = $1")
        .bind(u.id)
        .bind(b.display_name.as_deref().map(str::trim))
        .execute(&s.pool)
        .await?;
    Ok(Json(json!({ "ok": true })).into_response())
}

#[derive(Deserialize)]
pub(super) struct ChangePassword {
    pub(super) current: String,
    pub(super) new: String,
}

pub(super) async fn change_password(
    State(s): State<Arc<AppState>>,
    crate::limit::ClientIp(ip): crate::limit::ClientIp,
    headers: HeaderMap,
    Json(b): Json<ChangePassword>,
) -> Result<Response, ApiError> {
    let u = require_user(&s, &headers).await?;
    if !valid_password(&b.new) {
        return Err(Refusal::PasswordTooShort.into());
    }
    if b.new == b.current {
        return Err(Refusal::PasswordUnchanged.into());
    }
    check_own_password(&s, ip, u.id, b.current, Refusal::WrongCurrentPassword).await?;
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
pub(super) struct DeleteAccount {
    pub(super) password: String,
}

pub(super) async fn delete_account(
    State(s): State<Arc<AppState>>,
    crate::limit::ClientIp(ip): crate::limit::ClientIp,
    headers: HeaderMap,
    Json(b): Json<DeleteAccount>,
) -> Result<Response, ApiError> {
    let u = require_user(&s, &headers).await?;
    // ADMIN_USERNAMES makes an account admin by its name: once deleted, whoever registered the name next would be one
    let is_admin: bool = sqlx::query_scalar("select is_admin from users where id = $1").bind(u.id).fetch_one(&s.pool).await?;
    if is_admin {
        return Err(Refusal::AdminCannotDelete.into());
    }
    check_own_password(&s, ip, u.id, b.password, Refusal::WrongPassword).await?;
    // sessions, favourites, viewed items and saved searches cascade
    sqlx::query("delete from users where id = $1").bind(u.id).execute(&s.pool).await?;
    Ok(([(header::SET_COOKIE, clear_cookie())], Json(json!({ "ok": true }))).into_response())
}

pub(super) async fn get_preferences(State(s): State<Arc<AppState>>, headers: HeaderMap) -> Result<Response, ApiError> {
    let u = require_user(&s, &headers).await?;
    let prefs: Value = sqlx::query_scalar("select preferences from users where id = $1").bind(u.id).fetch_one(&s.pool).await?;
    Ok(Json(prefs).into_response())
}

/// Merges the given keys; an empty string removes a key.
pub(super) async fn put_preferences(State(s): State<Arc<AppState>>, headers: HeaderMap, Json(b): Json<Value>) -> Result<Response, ApiError> {
    let u = require_user(&s, &headers).await?;
    if let Err(msg) = validate_preferences(&b) {
        return Err(Refusal::BadPreferences.with(msg));
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

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use sqlx::PgPool;

    use super::*;
    use crate::account::test_state;
    use crate::account::tests::{headers, signed_in};

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
}
