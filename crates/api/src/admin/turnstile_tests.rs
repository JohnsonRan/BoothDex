//! The Turnstile settings through their handlers, against a database and a stand-in for siteverify.

use std::sync::atomic::{AtomicUsize, Ordering};

use axum::extract::State;
use axum::http::{StatusCode, header};

use super::*;
use crate::account;

/// A session for a new user named `name`, an admin or not.
pub(super) async fn session(pool: &sqlx::PgPool, name: &str, is_admin: bool) -> HeaderMap {
    let id: i64 = sqlx::query_scalar("insert into users (username, email, display_name, password_hash, is_admin) values ($1, $1 || '@example.test', $1, 'x', $2) returning id")
        .bind(name)
        .bind(is_admin)
        .fetch_one(pool)
        .await
        .unwrap();
    sqlx::query("insert into sessions (token_hash, user_id, expires_at) values ($1, $2, now() + interval '1 day')")
        .bind(account::token_hash(&format!("tok-{name}")))
        .bind(id)
        .execute(pool)
        .await
        .unwrap();
    let mut h = HeaderMap::new();
    h.insert(header::COOKIE, format!("session=tok-{name}").parse().unwrap());
    h
}

pub(super) async fn admin(pool: &sqlx::PgPool) -> HeaderMap {
    session(pool, "boss", true).await
}

/// siteverify, counting what it is asked. The secrets "0x-right", "0x-new" and "0x-testing" are known; any other is
/// refused as a wrong secret. A known secret passes the token "settings-<secret>" for the console's action ("slow-"
/// in front: `arrived` is notified when it comes, and the answer waits for `release`), "in" for sign-in, and as a
/// testing secret answers, "testing-pass" without an action and "testing-fail" as a failure; "failing" is answered
/// HTTP 500; anything else, a token made no widget of this secret made, is refused.
pub(super) struct StandIn {
    pub(super) url: String,
    pub(super) asked: Arc<AtomicUsize>,
    pub(super) arrived: Arc<tokio::sync::Notify>,
    pub(super) release: Arc<tokio::sync::Notify>,
}

pub(super) async fn stand_in() -> StandIn {
    let asked = Arc::new(AtomicUsize::new(0));
    let (arrived, release) = (Arc::new(tokio::sync::Notify::new()), Arc::new(tokio::sync::Notify::new()));
    let (count, came, go) = (asked.clone(), arrived.clone(), release.clone());
    let app = axum::Router::new().route(
        "/",
        axum::routing::post(move |Json(b): Json<Value>| async move {
            count.fetch_add(1, Ordering::SeqCst);
            let secret = b["secret"].as_str().unwrap_or_default();
            let token = b["response"].as_str().unwrap_or_default();
            if !["0x-right", "0x-new", "0x-testing"].contains(&secret) {
                return Ok(Json(json!({ "success": false, "error-codes": ["invalid-input-secret"] })));
            }
            let token = match token.strip_prefix("slow-") {
                Some(rest) => {
                    came.notify_one();
                    go.notified().await;
                    rest
                }
                None => token,
            };
            let testing = json!({ "result_with_testing_key": true });
            Ok(Json(match token {
                "failing" => return Err(axum::http::StatusCode::INTERNAL_SERVER_ERROR),
                "in" => json!({ "success": true, "action": "sign-in" }),
                "testing-pass" if secret == "0x-testing" => json!({ "success": true, "metadata": testing }),
                "testing-fail" if secret == "0x-testing" => json!({ "success": false, "error-codes": ["invalid-input-response"], "metadata": testing }),
                t if t == format!("settings-{secret}") => json!({ "success": true, "action": "settings" }),
                _ => json!({ "success": false, "error-codes": ["invalid-input-response"] }),
            }))
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    StandIn { url, asked, arrived, release }
}

pub(super) const ADMIN_IP: ClientIp = ClientIp(std::net::IpAddr::V4(std::net::Ipv4Addr::new(203, 0, 113, 5)));

pub(super) async fn body(r: Result<Response, ApiError>) -> (StatusCode, Value) {
    let r = r.into_response();
    (r.status(), serde_json::from_slice(&axum::body::to_bytes(r.into_body(), 1 << 20).await.unwrap()).unwrap())
}

#[sqlx::test(migrations = "../core/migrations")]
#[ignore = "requires PostgreSQL with pgvector"]
async fn the_secret_is_write_only_and_tested(pool: sqlx::PgPool) {
    let h = admin(&pool).await;
    let s = Arc::try_unwrap(account::test_state(pool.clone())).ok().unwrap();
    let s = Arc::new(AppState { siteverify_url: stand_in().await.url, ..s });
    let save = |site_key: Option<&str>, secret: Option<&str>| {
        let b = json!({ "site_key": site_key, "secret": secret });
        put(State(s.clone()), ADMIN_IP, h.clone(), Json(serde_json::from_value(b).unwrap()))
    };

    assert_eq!(body(test(State(s.clone()), h.clone()).await).await.1["code"], "verification_unavailable");
    let (status, saved) = body(save(Some(" 0x-site "), Some("0x-wrong-secret")).await).await;
    assert_eq!((status, &saved["site_key"], &saved["secret_set"]), (StatusCode::OK, &json!("0x-site"), &json!(true)));
    assert!(!saved.to_string().contains("0x-wrong-secret"), "{saved}");
    assert_eq!(body(test(State(s.clone()), h.clone()).await).await.1, json!({ "ok": false, "codes": ["invalid-input-secret"] }));

    // a blank secret keeps the stored one; a new one replaces it
    assert_eq!(body(save(Some("0x-site"), Some(" ")).await).await.1["secret_hint"], saved["secret_hint"]);
    body(save(None, Some("0x-right")).await).await;
    assert_eq!(body(test(State(s.clone()), h.clone()).await).await.1, json!({ "ok": true, "codes": ["invalid-input-response"] }));
    assert_eq!(turnstile::stored(&pool).await.unwrap(), json!({ "site_key": "0x-site", "secret": "0x-right" }));

    for (site_key, secret) in [(Some("0x site"), None), (None, Some("0x\0")), (Some(&*"k".repeat(201)), None)] {
        assert_eq!(body(save(site_key, secret).await).await.1["code"], "bad_turnstile_settings");
    }
    // the switch for sign-in and registration is saved alone, beside the keys, and a save without it keeps it
    let switch = |on: bool| {
        let b = json!({ "auth": on, "token": "settings-0x-right", "proved_site_key": "0x-site" });
        put(State(s.clone()), ADMIN_IP, h.clone(), Json(serde_json::from_value(b).unwrap()))
    };
    assert_eq!(body(save(None, None).await).await.1["auth"], false);
    assert_eq!(body(switch(true).await).await.1["auth"], true);
    assert_eq!(turnstile::stored(&pool).await.unwrap(), json!({ "site_key": "0x-site", "secret": "0x-right", "auth": true }));
    assert_eq!(body(save(Some("0x-site"), None).await).await.1["auth"], true);
    assert!(turnstile::config(&pool).await.unwrap().auth_keys().is_some());
    assert_eq!(body(switch(false).await).await.1["auth"], false);
    // the switch for stricter limits is saved the same way, apart from the other, and reaches the API's copy that
    // the rate limits read
    let strict = |on: bool| put(State(s.clone()), ADMIN_IP, h.clone(), Json(serde_json::from_value(json!({ "strict_limits": on })).unwrap()));
    assert!(s.turnstile_config().await.unwrap().strict_keys().is_none());
    let (_, saved) = body(strict(true).await).await;
    assert_eq!((&saved["strict_limits"], &saved["auth"]), (&json!(true), &json!(false)));
    assert!(s.turnstile_config().await.unwrap().strict_keys().is_some());
    assert_eq!(body(save(Some("0x-site"), None).await).await.1["strict_limits"], true);
    assert_eq!(body(strict(false).await).await.1["strict_limits"], false);
    assert!(s.turnstile_config().await.unwrap().strict_keys().is_none());

    // an empty site key closes the guestbook again
    assert_eq!(body(save(Some(""), None).await).await.1["site_key"], Value::Null);
    assert_eq!(turnstile::config(&pool).await.unwrap().keys(), None);
}

/// The gate comes after the admin check: a member or a signed-out caller is refused as on any admin route, and
/// siteverify is not asked.
#[sqlx::test(migrations = "../core/migrations")]
#[ignore = "requires PostgreSQL with pgvector"]
async fn only_an_admin_reaches_the_check(pool: sqlx::PgPool) {
    let StandIn { url, asked, .. } = stand_in().await;
    let s = Arc::try_unwrap(account::test_state(pool.clone())).ok().unwrap();
    let s = Arc::new(AppState { siteverify_url: url, ..s });
    sqlx::query("insert into app_settings (key, value) values ('turnstile', $1)")
        .bind(json!({ "site_key": "0x-site", "secret": "0x-right" }))
        .execute(&pool)
        .await
        .unwrap();
    let member = session(&pool, "ann", false).await;
    let b = || serde_json::from_value(json!({ "auth": true, "token": "settings-0x-right", "proved_site_key": "0x-site" })).unwrap();
    for (h, status) in [(HeaderMap::new(), StatusCode::UNAUTHORIZED), (member, StatusCode::FORBIDDEN)] {
        assert_eq!(body(put(State(s.clone()), ADMIN_IP, h, Json(b())).await).await.0, status);
    }
    assert_eq!(asked.load(Ordering::SeqCst), 0);
    assert_eq!(turnstile::stored(&pool).await.unwrap(), json!({ "site_key": "0x-site", "secret": "0x-right" }));
}
