//! The Turnstile keys in the admin console. The secret is write-only: the console sees whether one is set and its hint.
//!
//! A save that would put Turnstile in front of sign-in and registration with a key pair no check has proved (the switch
//! turned on, or a key changed while it is on) first needs a token from the widget with the new site key, passed with
//! the new secret, from the admin's own browser: a site key and secret that do not belong together refuse every
//! sign-in, and nobody could sign in to turn the switch off again.

use std::sync::Arc;

use axum::Json;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use serde_json::{Value, json};

use super::{key_hint, require_admin};
use crate::error::Refusal;
use crate::limit::ClientIp;
use crate::turnstile::{self, TurnstileConfig, Verdict};
use crate::{ApiError, AppState};

/// The action the console's widget names; a token for another form does not prove the keys.
pub(crate) const SETTINGS_ACTION: &str = "settings";

pub(super) fn public(c: &TurnstileConfig) -> Value {
    json!({
        "site_key": c.site_key,
        "secret_set": c.secret.is_some(),
        "secret_hint": c.secret.as_deref().map(key_hint),
        // fields set by TURNSTILE_* environment variables can't be edited in the console
        "from_env": { "site_key": c.site_key_from_env, "secret": c.secret_from_env },
        // sign-in and registration are checked too; it takes effect while both keys are set
        "auth": c.auth,
        // stricter limits for visitors without a pass (pass.rs); it takes effect while both keys are set
        "strict_limits": c.strict_limits,
    })
}

#[derive(Deserialize)]
pub(super) struct TurnstileSettings {
    /// empty clears it, which closes the forms behind Turnstile
    site_key: Option<String>,
    /// empty or missing keeps the stored secret
    secret: Option<String>,
    /// whether sign-in and registration are checked; missing keeps the stored choice
    auth: Option<bool>,
    /// whether visitors without a pass get the stricter limits; missing keeps the stored choice
    strict_limits: Option<bool>,
    /// the console widget's token, needed when the save puts unproved keys in front of sign-in
    #[serde(default)]
    token: String,
    /// the site key the token's widget was drawn with, as `proof_required` named it
    proved_site_key: Option<String>,
}

/// The refusal that asks for a token from the widget with `site_key`, the key the save would put in effect.
fn proof_required(site_key: &str) -> Response {
    let (status, error) = Refusal::VerificationRequired.parts();
    (status, Json(json!({ "ok": false, "code": Refusal::VerificationRequired, "error": error, "site_key": site_key }))).into_response()
}

/// The settings in effect before and after `change` is merged into the stored row (the same top-level merge as the save's
/// SQL), as `resolve` reads them with `env`: a key the environment sets wins over the stored one either way.
fn before_after(stored: &Value, change: &Value, env: impl Fn(&str) -> Option<String>) -> (TurnstileConfig, TurnstileConfig) {
    let mut merged = stored.clone();
    if let (Some(row), Some(change)) = (merged.as_object_mut(), change.as_object()) {
        row.extend(change.clone());
    }
    (turnstile::resolve(stored, &env), turnstile::resolve(&merged, &env))
}

/// The key pair a save puts in front of sign-in that was not there before, which needs proof; turning the switch off,
/// or a save that leaves sign-in unchecked or its pair as it was, needs none.
fn unproved<'a>(before: &TurnstileConfig, after: &'a TurnstileConfig) -> Option<(&'a str, &'a str)> {
    after.auth_keys().filter(|keys| Some(*keys) != before.auth_keys())
}

fn valid_key(k: &str) -> bool {
    k.len() <= 200 && k.bytes().all(|b| b.is_ascii_graphic())
}

pub(super) async fn put(
    State(s): State<Arc<AppState>>,
    ClientIp(ip): ClientIp,
    headers: HeaderMap,
    Json(b): Json<TurnstileSettings>,
) -> Result<Response, ApiError> {
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
    if let Some(on) = b.auth {
        change["auth"] = json!(on);
    }
    if let Some(on) = b.strict_limits {
        change["strict_limits"] = json!(on);
    }
    // held from reading the row until the saved settings are in the API's copy: a slower save cannot put older settings
    // there, and two saves (one a key, one the switch) are each judged against the other's result
    let _save = s.turnstile_save.lock().await;
    let stored = turnstile::stored(&s.pool).await?;
    let (before, after) = before_after(&stored, &change, |k| std::env::var(k).ok());
    if let Some((site_key, secret)) = unproved(&before, &after) {
        if b.token.is_empty() || b.proved_site_key.as_deref() != Some(site_key) {
            return Ok(proof_required(site_key));
        }
        match turnstile::verdict(&s, secret, &b.token, ip, SETTINGS_ACTION).await {
            Verdict::Passed => {}
            Verdict::Refused => return Err(Refusal::VerificationFailed.into()),
            Verdict::Unjudged => {
                return Err(Refusal::ProviderFailed.with("Cloudflare could not check the token with these keys, so nothing was saved. Check the secret key, or try again."));
            }
        }
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
    let saved = Arc::new(turnstile::config(&s.pool).await?);
    s.turnstile.store(Some(saved.clone()));
    Ok(Json(public(&saved)).into_response())
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
#[path = "turnstile_tests.rs"]
mod tests;
