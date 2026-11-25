//! Proof-gate, environment-lock and concurrency tests for Turnstile settings.

use std::sync::atomic::Ordering;

use axum::extract::State;
use axum::http::StatusCode;

use super::tests::{ADMIN_IP, StandIn, admin, body, stand_in};
use super::*;
use crate::account;

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

/// Keys locked by `TURNSTILE_*` environment variables are never written to `app_settings`, so removing the environment
/// variable later cannot activate an unproved pair that was saved while it was hidden.
#[sqlx::test(migrations = "../core/migrations")]
#[ignore = "requires PostgreSQL with pgvector"]
async fn env_locked_keys_are_never_written_to_the_database(pool: sqlx::PgPool) {
    let StandIn { url, .. } = stand_in().await;
    let s = Arc::try_unwrap(account::test_state(pool.clone())).ok().unwrap();
    let s = Arc::new(AppState { siteverify_url: url, ..s });
    sqlx::query("insert into app_settings (key, value) values ('turnstile', $1)")
        .bind(json!({ "auth": true }))
        .execute(&pool)
        .await
        .unwrap();
    let both_env = |k: &str| match k {
        "TURNSTILE_SITE_KEY" => Some("0x-env-site".into()),
        "TURNSTILE_SECRET_KEY" => Some("0x-right".into()),
        _ => None,
    };
    let payload: TurnstileSettings = serde_json::from_value(json!({ "site_key": "0x-broken", "secret": "0x-broken-sec" })).unwrap();
    let (status, res) = body(save(&s, ADMIN_IP.0, "boss", payload, both_env).await).await;
    assert_eq!((status, &res["site_key"]), (StatusCode::OK, &json!("0x-env-site")));
    assert_eq!(turnstile::stored(&pool).await.unwrap(), json!({ "auth": true }));

    // with only the site key in the environment, a new secret still needs proof against the env site key and writes only secret
    let site_env = |k: &str| (k == "TURNSTILE_SITE_KEY").then(|| "0x-env-site".into());
    let unproved_req: TurnstileSettings = serde_json::from_value(json!({ "site_key": "0x-broken", "secret": "0x-new" })).unwrap();
    let (status, res) = body(save(&s, ADMIN_IP.0, "boss", unproved_req, site_env).await).await;
    assert_eq!((status, &res["code"], &res["site_key"]), (StatusCode::FORBIDDEN, &json!("verification_required"), &json!("0x-env-site")));
    let proved_req: TurnstileSettings = serde_json::from_value(json!({
        "site_key": "0x-broken", "secret": "0x-new", "token": "settings-0x-new", "proved_site_key": "0x-env-site"
    }))
    .unwrap();
    assert_eq!(body(save(&s, ADMIN_IP.0, "boss", proved_req, site_env).await).await.0, StatusCode::OK);
    assert_eq!(turnstile::stored(&pool).await.unwrap(), json!({ "auth": true, "secret": "0x-new" }));
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
