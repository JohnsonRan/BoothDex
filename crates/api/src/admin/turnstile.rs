//! The Turnstile keys in the admin console. The secret is write-only: the console sees whether one is set and its hint.

use std::sync::Arc;

use axum::Json;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use serde_json::{Value, json};

use super::{key_hint, require_admin};
use crate::error::Refusal;
use crate::turnstile::{self, TurnstileConfig};
use crate::{ApiError, AppState};

pub(super) fn public(c: &TurnstileConfig) -> Value {
    json!({
        "site_key": c.site_key,
        "secret_set": c.secret.is_some(),
        "secret_hint": c.secret.as_deref().map(key_hint),
        // fields set by TURNSTILE_* environment variables can't be edited in the console
        "from_env": { "site_key": c.site_key_from_env, "secret": c.secret_from_env },
    })
}

#[derive(Deserialize)]
pub(super) struct TurnstileSettings {
    /// empty clears it, which closes the forms behind Turnstile
    site_key: Option<String>,
    /// empty or missing keeps the stored secret
    secret: Option<String>,
}

fn valid_key(k: &str) -> bool {
    k.len() <= 200 && k.bytes().all(|b| b.is_ascii_graphic())
}

pub(super) async fn put(State(s): State<Arc<AppState>>, headers: HeaderMap, Json(b): Json<TurnstileSettings>) -> Result<Response, ApiError> {
    let who = require_admin(&s, &headers).await?;
    let site_key = b.site_key.map(|k| k.trim().to_owned());
    let secret = b.secret.map(|k| k.trim().to_owned()).filter(|k| !k.is_empty());
    if [&site_key, &secret].iter().any(|k| k.as_deref().is_some_and(|k| !valid_key(k))) {
        return Err(Refusal::BadTurnstileSettings.into());
    }
    let mut change = json!({});
    if let Some(k) = site_key {
        change["site_key"] = json!(k);
    }
    if let Some(k) = secret {
        change["secret"] = json!(k);
    }
    // merged in one statement, so two saves of different fields both land
    sqlx::query(
        "insert into app_settings (key, value) values ('turnstile', $1)
         on conflict (key) do update set value = app_settings.value || excluded.value, updated_at = now()",
    )
    .bind(&change)
    .execute(&s.pool)
    .await?;
    println!("admin {who} saved the Turnstile settings");
    Ok(Json(public(&turnstile::config(&s.pool).await?)).into_response())
}

/// Checks the secret with a token no widget made: siteverify answers `invalid-input-secret` for a wrong secret and
/// only `invalid-input-response` for a right one. The admin sees Cloudflare's codes, to troubleshoot.
pub(super) async fn test(State(s): State<Arc<AppState>>, headers: HeaderMap) -> Result<Response, ApiError> {
    require_admin(&s, &headers).await?;
    let config = turnstile::config(&s.pool).await?;
    let Some((_, secret)) = config.keys() else { return Err(Refusal::VerificationUnavailable.into()) };
    match turnstile::siteverify(&s, secret, "boothdex-console-test", None, "").await {
        Ok((_, codes)) => Ok(Json(json!({ "ok": !codes.iter().any(|c| c == "invalid-input-secret"), "codes": codes })).into_response()),
        Err(e) => Err(Refusal::ProviderFailed.with(format!("Could not reach Turnstile: {e:#}"))),
    }
}

#[cfg(test)]
mod tests {
    use axum::extract::State;
    use axum::http::{StatusCode, header};

    use super::*;
    use crate::account;

    async fn admin(pool: &sqlx::PgPool) -> HeaderMap {
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

    /// siteverify as it answers a token no widget made: the secret "0x-right" is known, any other is not.
    async fn stand_in() -> String {
        let app = axum::Router::new().route(
            "/",
            axum::routing::post(|Json(b): Json<Value>| async move {
                let code = if b["secret"] == "0x-right" { "invalid-input-response" } else { "invalid-input-secret" };
                Json(json!({ "success": false, "error-codes": [code] }))
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/", listener.local_addr().unwrap());
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        url
    }

    async fn body(r: Result<Response, ApiError>) -> (StatusCode, Value) {
        let r = r.into_response();
        (r.status(), serde_json::from_slice(&axum::body::to_bytes(r.into_body(), 1 << 20).await.unwrap()).unwrap())
    }

    #[sqlx::test(migrations = "../core/migrations")]
    #[ignore = "requires PostgreSQL with pgvector"]
    async fn the_secret_is_write_only_and_tested(pool: sqlx::PgPool) {
        let h = admin(&pool).await;
        let s = Arc::try_unwrap(account::test_state(pool.clone())).ok().unwrap();
        let s = Arc::new(AppState { siteverify_url: stand_in().await, ..s });
        let save = |site_key: Option<&str>, secret: Option<&str>| {
            put(State(s.clone()), h.clone(), Json(TurnstileSettings { site_key: site_key.map(Into::into), secret: secret.map(Into::into) }))
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
        // an empty site key closes the guestbook again
        assert_eq!(body(save(Some(""), None).await).await.1["site_key"], Value::Null);
        assert_eq!(turnstile::config(&pool).await.unwrap().keys(), None);
    }
}
