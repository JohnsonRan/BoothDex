//! System prompts, request builders, reply schema and enrichment reply validation.

use anyhow::{Result, bail};
use serde_json::{Value, json};

use super::{AvatarName, TAG_LANGUAGES};

pub const LOCALIZE_SYSTEM: &str = "You translate UI labels of a shop for VRChat avatar assets (clothing, hair, accessories, shaders, tools) \
into Simplified Chinese (zh-CN). Use the terms Chinese VRChat users use (e.g. 衣装/服装, 发型, 配饰; 虚拟形象 for an avatar, \
3D模型 for a 3D model file) and the terms of Unity's Chinese manual: 纹理 for a texture, X贴图 for an X map (法线贴图 for a normal map), \
材质 for a material, 着色器 for a shader, 网格 for a mesh. \
Keep brand and tool names (VRChat, Quest, lilToon, Poiyomi, Modular Avatar, VRCFury, FBX, Unity) untranslated. Labels are short; no trailing punctuation. \
Reply with a JSON object mapping each given key to its translation, nothing else.";

pub const TAG_SYSTEM: &str = "You translate BOOTH tags for a catalogue of VRChat avatar assets (clothing, hair, accessories, shaders, tools, avatars). \
The user message may first name the avatars the tags hold; then it is a JSON array of items {\"id\", \"tag\", \"context\"}. \
Translate only \"tag\". \"context\" holds titles of listings that carry the tag: never translate it or copy it into an answer. \
For every item give an object with keys tag, en, ja, ko and zh: tag copies the item's tag character for character; en, ja, ko \
and zh give the tag as a short search keyword in English, Japanese, Korean and Simplified Chinese. Copy the tag where it \
already is in that language. Translate the tag itself: add nothing it does not say. \
Avatar (character model) names are never translated or transliterated: keep each one as the tag writes it, in Japanese or Latin \
letters, in every value; the English value uses its romaji. Product, app, tool, brand and shop names stay as written (VRChat, \
Quest, lilToon, Poiyomi, Modular Avatar, VRCFury, FBX, Unity, GIFTuber). Community jargon is translated by its meaning, not \
word by word: 白モデル is an untextured base model (zh 白模), 地雷系 is jirai kei in English. zh uses the terms Chinese VRChat \
users use (衣装/服装, 发型, 配饰; 虚拟形象 for an avatar, 3D模型 for a 3D model file, 虚拟主播 for a VTuber) and the terms of \
Unity's Chinese manual: 纹理 for a texture, X贴图 for an X map (法线贴图 for a normal map), 材质 for a material, 着色器 for a \
shader, 网格 for a mesh. Each value is one line of at most 60 characters, in the script of its language. \
Reply with one JSON object whose keys are the ids and whose values are objects with exactly the keys tag, en, ja, ko and zh, \
nothing else.";

/// User message for one batch of BOOTH tags: the avatars the matcher found in the tags (avatars.rs), so the model keeps
/// their names, then the tags as a JSON array of {id, tag, context}, the ids `tag_id`, the context up to 3 titles of
/// listings that carry the tag (each cut to 120 characters). Keying the reply by id, not by the tag, keeps the model from
/// echoing tags and titles back (owner's test of the formats, 2026-11-21).
pub fn tag_prompt(tags: &[(String, Vec<String>)], avatars: &[&AvatarName]) -> String {
    let items: Vec<Value> = tags
        .iter()
        .enumerate()
        .map(|(i, (tag, titles))| json!({ "id": tag_id(i), "tag": tag, "context": titles.iter().take(3).map(|t| t.chars().take(120).collect::<String>()).collect::<Vec<_>>() }))
        .collect();
    format!("{}{}", avatar_names_line(avatars), Value::Array(items))
}

/// The id of the `i`-th tag of a batch, which keys its answer.
pub fn tag_id(i: usize) -> String {
    format!("t{}", i + 1)
}

/// The reply schema for a batch of `n` tags (`Ai::json_schema`): an object with every id, each an object with the tag
/// copied back (`answers_tag`) and the four languages as strings, and nothing else at either level. Length constraints
/// stay out of the schema because strict OpenAI/Gemini schema validators reject `minLength`/`maxLength`; `tag_values`
/// checks the 1..=60 bound on every reply.
pub fn tag_schema(n: usize) -> Value {
    let value = json!({ "type": "string" });
    let mut properties: serde_json::Map<String, Value> = TAG_LANGUAGES.iter().map(|k| (k.to_string(), value.clone())).collect();
    properties.insert("tag".into(), value);
    let tag = json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["tag", "en", "ja", "ko", "zh"],
        "properties": properties,
    });
    let ids: Vec<String> = (0..n).map(tag_id).collect();
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ids,
        "properties": ids.iter().map(|id| (id.clone(), tag.clone())).collect::<serde_json::Map<_, _>>(),
    })
}

/// The prompt line that names the avatars the matcher found, so the model keeps their names; empty without any.
fn avatar_names_line(avatars: &[&AvatarName]) -> String {
    if avatars.is_empty() {
        return String::new();
    }
    let names: Vec<String> = avatars.iter().map(|a| a.to_string()).collect();
    format!("Avatar names (Japanese / romaji): {}\n", names.join(", "))
}

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
\"translations\": {\"en\": {\"title\": ...}, \"ja\": {\"title\": ...}, \"ko\": {\"title\": ...}, \"zh\": {\"title\": ...}}: the title only \
(zh is Simplified Chinese, in the terms Chinese VRChat buyers use: 虚拟形象 for an avatar, 3D模型 for a 3D model file, 贴图 for a \
texture (眼睛贴图); and in the terms of Unity's Chinese manual: X贴图 for an X map such as 法线贴图 for a normal map, 材质 for a material, \
着色器 for a shader, 网格 for a mesh); for the item's own language, copy the original title with proper JSON escaping. Each title must be non-empty and at most 300 characters. Do not translate the \
description or variations. Avatar (character model) names are never translated or transliterated: keep each one as the original title writes \
it, in Japanese or Latin letters, in every title; the English title uses its romaji. The listing names the avatars found in it, with their romaji.\n\
\"original_language\": ISO 639-1 code of the listing's main language;\n\
\"features\": array with any of \"ma\" (Modular Avatar setup), \"liltoon\", \"poiyomi\" (shaders it supports), \"vrcfury\" that the listing states it supports; empty if none.\n\
Before replying, check that the JSON is syntactically valid, every required key is present, and categories contains at least one exact listed slug.";

/// User message for one item. Long descriptions are cut to keep cost bounded. `avatars`: the avatars the matcher
/// found in the listing (avatars.rs), named so the model keeps their names as written.
pub fn enrich_prompt(item: &EnrichItem, category_list: &str, avatars: &[&AvatarName]) -> String {
    let bio: String = item.bio.chars().take(2500).collect();
    let variations: Vec<String> = item.variations.0.iter().map(|v| format!("- {} (¥{})", v.name, v.price_jpy)).collect();
    let avatars = avatar_names_line(avatars);
    format!(
        "Categories (slug: meaning):\n{category_list}\n\nListing\nTitle: {}\nBooth category: {}\nListed price: ¥{}\nTags: {}\nVariations:\n{}\n{avatars}Description:\n{bio}",
        item.title,
        item.booth_category.as_deref().unwrap_or_default(),
        item.price_jpy.map_or_else(|| "null".to_owned(), |p| p.to_string()),
        item.booth_tags.join(", "),
        if variations.is_empty() { "(none)".to_owned() } else { variations.join("\n") },
    )
}

/// One `items` row waiting for enrichment. `price_jpy` is null for a few malformed listings; a variation's
/// price is kept as the stored JSON, which prints as `null` when Booth gave none.
#[derive(sqlx::FromRow)]
pub struct EnrichItem {
    pub id: i64,
    pub title: String,
    pub bio: String,
    pub booth_category: Option<String>,
    pub price_jpy: Option<i32>,
    pub booth_tags: Vec<String>,
    pub variations: sqlx::types::Json<Vec<Variation>>,
    pub translations: Value,
}

#[derive(serde::Deserialize)]
pub struct Variation {
    pub name: String,
    pub price_jpy: Value,
}

impl EnrichItem {
    pub fn match_text(&self) -> String {
        crate::avatars::match_text(&self.title, &self.booth_tags, self.variations.0.iter().map(|v| v.name.as_str()))
    }
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
    pub translations: Value,
}

/// An exact slug, or one the model shortened at the front ("animations-…--dance-…" for "avatar-animations-…--dance-…")
/// when exactly one known slug ends with it. The child part (after "--") must be there; anything else is dropped.
pub(crate) fn resolve_category<'a>(s: &str, known: &'a [String]) -> Option<&'a str> {
    if let Some(k) = known.iter().find(|k| *k == s) {
        return Some(k);
    }
    if !s.contains("--") || s.starts_with('-') {
        return None;
    }
    let suffix = format!("-{s}");
    let mut hits = known.iter().filter(|k| k.ends_with(&suffix));
    match (hits.next(), hits.next()) {
        (Some(k), None) => Some(k),
        _ => None,
    }
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
        .filter_map(|s| resolve_category(s, known_categories))
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
    for lang in TAG_LANGUAGES {
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
    if let Some(l) = v["original_language"]
        .as_str()
        .filter(|l| l.len() == 2 && l.bytes().all(|b| b.is_ascii_lowercase()))
        .or_else(|| existing["original_language"].as_str())
    {
        translations["original_language"] = json!(l);
    }

    Ok(Enrichment {
        categories,
        features,
        content_rating: Some(content_rating),
        quest: pick("quest_compatible", &["Yes", "No", "Non-Applicable"]),
        smart_price,
        translations,
    })
}
