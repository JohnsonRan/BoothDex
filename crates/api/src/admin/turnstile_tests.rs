//! The Turnstile settings through their handlers, against a database and a stand-in for siteverify.

use std::sync::atomic::{AtomicUsize, Ordering};

use axum::extract::State;
use axum::http::{StatusCode, header};

use super::*;
use crate::account;

/// A session for a new user named `name`, an admin or not.
async fn session(pool: &sqlx::PgPool, name: &str, is_admin: bool) -> HeaderMap {
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

async fn admin(pool: &sqlx::PgPool) -> HeaderMap {
    session(pool, "boss", true).await
}

/// siteverify, counting what it is asked. The secrets "0x-right", "0x-new" and "0x-testing" are known; any other is
/// refused as a wrong secret. A known secret passes the token "settings-<secret>" for the console's action ("slow-"
/// in front: `arrived` is notified when it comes, and the answer waits for `release`), "in" for sign-in, and as a
/// testing secret answers, "testing-pass" without an action and "testing-fail" as a failure; "failing" is answered
/// HTTP 500; anything else, a token made no widget of this secret made, is refused.
struct StandIn {
    url: String,
    asked: Arc<AtomicUsize>,
    arrived: Arc<tokio::sync::Notify>,
    release: Arc<tokio::sync::Notify>,
}

async fn stand_in() -> StandIn {
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

const ADMIN_IP: ClientIp = ClientIp(std::net::IpAddr::V4(std::net::Ipv4Addr::new(203, 0, 113, 5)));

async fn body(r: Result<Response, ApiError>) -> (StatusCode, Value) {
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

/// A key from `TURNSTILE_*` is the one in effect whatever the row holds: changing or clearing the stored site key under
/// it changes nothing in effect and needs no proof, while a new secret beside it does, with the environment's site key.
#[test]
fn a_key_from_the_environment_decides_what_the_save_puts_in_effect() {
    let env = |k: &str| (k == "TURNSTILE_SITE_KEY").then(|| "0x-env-site".to_owned());
    let stored = json!({ "site_key": "0x-db-site", "secret": "0x-right", "auth": true });
    let check = |change: Value, env: &dyn Fn(&str) -> Option<String>| {
        let (before, after) = before_after(&stored, &change, env);
        unproved(&before, &after).map(|(site, secret)| (site.to_owned(), secret.to_owned()))
    };
    assert_eq!(check(json!({ "site_key": "0x-other" }), &env), None);
    assert_eq!(check(json!({ "site_key": "" }), &env), None);
    assert_eq!(check(json!({ "secret": "0x-new" }), &env), Some(("0x-env-site".into(), "0x-new".into())));
    // without the environment the same site key change is a new pair
    assert_eq!(check(json!({ "site_key": "0x-other" }), &|_| None), Some(("0x-other".into(), "0x-right".into())));
}

/// What a save put in effect for sign-in: the stored row's switch and keys.
async fn auth_row(pool: &sqlx::PgPool) -> Value {
    let row = turnstile::stored(pool).await.unwrap();
    json!([row["auth"], row["site_key"], row["secret"]])
}

/// A save that would put a key pair in front of sign-in that was not there before needs a token from the widget
/// with the new site key, passed with the new secret, for the console's own action; a save that leaves sign-in
/// unchecked, or its keys as they were, never does.
#[sqlx::test(migrations = "../core/migrations")]
#[ignore = "requires PostgreSQL with pgvector"]
async fn checking_sign_in_with_new_keys_needs_a_token_for_them(pool: sqlx::PgPool) {
    let h = admin(&pool).await;
    let StandIn { url, asked, .. } = stand_in().await;
    let s = Arc::try_unwrap(account::test_state(pool.clone())).ok().unwrap();
    let s = Arc::new(AppState { siteverify_url: url, ..s });
    let save = |b: Value| put(State(s.clone()), ADMIN_IP, h.clone(), Json(serde_json::from_value(b).unwrap()));
    let asked_now = || asked.load(Ordering::SeqCst);
    let required = |site_key: &str| (StatusCode::FORBIDDEN, json!({ "ok": false, "code": "verification_required", "error": "Pass the verification with these keys first.", "site_key": site_key }));
    let code = |r: (StatusCode, Value)| (r.0, r.1["code"].clone());

    // keys alone, with the switch off, check nothing
    assert_eq!(body(save(json!({ "site_key": "0x-site", "secret": "0x-right" })).await).await.0, StatusCode::OK);
    // the switch alone: refused without a token, before asking siteverify, even when it names the right site key;
    // refused with a token for sign-in
    assert_eq!(body(save(json!({ "auth": true })).await).await, required("0x-site"));
    assert_eq!(body(save(json!({ "auth": true, "proved_site_key": "0x-site" })).await).await, required("0x-site"));
    assert_eq!(asked_now(), 0);
    let wrong_action = json!({ "auth": true, "token": "in", "proved_site_key": "0x-site" });
    assert_eq!(code(body(save(wrong_action).await).await), (StatusCode::FORBIDDEN, json!("verification_failed")));
    assert_eq!(auth_row(&pool).await, json!([null, "0x-site", "0x-right"]));
    let proved = json!({ "auth": true, "token": "settings-0x-right", "proved_site_key": "0x-site" });
    assert_eq!(body(save(proved).await).await.1["auth"], true);

    // a new site key while it is on, alone; a token for the key that was in effect when the console asked is not
    // one for the key in effect now
    assert_eq!(body(save(json!({ "site_key": "0x-site-2" })).await).await, required("0x-site-2"));
    let stale = json!({ "site_key": "0x-site-2", "token": "settings-0x-right", "proved_site_key": "0x-site" });
    assert_eq!(body(save(stale).await).await, required("0x-site-2"));
    let proved = json!({ "site_key": "0x-site-2", "token": "settings-0x-right", "proved_site_key": "0x-site-2" });
    assert_eq!(body(save(proved).await).await.0, StatusCode::OK);
    assert_eq!(auth_row(&pool).await, json!([true, "0x-site-2", "0x-right"]));

    // a new secret while it is on: the token must pass with the new secret, not the old one
    assert_eq!(body(save(json!({ "secret": "0x-new" })).await).await, required("0x-site-2"));
    let old = json!({ "secret": "0x-new", "token": "settings-0x-right", "proved_site_key": "0x-site-2" });
    assert_eq!(code(body(save(old).await).await), (StatusCode::FORBIDDEN, json!("verification_failed")));
    let new = json!({ "secret": "0x-new", "token": "settings-0x-new", "proved_site_key": "0x-site-2" });
    assert_eq!(body(save(new).await).await.0, StatusCode::OK);
    assert_eq!(auth_row(&pool).await, json!([true, "0x-site-2", "0x-new"]));

    // the keys saved as they are (the secret field left blank keeps the stored one): nothing to prove
    let before = asked_now();
    assert_eq!(body(save(json!({ "site_key": "0x-site-2", "secret": " " })).await).await.0, StatusCode::OK);
    assert_eq!(asked_now(), before);

    // a check siteverify cannot judge proves nothing: a secret it refuses, or its own failure
    let unknown = json!({ "secret": "0x-unknown", "token": "settings-0x-unknown", "proved_site_key": "0x-site-2" });
    assert_eq!(code(body(save(unknown).await).await), (StatusCode::BAD_GATEWAY, json!("provider_failed")));
    let failing = json!({ "secret": "0x-right", "token": "failing", "proved_site_key": "0x-site-2" });
    assert_eq!(code(body(save(failing).await).await), (StatusCode::BAD_GATEWAY, json!("provider_failed")));
    assert_eq!(auth_row(&pool).await, json!([true, "0x-site-2", "0x-new"]));

    // a testing secret's answer passes only when it says success
    let fail = json!({ "secret": "0x-testing", "token": "testing-fail", "proved_site_key": "0x-site-2" });
    assert_eq!(code(body(save(fail).await).await), (StatusCode::FORBIDDEN, json!("verification_failed")));
    let pass = json!({ "secret": "0x-testing", "token": "testing-pass", "proved_site_key": "0x-site-2" });
    assert_eq!(body(save(pass).await).await.0, StatusCode::OK);

    // the way out needs no token, even while every proof would fail: the switch off, or the site key cleared
    let before = asked_now();
    assert_eq!(body(save(json!({ "auth": false })).await).await.1["auth"], false);
    let proved = json!({ "auth": true, "token": "testing-pass", "proved_site_key": "0x-site-2" });
    assert_eq!(body(save(proved).await).await.1["auth"], true);
    let before_clear = asked_now();
    assert_eq!(body(save(json!({ "site_key": "" })).await).await.0, StatusCode::OK);
    assert_eq!(asked_now(), before_clear);
    assert!(asked_now() == before + 1, "only the proved switch asked siteverify");
    // the switch left on while a key was unset does nothing; the key that makes it take effect needs a token
    assert_eq!(auth_row(&pool).await, json!([true, "", "0x-testing"]));
    assert_eq!(body(save(json!({ "site_key": "0x-site-3" })).await).await, required("0x-site-3"));
    // the stricter limits' switch alone leaves sign-in as it is
    assert_eq!(body(save(json!({ "strict_limits": true })).await).await.0, StatusCode::OK);
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

/// Two saves at once, one turning the switch on with a token for the keys in effect and one changing the site key
/// without a token, are judged one after the other: the second, started while the first holds the lock and waits for
/// siteverify, sees the switch on and is asked for a token, so the row never holds the switch on with a site key nobody
/// proved.
#[sqlx::test(migrations = "../core/migrations")]
#[ignore = "requires PostgreSQL with pgvector"]
async fn two_saves_at_once_cannot_land_an_unproved_pair(pool: sqlx::PgPool) {
    let h = admin(&pool).await;
    let StandIn { url, arrived, release, .. } = stand_in().await;
    let s = Arc::try_unwrap(account::test_state(pool.clone())).ok().unwrap();
    let s = Arc::new(AppState { siteverify_url: url, ..s });
    sqlx::query("insert into app_settings (key, value) values ('turnstile', $1)")
        .bind(json!({ "site_key": "0x-site", "secret": "0x-right" }))
        .execute(&pool)
        .await
        .unwrap();
    let save = |b: Value| {
        let (s, h) = (s.clone(), h.clone());
        tokio::spawn(async move { body(put(State(s), ADMIN_IP, h, Json(serde_json::from_value(b).unwrap())).await).await })
    };
    let on = save(json!({ "auth": true, "token": "slow-settings-0x-right", "proved_site_key": "0x-site" }));
    // the first save holds the lock and waits for siteverify; the second starts now and reaches the row and the lock. A
    // gate that asks siteverify nothing never gets here, and the test fails instead of waiting
    tokio::time::timeout(std::time::Duration::from_secs(5), arrived.notified())
        .await
        .expect("the first save never asked siteverify: turning the switch on was not asked for proof");
    let key = save(json!({ "site_key": "0x-site-b" }));
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    release.notify_one();
    let (on, key) = (on.await.unwrap(), key.await.unwrap());
    assert_eq!(on.0, StatusCode::OK);
    assert_eq!((key.0, &key.1["code"]), (StatusCode::FORBIDDEN, &json!("verification_required")), "{key:?}");
    assert_eq!(auth_row(&pool).await, json!([true, "0x-site", "0x-right"]));
}
