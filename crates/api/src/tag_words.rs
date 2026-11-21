//! The words of the BOOTH tag dictionary (`booth_tag_translations`): each tag in English, Japanese, Korean and
//! Simplified Chinese, and its search-only keywords. Search matches them as words of an item's tags (text.rs), never
//! as substrings: "chair" holds "hair" and "Twin Tails" holds "tail" (evaluation on the development catalogue,
//! 2026-11-21). Filters, facets and the expansion's candidates keep the raw BOOTH tags.

use std::hash::{DefaultHasher, Hash, Hasher};

use rustc_hash::{FxHashMap, FxHashSet};

use crate::text::fold;

/// One row of the dictionary: a BOOTH tag, its value in each language of `expansion::LANGUAGES` (en, ja, ko, zh; none
/// where the translation failed its checks) and its search-only keywords.
pub struct TagRow {
    pub tag: String,
    pub values: [Option<String>; 4],
    pub keywords: Vec<String>,
}

impl TagRow {
    /// The tag with its words, as `TagWords::new` takes them.
    pub fn words(&self) -> (String, Vec<String>) {
        (self.tag.clone(), self.values.iter().flatten().chain(&self.keywords).cloned().collect())
    }
}

/// One dictionary row, as search reads it.
struct Entry {
    /// the tag folded with `fold`
    folded: String,
    /// its words, each once: (as written, folded)
    words: Box<[(String, String)]>,
}

#[derive(Default)]
pub struct TagWords {
    entries: FxHashMap<String, Entry>,
    fingerprint: u64,
}

impl TagWords {
    /// From (tag, its translations and keywords) rows; empty and repeated words are skipped.
    pub fn new(rows: impl IntoIterator<Item = (String, Vec<String>)>) -> Self {
        let mut rows: Vec<(String, Vec<String>)> = rows.into_iter().collect();
        rows.sort_unstable();
        let mut hasher = DefaultHasher::new();
        rows.hash(&mut hasher);
        let entries = rows
            .into_iter()
            .map(|(tag, words)| {
                let folded = fold(&tag);
                let mut kept: Vec<(String, String)> = Vec::new();
                for w in words {
                    let w = w.trim();
                    let f = fold(w);
                    if !f.is_empty() && !kept.iter().any(|(_, k)| *k == f) {
                        kept.push((w.to_owned(), f));
                    }
                }
                (tag, Entry { folded, words: kept.into() })
            })
            .collect();
        Self { entries, fingerprint: hasher.finish() }
    }

    /// Changes with any row, so a text index on disk built with other words is not reused (`Catalog::build_with_tag_words`).
    pub fn fingerprint(&self) -> u64 {
        self.fingerprint
    }

    /// The words of an item's `tags` that none of its tags or earlier words spells already (after `fold`), one per
    /// line; empty when the dictionary has none of them.
    pub fn text(&self, tags: &[String]) -> String {
        let found: Vec<&Entry> = tags.iter().filter_map(|t| self.entries.get(t)).collect();
        if found.iter().all(|e| e.words.is_empty()) {
            return String::new();
        }
        let unknown: Vec<String> = tags.iter().filter(|t| !self.entries.contains_key(*t)).map(|t| fold(t)).collect();
        let mut seen: FxHashSet<&str> = found.iter().map(|e| e.folded.as_str()).chain(unknown.iter().map(String::as_str)).collect();
        let mut out = String::new();
        for (word, folded) in found.iter().flat_map(|e| e.words.iter()) {
            if seen.insert(folded) {
                if !out.is_empty() {
                    out.push('\n');
                }
                out.push_str(word);
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dictionary() -> TagWords {
        let row = |tag: &str, words: &[&str]| (tag.to_owned(), words.iter().map(|w| w.to_string()).collect());
        TagWords::new([
            row("スカート", &["skirt", "スカート", "스커트", "裙子"]),
            row("Skirt", &["Skirt", "すかーと", "스커트", "裙子"]),
            row("テクスチャ", &["texture", "テクスチャ", "텍스처", "纹理", "贴图"]),
            row("空", &["", " ", "sky"]),
        ])
    }

    fn tags(t: &[&str]) -> Vec<String> {
        t.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn an_item_gets_each_word_its_tags_do_not_spell() {
        let d = dictionary();
        assert_eq!(d.text(&tags(&["スカート"])), "skirt\n스커트\n裙子");
        // a word another tag spells, in any case, width or kana, is left out, and so is a word already given
        assert_eq!(d.text(&tags(&["SKIRT", "スカート"])), "스커트\n裙子");
        assert_eq!(d.text(&tags(&["スカート", "Skirt"])), "스커트\n裙子");
        // keywords are words like the translations; blank ones are skipped
        assert_eq!(d.text(&tags(&["テクスチャ", "空"])), "texture\n텍스처\n纹理\n贴图\nsky");
        assert_eq!(d.text(&tags(&["unknown"])), "");
        assert_eq!(d.text(&[]), "");
    }

    #[test]
    fn the_fingerprint_follows_the_rows_not_their_order() {
        let a = TagWords::new([("a".to_owned(), vec!["x".to_owned()]), ("b".to_owned(), vec!["y".to_owned()])]);
        let b = TagWords::new([("b".to_owned(), vec!["y".to_owned()]), ("a".to_owned(), vec!["x".to_owned()])]);
        let c = TagWords::new([("a".to_owned(), vec!["x".to_owned()]), ("b".to_owned(), vec!["z".to_owned()])]);
        assert_eq!(a.fingerprint(), b.fingerprint());
        assert_ne!(a.fingerprint(), c.fingerprint());
    }
}
