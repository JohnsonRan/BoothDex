//! OpenAI-compatible chat client (JSON mode) plus the prompts used by the `localize` and `enrich` jobs.

use std::time::Duration;

use anyhow::{Context, Result, bail};
use catalogue_core::settings::AiConfig;
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

    fn body(&self, system: &str, user: &str) -> Value {
        json!({
            "model": self.model,
            "temperature": 0,
            "reasoning_effort": self.reasoning_effort,
            "response_format": { "type": "json_object" },
            "messages": [{ "role": "system", "content": system }, { "role": "user", "content": user }],
        })
    }

    /// One JSON-mode chat completion; retries 429/5xx with backoff. Returns the parsed JSON object.
    pub async fn json(&self, system: &str, user: &str) -> Result<Value> {
        let body = self.body(system, user);
        for attempt in 0..5u32 {
            let res = self.http.post(format!("{}/chat/completions", self.base_url)).bearer_auth(&self.api_key).json(&body).send().await;
            match res {
                Ok(r) if r.status().is_success() => {
                    let v: Value = r.json().await?;
                    let text = v["choices"][0]["message"]["content"].as_str().context("provider reply without content")?;
                    return parse_json_reply(text);
                }
                Ok(r) if r.status() == 429 || r.status().is_server_error() => {
                    tokio::time::sleep(Duration::from_secs(2u64.pow(attempt + 1))).await;
                }
                Ok(r) => {
                    let status = r.status();
                    let text: String = r.text().await.unwrap_or_default().chars().take(300).collect();
                    return Err(ProviderError(format!("provider answered {status}: {text}")).into());
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

/// Models sometimes wrap JSON in ```json fences or add text around it; take the outermost object.
pub fn parse_json_reply(text: &str) -> Result<Value> {
    let start = text.find('{').context("no JSON object in reply")?;
    let end = text.rfind('}').context("no JSON object in reply")?;
    if end < start {
        bail!("no JSON object in reply: '}}' before '{{'");
    }
    Ok(serde_json::from_str(&text[start..=end])?)
}

pub const LOCALIZE_SYSTEM: &str = "You translate UI labels of a shop for VRChat avatar assets (clothing, hair, accessories, shaders, tools) \
into Simplified Chinese (zh-CN). Use the terms Chinese VRChat users use (e.g. 衣装/服装, 发型, 配饰, 着色器, 模型 for avatar). \
Keep brand and tool names (VRChat, Quest, lilToon, Poiyomi, Modular Avatar, VRCFury, FBX, Unity) untranslated. Labels are short; no trailing punctuation. \
Reply with a JSON object mapping each given key to its translation, nothing else.";

pub const ENRICH_SYSTEM: &str = "You classify and translate listings from BOOTH, a Japanese marketplace, for a catalogue of VRChat avatar assets. \
Reply with one JSON object with exactly these keys:\n\
\"categories\": array of 1-3 category slugs from the given list, best match first. Judge from the title, tags, variations and \
description: sellers often pick the Booth category carelessly or too broadly, so treat it only as a weak hint;\n\
\"content_rating\": \"SFW\", \"Suggestive\" (revealing swimwear/lingerie, suggestive poses) or \"NSFW\" (nudity, sexual content);\n\
\"quest_compatible\": \"Yes\", \"No\", or \"Non-Applicable\" (not an avatar or avatar asset);\n\
\"smart_price\": integer yen a typical buyer pays for the main product (ignore support/donation tiers, pick the full set if that is the obvious main item), or null if unclear;\n\
\"smart_price_reason\": one short English sentence;\n\
\"translations\": {\"en\": {...}, \"ja\": {...}, \"ko\": {...}} where each has \"title\", \"bio\" (the description, translated faithfully, keep URLs), \
and \"variations\" (object mapping each original variation name to its translation). For the item's own language, copy the original text.\n\
\"original_language\": ISO 639-1 code of the listing's main language;\n\
\"features\": array with any of \"ma\" (Modular Avatar setup), \"liltoon\", \"poiyomi\" (shaders it supports), \"vrcfury\" that the listing states it supports; empty if none.";

/// User message for one item. Long descriptions are cut to keep cost bounded.
pub fn enrich_prompt(item: &Value, category_list: &str) -> String {
    let bio: String = item["bio"].as_str().unwrap_or_default().chars().take(2500).collect();
    let variations: Vec<String> = item["variations"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|v| format!("- {} (¥{})", v["name"].as_str().unwrap_or_default(), v["price_jpy"]))
        .collect();
    format!(
        "Categories (slug: meaning):\n{category_list}\n\nListing\nTitle: {}\nBooth category: {}\nListed price: ¥{}\nTags: {}\nVariations:\n{}\nDescription:\n{bio}",
        item["title"].as_str().unwrap_or_default(),
        item["booth_category"].as_str().unwrap_or_default(),
        item["price_jpy"],
        item["booth_tags"].as_array().map(|t| t.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(", ")).unwrap_or_default(),
        if variations.is_empty() { "(none)".to_owned() } else { variations.join("\n") },
    )
}

/// Validated enrichment result, ready to write.
#[derive(Debug, PartialEq)]
pub struct Enrichment {
    pub categories: Vec<String>,
    pub features: Vec<String>,
    pub content_rating: Option<String>,
    pub quest: Option<String>,
    pub smart_price: Option<i32>,
    pub smart_price_reason: Option<String>,
    pub translations: Value,
}

/// Keeps only well-formed values; unknown category slugs are dropped rather than trusted.
/// Requires the essentials (categories, rating, features, price, three titles); incomplete replies return Err.
/// Optional translation parts the reply omits or gets wrong (bio, variations, original_language) keep the
/// values from `existing` (the item's current translations) instead of being blanked.
pub fn validate_enrichment(v: &Value, known_categories: &[String], bio_truncated: bool, existing: &Value) -> Result<Enrichment> {
    let pick = |key: &str, allowed: &[&str]| v[key].as_str().filter(|s| allowed.contains(s)).map(str::to_owned);
    let categories: Vec<String> = v["categories"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .filter(|s| known_categories.iter().any(|k| k == s))
        .fold(Vec::<String>::new(), |mut acc, s| {
            if !acc.iter().any(|x| x == s) {
                acc.push(s.to_owned());
            }
            acc
        })
        .into_iter()
        .take(3)
        .collect();
    if categories.is_empty() {
        bail!("missing or invalid categories in AI reply");
    }

    let content_rating = pick("content_rating", &["SFW", "Suggestive", "NSFW"])
        .ok_or_else(|| anyhow::anyhow!("missing or invalid content_rating in AI reply"))?;

    let features_arr = v.get("features").and_then(Value::as_array).ok_or_else(|| anyhow::anyhow!("missing features array in AI reply"))?;
    let features: Vec<String> = features_arr
        .iter()
        .filter_map(Value::as_str)
        .filter(|f| ["ma", "liltoon", "poiyomi", "vrcfury"].contains(f))
        .fold(Vec::<String>::new(), |mut acc, s| {
            if !acc.iter().any(|x| x == s) {
                acc.push(s.to_owned());
            }
            acc
        });

    let smart_price = match v.get("smart_price") {
        Some(Value::Null) => None,
        Some(val) => {
            if let Some(p) = val.as_i64() {
                if (0..=10_000_000).contains(&p) {
                    Some(p as i32)
                } else {
                    bail!("smart_price out of range in AI reply");
                }
            } else {
                bail!("invalid smart_price in AI reply (expected integer or null)");
            }
        }
        None => bail!("missing smart_price in AI reply"),
    };

    let mut translations = json!({});
    for lang in ["en", "ja", "ko"] {
        let t = &v["translations"][lang];
        let title_str = t["title"].as_str().ok_or_else(|| anyhow::anyhow!("missing translations.{lang}.title string in AI reply"))?;
        if title_str.is_empty() || title_str.chars().count() > 300 {
            bail!("translations.{lang}.title must be between 1 and 300 characters");
        }
        let old = &existing[lang];
        let bio = if bio_truncated {
            // the model only saw the first 2500 chars; "" makes the UI show the full original instead
            Value::String(String::new())
        } else {
            match t["bio"].as_str().filter(|b| b.chars().count() <= 20000) {
                Some(b) => Value::String(b.to_owned()),
                None => old.get("bio").filter(|b| b.is_string()).cloned().unwrap_or_else(|| Value::String(String::new())),
            }
        };
        let variations = match t["variations"].as_object() {
            Some(obj) => Value::Object(
                obj.iter()
                    .filter_map(|(k, val)| val.as_str().filter(|s| s.chars().count() <= 300).map(|s| (k.clone(), Value::String(s.to_owned()))))
                    .collect(),
            ),
            None => old.get("variations").filter(|o| o.is_object()).cloned().unwrap_or_else(|| json!({})),
        };
        translations[lang] = json!({ "title": title_str, "bio": bio, "variations": variations });
    }
    if let Some(l) = v["original_language"].as_str().filter(|l| l.len() == 2).or_else(|| existing["original_language"].as_str()) {
        translations["original_language"] = json!(l);
    }

    Ok(Enrichment {
        categories,
        features,
        content_rating: Some(content_rating),
        quest: pick("quest_compatible", &["Yes", "No", "Non-Applicable"]),
        smart_price,
        smart_price_reason: v["smart_price_reason"].as_str().map(|s| s.chars().take(300).collect()),
        translations,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_fenced_json() {
        assert_eq!(parse_json_reply("```json\n{\"a\": 1}\n```").unwrap(), json!({"a": 1}));
        assert!(parse_json_reply("sorry").is_err());
    }

    #[test]
    fn reversed_braces_does_not_panic() {
        assert!(parse_json_reply("} text {").is_err());
    }

    #[test]
    fn validation_rejects_empty_or_missing_essentials() {
        let known = vec!["clothing--dresses".to_owned(), "hair--long".to_owned()];
        // {} rejected
        assert!(validate_enrichment(&json!({}), &known, false, &json!({})).is_err());
        // reviewer repro: {"content_rating":"SFW","categories":["clothing--dresses"]} rejected (missing translations, features, smart_price)
        assert!(validate_enrichment(
            &json!({"content_rating": "SFW", "categories": ["clothing--dresses"]}),
            &known,
            false,
            &json!({})
        ).is_err());
        // missing categories rejected
        assert!(validate_enrichment(
            &json!({
                "content_rating": "SFW",
                "features": [],
                "smart_price": 500,
                "translations": {
                    "en": {"title": "Dress"},
                    "ja": {"title": "ドレス"},
                    "ko": {"title": "드레스"}
                }
            }),
            &known,
            false,
            &json!({})
        ).is_err());
        // missing content_rating rejected
        assert!(validate_enrichment(
            &json!({
                "categories": ["clothing--dresses"],
                "features": [],
                "smart_price": 500,
                "translations": {
                    "en": {"title": "Dress"},
                    "ja": {"title": "ドレス"},
                    "ko": {"title": "드레스"}
                }
            }),
            &known,
            false,
            &json!({})
        ).is_err());
        // missing one of the titles rejected
        assert!(validate_enrichment(
            &json!({
                "categories": ["clothing--dresses"],
                "content_rating": "SFW",
                "features": [],
                "smart_price": 500,
                "translations": {
                    "en": {"title": "Dress"},
                    "ja": {"title": "ドレス"}
                }
            }),
            &known,
            false,
            &json!({})
        ).is_err());
        // missing features array rejected
        assert!(validate_enrichment(
            &json!({
                "categories": ["clothing--dresses"],
                "content_rating": "SFW",
                "smart_price": 500,
                "translations": {
                    "en": {"title": "Dress"},
                    "ja": {"title": "ドレス"},
                    "ko": {"title": "드레스"}
                }
            }),
            &known,
            false,
            &json!({})
        ).is_err());
        // missing smart_price key rejected
        assert!(validate_enrichment(
            &json!({
                "categories": ["clothing--dresses"],
                "content_rating": "SFW",
                "features": [],
                "translations": {
                    "en": {"title": "Dress"},
                    "ja": {"title": "ドレス"},
                    "ko": {"title": "드레스"}
                }
            }),
            &known,
            false,
            &json!({})
        ).is_err());
    }

    #[test]
    fn validation_accepts_full_reply_and_null_smart_price() {
        let known = vec!["clothing--dresses".to_owned(), "hair--long".to_owned()];
        let mut v = json!({
            "categories": ["clothing--dresses", "made-up", "hair--long", "hair--long"],
            "features": ["liltoon"],
            "content_rating": "SFW",
            "quest_compatible": "Yes",
            "smart_price": 890,
            "smart_price_reason": "main set",
            "translations": {
                "en": {
                    "title": "Dress",
                    "bio": "b",
                    "variations": {
                        "A": "String Val",
                        "B": {"nested": "bad"},
                        "C": 123
                    }
                },
                "ja": {"title": "ドレス"},
                "ko": {"title": "드레스"}
            },
            "original_language": "ja"
        });
        // normal (not truncated): bio is kept
        let e = validate_enrichment(&v, &known, false, &json!({})).unwrap();
        assert_eq!(e.categories, ["clothing--dresses", "hair--long"]);
        assert_eq!(e.features, ["liltoon"]);
        assert_eq!(e.content_rating.as_deref(), Some("SFW"));
        assert_eq!(e.quest.as_deref(), Some("Yes"));
        assert_eq!(e.smart_price, Some(890));
        assert_eq!(
            e.translations,
            json!({
                "en": {
                    "title": "Dress",
                    "bio": "b",
                    "variations": {"A": "String Val"}
                },
                "ja": {"title": "ドレス", "bio": "", "variations": {}},
                "ko": {"title": "드레스", "bio": "", "variations": {}},
                "original_language": "ja"
            })
        );

        // smart_price: null accepted
        v["smart_price"] = Value::Null;
        let e_null = validate_enrichment(&v, &known, false, &json!({})).unwrap();
        assert_eq!(e_null.smart_price, None);

        // truncated: bio is cleared to "" so UI falls back to original
        let e_trunc = validate_enrichment(&v, &known, true, &json!({})).unwrap();
        assert_eq!(e_trunc.translations["en"]["bio"], "");
        assert_eq!(e_trunc.translations["en"]["title"], "Dress");
        assert_eq!(e_trunc.translations["en"]["variations"], json!({"A": "String Val"}));
    }

    #[test]
    fn omitted_translation_parts_keep_existing_values() {
        let known = vec!["clothing--dresses".to_owned()];
        // review repro: titles only, no bio / variations / original_language
        let reply = json!({
            "categories": ["clothing--dresses"], "content_rating": "SFW", "features": [], "smart_price": null,
            "translations": {"en": {"title": "Dress"}, "ja": {"title": "ドレス"}, "ko": {"title": "드레스"}}
        });
        let existing = json!({
            "en": {"title": "Old", "bio": "Full English description", "variations": {"A": "Set A"}},
            "ja": {"title": "旧", "bio": "日本語の説明", "variations": {"A": "セットA"}},
            "original_language": "ja"
        });
        let e = validate_enrichment(&reply, &known, false, &existing).unwrap();
        assert_eq!(e.translations["en"]["title"], "Dress");
        assert_eq!(e.translations["en"]["bio"], "Full English description");
        assert_eq!(e.translations["en"]["variations"], json!({"A": "Set A"}));
        assert_eq!(e.translations["ja"]["bio"], "日本語の説明");
        // no previous Korean data: falls back to empty, not an error
        assert_eq!(e.translations["ko"]["bio"], "");
        assert_eq!(e.translations["ko"]["variations"], json!({}));
        assert_eq!(e.translations["original_language"], "ja");
        // values the reply does send still replace the old ones
        let reply2 = json!({
            "categories": ["clothing--dresses"], "content_rating": "SFW", "features": [], "smart_price": null,
            "translations": {"en": {"title": "Dress", "bio": "New", "variations": {"A": "New A"}}, "ja": {"title": "ド"}, "ko": {"title": "드"}},
            "original_language": "en"
        });
        let e2 = validate_enrichment(&reply2, &known, false, &existing).unwrap();
        assert_eq!(e2.translations["en"]["bio"], "New");
        assert_eq!(e2.translations["en"]["variations"], json!({"A": "New A"}));
        assert_eq!(e2.translations["original_language"], "en");
        // a truncated source still blanks the bio on purpose, even with an old one present
        let e3 = validate_enrichment(&reply, &known, true, &existing).unwrap();
        assert_eq!(e3.translations["en"]["bio"], "");
    }

    #[test]
    fn request_sends_reasoning_effort_as_configured() {
        let mut c = catalogue_core::settings::resolve(&json!({ "base_url": "http://x/v1", "model": "m", "api_key": "k" }), |_| None);
        let ai = Ai::from_config(&c).unwrap().unwrap();
        assert_eq!(ai.body("s", "u")["reasoning_effort"], "none");
        c.reasoning_effort = "high";
        assert_eq!(Ai::from_config(&c).unwrap().unwrap().body("s", "u")["reasoning_effort"], "high");
    }
}
