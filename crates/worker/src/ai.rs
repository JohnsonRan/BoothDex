//! OpenAI-compatible chat client (JSON mode) plus the prompts used by the `localize` and `enrich` jobs.

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
                    if attempt == 4 {
                        break;
                    }
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

pub const ENRICH_SYSTEM: &str = "You classify listings and translate their titles from BOOTH, a Japanese marketplace, for a catalogue of VRChat avatar assets. \
Reply with one valid JSON object only, without Markdown fences, comments or extra text. \
Escape double quotes, backslashes and control characters inside every JSON string, including copied original titles. \
Use commas between fields and array elements, with no trailing commas. Include exactly these keys:\n\
\"categories\": non-empty array of 1-3 exact category slugs from the given list, best match first. Judge from the title, tags, variations and \
description: sellers often pick the Booth category carelessly or too broadly, so treat it only as a weak hint. \
If the listing lacks an explicit category or no category fits exactly, infer the closest available category from the product's \
purpose and description. Always choose at least one listed slug; never return an empty array, invent a slug, or use a parent name instead;\n\
\"content_rating\": \"SFW\", \"Suggestive\" (revealing swimwear/lingerie, suggestive poses) or \"NSFW\" (nudity, sexual content);\n\
\"quest_compatible\": \"Yes\", \"No\", or \"Non-Applicable\" (not an avatar or avatar asset);\n\
\"smart_price\": integer yen a typical buyer pays for the main product (ignore support/donation tiers, pick the full set if that is the obvious main item), or null if unclear;\n\
\"smart_price_reason\": one short English sentence;\n\
\"translations\": {\"en\": {\"title\": ...}, \"ja\": {\"title\": ...}, \"ko\": {\"title\": ...}, \"zh\": {\"title\": ...}}: the title only \
(zh is Simplified Chinese, in the terms Chinese VRChat users use); for the item's own language, copy the original title with proper JSON escaping. \
Each title must be non-empty and at most 300 characters. Do not translate the description or variations.\n\
\"original_language\": ISO 639-1 code of the listing's main language;\n\
\"features\": array with any of \"ma\" (Modular Avatar setup), \"liltoon\", \"poiyomi\" (shaders it supports), \"vrcfury\" that the listing states it supports; empty if none.\n\
Before replying, check that the JSON is syntactically valid, every required key is present, and categories contains at least one exact listed slug.";

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

/// Validated enrichment result, ready to write. Only titles are translated: they feed cross-language search
/// and embeddings; descriptions and variation names went unread and made replies long enough to break.
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
/// Requires the essentials (categories, rating, features, price, en/ja/ko titles); incomplete replies return Err.
/// A missing zh title or original_language keeps the value from `existing` (the item's current translations).
pub fn validate_enrichment(v: &Value, known_categories: &[String], existing: &Value) -> Result<Enrichment> {
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
        let got: String = v["categories"].to_string().chars().take(200).collect();
        bail!("missing or invalid categories in AI reply: {got}");
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
    for lang in ["en", "ja", "ko", "zh"] {
        let t = &v["translations"][lang];
        let old = &existing[lang];
        if lang == "zh" && !t["title"].as_str().is_some_and(|s| !s.is_empty() && s.chars().count() <= 300) {
            // zh came after en/ja/ko: optional, so a reply without it doesn't fail the rest (an earlier zh is kept)
            if let Some(title) = old["title"].as_str() {
                translations[lang] = json!({ "title": title });
            }
            continue;
        }
        let title_str = t["title"].as_str().ok_or_else(|| anyhow::anyhow!("missing translations.{lang}.title string in AI reply"))?;
        if title_str.is_empty() || title_str.chars().count() > 300 {
            bail!("translations.{lang}.title must be between 1 and 300 characters");
        }
        translations[lang] = json!({ "title": title_str });
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

    #[tokio::test(start_paused = true)]
    async fn exhausted_retries_return_provider_error_without_extra_sleep() {
        crate::install_tls();
        for status in [429, 503] {
            let (base_url, requests) = crate::retry_test_server(status.try_into().unwrap(), None).await;
            let ai = Ai {
                http: reqwest::Client::builder().no_proxy().pool_max_idle_per_host(0).build().unwrap(),
                base_url,
                model: "test".into(),
                api_key: "test".into(),
                reasoning_effort: "none",
            };
            let start = tokio::time::Instant::now();
            let error = ai.json("system", "user").await.unwrap_err();
            assert!(error.downcast_ref::<ProviderError>().is_some());
            assert_eq!(requests.load(std::sync::atomic::Ordering::Relaxed), 5);
            assert_eq!(start.elapsed(), Duration::from_secs(30));
        }
    }

    #[test]
    fn parses_fenced_json() {
        assert_eq!(parse_json_reply("```json\n{\"a\": 1}\n```").unwrap(), json!({"a": 1}));
        assert!(parse_json_reply("sorry").is_err());
    }

    #[test]
    fn parses_escaped_title_quotes_and_backslashes() {
        let reply = r#"{"translations":{"ja":{"title":"SPS対応『最強快感巧具\"轟\" HM-25V Pecker』 \\ VRChat"}}}"#;
        assert_eq!(
            parse_json_reply(reply).unwrap()["translations"]["ja"]["title"],
            "SPS対応『最強快感巧具\"轟\" HM-25V Pecker』 \\ VRChat"
        );
        assert!(parse_json_reply(r#"{"title":"最強快感巧具"轟""}"#).is_err());
    }

    #[test]
    fn enrichment_prompt_includes_classification_evidence() {
        let categories = "accessories--rings: Accessories / Rings\nprops--other: Props / Other";
        let item = json!({
            "title": "【VRChat想定】No Signal Ring【MA対応】",
            "booth_category": "3D小道具",
            "price_jpy": 500,
            "booth_tags": ["VRChat", "Modular Avatar"],
            "variations": [{"name": "Full set", "price_jpy": 500}],
            "bio": "An animated ring accessory for avatar fingers."
        });
        let prompt = enrich_prompt(&item, categories);
        for evidence in [
            categories,
            "Title: 【VRChat想定】No Signal Ring【MA対応】",
            "Booth category: 3D小道具",
            "Listed price: ¥500",
            "Tags: VRChat, Modular Avatar",
            "- Full set (¥500)",
            "Description:\nAn animated ring accessory for avatar fingers.",
        ] {
            assert!(prompt.contains(evidence), "missing evidence: {evidence}");
        }
    }

    #[test]
    fn reversed_braces_does_not_panic() {
        assert!(parse_json_reply("} text {").is_err());
    }

    #[test]
    fn validation_rejects_empty_or_missing_essentials() {
        let known = vec!["clothing--dresses".to_owned(), "hair--long".to_owned()];
        // {} rejected
        assert!(validate_enrichment(&json!({}), &known, &json!({})).is_err());
        // reviewer repro: {"content_rating":"SFW","categories":["clothing--dresses"]} rejected (missing translations, features, smart_price)
        assert!(validate_enrichment(
            &json!({"content_rating": "SFW", "categories": ["clothing--dresses"]}),
            &known,
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
        // a bio or variations the model sends anyway are dropped: only titles are stored
        let e = validate_enrichment(&v, &known, &json!({})).unwrap();
        assert_eq!(e.categories, ["clothing--dresses", "hair--long"]);
        assert_eq!(e.features, ["liltoon"]);
        assert_eq!(e.content_rating.as_deref(), Some("SFW"));
        assert_eq!(e.quest.as_deref(), Some("Yes"));
        assert_eq!(e.smart_price, Some(890));
        assert_eq!(
            e.translations,
            json!({
                "en": {"title": "Dress"},
                "ja": {"title": "ドレス"},
                "ko": {"title": "드레스"},
                "original_language": "ja"
            })
        );

        // smart_price: null accepted
        v["smart_price"] = Value::Null;
        let e_null = validate_enrichment(&v, &known, &json!({})).unwrap();
        assert_eq!(e_null.smart_price, None);
    }

    #[test]
    fn titles_replace_old_translations_and_original_language_is_kept() {
        let known = vec!["clothing--dresses".to_owned()];
        let reply = json!({
            "categories": ["clothing--dresses"], "content_rating": "SFW", "features": [], "smart_price": null,
            "translations": {"en": {"title": "Dress"}, "ja": {"title": "ドレス"}, "ko": {"title": "드레스"}}
        });
        let existing = json!({
            "en": {"title": "Old", "bio": "Full English description", "variations": {"A": "Set A"}},
            "ja": {"title": "旧", "bio": "日本語の説明", "variations": {"A": "セットA"}},
            "original_language": "ja"
        });
        // old descriptions and variation names go; a missing original_language keeps the old one
        let e = validate_enrichment(&reply, &known, &existing).unwrap();
        assert_eq!(
            e.translations,
            json!({"en": {"title": "Dress"}, "ja": {"title": "ドレス"}, "ko": {"title": "드레스"}, "original_language": "ja"})
        );
    }

    #[test]
    fn chinese_translation_is_optional() {
        let known = vec!["clothing--dresses".to_owned()];
        let reply = |zh: Value| {
            json!({
                "categories": ["clothing--dresses"], "content_rating": "SFW", "features": [], "smart_price": null,
                "translations": {"en": {"title": "Dress"}, "ja": {"title": "ドレス"}, "ko": {"title": "드레스"}, "zh": zh}
            })
        };
        let e = validate_enrichment(&reply(json!({"title": "连衣裙", "bio": "说明"})), &known, &json!({})).unwrap();
        assert_eq!(e.translations["zh"], json!({"title": "连衣裙"}));
        // missing or invalid zh: no error, no zh entry, or the earlier one kept
        let e = validate_enrichment(&reply(Value::Null), &known, &json!({})).unwrap();
        assert!(e.translations.get("zh").is_none());
        let old = json!({"zh": {"title": "旧", "bio": "", "variations": {}}});
        let e = validate_enrichment(&reply(json!({"title": ""})), &known, &old).unwrap();
        assert_eq!(e.translations["zh"], json!({"title": "旧"}));
    }

    #[test]
    fn request_sends_reasoning_effort_as_configured() {
        crate::install_tls();
        let mut c = boothdex_core::settings::resolve(&json!({ "base_url": "http://x/v1", "model": "m", "api_key": "k" }), |_| None);
        let ai = Ai::from_config(&c).unwrap().unwrap();
        assert_eq!(ai.body("s", "u")["reasoning_effort"], "none");
        c.reasoning_effort = "high";
        assert_eq!(Ai::from_config(&c).unwrap().unwrap().body("s", "u")["reasoning_effort"], "high");
    }
}
