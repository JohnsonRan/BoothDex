//! OpenAI-compatible chat client (JSON mode) plus the prompts and reply checks for `localize` and `enrich`.

mod check;
mod prompts;
#[cfg(test)]
mod check_tests;
#[cfg(test)]
mod tests;

pub use check::*;
pub use prompts::*;

use std::time::Duration;

use anyhow::{Context, Result, bail};
use boothdex_core::settings::AiConfig;
use serde_json::{Value, json};

pub struct Ai {
    http: reqwest::Client,
    base_url: String,
    model: String,
    api_key: String,
    reasoning_effort: &'static str,
}

/// The provider refused or kept failing (bad key/model/parameter, outage): a configuration problem, not the item's fault.
#[derive(Debug)]
pub struct ProviderError(pub String);

impl std::fmt::Display for ProviderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ProviderError {}

impl Ai {
    /// `None` when the provider isn't configured (base URL, model and key are all needed).
    pub fn from_config(c: &AiConfig) -> Result<Option<Self>> {
        let (Some(base_url), Some(model), Some(api_key)) = (c.base_url.clone(), c.model.clone(), c.api_key.clone()) else { return Ok(None) };
        let http = reqwest::Client::builder().timeout(Duration::from_secs(120)).build()?;
        Ok(Some(Self { http, base_url, model, api_key, reasoning_effort: c.reasoning_effort }))
    }

    pub(crate) fn body(&self, messages: Value, response_format: Value) -> Value {
        json!({
            "model": self.model,
            "temperature": 0,
            "reasoning_effort": self.reasoning_effort,
            "response_format": response_format,
            "messages": messages,
        })
    }

    /// One JSON-mode chat completion; retries 429/5xx with backoff. Returns the parsed JSON object.
    pub async fn json(&self, system: &str, user: &str) -> Result<Value> {
        self.chat(json!([{ "role": "system", "content": system }, { "role": "user", "content": user }])).await
    }

    /// Like `json`, with the reply held to `schema` (`response_format` json_schema, strict). `Ok(None)` when the
    /// provider refuses that format (`refuses_format`), so the caller can ask with `json` instead.
    pub async fn json_schema(&self, system: &str, user: &str, schema: Value) -> Result<Option<Value>> {
        let messages = json!([{ "role": "system", "content": system }, { "role": "user", "content": user }]);
        let format = json!({ "type": "json_schema", "json_schema": { "name": "reply", "strict": true, "schema": schema } });
        match self.send(&self.body(messages, format)).await? {
            Ok(v) => Ok(Some(v)),
            Err(refusal) if refuses_format(refusal.status, &refusal.text) => Ok(None),
            Err(refusal) => Err(refusal.into()),
        }
    }

    /// Asks again in the same conversation after `rejected` failed validation with `error`, for a full corrected reply.
    pub async fn json_correction(&self, system: &str, user: &str, rejected: &Value, error: &str) -> Result<Value> {
        let mut fix = format!("Your reply was rejected: {error}. Reply again with the complete corrected JSON object.");
        if error.contains("categories") {
            fix.push_str(" Copy every category slug character for character from the list, including its first word, and use a child slug (with \"--\"), never a parent alone.");
        }
        self.chat(json!([
            { "role": "system", "content": system },
            { "role": "user", "content": user },
            { "role": "assistant", "content": rejected.to_string() },
            { "role": "user", "content": fix },
        ]))
        .await
    }

    async fn chat(&self, messages: Value) -> Result<Value> {
        Ok(self.send(&self.body(messages, json!({ "type": "json_object" }))).await??)
    }

    /// Sends one request; retries 429/5xx with backoff. The inner `Err` is a request the provider refused outright.
    async fn send(&self, body: &Value) -> Result<std::result::Result<Value, Refusal>> {
        for attempt in 0..5u32 {
            let res = self.http.post(format!("{}/chat/completions", self.base_url)).bearer_auth(&self.api_key).json(body).send().await;
            match res {
                Ok(r) if r.status().is_success() => {
                    let v: Value = r.json().await?;
                    let text = v["choices"][0]["message"]["content"].as_str().context("provider reply without content")?;
                    return Ok(Ok(parse_json_reply(text)?));
                }
                Ok(r) if r.status() == 429 || r.status().is_server_error() => {
                    if attempt == 4 {
                        break;
                    }
                    tokio::time::sleep(Duration::from_secs(2u64.pow(attempt + 1))).await;
                }
                Ok(r) => {
                    let status = r.status().as_u16();
                    let text: String = r.text().await.unwrap_or_default().chars().take(300).collect();
                    return Ok(Err(Refusal { status, text }));
                }
                Err(e) if attempt < 4 => {
                    eprintln!("ai request: {e}");
                    tokio::time::sleep(Duration::from_secs(2u64.pow(attempt + 1))).await;
                }
                Err(e) => return Err(ProviderError(format!("provider unreachable: {e}")).into()),
            }
        }
        Err(ProviderError("provider kept failing (rate limit or server errors)".into()).into())
    }
}

/// A request the provider refused outright (not 429 or 5xx): its status and the start of its body.
struct Refusal {
    status: u16,
    text: String,
}

impl From<Refusal> for anyhow::Error {
    fn from(r: Refusal) -> Self {
        ProviderError(format!("provider answered {}: {}", r.status, r.text)).into()
    }
}

/// Whether a refusal is about the reply format: HTTP 400, or an error that names `response_format` (providers without
/// json_schema answer either way).
fn refuses_format(status: u16, text: &str) -> bool {
    status == 400 || text.contains("response_format")
}

/// Models sometimes wrap JSON in ```json fences or add text around it; take the outermost object.
pub fn parse_json_reply(text: &str) -> Result<Value> {
    let start = text.find('{').context("no JSON object in reply")?;
    let end = text.rfind('}').context("no JSON object in reply")?;
    if end < start {
        bail!("no JSON object in reply: '}}' before '{{'");
    }
    Ok(serde_json::from_str(&text[start..=end])?)
}
