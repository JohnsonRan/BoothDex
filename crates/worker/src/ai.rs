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

    fn body(&self, messages: Value, response_format: Value) -> Value {
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
                    let bytes = r.bytes().await.map_err(|e| ProviderError(format!("provider response body interrupted: {}", e.without_url())))?;
                    let v: Value = serde_json::from_slice(&bytes)?;
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
/// copied back (`answers_tag`) and the four languages as strings of 1 to 60 characters, and nothing else at either level.
pub fn tag_schema(n: usize) -> Value {
    let value = json!({ "type": "string", "minLength": 1, "maxLength": 60 });
    let mut properties: serde_json::Map<String, Value> = TAG_LANGUAGES.iter().map(|k| (k.to_string(), value.clone())).collect();
    properties.insert("tag".into(), json!({ "type": "string" }));
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

/// Whether `answer` (the reply's entry under a tag's id) copies `tag` back, ignoring case, width and surrounding space:
/// a model that loses its place in a batch files a tag's translation under another tag's id (Rira -> 花鸟风月巫女礼服 in
/// the local backfill; owner ruling 29, 2026-11-21).
pub fn answers_tag(answer: &Value, tag: &str) -> bool {
    answer["tag"].as_str().is_some_and(|echo| crate::avatars::norm(echo.trim()) == crate::avatars::norm(tag.trim()))
}

/// The languages of a tag's dictionary row, in the column order of `booth_tag_translations`.
pub const TAG_LANGUAGES: [&str; 4] = ["en", "ja", "ko", "zh"];

/// An avatar name a tag or title holds, as the forms a value may keep it in, NFKC and lowercase: the forms the text
/// writes it in, its Japanese name and its romaji (`forms`), and those in Latin letters (`latin`, `latin_letters`),
/// which an English title or tag value keeps. A name is never translated or transliterated (owner ruling 13,
/// 2026-11-20).
pub struct HeldName {
    pub forms: Vec<String>,
    pub latin: Vec<String>,
}

impl HeldName {
    /// The forms a value keeps the name in: English the Latin ones, the other languages any.
    fn kept_in(&self, english: bool) -> &[String] {
        if english { &self.latin } else { &self.forms }
    }

    /// Whether `folded` (NFKC, lowercase) drops the name. A name without a form in Latin letters gives an English value
    /// nothing to keep.
    fn dropped_from(&self, folded: &str, english: bool) -> bool {
        let forms = self.kept_in(english);
        !forms.is_empty() && !forms.iter().any(|f| contains_name(folded, f))
    }
}

/// Both inputs are NFKC and lowercase. CJK names can occur inside compounds; Latin names must stand apart
/// from other letters and digits, but may directly precede or follow CJK text.
pub fn contains_name(folded: &str, name: &str) -> bool {
    if !latin_letters(name) {
        return folded.contains(name);
    }
    let joins_word = |c: char| c.is_alphanumeric() && !(is_han(c) || is_kana(c) || is_hangul(c));
    folded.match_indices(name).any(|(i, _)| {
        !folded[..i].chars().next_back().is_some_and(joins_word) && !folded[i + name.len()..].chars().next().is_some_and(joins_word)
    })
}

/// One tag's values from its answer (`answer` is the reply's entry for the tag's id), in `TAG_LANGUAGES` order: each
/// trimmed, or why it is left out. A value is left out unless it is at most 60 non-control characters, has no script
/// foreign to its language (Han, kana or Hangul in English; Hangul in Japanese; Han or kana in Korean; kana or Hangul in
/// Chinese) that the tag is not written in, is not one of the tag's example `titles` (nor, over 20 characters, a part
/// of one) unless it is the tag itself, and keeps every avatar name the tag holds (English in Latin letters, the romaji).
/// `japanese`: every avatar's Japanese name (`AvatarName::japanese_names`), which a Japanese, Korean or Chinese value may
/// hold in any script.
pub fn tag_values(answer: &Value, tag: &str, titles: &[String], names: &[HeldName], japanese: &[String]) -> [std::result::Result<String, String>; 4] {
    let scripts: [fn(char) -> bool; 3] = [is_han, is_kana, is_hangul];
    let foreign: [&[usize]; 4] = [&[0, 1, 2], &[2], &[0, 1], &[1, 2]];
    std::array::from_fn(|k| {
        let v = answer[TAG_LANGUAGES[k]].as_str().map(str::trim).filter(|v| !v.is_empty()).ok_or("no value")?;
        // a control character (a line break, or a NUL, which PostgreSQL text refuses) is no part of a keyword
        if v.chars().count() > 60 || v.chars().any(char::is_control) {
            return Err("not one line of at most 60 characters without control characters".into());
        }
        // English keeps an avatar's romaji instead (owner ruling 13)
        let unnamed = if TAG_LANGUAGES[k] == "en" { v.to_owned() } else { without_names(v, japanese) };
        if foreign[k].iter().any(|&s| unnamed.chars().any(scripts[s]) && !tag.chars().any(scripts[s])) {
            return Err("in a script foreign to its language".into());
        }
        if v != tag.trim() && titles.iter().any(|t| t.trim() == v || (v.chars().count() > 20 && t.contains(v))) {
            return Err("copies an example title".into());
        }
        let (folded, english) = (crate::avatars::norm(v), TAG_LANGUAGES[k] == "en");
        if let Some(lost) = names.iter().find(|n| n.dropped_from(&folded, english)) {
            return Err(format!("drops the avatar name {}", lost.kept_in(english)[0]));
        }
        Ok(v.to_owned())
    })
}

/// A script by its name, and whether a character is written in it.
type Script = (&'static str, fn(char) -> bool);

/// The checks of each translated title an enrich reply gives (owner ruling 30, 2026-11-21), for one listing.
pub struct TitleCheck<'a> {
    /// the listing's own title
    pub original: &'a str,
    /// the avatar names the matcher finds in that title
    pub names: Vec<HeldName>,
    /// every avatar's Japanese name (`AvatarName::japanese_names`)
    pub japanese: &'a [String],
}

impl TitleCheck<'_> {
    /// Why the title in `lang` fails, or `None`. A title fails when it drops an avatar name the original holds (English
    /// keeps the romaji), has a number the original lacks, or holds a script foreign to its language (as `tag_values`
    /// reads scripts; an avatar's Japanese name may stay in any but English). The title in the listing's own language,
    /// `own`, is not checked when it copies the original, as the prompt asks.
    pub fn problem(&self, lang: &str, title: &str, own: Option<&str>) -> Option<String> {
        if own == Some(lang) && title.trim() == self.original.trim() {
            return None;
        }
        // PostgreSQL's jsonb refuses a NUL, which would fail the item's write and stop the batch
        if title.contains('\0') {
            return Some("holds a NUL character".into());
        }
        let folded = crate::avatars::norm(title);
        let english = lang == "en";
        if let Some(lost) = self.names.iter().find(|n| n.dropped_from(&folded, english)) {
            return Some(format!("drops the avatar name {}", lost.kept_in(english)[0]));
        }
        let allowed = numbers_of_original(self.original);
        if let Some(n) = numbers(title).into_iter().find(|n| !allowed.contains(n)) {
            return Some(format!("has the number {n}, which the original title lacks"));
        }
        let scripts: [Script; 3] = [("Han characters", is_han), ("kana", is_kana), ("Hangul", is_hangul)];
        let foreign: &[usize] = match lang {
            "en" => &[0, 1, 2],
            "ja" => &[2],
            "ko" => &[0, 1],
            _ => &[1, 2],
        };
        let unnamed = if english { folded } else { without_names(title, self.japanese) };
        foreign.iter().map(|&s| scripts[s]).find(|(_, is)| unnamed.chars().any(is)).map(|(script, _)| format!("holds {script}, which a {lang} title does not use"))
    }

    /// The problems of the titles in `translations` (`Enrichment::translations`), as (language, why).
    pub fn problems(&self, translations: &Value) -> Vec<(&'static str, String)> {
        let own = translations["original_language"].as_str();
        TAG_LANGUAGES.iter().filter_map(|&lang| Some((lang, self.problem(lang, translations[lang]["title"].as_str()?, own)?))).collect()
    }

    /// `e` without the titles that fail: each is replaced by the item's earlier title in that language (`existing`) when
    /// that one passes, else left out.
    pub fn keep_passing(&self, mut e: Enrichment, existing: &Value) -> Enrichment {
        let own = e.translations["original_language"].as_str().map(str::to_owned);
        for (lang, _) in self.problems(&e.translations) {
            let earlier = existing[lang]["title"].as_str().filter(|t| self.problem(lang, t, own.as_deref()).is_none());
            match (earlier, e.translations.as_object_mut()) {
                (Some(t), _) => e.translations[lang] = json!({ "title": t }),
                (None, Some(map)) => {
                    map.remove(lang);
                }
                (None, None) => {}
            }
        }
        e
    }
}

/// The correction a reply with title `problems` (`TitleCheck::problems`) gets.
pub fn title_problems_message(problems: &[(&str, String)]) -> String {
    problems.iter().map(|(lang, why)| format!("translations.{lang}.title {why}")).collect::<Vec<_>>().join("; ")
}

/// The numbers `text` writes in digits, after NFKC (full-width, circled and mathematical digits are digits), without
/// leading zeros; a comma between digits is a group separator.
fn numbers(text: &str) -> std::collections::BTreeSet<String> {
    use unicode_normalization::UnicodeNormalization;
    let chars: Vec<char> = text.nfkc().collect();
    let mut out = std::collections::BTreeSet::new();
    let mut digits = String::new();
    for (i, &c) in chars.iter().enumerate() {
        if c.is_ascii_digit() {
            digits.push(c);
        } else if c == ',' && !digits.is_empty() && chars.get(i + 1).is_some_and(char::is_ascii_digit) {
            continue;
        } else if !digits.is_empty() {
            out.insert(number(&digits));
            digits.clear();
        }
    }
    if !digits.is_empty() {
        out.insert(number(&digits));
    }
    out
}

fn number(digits: &str) -> String {
    let n = digits.trim_start_matches('0');
    if n.is_empty() { "0".to_owned() } else { n.to_owned() }
}

/// The numbers a title may write in digits: its original digits and whole CJK numerals up to 99.
/// Larger units stay in the same run so a fragment of a larger number cannot license a smaller one.
fn numbers_of_original(original: &str) -> std::collections::BTreeSet<String> {
    let mut out = numbers(original);
    let runs = original.split(|c: char| !"〇零一二两兩三四五六七八九十百千万萬亿億兆".contains(c));
    out.extend(runs.filter(|s| !s.is_empty()).filter_map(small_numeral).map(|n| n.to_string()));
    out
}

fn small_numeral(s: &str) -> Option<u8> {
    let digit = |s| match s {
        "〇" | "零" => Some(0),
        "两" | "兩" => Some(2),
        _ => ["一", "二", "三", "四", "五", "六", "七", "八", "九"].iter().position(|&d| d == s).map(|i| i as u8 + 1),
    };
    let Some((tens, ones)) = s.split_once('十') else { return digit(s) };
    let tens = if tens.is_empty() { 1 } else { digit(tens)? };
    let ones = if ones.is_empty() { 0 } else { digit(ones)? };
    Some(tens * 10 + ones)
}

/// `text` (NFKC, lowercase) with each of the `japanese` avatar names blanked out: a name is never translated or
/// transliterated, so it may stay in the script it is written in (owner ruling 13; the tag 신라 is the avatar 森羅).
fn without_names(text: &str, japanese: &[String]) -> String {
    japanese.iter().fold(crate::avatars::norm(text), |t, name| t.replace(name.as_str(), " "))
}

/// Search-only keywords for a tag's Chinese value (owner ruling 20, 2026-11-21): tags use Unity's 纹理 for a texture,
/// buyers and titles say 贴图, so a value with 纹理 is also found as the same words with 贴图.
pub fn tag_keywords(zh: &str) -> Vec<String> {
    if zh.contains("纹理") { vec![zh.replace("纹理", "贴图")] } else { Vec::new() }
}

fn is_han(c: char) -> bool {
    matches!(c, '\u{3400}'..='\u{4DBF}' | '\u{4E00}'..='\u{9FFF}' | '\u{F900}'..='\u{FAFF}' | '\u{20000}'..='\u{2FA1F}')
}

fn is_kana(c: char) -> bool {
    matches!(c, '\u{3040}'..='\u{30FF}' | '\u{31F0}'..='\u{31FF}' | '\u{FF66}'..='\u{FF9F}')
}

fn is_hangul(c: char) -> bool {
    matches!(c, '\u{1100}'..='\u{11FF}' | '\u{3130}'..='\u{318F}' | '\u{A960}'..='\u{A97F}' | '\u{AC00}'..='\u{D7AF}' | '\u{FFA0}'..='\u{FFDC}')
}

/// Whether a name is written in Latin letters, accents included (the romaji Alué, Kyalóng): no Han, kana or Hangul.
pub fn latin_letters(name: &str) -> bool {
    !name.chars().any(|c| is_han(c) || is_kana(c) || is_hangul(c))
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

/// An avatar's names from the `avatars` table: Japanese (when known) and romaji (`name_en`).
pub struct AvatarName {
    pub ja: Option<String>,
    pub romaji: String,
}

impl AvatarName {
    /// The Japanese names of `avatars` that are not in Latin letters, NFKC and lowercase, longest first; names under two
    /// characters are left out, as the matcher leaves them out (avatars.rs).
    pub fn japanese_names<'a>(avatars: impl IntoIterator<Item = &'a AvatarName>) -> Vec<String> {
        let mut names: Vec<String> =
            avatars.into_iter().filter_map(|a| a.ja.as_deref()).map(crate::avatars::norm).filter(|n| !n.is_ascii() && n.chars().count() >= 2).collect();
        names.sort_by(|a, b| b.chars().count().cmp(&a.chars().count()).then_with(|| a.cmp(b)));
        names.dedup();
        names
    }
}

impl std::fmt::Display for AvatarName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.ja.as_deref().filter(|ja| *ja != self.romaji) {
            Some(ja) => write!(f, "{ja} / {}", self.romaji),
            None => f.write_str(&self.romaji),
        }
    }
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
fn resolve_category<'a>(s: &str, known: &'a [String]) -> Option<&'a str> {
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

    #[tokio::test]
    async fn correction_resends_rejected_reply_and_error() {
        crate::install_tls();
        let seen = std::sync::Arc::new(std::sync::Mutex::new(Value::Null));
        let store = seen.clone();
        let app = axum::Router::new().fallback(move |axum::Json(body): axum::Json<Value>| {
            *store.lock().unwrap() = body;
            async { axum::Json(json!({"choices": [{"message": {"content": "{\"ok\": true}"}}]})) }
        });
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base_url = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let ai = Ai { http: reqwest::Client::builder().no_proxy().build().unwrap(), base_url, model: "m".into(), api_key: "k".into(), reasoning_effort: "none" };

        let rejected = json!({"categories": ["animations-x"]});
        let reply = ai.json_correction("sys", "item", &rejected, "missing or invalid categories in AI reply: [\"animations-x\"]").await.unwrap();
        assert_eq!(reply, json!({"ok": true}));
        let msgs = seen.lock().unwrap()["messages"].clone();
        let roles: Vec<&str> = msgs.as_array().unwrap().iter().map(|m| m["role"].as_str().unwrap()).collect();
        assert_eq!(roles, ["system", "user", "assistant", "user"]);
        assert_eq!(msgs[2]["content"], rejected.to_string());
        let fix = msgs[3]["content"].as_str().unwrap();
        assert!(fix.contains("animations-x") && fix.contains("first word"), "{fix}");
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
    fn localize_uses_the_chinese_terms_of_vrchat_users_and_unity() {
        for term in ["虚拟形象 for an avatar", "3D模型 for a 3D model file", "纹理 for a texture", "法线贴图 for a normal map", "材质 for a material", "着色器 for a shader", "网格 for a mesh"] {
            assert!(LOCALIZE_SYSTEM.contains(term), "{term}");
        }
    }

    #[test]
    fn enrichment_prompt_includes_classification_evidence() {
        let categories = "accessories--rings: Accessories / Rings\nprops--other: Props / Other";
        let item = EnrichItem {
            id: 1,
            title: "【VRChat想定】No Signal Ring【MA対応】".into(),
            booth_category: Some("3D小道具".into()),
            price_jpy: Some(500),
            booth_tags: vec!["VRChat".into(), "Modular Avatar".into()],
            variations: sqlx::types::Json(vec![Variation { name: "Full set".into(), price_jpy: json!(500) }]),
            bio: "An animated ring accessory for avatar fingers.".into(),
            translations: json!({}),
        };
        let prompt = enrich_prompt(&item, categories, &[]);
        for evidence in [
            categories,
            "Title: 【VRChat想定】No Signal Ring【MA対応】",
            "Booth category: 3D小道具",
            "Listed price: ¥500",
            "Tags: VRChat, Modular Avatar",
            "- Full set (¥500)",
            "Variations:\n- Full set (¥500)\nDescription:\nAn animated ring accessory for avatar fingers.",
        ] {
            assert!(prompt.contains(evidence), "missing evidence: {evidence}");
        }
        // the avatars found in the listing, each name once when the dictionary spells it the same in both scripts
        let manuka = AvatarName { ja: Some("マヌカ".into()), romaji: "Manuka".into() };
        let adolescent = AvatarName { ja: Some("Adolescent U".into()), romaji: "Adolescent U".into() };
        let unnamed = AvatarName { ja: None, romaji: "Rusk".into() };
        let prompt = enrich_prompt(&item, categories, &[&manuka, &adolescent, &unnamed]);
        assert!(
            prompt.contains("- Full set (¥500)\nAvatar names (Japanese / romaji): マヌカ / Manuka, Adolescent U, Rusk\nDescription:\n"),
            "{prompt}"
        );
    }

    /// Which of a tag's values `tag_values` keeps: `Some` for a kept value, `None` for one left out. The avatars known
    /// by a Japanese name are マヌカ and 森羅.
    fn kept(answer: &Value, tag: &str, titles: &[&str], names: &[&[&str]]) -> [Option<String>; 4] {
        let titles: Vec<String> = titles.iter().map(|t| t.to_string()).collect();
        let held = |forms: &&[&str]| {
            let forms: Vec<String> = forms.iter().map(|f| f.to_string()).collect();
            HeldName { latin: forms.iter().filter(|f| latin_letters(f)).cloned().collect(), forms }
        };
        let names: Vec<HeldName> = names.iter().map(held).collect();
        let avatars = [AvatarName { ja: Some("マヌカ".into()), romaji: "Manuka".into() }, AvatarName { ja: Some("森羅".into()), romaji: "Shinra".into() }];
        tag_values(answer, tag, &titles, &names, &AvatarName::japanese_names(&avatars)).map(Result::ok)
    }

    #[test]
    fn each_tag_value_is_checked_on_its_own() {
        let skirt = |en: &str, ja: &str, ko: &str, zh: &str| json!({"en": en, "ja": ja, "ko": ko, "zh": zh});
        let all = |v: [&str; 4]| v.map(|s| Some(s.to_owned()));
        assert_eq!(kept(&skirt(" skirt ", "スカート", "스커트", "裙子"), "スカート", &[], &[]), all(["skirt", "スカート", "스커트", "裙子"]));
        // a second line, a control character, an overlong or empty value: only that language is left out
        for bad in ["裙子\n短裙", "裙\0子", "裙\t子", &"裙".repeat(61), " "] {
            assert_eq!(kept(&skirt("skirt", "スカート", "스커트", bad), "スカート", &[], &[])[3], None, "{bad:?}");
        }
        let overlong = kept(&skirt("skirt", "スカート", "스커트", &"裙".repeat(61)), "スカート", &[], &[]);
        assert!(overlong[..3].iter().all(Option::is_some) && overlong[3].is_none());
        // a script foreign to the language that the tag is not written in
        assert_eq!(kept(&skirt("スカート", "스커트", "裙子", "スカート"), "skirt", &[], &[]), [None, None, None, None]);
        assert_eq!(kept(&skirt("skirt", "スカート", "스커트", "스커트"), "Skirt", &[], &[])[3], None);
        // a script the tag is written in may stay: the avatar name in マヌカ用
        let manuka: &[&str] = &["マヌカ", "manuka"];
        let manuka_for = skirt("for Manuka", "マヌカ用", "マヌカ용", "マヌカ用");
        assert_eq!(kept(&manuka_for, "マヌカ用", &[], &[manuka]), all(["for Manuka", "マヌカ用", "マヌカ용", "マヌカ用"]));
        // an avatar's Japanese name may stay in the Japanese, Korean and Chinese values of a tag written in other
        // letters (신라 is 森羅); English keeps the romaji, and other kana still count against Korean
        let shinra = skirt("Shinra", "森羅", "森羅", "森羅");
        assert_eq!(kept(&shinra, "신라", &[], &[]), [Some("Shinra".into()), Some("森羅".into()), Some("森羅".into()), Some("森羅".into())]);
        assert_eq!(kept(&skirt("森羅", "森羅", "マヌカ 텍스처", "マヌカ纹理"), "Manukatexture", &[], &[])[..], [None, Some("森羅".into()), Some("マヌカ 텍스처".into()), Some("マヌカ纹理".into())]);
        assert_eq!(kept(&skirt("Manuka eye", "マヌカの目", "マヌカの 눈", "マヌカの眼睛"), "Manukaeye", &[], &[])[2..], [None, None]);
        // a name transliterated or translated is left out in that language only; English may give the romaji
        let transliterated = skirt("for Manuka", "マヌカ用", "마누카용", "麦奴卡用");
        assert_eq!(kept(&transliterated, "マヌカ用", &[], &[manuka]), [Some("for Manuka".into()), Some("マヌカ用".into()), None, None]);
        assert_eq!(kept(&skirt("for the avatar", "マヌカ用", "マヌカ용", "マヌカ用"), "マヌカ用", &[], &[manuka])[0], None);
        // English keeps the romaji, not the name in Japanese; a name with no form in Latin letters has none to keep
        assert_eq!(kept(&skirt("マヌカ用", "マヌカ用", "マヌカ용", "マヌカ用"), "マヌカ用", &[], &[manuka])[0], None);
        assert_eq!(kept(&skirt("for Tien", "ティエン用", "티엔용", "ティエン用"), "ティエン用", &[], &[&["ティエン"]])[0], Some("for Tien".into()));
        // an example title copied, whole or as a long part, unless it is the tag itself
        let titles = ["【無料】ふわふわロングスカート for Manuka", "スカート"];
        let copied = skirt("skirt", "ふわふわロングスカート for Manuka", "스커트", "【無料】ふわふわロングスカート for Manuka");
        assert_eq!(kept(&copied, "スカート", &titles, &[]), [Some("skirt".into()), None, Some("스커트".into()), None]);
        assert_eq!(kept(&skirt("skirt", "スカート", "스커트", "裙子"), "スカート", &titles, &[])[1], Some("スカート".into()));
        // no object with the four languages under the tag's id: a positional array, or the input structure echoed back
        assert_eq!(kept(&json!(["skirt", "スカート", "스커트", "裙子"]), "スカート", &[], &[]), [None, None, None, None]);
        let echoed: Value = serde_json::from_str(r#"{"id": "t1", "tag": "スカート", "id": "t1", "context": ["スカート"]}"#).unwrap();
        assert_eq!(kept(&echoed, "スカート", &[], &[]), [None, None, None, None]);
        assert_eq!(kept(&Value::Null, "スカート", &[], &[]), [None, None, None, None]);
    }

    #[test]
    fn each_translated_title_keeps_the_names_adds_no_number_and_uses_its_script() {
        let japanese = AvatarName::japanese_names(&[AvatarName { ja: Some("マヌカ".into()), romaji: "Manuka".into() }]);
        let manuka = HeldName { forms: vec!["マヌカ".into(), "manuka".into()], latin: vec!["manuka".into()] };
        let check = TitleCheck { original: "【マヌカ対応】フリルドレス 2着セット", names: vec![manuka], japanese: &japanese };
        let problem = |lang: &str, title: &str| check.problem(lang, title, Some("ja"));
        for (lang, title) in [
            ("en", "[For Manuka] Frill Dress, Set of 2"),
            ("ko", "【マヌカ 대응】프릴 드레스 2벌 세트"),
            ("zh", "【マヌカ対応】荷叶边连衣裙 两件套"),
            ("ja", "【マヌカ対応】フリルドレス 2着セット"),
        ] {
            assert_eq!(problem(lang, title), None, "{lang} {title}");
        }
        // a NUL, which the database refuses
        assert_eq!(problem("en", "[For Manuka] Frill\0 Dress, Set of 2"), Some("holds a NUL character".into()));
        // English keeps the romaji; the others keep the name in any of its forms
        assert_eq!(problem("en", "[For マヌカ] Frill Dress 2"), Some("drops the avatar name manuka".into()));
        assert_eq!(problem("zh", "【适配麦奴卡】荷叶边连衣裙 2"), Some("drops the avatar name マヌカ".into()));
        assert_eq!(problem("zh", "【Manuka】荷叶边连衣裙 2"), None);
        // a name with no form in Latin letters gives an English title nothing to keep
        let tien = TitleCheck { original: "【ティエン対応】ドレス", names: vec![HeldName { forms: vec!["ティエン".into()], latin: vec![] }], japanese: &japanese };
        assert_eq!(tien.problem("en", "[For Tien] Dress", Some("ja")), None);
        assert_eq!(tien.problem("zh", "【适配蒂恩】连衣裙", Some("ja")), Some("drops the avatar name ティエン".into()));
        // a number the original lacks, in any digits; a Chinese numeral the original writes counts as its number
        for title in ["[For Manuka] Frill Dress, Set of 3", "[For Manuka] Frill Dress ３", "[For Manuka] Frill Dress 𝟑", "[For Manuka] 1,000 Frill Dresses"] {
            assert!(problem("en", title).is_some_and(|p| p.starts_with("has the number")), "{title}");
        }
        let three = TitleCheck { original: "三点セット 1,000円", names: vec![], japanese: &japanese };
        assert_eq!(three.problem("en", "3-piece set, 1000 yen", Some("ja")), None);
        assert_eq!(three.problem("en", "3-piece set, 01000 yen", Some("ja")), None);
        // a Chinese numeral in the title is a word of it, not a number it adds
        assert_eq!(TitleCheck { original: "ワンクリック設定", names: vec![], japanese: &japanese }.problem("zh", "一键设置", Some("ja")), None);
        // a script foreign to the language; an avatar's Japanese name is not, but in English
        assert_eq!(problem("zh", "【マヌカ対応】フリル连衣裙 2"), Some("holds kana, which a zh title does not use".into()));
        assert_eq!(problem("ko", "【マヌカ 대응】프릴 ドレス 2"), Some("holds kana, which a ko title does not use".into()));
        assert_eq!(problem("ko", "【マヌカ対応】프릴 드레스 2"), Some("holds Han characters, which a ko title does not use".into()));
        assert_eq!(problem("ja", "【マヌカ対応】フリルドレス 2벌"), Some("holds Hangul, which a ja title does not use".into()));
        assert_eq!(problem("en", "[For Manuka] マヌカ Frill Dress 2"), Some("holds kana, which a en title does not use".into()));
        // the listing's own language copies the original as it is; another language may not
        let chinese = TitleCheck { original: "猫の连衣裙", names: vec![], japanese: &japanese };
        assert_eq!(chinese.problem("zh", "猫の连衣裙", Some("zh")), None);
        assert!(chinese.problem("zh", "猫の连衣裙", Some("ja")).is_some());
        assert!(chinese.problem("ko", "猫の连衣裙", Some("zh")).is_some());
    }

    #[test]
    fn compound_numerals_allow_only_the_whole_number() {
        let allowed = numbers_of_original("十二点セット・二十色・二十四枚・三種類");
        assert_eq!(allowed, ["12", "20", "24", "3"].map(str::to_owned).into());
        assert!(numbers_of_original("一百十二色").is_empty());
    }

    #[test]
    fn a_latin_avatar_name_is_not_part_of_another_word() {
        let names = vec![HeldName { forms: vec!["lime".into(), "ライム".into()], latin: vec!["lime".into()] }];
        let check = TitleCheck { original: "ライム用", names, japanese: &[] };
        for title in ["Sublime dress", "Limelight dress", "Lime2"] {
            assert!(check.problem("en", title, None).is_some(), "{title}");
        }
        for (lang, title) in [("en", "Sublime dress (ＬＩＭＥ)"), ("zh", "适配Lime用"), ("ja", "新ライム用")] {
            assert_eq!(check.problem(lang, title, None), None, "{title}");
        }
        let reply = json!({"en": "Sublime", "ja": "ライム用", "zh": "适配Sublime"});
        let values = kept(&reply, "ライム用", &[], &[&["lime", "ライム"]]);
        assert_eq!(values, [None, Some("ライム用".into()), None, None]);
    }

    #[test]
    fn a_failing_title_gives_way_to_an_earlier_one_that_passes_or_to_none() {
        let check = TitleCheck { original: "フリルドレス", names: vec![], japanese: &[] };
        let e = |translations: Value| Enrichment { categories: vec![], features: vec![], content_rating: None, quest: None, smart_price: None, translations };
        let reply = json!({"en": {"title": "Frill Dress 2"}, "ja": {"title": "フリルドレス"}, "ko": {"title": "프릴 ドレス"}, "zh": {"title": "荷叶边连衣裙"}, "original_language": "ja"});
        let existing = json!({"en": {"title": "Frill Dress"}, "ko": {"title": "프릴 드레스 3"}});
        assert_eq!(
            title_problems_message(&check.problems(&reply)),
            "translations.en.title has the number 2, which the original title lacks; translations.ko.title holds kana, which a ko title does not use"
        );
        let kept = check.keep_passing(e(reply), &existing).translations;
        assert_eq!(kept, json!({"en": {"title": "Frill Dress"}, "ja": {"title": "フリルドレス"}, "zh": {"title": "荷叶边连衣裙"}, "original_language": "ja"}));
    }

    #[test]
    fn japanese_avatar_names_are_the_non_latin_ones_of_two_characters_or_more_longest_first() {
        let name = |ja: Option<&str>| AvatarName { ja: ja.map(str::to_owned), romaji: "x".into() };
        let avatars = [name(Some("森羅")), name(Some("ルーナリット")), name(Some("Adolescent U")), name(Some("萌")), name(None), name(Some("ﾏﾇｶ")), name(Some("マヌカ"))];
        assert_eq!(AvatarName::japanese_names(&avatars), ["ルーナリット", "マヌカ", "森羅"]);
    }

    #[test]
    fn a_provider_without_json_schema_is_told_apart_from_other_refusals() {
        assert!(refuses_format(400, "bad request"));
        assert!(refuses_format(422, r#"{"error": "unsupported response_format type json_schema"}"#));
        assert!(!refuses_format(401, "invalid api key") && !refuses_format(404, "model not found"));
    }

    #[test]
    fn the_reply_schema_asks_for_every_id_with_the_tag_and_four_short_values() {
        let schema = tag_schema(2);
        assert_eq!(schema["required"], json!(["t1", "t2"]));
        assert_eq!(schema["additionalProperties"], json!(false));
        let tag = &schema["properties"]["t2"];
        assert_eq!((tag["required"].clone(), tag["additionalProperties"].clone()), (json!(["tag", "en", "ja", "ko", "zh"]), json!(false)));
        assert_eq!(tag["properties"]["zh"], json!({"type": "string", "minLength": 1, "maxLength": 60}));
        assert_eq!(tag["properties"]["tag"], json!({"type": "string"}));
    }

    #[test]
    fn an_answer_is_for_the_tag_it_copies_back() {
        assert!(answers_tag(&json!({"tag": " ｽｶｰﾄ ", "en": "skirt"}), "スカート"));
        assert!(answers_tag(&json!({"tag": "SKIRT"}), "skirt"));
        for wrong in [json!({"tag": "ドレス"}), json!({"en": "skirt"}), json!({"tag": 1}), json!(["skirt"]), Value::Null] {
            assert!(!answers_tag(&wrong, "スカート"), "{wrong}");
        }
    }

    #[test]
    fn a_chinese_texture_is_also_found_as_tietu() {
        assert_eq!(tag_keywords("眼睛纹理"), ["眼睛贴图"]);
        assert_eq!(tag_keywords("纹理"), ["贴图"]);
        assert!(tag_keywords("法线贴图").is_empty() && tag_keywords("裙子").is_empty());
    }

    #[test]
    fn tag_prompt_names_the_avatars_and_keys_the_tags_by_id() {
        let tags = vec![("マヌカ用".to_owned(), vec!["a".repeat(130), "b".into(), "c".into(), "d".into()]), ("白モデル".to_owned(), vec![])];
        let manuka = AvatarName { ja: Some("マヌカ".into()), romaji: "Manuka".into() };
        let prompt = tag_prompt(&tags, &[&manuka]);
        let (head, body) = prompt.split_once('\n').unwrap();
        assert_eq!(head, "Avatar names (Japanese / romaji): マヌカ / Manuka");
        let items: Value = serde_json::from_str(body).unwrap();
        assert_eq!(items, json!([{"id": "t1", "tag": "マヌカ用", "context": ["a".repeat(120), "b", "c"]}, {"id": "t2", "tag": "白モデル", "context": []}]));
        assert!(tag_prompt(&tags, &[]).starts_with("[{"));
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
    fn category_with_dropped_leading_word_resolves() {
        let known = vec![
            "avatar-animations-physbones-gestures-and-gimmicks--dance-pose-and-emote-animations".to_owned(),
            "avatar-accessories--other".to_owned(),
            "avatar-props--other".to_owned(),
        ];
        let r = |s| resolve_category(s, &known);
        assert_eq!(r("animations-physbones-gestures-and-gimmicks--dance-pose-and-emote-animations"), Some(known[0].as_str()));
        assert_eq!(r("avatar-props--other"), Some(known[2].as_str()));
        // parent alone, leaf alone, ambiguous, or a cut mid-word: still rejected
        assert_eq!(r("animations-physbones-gestures-and-gimmicks"), None);
        assert_eq!(r("dance-pose-and-emote-animations"), None);
        assert_eq!(r("--other"), None);
        assert_eq!(r("nimations-physbones-gestures-and-gimmicks--dance-pose-and-emote-animations"), None);
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
        assert_eq!(ai.body(json!([]), json!({}))["reasoning_effort"], "none");
        c.reasoning_effort = "high";
        assert_eq!(Ai::from_config(&c).unwrap().unwrap().body(json!([]), json!({}))["reasoning_effort"], "high");
    }
}
