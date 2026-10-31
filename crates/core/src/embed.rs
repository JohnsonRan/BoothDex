//! Text embeddings for semantic search through the provider's OpenAI-compatible `/embeddings` endpoint.
//! The worker embeds items; the api embeds search queries.

use std::time::Duration;

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};

/// Matches `items.embedding vector(1024)`. Requested with `dimensions`; models that can't shorten their output fail.
pub const DIMS: usize = 1024;

/// Qwen3-Embedding style instruction; only queries get it, documents are embedded as they are.
pub const QUERY_PREFIX: &str = "Instruct: Given a search query for VRChat avatar assets sold on BOOTH, retrieve matching product listings\nQuery: ";

/// One request for several inputs; `attempts` > 1 retries 429/5xx and network errors with backoff.
pub async fn embed(http: &reqwest::Client, base_url: &str, api_key: &str, model: &str, inputs: &[String], attempts: u32) -> Result<Vec<Vec<f32>>> {
    let body = json!({ "model": model, "input": inputs, "dimensions": DIMS, "encoding_format": "float" });
    let mut last = String::new();
    for attempt in 0..attempts.max(1) {
        if attempt > 0 {
            tokio::time::sleep(Duration::from_secs(2u64.pow(attempt))).await;
        }
        match http.post(format!("{base_url}/embeddings")).bearer_auth(api_key).json(&body).send().await {
            Ok(r) if r.status().is_success() => return parse(&r.json().await?, inputs.len()),
            Ok(r) if r.status() == 429 || r.status().is_server_error() => last = format!("provider answered {}", r.status()),
            Ok(r) => {
                let status = r.status();
                bail!("provider answered {status}: {}", r.text().await.unwrap_or_default().chars().take(300).collect::<String>());
            }
            Err(e) => last = format!("provider unreachable: {:#}", anyhow::Error::from(e)),
        }
    }
    bail!("{last}")
}

fn parse(v: &Value, expected: usize) -> Result<Vec<Vec<f32>>> {
    let mut rows: Vec<(u64, Vec<f32>)> = v["data"]
        .as_array()
        .context("embeddings reply without data")?
        .iter()
        .enumerate()
        .map(|(i, d)| {
            let e: Vec<f32> = d["embedding"].as_array().context("embedding is not an array")?.iter().map(|x| x.as_f64().unwrap_or(0.0) as f32).collect();
            Ok((d["index"].as_u64().unwrap_or(i as u64), e))
        })
        .collect::<Result<_>>()?;
    rows.sort_by_key(|r| r.0);
    if rows.len() != expected {
        bail!("asked for {expected} embeddings, got {}", rows.len());
    }
    if let Some((_, e)) = rows.iter().find(|(_, e)| e.len() != DIMS) {
        bail!("model returned {} dimensions instead of {DIMS}; it must support the `dimensions` parameter", e.len());
    }
    Ok(rows.into_iter().map(|r| r.1).collect())
}

/// What gets embedded for an item: title (original and English), tags, and the start of the description.
/// The description is cut: the opening says what the item is; the rest is mostly terms, changelogs and links.
pub fn document_text(title: &str, title_en: Option<&str>, tags: &[String], bio: &str) -> String {
    let mut s = title.trim().to_owned();
    if let Some(en) = title_en.map(str::trim).filter(|en| !en.is_empty() && *en != title.trim()) {
        s.push('\n');
        s.push_str(en);
    }
    if !tags.is_empty() {
        s.push('\n');
        s.push_str(&tags.join(", "));
    }
    let bio: String = bio.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(1500).collect();
    if !bio.is_empty() {
        s.push('\n');
        s.push_str(&bio);
    }
    s
}

/// pgvector text form, bound as text and cast with `::halfvec`.
pub fn to_pg(v: &[f32]) -> String {
    let mut s = String::with_capacity(v.len() * 10 + 2);
    s.push('[');
    for (i, x) in v.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        s.push_str(&x.to_string());
    }
    s.push(']');
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_in_index_order_and_checks_dimensions() {
        let row = |i: u64, x: f32| json!({ "index": i, "embedding": vec![x; DIMS] });
        let v = parse(&json!({ "data": [row(1, 0.5), row(0, 0.25)] }), 2).unwrap();
        assert_eq!((v[0][0], v[1][0]), (0.25, 0.5));
        assert!(parse(&json!({ "data": [row(0, 0.1)] }), 2).is_err());
        assert!(parse(&json!({ "data": [{ "index": 0, "embedding": [0.1, 0.2] }] }), 1).unwrap_err().to_string().contains("dimensions"));
        assert_eq!(to_pg(&[0.5, -1.0]), "[0.5,-1]");
    }

    #[test]
    fn document_text_skips_duplicates_and_cuts_the_description() {
        let t = document_text("Short Hair", Some("Short Hair"), &["hair".into(), "VRChat".into()], &"a  b\n".repeat(2000));
        assert!(t.starts_with("Short Hair\nhair, VRChat\na b a b"));
        assert!(t.chars().count() < 1600);
        assert_eq!(document_text("髪", Some("Hair"), &[], ""), "髪\nHair");
    }
}
