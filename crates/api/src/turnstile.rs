//! Cloudflare Turnstile: the site key the browser's widget needs, the secret the API checks a token with, and the
//! check itself. The keys are edited in the admin console (`app_settings` 'turnstile'); TURNSTILE_SITE_KEY and
//! TURNSTILE_SECRET_KEY win over the stored row, as the AI_* variables do for the AI settings.
//!
//! Two kinds of form stand behind it. The guestbook is closed without a passed check: with a key unset, or when
//! siteverify cannot judge a token, nobody posts. Sign-in and registration are checked only while an admin's switch
//! is on, and are let through when siteverify cannot judge a token, so a Cloudflare outage or a mistyped secret
//! never keeps an admin out of the console that would fix it; the password, the per-account lockout and the
//! per-address limit stand either way.

use std::net::IpAddr;
use std::time::Duration;

use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::PgPool;

use crate::error::Refusal;
use crate::limit::is_internal;
use crate::{ApiError, AppState};

/// Where a token is checked. `AppState::siteverify_url` holds it, so tests can point the check at a local stand-in.
pub(crate) const SITEVERIFY_URL: &str = "https://challenges.cloudflare.com/turnstile/v0/siteverify";

#[derive(Clone, PartialEq)]
pub(crate) struct TurnstileConfig {
    pub(crate) site_key: Option<String>,
    pub(crate) secret: Option<String>,
    pub(crate) site_key_from_env: bool,
    pub(crate) secret_from_env: bool,
    /// the admin's switch: sign-in and registration are checked too
    pub(crate) auth: bool,
}

/// Prints whether a secret is set, never the secret: a `{:?}` in a log must not leak it.
impl std::fmt::Debug for TurnstileConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TurnstileConfig")
            .field("site_key", &self.site_key)
            .field("secret", &self.secret.as_ref().map(|_| "<redacted>"))
            .field("site_key_from_env", &self.site_key_from_env)
            .field("secret_from_env", &self.secret_from_env)
            .field("auth", &self.auth)
            .finish()
    }
}

impl TurnstileConfig {
    /// Both keys, or None: a form that needs a check stays closed until both are set.
    pub(crate) fn keys(&self) -> Option<(&str, &str)> {
        self.site_key.as_deref().zip(self.secret.as_deref())
    }

    /// Both keys while sign-in and registration are checked, or None: the switch does nothing without the keys.
    pub(crate) fn auth_keys(&self) -> Option<(&str, &str)> {
        self.keys().filter(|_| self.auth)
    }
}

pub(crate) async fn stored(pool: &PgPool) -> Result<Value, sqlx::Error> {
    Ok(sqlx::query_scalar("select value from app_settings where key = 'turnstile'").fetch_optional(pool).await?.unwrap_or_else(|| json!({})))
}

/// `env` is injected for testability; callers pass `|k| std::env::var(k).ok()`.
pub(crate) fn resolve(stored: &Value, env: impl Fn(&str) -> Option<String>) -> TurnstileConfig {
    let pick = |var: &str, field: &str| match env(var).map(|v| v.trim().to_owned()).filter(|v| !v.is_empty()) {
        Some(v) => (Some(v), true),
        None => (stored[field].as_str().filter(|v| !v.is_empty()).map(str::to_owned), false),
    };
    let (site_key, site_key_from_env) = pick("TURNSTILE_SITE_KEY", "site_key");
    let (secret, secret_from_env) = pick("TURNSTILE_SECRET_KEY", "secret");
    TurnstileConfig { site_key, secret, site_key_from_env, secret_from_env, auth: stored["auth"].as_bool().unwrap_or(false) }
}

pub(crate) async fn config(pool: &PgPool) -> Result<TurnstileConfig, sqlx::Error> {
    Ok(resolve(&stored(pool).await?, |k| std::env::var(k).ok()))
}

#[derive(Deserialize)]
struct Siteverify {
    success: bool,
    #[serde(default, rename = "error-codes")]
    error_codes: Vec<String>,
    #[serde(default)]
    action: Option<String>,
    #[serde(default)]
    metadata: Metadata,
}

#[derive(Deserialize, Default)]
struct Metadata {
    /// set when one of Cloudflare's testing secrets answered; its answer names no action
    #[serde(default)]
    result_with_testing_key: bool,
}

/// What siteverify said of a token: Ok(true) when it passed for `action`, Ok(false) with its error codes otherwise.
pub(crate) async fn siteverify(
    s: &AppState,
    secret: &str,
    token: &str,
    ip: Option<IpAddr>,
    action: &str,
) -> anyhow::Result<(bool, Vec<String>)> {
    let mut body = json!({ "secret": secret, "response": token });
    if let Some(ip) = ip {
        body["remoteip"] = json!(ip.to_string());
    }
    let r: Siteverify =
        s.http.post(&s.siteverify_url).json(&body).timeout(Duration::from_secs(5)).send().await?.error_for_status()?.json().await?;
    if r.metadata.result_with_testing_key {
        eprintln!("warn: turnstile: a testing secret answered, which passes every visitor; set the real keys before going live");
    }
    // a token minted for another form's widget does not open this one; a testing secret's answer names no action
    let passed = r.success && (r.action.as_deref() == Some(action) || r.metadata.result_with_testing_key);
    Ok((passed, r.error_codes))
}

/// What a check of one token came to.
#[derive(Debug, PartialEq)]
enum Verdict {
    Passed,
    /// Cloudflare judged the token: missing, wrong, used, expired, or made for another form
    Refused,
    /// siteverify did not judge the token: it was not reached, failed, or refused the secret key
    Unjudged,
}

/// siteverify's error codes that say nothing of the token: the secret key is missing or wrong, or Cloudflare failed.
const NOT_THE_TOKEN: [&str; 3] = ["missing-input-secret", "invalid-input-secret", "internal-error"];

async fn verdict(s: &AppState, secret: &str, token: &str, ip: IpAddr, action: &str) -> Verdict {
    // the web container's or a LAN address tells Cloudflare nothing
    let ip = (!is_internal(ip)).then_some(ip);
    match siteverify(s, secret, token, ip, action).await {
        Ok((true, _)) => Verdict::Passed,
        // a wrong secret fails every visitor; anything else is one visitor's token
        Ok((false, codes)) if codes.iter().any(|c| NOT_THE_TOKEN.contains(&c.as_str())) => {
            eprintln!("error: turnstile: siteverify could not check a token ({}); check the secret key in the admin console", codes.join(", "));
            Verdict::Unjudged
        }
        Ok((false, _)) => Verdict::Refused,
        Err(e) => {
            eprintln!("warn: turnstile: siteverify failed: {e:#}");
            Verdict::Unjudged
        }
    }
}

/// Refuses the request unless `token` passes a check for `action`. With either key unset the form is closed; when
/// siteverify cannot judge the token the request is refused too, and the operator reads why in the log.
pub(crate) async fn require(s: &AppState, token: &str, ip: IpAddr, action: &str) -> Result<(), ApiError> {
    let config = config(&s.pool).await?;
    let Some((_, secret)) = config.keys() else { return Err(Refusal::VerificationUnavailable.into()) };
    match verdict(s, secret, token, ip, action).await {
        Verdict::Passed => Ok(()),
        Verdict::Refused | Verdict::Unjudged => Err(Refusal::VerificationFailed.into()),
    }
}

/// The check on sign-in and registration: nothing while the admin's switch is off or a key is unset; otherwise the
/// request is refused when Cloudflare judges `token` bad, and let through, with an error in the log, when
/// siteverify cannot judge it. An empty token is sent to siteverify like any other: a visitor whose browser could
/// not load the widget during a Cloudflare outage has none, and only siteverify's own silence lets it through.
pub(crate) async fn require_for_account(s: &AppState, token: &str, ip: IpAddr, action: &str) -> Result<(), ApiError> {
    let config = config(&s.pool).await?;
    let Some((_, secret)) = config.auth_keys() else { return Ok(()) };
    match verdict(s, secret, token, ip, action).await {
        Verdict::Passed => Ok(()),
        Verdict::Refused => Err(Refusal::VerificationFailed.into()),
        Verdict::Unjudged => {
            eprintln!("error: turnstile: {action} from {ip} let through unchecked");
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn env_wins_and_debug_hides_the_secret() {
        let stored = json!({ "site_key": "0x-db", "secret": "0x-db-secret" });
        let db = resolve(&stored, |_| None);
        assert_eq!(db.keys(), Some(("0x-db", "0x-db-secret")));
        assert!(!db.site_key_from_env && !db.secret_from_env);
        let env = resolve(&stored, |k| (k == "TURNSTILE_SECRET_KEY").then(|| " 0x-env-secret ".into()));
        assert_eq!(env.keys(), Some(("0x-db", "0x-env-secret")));
        assert!(env.secret_from_env && !env.site_key_from_env);
        assert!(!format!("{env:?}{db:?}").contains("0x-env-secret") && !format!("{db:?}").contains("0x-db-secret"));
        // the switch is the stored row's, off unless set, and does nothing without both keys
        assert_eq!((db.auth, db.auth_keys()), (false, None));
        let on = resolve(&json!({ "site_key": "0x-db", "secret": "0x-db-secret", "auth": true }), |_| None);
        assert_eq!(on.auth_keys(), Some(("0x-db", "0x-db-secret")));
        assert_eq!(resolve(&json!({ "site_key": "0x-db", "auth": true }), |_| None).auth_keys(), None);
        // one key alone keeps the form closed
        assert_eq!(resolve(&json!({ "site_key": "0x-db" }), |_| None).keys(), None);
        assert_eq!(resolve(&json!({ "site_key": "", "secret": "s" }), |_| None).keys(), None);
    }
}
