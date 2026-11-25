//! Script, avatar-name and number checks shared by BOOTH tag translations (`tag_values`) and enriched titles (`TitleCheck`).

use std::collections::BTreeSet;

use serde_json::{Value, json};

use super::Enrichment;

/// The languages of a tag's dictionary row, in the column order of `booth_tag_translations`.
pub const TAG_LANGUAGES: [&str; 4] = ["en", "ja", "ko", "zh"];

/// A script by its name, and whether a character is written in it.
type Script = (&'static str, fn(char) -> bool);

const SCRIPTS: [Script; 3] = [("Han characters", is_han), ("kana", is_kana), ("Hangul", is_hangul)];

/// Which indices into `SCRIPTS` are foreign to `lang`.
fn foreign_scripts(lang: &str) -> &'static [usize] {
    match lang {
        "en" => &[0, 1, 2],
        "ja" => &[2],
        "ko" => &[0, 1],
        _ => &[1, 2],
    }
}

/// Whether `answer` (the reply's entry under a tag's id) copies `tag` back, ignoring case, width and surrounding space:
/// a model that loses its place in a batch files a tag's translation under another tag's id (Rira -> 花鸟风月巫女礼服 in
/// the local backfill; owner ruling 29, 2026-11-21).
pub fn answers_tag(answer: &Value, tag: &str) -> bool {
    answer["tag"].as_str().is_some_and(|echo| crate::avatars::norm(echo.trim()) == crate::avatars::norm(tag.trim()))
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
        !forms.is_empty() && !forms.iter().any(|f| folded.contains(f.as_str()))
    }
}

/// One tag's values from its answer (`answer` is the reply's entry for the tag's id), in `TAG_LANGUAGES` order: each
/// trimmed, or why it is left out. A value is left out unless it is at most 60 non-control characters, has no script
/// foreign to its language (Han, kana or Hangul in English; Hangul in Japanese; Han or kana in Korean; kana or Hangul in
/// Chinese; non-English values allow a script the tag itself is written in), is not one of the tag's example `titles`
/// (nor, over 20 characters, a part of one) unless it is the tag itself, and keeps every avatar name the tag holds
/// (English in Latin letters, the romaji). `japanese`: every avatar's Japanese name (`AvatarName::japanese_names`),
/// which a Japanese, Korean or Chinese value may hold in any script.
pub fn tag_values(answer: &Value, tag: &str, titles: &[String], names: &[HeldName], japanese: &[String]) -> [std::result::Result<String, String>; 4] {
    std::array::from_fn(|k| {
        let lang = TAG_LANGUAGES[k];
        let english = lang == "en";
        let v = answer[lang].as_str().map(str::trim).filter(|v| !v.is_empty()).ok_or("no value")?;
        // a control character (a line break, or a NUL, which PostgreSQL text refuses) is no part of a keyword
        if v.chars().count() > 60 || v.chars().any(char::is_control) {
            return Err("not one line of at most 60 characters without control characters".into());
        }
        // English never keeps Han, kana or Hangul from the source tag: avatar names stay in romaji (owner ruling 13)
        let unnamed = if english { v.to_owned() } else { without_names(v, japanese) };
        if foreign_scripts(lang).iter().any(|&s| unnamed.chars().any(SCRIPTS[s].1) && (english || !tag.chars().any(SCRIPTS[s].1))) {
            return Err("in a script foreign to its language".into());
        }
        if v != tag.trim() && titles.iter().any(|t| t.trim() == v || (v.chars().count() > 20 && t.contains(v))) {
            return Err("copies an example title".into());
        }
        let folded = crate::avatars::norm(v);
        if let Some(lost) = names.iter().find(|n| n.dropped_from(&folded, english)) {
            return Err(format!("drops the avatar name {}", lost.kept_in(english)[0]));
        }
        Ok(v.to_owned())
    })
}

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
    /// keeps the romaji), has a number the original and its avatar names lack, or holds a script foreign to its
    /// language (as `tag_values` reads scripts; an avatar's Japanese name may stay in any but English). The title in
    /// the listing's own language, `own`, is not checked when it copies the original, as the prompt asks.
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
        let mut allowed = numbers_of_original(self.original);
        // an avatar's romaji or alternate name may contain digits the original spelled in kana (e.g. さん↑ぷらす→うの↑ / 3+1)
        allowed.extend(self.names.iter().flat_map(|n| &n.forms).flat_map(|f| numbers(f)));
        if let Some(n) = numbers(title).into_iter().find(|n| !allowed.contains(n)) {
            return Some(format!("has the number {n}, which the original title lacks"));
        }
        let unnamed = if english { folded } else { without_names(title, self.japanese) };
        foreign_scripts(lang)
            .iter()
            .map(|&s| SCRIPTS[s])
            .find(|(_, is)| unnamed.chars().any(is))
            .map(|(script, _)| format!("holds {script}, which a {lang} title does not use"))
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
fn numbers(text: &str) -> BTreeSet<String> {
    use unicode_normalization::UnicodeNormalization;
    let chars: Vec<char> = text.nfkc().collect();
    let mut out = BTreeSet::new();
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

/// The numbers a title may write in digits: those of `original` in digits, and those it writes as a Chinese numeral
/// from 〇 to 十 (三点セット is a 3-piece set).
fn numbers_of_original(original: &str) -> BTreeSet<String> {
    const NUMERALS: [(char, &str); 14] =
        [('〇', "0"), ('零', "0"), ('一', "1"), ('二', "2"), ('两', "2"), ('兩', "2"), ('三', "3"), ('四', "4"), ('五', "5"), ('六', "6"), ('七', "7"), ('八', "8"), ('九', "9"), ('十', "10")];
    let mut out = numbers(original);
    out.extend(NUMERALS.iter().filter(|(c, _)| original.contains(*c)).map(|(_, n)| n.to_string()));
    out
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

/// Hiragana and Katakana letters/marks, excluding punctuation inside the Katakana block (`U+30A0` ゠ and `U+30FB` ・,
/// which Chinese and Korean titles also use as middle dots).
fn is_kana(c: char) -> bool {
    matches!(c, '\u{3040}'..='\u{309F}' | '\u{30A1}'..='\u{30FA}' | '\u{30FC}'..='\u{30FF}' | '\u{31F0}'..='\u{31FF}' | '\u{FF66}'..='\u{FF9F}')
}

fn is_hangul(c: char) -> bool {
    matches!(c, '\u{1100}'..='\u{11FF}' | '\u{3130}'..='\u{318F}' | '\u{A960}'..='\u{A97F}' | '\u{AC00}'..='\u{D7AF}' | '\u{FFA0}'..='\u{FFDC}')
}

/// Whether a name is written in Latin letters, accents included (the romaji Alué, Kyalóng): no Han, kana or Hangul.
pub fn latin_letters(name: &str) -> bool {
    !name.chars().any(|c| is_han(c) || is_kana(c) || is_hangul(c))
}
