//! AI provider settings. Environment variables win over the admin-edited row in `app_settings`, so a
//! deployment can keep the key out of the database: AI_API_KEY, AI_BASE_URL, AI_MODEL, AI_REASONING_EFFORT.

use serde_json::{Value, json};
use sqlx::PgPool;

/// Values for the `reasoning_effort` request field; sent as is, "none" included.
pub const REASONING_EFFORTS: [&str; 6] = ["none", "minimal", "low", "medium", "high", "xhigh"];

#[derive(Debug, Clone, PartialEq)]
pub struct AiConfig {
    pub base_url: Option<String>,
    pub model: Option<String>,
    pub api_key: Option<String>,
    /// one of REASONING_EFFORTS; "none" by default
    pub reasoning_effort: &'static str,
    /// which fields came from the environment (the admin UI locks those)
    pub base_url_from_env: bool,
    pub model_from_env: bool,
    pub api_key_from_env: bool,
    pub reasoning_effort_from_env: bool,
}

pub fn reasoning_effort(s: &str) -> Option<&'static str> {
    REASONING_EFFORTS.into_iter().find(|e| e.eq_ignore_ascii_case(s.trim()))
}

pub async fn stored_ai(pool: &PgPool) -> Result<Value, sqlx::Error> {
    Ok(sqlx::query_scalar("select value from app_settings where key = 'ai'").fetch_optional(pool).await?.unwrap_or_else(|| json!({})))
}

/// `env` is injected for testability; callers pass `|k| std::env::var(k).ok()`.
pub fn resolve(stored: &Value, env: impl Fn(&str) -> Option<String>) -> AiConfig {
    let pick = |var: &str, field: &str, trim_slash: bool| match env(var).filter(|v| !v.trim().is_empty()) {
        Some(v) => {
            let s = v.trim();
            let s = if trim_slash { s.trim_end_matches('/') } else { s };
            (Some(s.to_owned()), true)
        }
        None => (stored[field].as_str().filter(|v| !v.is_empty()).map(str::to_owned), false),
    };
    let (base_url, base_url_from_env) = pick("AI_BASE_URL", "base_url", true);
    let (model, model_from_env) = pick("AI_MODEL", "model", false);
    let (api_key, api_key_from_env) = pick("AI_API_KEY", "api_key", false);
    // an unknown value in the environment is ignored rather than sent to the provider
    let (reasoning_effort, reasoning_effort_from_env) = match env("AI_REASONING_EFFORT").as_deref().and_then(reasoning_effort) {
        Some(e) => (e, true),
        None => (stored["reasoning_effort"].as_str().and_then(reasoning_effort).unwrap_or("none"), false),
    };
    AiConfig { base_url, model, api_key, reasoning_effort, base_url_from_env, model_from_env, api_key_from_env, reasoning_effort_from_env }
}

pub async fn ai(pool: &PgPool) -> Result<AiConfig, sqlx::Error> {
    Ok(resolve(&stored_ai(pool).await?, |k| std::env::var(k).ok()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn env_overrides_database() {
        let stored = json!({ "base_url": "https://db.example/v1", "model": "db-model", "api_key": "sk-db" });
        let none = resolve(&stored, |_| None);
        assert_eq!(none.api_key.as_deref(), Some("sk-db"));
        assert!(!none.api_key_from_env);
        let env = resolve(&stored, |k| (k == "AI_API_KEY").then(|| " sk-env ".to_owned()));
        assert_eq!(env.api_key.as_deref(), Some("sk-env"));
        assert!(env.api_key_from_env);
        assert_eq!(env.model.as_deref(), Some("db-model"));
        let blank = resolve(&json!({}), |_| Some(String::new()));
        assert_eq!((blank.base_url, blank.model, blank.api_key, blank.reasoning_effort), (None, None, None, "none"));
        assert!(!blank.api_key_from_env && !blank.reasoning_effort_from_env);
    }

    #[test]
    fn reasoning_effort_defaults_to_none_and_env_wins() {
        assert_eq!(resolve(&json!({}), |_| None).reasoning_effort, "none");
        let stored = json!({ "reasoning_effort": "low" });
        assert_eq!(resolve(&stored, |_| None).reasoning_effort, "low");
        let env = resolve(&stored, |k| (k == "AI_REASONING_EFFORT").then(|| " High ".to_owned()));
        assert_eq!((env.reasoning_effort, env.reasoning_effort_from_env), ("high", true));
        // unknown values fall back instead of reaching the provider
        let bad = resolve(&json!({ "reasoning_effort": "max" }), |k| (k == "AI_REASONING_EFFORT").then(|| "ultra".to_owned()));
        assert_eq!((bad.reasoning_effort, bad.reasoning_effort_from_env), ("none", false));
    }

    #[test]
    fn preserves_slashes_in_key_and_model() {
        let stored = json!({});
        let env = resolve(&stored, |k| match k {
            "AI_BASE_URL" => Some(" https://env.example/v1/ ".to_owned()),
            "AI_MODEL" => Some(" model/with/slash/ ".to_owned()),
            "AI_API_KEY" => Some(" sk-env/key/ ".to_owned()),
            _ => None,
        });
        assert_eq!(env.base_url.as_deref(), Some("https://env.example/v1"));
        assert_eq!(env.model.as_deref(), Some("model/with/slash/"));
        assert_eq!(env.api_key.as_deref(), Some("sk-env/key/"));
    }
}
