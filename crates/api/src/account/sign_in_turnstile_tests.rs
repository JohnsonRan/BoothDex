//! Sign-in and registration behind Turnstile (turnstile.rs `require_for_account`): off until an admin's switch is on
//! with both keys set, a token Cloudflare judges bad is refused before the account is looked at, and a check
//! siteverify cannot make lets the request through.

use std::net::IpAddr;
use std::sync::Arc;

use axum::Json;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use serde_json::{Value, json};
use sqlx::PgPool;

use super::super::password::MAX_FAILURES;
use super::*;
use crate::account::test_state;
use crate::account::tests::answer;
use crate::limit::ClientIp;

const PASSWORD: &str = "0123456789";

/// A stand-in for siteverify. The token "in" passes for sign-in and "up" for registration; "failing" is answered
/// HTTP 500, "no-secret" as a wrong secret key is, "broken" as Cloudflare's own failure is, "turned-down" HTTP 400
/// and "slow-down" HTTP 429; a token longer than Cloudflare issues would pass, to show it is never sent; anything
/// else, the empty token included, is a bad token.
async fn siteverify_stand_in() -> String {
    let app = axum::Router::new().route(
        "/",
        axum::routing::post(|Json(b): Json<Value>| async move {
            let refused = |code: &str| json!({ "success": false, "error-codes": [code] });
            let body = match b["response"].as_str() {
                Some("in") => json!({ "success": true, "action": SIGN_IN_ACTION }),
                Some("up") => json!({ "success": true, "action": REGISTER_ACTION }),
                Some("failing") => return Err(StatusCode::INTERNAL_SERVER_ERROR),
                Some("turned-down") => return Err(StatusCode::BAD_REQUEST),
                Some("slow-down") => return Err(StatusCode::TOO_MANY_REQUESTS),
                Some(long) if long.len() > 2048 => json!({ "success": true, "action": SIGN_IN_ACTION }),
                Some("no-secret") => refused("invalid-input-secret"),
                Some("broken") => refused("internal-error"),
                Some("") => refused("missing-input-response"),
                _ => refused("invalid-input-response"),
            };
            Ok(Json(body))
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    url
}

async fn state(pool: PgPool, siteverify_url: String) -> Arc<AppState> {
    let state = Arc::try_unwrap(test_state(pool)).ok().unwrap();
    Arc::new(AppState { siteverify_url, ..state })
}

async fn settings(pool: &PgPool, value: Value) {
    sqlx::query("insert into app_settings (key, value) values ('turnstile', $1) on conflict (key) do update set value = excluded.value")
        .bind(value)
        .execute(pool)
        .await
        .unwrap();
}

fn checked() -> Value {
    json!({ "site_key": "0x-site", "secret": "0x-secret", "auth": true })
}

/// Each request from its own address, so the per-address limit never answers for the check.
fn ip(n: u8) -> ClientIp {
    ClientIp(IpAddr::from([203, 0, 113, n]))
}

async fn sign_in(s: &Arc<AppState>, n: u8, password: &str, token: &str) -> (StatusCode, Option<String>) {
    let form = Login { login: "ann".into(), password: password.into(), keep: false, token: token.into() };
    let (status, body) = answer(login(State(s.clone()), ip(n), HeaderMap::new(), Json(form)).await).await;
    (status, body["code"].as_str().map(str::to_owned))
}

async fn sign_up(s: &Arc<AppState>, n: u8, username: &str, token: &str) -> (StatusCode, Option<String>) {
    let form = Register {
        display_name: "A".into(),
        username: username.into(),
        email: format!("{username}@example.test"),
        password: PASSWORD.into(),
        keep: false,
        token: token.into(),
    };
    let (status, body) = answer(register(State(s.clone()), ip(n), HeaderMap::new(), Json(form)).await).await;
    (status, body["code"].as_str().map(str::to_owned))
}

async fn site_key(s: &Arc<AppState>) -> Value {
    answer(registration(State(s.clone())).await).await.1["site_key"].clone()
}

fn refused() -> (StatusCode, Option<String>) {
    (StatusCode::FORBIDDEN, Some("verification_failed".into()))
}

const OK: (StatusCode, Option<String>) = (StatusCode::OK, None);

/// Whether each session's sign-in or registration passed the check, oldest first.
async fn passed(pool: &PgPool) -> Vec<bool> {
    sqlx::query_scalar("select turnstile_passed from sessions order by id").fetch_all(pool).await.unwrap()
}

/// Keys set for the guestbook change nothing here until the switch is on, and the switch does nothing without both.
#[sqlx::test(migrations = "../core/migrations")]
#[ignore = "requires PostgreSQL with pgvector"]
async fn unchecked_until_the_switch_is_on_with_both_keys(pool: PgPool) {
    let s = state(pool.clone(), siteverify_stand_in().await).await;
    for (n, value) in [json!({}), json!({ "site_key": "0x-site", "secret": "0x-secret" }), json!({ "site_key": "0x-site", "auth": true })].into_iter().enumerate() {
        settings(&pool, value.clone()).await;
        assert_eq!(site_key(&s).await, Value::Null, "{value}");
        assert_eq!(sign_up(&s, n as u8, &format!("user{n}"), "").await, OK, "{value}");
    }
    sqlx::query("insert into users (username, email, display_name, password_hash) select 'ann', 'ann@example.test', 'Ann', password_hash from users limit 1")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(sign_in(&s, 10, PASSWORD, "").await, OK);

    settings(&pool, checked()).await;
    assert_eq!(site_key(&s).await, json!("0x-site"));
    assert_eq!(sign_in(&s, 11, PASSWORD, "").await, refused());
}

#[sqlx::test(migrations = "../core/migrations")]
#[ignore = "requires PostgreSQL with pgvector"]
async fn a_bad_token_is_refused_before_the_account_is_looked_at(pool: PgPool) {
    let s = state(pool.clone(), siteverify_stand_in().await).await;
    assert_eq!(sign_up(&s, 1, "ann", "").await, OK);
    settings(&pool, checked()).await;

    // no token, a wrong one, one made for the other form, a request siteverify turns down, and a token too long to
    // be a widget's
    let long = "x".repeat(2049);
    for (n, token) in [(2, ""), (3, "nope"), (4, "up"), (5, "turned-down"), (6, "slow-down"), (7, &*long)] {
        assert_eq!(sign_in(&s, n, PASSWORD, token).await, refused(), "{token}");
        assert_eq!(sign_up(&s, n + 10, "bob", if token == "up" { "in" } else { token }).await, refused(), "{token}");
    }
    assert_eq!(sqlx::query_scalar::<_, i64>("select count(*) from users").fetch_one(&pool).await.unwrap(), 1);

    // refused tokens with a wrong password, more than a lockout takes: none counted against the account
    for n in 0..=MAX_FAILURES as u8 {
        assert_eq!(sign_in(&s, 30 + n, "wrong-password", "nope").await, refused());
    }
    assert!(!s.throttle.blocked("ann"));
    // an unknown name is answered the same
    let ghost = Login { login: "ghost".into(), password: PASSWORD.into(), keep: false, token: "nope".into() };
    assert_eq!(answer(login(State(s.clone()), ip(60), HeaderMap::new(), Json(ghost)).await).await.1["code"], "verification_failed");

    assert_eq!(sign_in(&s, 61, PASSWORD, "in").await, OK);
    assert_eq!(sign_up(&s, 62, "bob", "up").await, OK);
    // a passed check does not stand in for the password, and its failures count against the account
    for n in 0..MAX_FAILURES as u8 {
        assert_eq!(sign_in(&s, 70 + n, "wrong-password", "in").await, (StatusCode::UNAUTHORIZED, Some("bad_credentials".into())));
    }
    assert!(s.throttle.blocked("ann"));
    // the registration before the switch, then the sign-in and the registration that passed
    assert_eq!(passed(&pool).await, [false, true, true]);
}

/// The way out of a check that cannot be made: siteverify failing, refusing the secret key or not answering lets the
/// request through, so an admin still reaches the console; the password is still asked.
#[sqlx::test(migrations = "../core/migrations")]
#[ignore = "requires PostgreSQL with pgvector"]
async fn a_check_that_cannot_be_made_lets_the_request_through(pool: PgPool) {
    let s = state(pool.clone(), siteverify_stand_in().await).await;
    assert_eq!(sign_up(&s, 1, "ann", "").await, OK);
    settings(&pool, checked()).await;
    for (n, token) in [(2, "failing"), (3, "no-secret"), (4, "broken")] {
        assert_eq!(sign_in(&s, n, PASSWORD, token).await, OK, "{token}");
        assert_eq!(sign_up(&s, n + 10, &format!("user{n}"), token).await, OK, "{token}");
        assert_eq!(sign_in(&s, n + 20, "wrong-password", token).await, (StatusCode::UNAUTHORIZED, Some("bad_credentials".into())), "{token}");
    }

    // nothing listens here, as when Cloudflare cannot be reached; the widget would not have loaded either, so no token
    let closed = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let unreachable = format!("http://{}/", closed.local_addr().unwrap());
    drop(closed);
    let s = state(pool.clone(), unreachable).await;
    assert_eq!(sign_in(&s, 40, PASSWORD, "").await, OK);
    assert_eq!(sign_in(&s, 41, "wrong-password", "").await, (StatusCode::UNAUTHORIZED, Some("bad_credentials".into())));
    // none of these sessions passed a check, so none gets today's rate limits (pass.rs)
    assert_eq!(passed(&pool).await, [false; 8]);
}
