//! The AI key goes only to the base URL it was entered for (`key_follows_url` in admin.rs).

use std::sync::{Arc, Mutex};

use axum::Json;
use axum::extract::State;
use axum::http::{HeaderMap, header};
use axum::response::IntoResponse;
use serde_json::{Value, json};
use sqlx::PgPool;

use super::{AiSettings, put_ai, save_ai, test_ai};
use crate::account;

const NO_ENV: fn(&str) -> Option<String> = |_| None;

fn ai(base_url: Option<&str>, api_key: Option<&str>) -> AiSettings {
    AiSettings { base_url: base_url.map(Into::into), model: None, api_key: api_key.map(Into::into), reasoning_effort: None, embed_model: None }
}

async fn stored(pool: &PgPool) -> Value {
    boothdex_core::settings::stored_ai(pool).await.unwrap()
}

/// The code of a refused save, or "ok".
async fn outcome(r: Result<impl Sized, crate::ApiError>) -> String {
    match r {
        Ok(_) => "ok".into(),
        Err(e) => {
            let body = axum::body::to_bytes(e.into_response().into_body(), 1 << 20).await.unwrap();
            serde_json::from_slice::<Value>(&body).unwrap()["code"].as_str().unwrap().to_owned()
        }
    }
}

async fn admin_session(pool: &PgPool) -> HeaderMap {
    let boss: i64 = sqlx::query_scalar("insert into users (username, email, display_name, password_hash, is_admin) values ('boss', 'boss@example.test', 'Boss', 'x', true) returning id")
        .fetch_one(pool)
        .await
        .unwrap();
    sqlx::query("insert into sessions (token_hash, user_id, expires_at) values ($1, $2, now() + interval '1 day')")
        .bind(account::token_hash("tok-boss"))
        .bind(boss)
        .execute(pool)
        .await
        .unwrap();
    let mut h = HeaderMap::new();
    h.insert(header::COOKIE, "session=tok-boss".parse().unwrap());
    h
}

#[sqlx::test(migrations = "../core/migrations")]
#[ignore = "requires PostgreSQL with pgvector"]
async fn a_new_base_url_needs_the_key_again(pool: PgPool) {
    let state = account::test_state(pool.clone());
    let save = |b: AiSettings| save_ai(&state, b, NO_ENV);

    // first setup: no key stored yet, so the URL may move freely
    assert_eq!(outcome(save(ai(Some("https://a.example/v1"), None)).await).await, "ok");
    assert_eq!(outcome(save(ai(Some("https://b.example/v1"), None)).await).await, "ok");
    assert_eq!(outcome(save(ai(Some("https://a.example/v1"), Some("sk-a"))).await).await, "ok");
    let saved = json!({ "base_url": "https://a.example/v1", "api_key": "sk-a" });
    let row = stored(&pool).await;
    assert_eq!((&row["base_url"], &row["api_key"]), (&saved["base_url"], &saved["api_key"]));

    // moving the URL without the key stores nothing; a blank key counts as none
    for url in ["https://evil.example/v1", "http://a.example/v1", "https://a.example:8443/v1", "https://a.example/v2"] {
        assert_eq!(outcome(save(ai(Some(url), None)).await).await, "ai_key_required", "{url}");
        assert_eq!(outcome(save(ai(Some(url), Some("  "))).await).await, "ai_key_required", "{url}");
        let row = stored(&pool).await;
        assert_eq!((&row["base_url"], &row["api_key"]), (&saved["base_url"], &saved["api_key"]), "{url}");
    }

    // the same URL after trimming keeps the key; so does a save that leaves the URL out
    assert_eq!(outcome(save(ai(Some("https://a.example/v1// "), None)).await).await, "ok");
    let mut keep = ai(None, None);
    keep.model = Some("m".into());
    assert_eq!(outcome(save(keep).await).await, "ok");
    let row = stored(&pool).await;
    assert_eq!((row["base_url"].as_str(), row["api_key"].as_str(), row["model"].as_str()), (Some("https://a.example/v1"), Some("sk-a"), Some("m")));

    // moving it with the key stores both
    assert_eq!(outcome(save(ai(Some("https://b.example/v1"), Some("sk-b"))).await).await, "ok");
    let row = stored(&pool).await;
    assert_eq!((row["base_url"].as_str(), row["api_key"].as_str()), (Some("https://b.example/v1"), Some("sk-b")));

    // sk-b was entered for b: going back to a needs a key as well
    assert_eq!(outcome(save(ai(Some("https://a.example/v1"), None)).await).await, "ai_key_required");

    // the refusal over HTTP: its status and the message other clients read
    let refused = put_ai(State(state.clone()), admin_session(&pool).await, Json(ai(Some("https://evil.example/v1"), None))).await.into_response();
    assert_eq!(refused.status().as_u16(), 400);
    let body: Value = serde_json::from_slice(&axum::body::to_bytes(refused.into_body(), 1 << 20).await.unwrap()).unwrap();
    assert_eq!(body["error"], "Enter the API key again to change the base URL");
}

/// A deployment with AI_API_KEY in the environment and the base URL in the database keeps working, and the console
/// cannot move that key.
#[sqlx::test(migrations = "../core/migrations")]
#[ignore = "requires PostgreSQL with pgvector"]
async fn a_key_from_the_environment_pins_the_base_url(pool: PgPool) {
    sqlx::query("insert into app_settings (key, value) values ('ai', $1)")
        .bind(json!({ "base_url": "https://a.example/v1", "model": "m" }))
        .execute(&pool)
        .await
        .unwrap();
    let env_key = |k: &str| (k == "AI_API_KEY").then(|| "sk-env".to_owned());
    let config = boothdex_core::settings::resolve(&stored(&pool).await, env_key);
    assert_eq!((config.base_url.as_deref(), config.api_key.as_deref()), (Some("https://a.example/v1"), Some("sk-env")));
    let public = super::public_ai(&config);
    assert_eq!((&public["from_env"]["base_url"], &public["from_env"]["api_key"]), (&json!(true), &json!(true)));

    let state = account::test_state(pool.clone());
    let save = |b: AiSettings| save_ai(&state, b, env_key);
    // the form sends the unchanged URL with every save
    let mut model = ai(Some("https://a.example/v1/"), None);
    model.model = Some("m2".into());
    assert_eq!(outcome(save(model).await).await, "ok");
    for key in [None, Some("sk-typed")] {
        assert_eq!(outcome(save(ai(Some("https://evil.example/v1"), key)).await).await, "ai_base_url_locked");
    }
    let row = stored(&pool).await;
    assert_eq!((row["base_url"].as_str(), row["model"].as_str(), row.get("api_key")), (Some("https://a.example/v1"), Some("m2"), None));

    // with AI_BASE_URL beside it, the save goes on but the URL it names is not stored
    let both = |k: &str| match k {
        "AI_API_KEY" => Some("sk-env".to_owned()),
        "AI_BASE_URL" => Some("https://env.example/v1".to_owned()),
        _ => None,
    };
    assert_eq!(outcome(save_ai(&state, ai(Some("https://other.example/v1"), None), both).await).await, "ok");
    let config = boothdex_core::settings::resolve(&stored(&pool).await, both);
    assert_eq!(config.base_url.as_deref(), Some("https://env.example/v1"));
    assert_eq!(super::public_ai(&config)["from_env"]["base_url"], true);
    assert_eq!(stored(&pool).await["base_url"], "https://a.example/v1");
    // without a key in the environment the URL field stays editable
    assert_eq!(super::public_ai(&boothdex_core::settings::resolve(&stored(&pool).await, NO_ENV))["from_env"]["base_url"], false);
}

/// A URL saved while AI_BASE_URL overrides it would receive the stored key, unseen, once AI_BASE_URL is removed.
#[sqlx::test(migrations = "../core/migrations")]
#[ignore = "requires PostgreSQL with pgvector"]
async fn a_url_hidden_by_ai_base_url_is_not_stored(pool: PgPool) {
    sqlx::query("insert into app_settings (key, value) values ('ai', $1)")
        .bind(json!({ "base_url": "https://a.example/v1", "api_key": "sk-a", "model": "m" }))
        .execute(&pool)
        .await
        .unwrap();
    let env_url = |k: &str| (k == "AI_BASE_URL").then(|| "https://env.example/v1".to_owned());
    let state = account::test_state(pool.clone());
    // the form sends the environment's value; the other fields of the save are still stored
    for url in ["https://env.example/v1", "https://evil.example/v1"] {
        let mut b = ai(Some(url), None);
        b.model = Some(format!("m-{url}"));
        let config = save_ai(&state, b, env_url).await.ok().unwrap();
        assert_eq!(config.base_url.as_deref(), Some("https://env.example/v1"), "{url}");
        let row = stored(&pool).await;
        assert_eq!((row["base_url"].as_str(), row["model"].as_str()), (Some("https://a.example/v1"), Some(format!("m-{url}").as_str())), "{url}");
    }
    // once AI_BASE_URL is removed, the stored key goes to the URL it was entered for
    let after = boothdex_core::settings::resolve(&stored(&pool).await, NO_ENV);
    assert_eq!((after.base_url.as_deref(), after.api_key.as_deref()), (Some("https://a.example/v1"), Some("sk-a")));
    // a key entered under AI_BASE_URL was entered for the environment's URL, whatever URL the save names
    save_ai(&state, ai(Some("https://evil.example/v1"), Some("sk-env-url")), env_url).await.ok().unwrap();
    let after = boothdex_core::settings::resolve(&stored(&pool).await, NO_ENV);
    assert_eq!((after.base_url.as_deref(), after.api_key.as_deref()), (Some("https://env.example/v1"), Some("sk-env-url")));
}

/// A loopback stand-in for a provider: answers `/models` and records the Authorization header of every request.
async fn provider() -> (String, Arc<Mutex<Vec<String>>>) {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let log = seen.clone();
    let app = axum::Router::new().fallback(move |headers: HeaderMap| {
        log.lock().unwrap().push(headers.get(header::AUTHORIZATION).and_then(|v| v.to_str().ok()).unwrap_or_default().to_owned());
        async { Json(json!({ "data": [{ "id": "m" }] })) }
    });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/v1", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (url, seen)
}

#[sqlx::test(migrations = "../core/migrations")]
#[ignore = "requires PostgreSQL with pgvector"]
async fn a_refused_save_sends_no_key_to_the_new_host(pool: PgPool) {
    // put_ai and test_ai read the process environment
    for k in ["AI_BASE_URL", "AI_API_KEY"] {
        assert!(std::env::var(k).ok().is_none_or(|v| v.trim().is_empty()), "unset {k} for this test");
    }
    let (old, old_seen) = provider().await;
    let (new, new_seen) = provider().await;
    let state = account::test_state(pool.clone());
    let h = admin_session(&pool).await;
    let put = |b: AiSettings| put_ai(State(state.clone()), h.clone(), Json(b));
    let test = || test_ai(State(state.clone()), h.clone());

    assert_eq!(outcome(put(ai(Some(&old), Some("sk-old"))).await).await, "ok");
    assert_eq!(outcome(put(ai(Some(&new), None)).await).await, "ai_key_required");
    assert_eq!(outcome(test().await).await, "ok");
    let leaked = new_seen.lock().unwrap().clone();
    assert!(leaked.is_empty(), "the new host got {leaked:?}");
    assert_eq!(*old_seen.lock().unwrap(), ["Bearer sk-old"]);

    // the listener does record a request: the new host gets the key entered for it, never the old one
    assert_eq!(outcome(put(ai(Some(&new), Some("sk-new"))).await).await, "ok");
    assert_eq!(outcome(test().await).await, "ok");
    assert_eq!(*new_seen.lock().unwrap(), ["Bearer sk-new"]);
    assert_eq!(old_seen.lock().unwrap().len(), 1);
}
