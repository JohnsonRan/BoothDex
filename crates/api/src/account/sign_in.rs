use std::sync::Arc;

use axum::extract::State;
use axum::http::{HeaderMap, header};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Deserialize;
use serde_json::json;
use sqlx::PgPool;

use super::password::{hash_password, log_attempt, log_failure, valid_password, verify_password};
use super::session::start_session;
use crate::error::Refusal;
use crate::{ApiError, AppState};

fn valid_username(u: &str) -> bool {
    (3..=24).contains(&u.chars().count()) && u.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn valid_email(e: &str) -> bool {
    e.len() <= 254 && e.split_once('@').is_some_and(|(a, d)| !a.is_empty() && d.contains('.') && e.bytes().all(|b| b.is_ascii_graphic()))
}

fn valid_login(l: &str) -> bool {
    l.len() <= 254
}

pub(super) fn valid_display_name(n: &str) -> bool {
    (1..=50).contains(&n.trim().chars().count())
}

/// Open unless an admin closed it.
pub async fn registration_open(pool: &PgPool) -> Result<bool, sqlx::Error> {
    let open: Option<Option<bool>> =
        sqlx::query_scalar("select (value->>'open')::boolean from app_settings where key = 'registration'").fetch_optional(pool).await?;
    Ok(open.flatten().unwrap_or(true))
}

fn is_unique_violation(e: &sqlx::Error) -> bool {
    e.as_database_error().and_then(|d| d.code()).as_deref() == Some("23505")
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
    s.limits.auth.check(ip)?;
    if !registration_open(&s.pool).await? {
        return Err(Refusal::RegistrationClosed.into());
    }
    if !valid_display_name(&b.display_name) {
        return Err(Refusal::DisplayNameRequired.into());
    }
    if !valid_username(&b.username) {
        return Err(Refusal::BadUsername.into());
    }
    if !valid_email(&b.email) {
        return Err(Refusal::BadEmail.into());
    }
    if !valid_password(&b.password) {
        return Err(Refusal::PasswordTooShort.into());
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
        Err(e) if is_unique_violation(&e) => return Err(Refusal::NameTaken.into()),
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
    s.limits.auth.check(ip)?;
    if !valid_login(&b.login) {
        return Err(Refusal::BadCredentials.into());
    }
    let typed = b.login.trim().to_lowercase();
    let row: Option<(i64, String, String, bool)> =
        sqlx::query_as("select id, username, password_hash, disabled_at is not null from users where lower(username) = $1 or lower(email) = $1")
            .bind(&typed)
            .fetch_optional(&s.pool)
            .await?;
    let username = row.as_ref().map(|(_, u, _, _)| u.to_lowercase());
    let account = username.as_deref();
    // one budget per account, whichever name was typed; an unknown name counts under the typed text, never logged
    let key = username.clone().unwrap_or(typed);
    let locked = s.throttle.blocked(&key);
    // verify against a dummy hash when the user is unknown, so timing doesn't reveal which logins exist
    let (id, _, hash, disabled) = row.unwrap_or((0, String::new(), s.dummy_hash.clone(), false));
    let ok = verify_password(b.password, hash.clone()).await?;
    // locked: answered and worked exactly as a wrong password
    if locked || !ok || id == 0 {
        let locked_now = s.throttle.fail(&key);
        log_failure(if locked { "sign-in failed (locked)" } else { "sign-in failed" }, account, ip, locked_now);
        return Err(Refusal::BadCredentials.into());
    }
    s.throttle.clear(&key);
    // said only to someone who knows the password
    if disabled {
        log_attempt("sign-in refused (disabled)", account, ip);
        return Err(Refusal::AccountDisabled.into());
    }
    let cookie = match start_session(&s.pool, id, b.keep, &headers, Some(&hash)).await? {
        Some(c) => c,
        None => {
            let locked_now = s.throttle.fail(&key);
            log_failure("sign-in failed (changed during sign-in)", account, ip, locked_now);
            return Err(Refusal::BadCredentials.into());
        }
    };
    log_attempt("sign-in ok", account, ip);
    Ok(([(header::SET_COOKIE, cookie)], Json(json!({ "ok": true }))).into_response())
}

pub(super) async fn registration(State(s): State<Arc<AppState>>) -> Result<Response, ApiError> {
    Ok(Json(json!({ "open": registration_open(&s.pool).await? })).into_response())
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;

    use super::*;
    use crate::account::test_state;
    use crate::account::tests::answer;

    #[test]
    fn validators() {
        assert!(valid_username("abc_123") && !valid_username("ab") && !valid_username("has space") && !valid_username(&"a".repeat(25)));
        assert!(valid_email("a@b.co") && !valid_email("nope") && !valid_email("a @b.co") && !valid_email("@b.co"));
        assert!(valid_email("a.b+c@example.test"));
        for e in ["ärzte@example.test", "ann@bücher.example", "ΑΣ@x.io", "a\0b@x.io", "a\x7fb@x.io"] {
            assert!(!valid_email(e), "{e:?}");
        }
        assert!(valid_login("user@example.com") && valid_login(&"a".repeat(254)) && !valid_login(&"a".repeat(255)));
        assert!(valid_password("0123456789") && !valid_password("short"));
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

    /// An account made before registration refused non-ASCII emails still signs in by its email.
    #[sqlx::test(migrations = "../core/migrations")]
    #[ignore = "requires PostgreSQL with pgvector"]
    async fn a_non_ascii_email_from_before_the_ascii_rule_still_signs_in(pool: PgPool) {
        sqlx::query("insert into users (username, email, display_name, password_hash) values ('zoe', 'zoë@example.test', 'Zoë', $1)")
            .bind(hash_password("0123456789".into()).await.ok().unwrap())
            .execute(&pool)
            .await
            .unwrap();
        let form = Login { login: "ZOË@example.test".into(), password: "0123456789".into(), keep: false };
        let r = login(State(test_state(pool)), crate::limit::ClientIp([198, 51, 100, 1].into()), HeaderMap::new(), Json(form)).await;
        assert_eq!(answer(r).await, (StatusCode::OK, json!({ "ok": true })));
    }

    /// Failures by the username and by the email share one budget per account, and a locked account answers like a
    /// wrong password. Bulk failures are seeded with `fail`, which skips Argon2 and the log.
    #[sqlx::test(migrations = "../core/migrations")]
    #[ignore = "requires PostgreSQL with pgvector"]
    async fn failed_sign_ins_count_against_the_account_and_stay_hidden(pool: PgPool) {
        sqlx::query("insert into users (username, email, display_name, password_hash) values ('Ann', 'ann@example.test', 'Ann', $1)")
            .bind(hash_password("0123456789".into()).await.ok().unwrap())
            .execute(&pool)
            .await
            .unwrap();
        let state = test_state(pool.clone());
        let sign_in = |i: u8, name: &str, password: &str| {
            let form = Login { login: name.into(), password: password.into(), keep: false };
            login(State(state.clone()), crate::limit::ClientIp([198, 51, 100, i].into()), HeaderMap::new(), Json(form))
        };
        let seed = |key: &str, n: u32| (0..n).for_each(|_| _ = state.throttle.fail(key));
        let ok = (StatusCode::OK, json!({ "ok": true }));
        let wrong = (StatusCode::UNAUTHORIZED, json!({ "ok": false, "code": "bad_credentials", "error": "Incorrect username or password." }));

        seed("ann", 9);
        assert_eq!(answer(sign_in(1, " ANN@example.test ", "0123456789").await).await, ok);
        assert_eq!(answer(sign_in(2, "ann", "wrong-password").await).await, wrong);
        assert_eq!(answer(sign_in(3, "ann", "0123456789").await).await, ok);

        seed("ann", 8);
        assert_eq!(answer(sign_in(4, "Ann", "wrong-password").await).await, wrong);
        assert_eq!(answer(sign_in(5, "ann@example.test", "wrong-password").await).await, wrong);
        assert!(state.throttle.blocked("ann"));
        assert_eq!(answer(sign_in(6, "ann", "0123456789").await).await, wrong);
        assert_eq!(answer(sign_in(7, "ann@example.test", "0123456789").await).await, wrong);
        assert_eq!(sqlx::query_scalar::<_, i64>("select count(*) from sessions").fetch_one(&pool).await.unwrap(), 2);

        // the 10th failure of a name no account has, which may hold a password: the log must not show it
        seed("hunter2-secret", 9);
        assert_eq!(answer(sign_in(8, "hunter2-secret", "wrong-password").await).await, wrong);
    }
}
