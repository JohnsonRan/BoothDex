//! AI provider settings. Environment variables win over the admin-edited row in `app_settings`, so a
//! deployment can keep the key out of the database: AI_API_KEY, AI_BASE_URL, AI_MODEL.

use serde_json::{Value, json};
use sqlx::PgPool;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct AiConfig {
    pub base_url: Option<String>,
    pub model: Option<String>,
    pub api_key: Option<String>,
    /// which fields came from the environment (the admin UI locks those)
    pub base_url_from_env: bool,
    pub model_from_env: bool,
    pub api_key_from_env: bool,
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
    AiConfig { base_url, model, api_key, base_url_from_env, model_from_env, api_key_from_env }
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
        assert_eq!(blank, AiConfig::default());
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
