//! Cross-language expansion (owner, 2026-11-20). Item titles carry translations, BOOTH tags and descriptions do not,
//! so "skirt" missed the items that are only tagged スカート. A query is also searched as the BOOTH tags that
//! translate it: tags carried by the items whose titles match it, whose own title matches fall mostly inside the
//! query's (containment) and cover a good part of them (coverage). Titles are matched by words only
//! (`TextIndex::title_hits`), so "치마" (skirt) is not read inside "앞치마" (apron).
//!
//! Both sides are measured on the items titled in the query's language: in production only 8.7% of items have a
//! Chinese title (2026-11-20), so a Chinese query's title matches come from those few while a tag's come from all,
//! and no Chinese query would pass containment measured on every item. The matches searched stay global.

use std::ops::RangeInclusive;
use std::sync::Arc;

use anyhow::Result;
use roaring::RoaringBitmap;
use rustc_hash::FxHashMap;

use crate::catalog::Index;
use crate::text::{TextIndex, compact, fold};

// The thresholds were tuned on the development catalogue (216k items, 42 sample queries, 2026-11-20).
/// A longer query describes something rather than naming one thing to translate.
const MAX_WORDS: usize = 3;
/// Fewer title matches say too little about which tags go with the query.
const MIN_TITLE_HITS: u64 = 10;
/// A candidate tag is on at least this many of the query's title matches,
const MIN_SHARED: u32 = 3;
/// and on at least this many listed items.
const MIN_ITEMS: u64 = 20;
/// Candidates checked, best first.
const CANDIDATES: usize = 30;
/// Share of the tag's title matches that the query matches too; below it the tag is broader (衣装 for skirt).
const CONTAINMENT: f64 = 0.7;
/// Share of the query's title matches that the tag matches too; below it the tag is narrower (ミニスカート for skirt).
const COVERAGE: f64 = 0.25;
const MAX_EQUIVALENTS: usize = 3;
/// Queries whose equivalents one snapshot remembers.
const CACHE_CAP: u64 = 2000;

/// The languages of item titles, in the bit order of `DocInput::title_languages`.
pub(crate) const LANGUAGES: [&str; 4] = ["en", "ja", "ko", "zh"];

/// The BOOTH tags of a snapshot that can translate a query or be offered as a related search (related.rs), and the
/// equivalents found so far.
pub(crate) struct Equivalents {
    /// per tag (folded with `fold`, on at least `MIN_ITEMS` listed items): its most common spelling, ties by the
    /// spelling, so a query shows the same words on every page
    names: Vec<String>,
    items: Vec<u64>,
    /// per tag: the shops its listed items come from
    shops: Vec<u32>,
    /// folded tag -> its index in `names`
    keys: FxHashMap<String, u32>,
    /// the tags of doc `d`: `ids[starts[d]..starts[d + 1]]`
    starts: Vec<u32>,
    ids: Vec<u32>,
    /// per language of `LANGUAGES`: the listed items titled in it
    titled: [RoaringBitmap; 4],
    /// keyed by the query as the title parser gets it; `fold` is no key, as it folds kana before Lindera segments
    cache: moka::sync::Cache<String, Arc<[String]>>,
}

impl Equivalents {
    /// From `raw` (spelling -> docs), the shop of each doc (`shop_of`, numbered from 0), and the docs `titled` in each
    /// of `LANGUAGES`; re-uploaded items count for nothing.
    pub(crate) fn build(raw: &Index, relisted: &RoaringBitmap, shop_of: &[u32], titled: [RoaringBitmap; 4]) -> Self {
        let docs = shop_of.len();
        let mut folded: FxHashMap<String, Vec<(&String, &RoaringBitmap)>> = FxHashMap::default();
        for (spelling, b) in raw.entries() {
            folded.entry(fold(spelling)).or_default().push((spelling, b));
        }
        let listed = |b: &RoaringBitmap| b.len() - b.intersection_len(relisted);
        let mut doc_tags: Vec<Vec<u32>> = vec![Vec::new(); docs];
        let (mut names, mut items, mut shops, mut keys) = (Vec::new(), Vec::new(), Vec::new(), FxHashMap::default());
        // per shop, the last tag that counted it
        let mut counted = vec![u32::MAX; shop_of.iter().max().map_or(0, |&s| s as usize + 1)];
        for (key, spellings) in folded {
            let tagged = match spellings.as_slice() {
                [(_, b)] if listed(b) < MIN_ITEMS => continue,
                [(_, b)] => *b - relisted,
                _ => spellings.iter().fold(RoaringBitmap::new(), |acc, (_, b)| acc | *b) - relisted,
            };
            if tagged.len() < MIN_ITEMS {
                continue;
            }
            let id = names.len() as u32;
            let (_, name) = spellings.iter().map(|(s, b)| (listed(b), *s)).max_by(|a, b| a.0.cmp(&b.0).then(b.1.cmp(a.1))).expect("one spelling");
            names.push(name.clone());
            items.push(tagged.len());
            keys.insert(key, id);
            let mut n = 0;
            for d in &tagged {
                doc_tags[d as usize].push(id);
                let shop = &mut counted[shop_of[d as usize] as usize];
                if *shop != id {
                    *shop = id;
                    n += 1;
                }
            }
            shops.push(n);
        }
        let mut starts = Vec::with_capacity(docs + 1);
        starts.push(0);
        let mut ids = Vec::with_capacity(doc_tags.iter().map(Vec::len).sum());
        for t in doc_tags {
            ids.extend(t);
            starts.push(ids.len() as u32);
        }
        Self { names, items, shops, keys, starts, ids, titled: titled.map(|b| b - relisted), cache: moka::sync::Cache::new(CACHE_CAP) }
    }

    /// The tags of doc `d`, as indexes.
    pub(crate) fn tags_of(&self, d: u32) -> &[u32] {
        &self.ids[self.starts[d as usize] as usize..self.starts[d as usize + 1] as usize]
    }

    pub(crate) fn len(&self) -> usize {
        self.names.len()
    }

    /// The index of a tag folded with `fold`.
    pub(crate) fn id(&self, key: &str) -> Option<u32> {
        self.keys.get(key).copied()
    }

    /// Each folded tag with its index.
    pub(crate) fn keys(&self) -> impl Iterator<Item = (&str, u32)> {
        self.keys.iter().map(|(k, &t)| (k.as_str(), t))
    }

    /// The tag's most common spelling.
    pub(crate) fn name(&self, t: u32) -> &str {
        &self.names[t as usize]
    }

    /// The listed items that carry it.
    pub(crate) fn items(&self, t: u32) -> u64 {
        self.items[t as usize]
    }

    /// The shops of those items.
    pub(crate) fn shops(&self, t: u32) -> u32 {
        self.shops[t as usize]
    }

    /// The BOOTH tags that translate `meaning` (the query without model names), at most `MAX_EQUIVALENTS`, best
    /// first; remembered for this snapshot.
    pub(crate) fn of(&self, meaning: &str, text: &TextIndex) -> Result<Arc<[String]>> {
        let key = meaning.trim();
        if let Some(found) = self.cache.get(key) {
            return Ok(found);
        }
        let found: Arc<[String]> = self.find(key, text)?.into();
        self.cache.insert(key.to_owned(), found.clone());
        Ok(found)
    }

    fn find(&self, q: &str, text: &TextIndex) -> Result<Vec<String>> {
        if q.split_whitespace().count() > MAX_WORDS {
            return Ok(Vec::new());
        }
        let titled = &self.titled[language(q)];
        let tq = text.title_hits(q)? & titled;
        if tq.len() < MIN_TITLE_HITS {
            return Ok(Vec::new());
        }
        let mut shared: FxHashMap<u32, u32> = FxHashMap::default();
        for d in &tq {
            for &t in self.tags_of(d) {
                *shared.entry(t).or_default() += 1;
            }
        }
        // how strongly the tag goes with the query's title matches (Ochiai, against the tag's size in all items); ties
        // by that size, then the spelling, so the same query picks the same tags whatever the hash order
        let mut candidates: Vec<(f64, u64, u32)> = shared
            .into_iter()
            .filter(|&(_, n)| n >= MIN_SHARED)
            .map(|(t, n)| {
                let items = self.items[t as usize];
                (f64::from(n) / (tq.len() as f64 * items as f64).sqrt(), items, t)
            })
            .collect();
        candidates.sort_by(|a, b| b.0.total_cmp(&a.0).then(b.1.cmp(&a.1)).then_with(|| self.names[a.2 as usize].cmp(&self.names[b.2 as usize])));
        let mut out = Vec::new();
        for (_, _, t) in candidates.into_iter().take(CANDIDATES) {
            let name = &self.names[t as usize];
            if same_word(q, name) {
                continue;
            }
            let tt = text.title_hits(name)? & titled;
            let both = tq.intersection_len(&tt) as f64;
            if !tt.is_empty() && both / tt.len() as f64 >= CONTAINMENT && both / tq.len() as f64 >= COVERAGE {
                out.push(name.clone());
                if out.len() == MAX_EQUIVALENTS {
                    break;
                }
            }
        }
        Ok(out)
    }
}

/// The language of a query, as an index into `LANGUAGES`, by its script: any Hangul is Korean, else any kana
/// Japanese, else any Han character Chinese, else English.
fn language(q: &str) -> usize {
    let has = |ranges: &[RangeInclusive<char>]| q.chars().any(|c| ranges.iter().any(|r| r.contains(&c)));
    let hangul = ['\u{1100}'..='\u{11FF}', '\u{3130}'..='\u{318F}', '\u{A960}'..='\u{A97F}', '\u{AC00}'..='\u{D7AF}', '\u{FFA0}'..='\u{FFDC}'];
    let kana = ['\u{3040}'..='\u{30FF}', '\u{31F0}'..='\u{31FF}', '\u{FF66}'..='\u{FF9F}'];
    let han = ['\u{3400}'..='\u{4DBF}', '\u{4E00}'..='\u{9FFF}', '\u{F900}'..='\u{FAFF}', '\u{20000}'..='\u{2FA1F}'];
    let name = if has(&hangul) {
        "ko"
    } else if has(&kana) {
        "ja"
    } else if has(&han) {
        "zh"
    } else {
        "en"
    };
    LANGUAGES.iter().position(|l| *l == name).expect("one of LANGUAGES")
}

/// Spellings of one word in one language: one holds the other once case, width, spaces and symbols are gone
/// ("swim" / "swimsuit", "shorthair" / "short hair"). They add no translation.
fn same_word(a: &str, b: &str) -> bool {
    let (a, b) = (compact(a), compact(b));
    a.contains(&b) || b.contains(&a)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::catalog::{Catalog, DocInput};

    fn doc(id: i64, title: String, translated: &[String], tags: &[&str]) -> DocInput {
        DocInput {
            id,
            title,
            titles_translated: translated.to_vec(),
            // en, ja and ko, as nearly every production item has; zh only where a test sets it
            title_languages: 0b0111,
            bio: String::new(),
            author: "a".into(),
            booth_category: Some("3D Clothing".into()),
            price: Some(1000),
            likes: id as i32,
            published: Some(0),
            is_adult: false,
            rating: Some("SFW".into()),
            features: vec![],
            avatars: vec![],
            categories: vec![],
            booth_tags: tags.iter().map(|t| t.to_string()).collect(),
            smart_tags: vec![],
            supported: false,
            gender: None,
            relisted: false,
        }
    }

    /// A catalogue where each group of ids shows one rule; the ids are referred to by the search tests too.
    pub(crate) fn catalogue() -> Catalog {
        let mut d = Vec::new();
        // 1-12: skirts with translated titles; 8-12 also say 衣装; 1 and 2 fit Shinano, 1 is free
        for i in 1..=12 {
            let title = if i <= 7 { format!("スカート {i}") } else { format!("スカート 衣装 {i}") };
            d.push(doc(i, title, &[format!("Skirt {i}"), format!("스커트 {i}")], &["スカート", "Skirt", "衣装", "스커트"]));
        }
        d[0].avatars = vec!["shinano".into()];
        d[1].avatars = vec!["shinano".into()];
        d[0].price = Some(0);
        // 13-15: mini skirts; 34-50 carry their tag without the word
        for i in 13..=15 {
            d.push(doc(i, format!("ミニスカート {i}"), &[format!("Mini Skirt {i}")], &["ミニスカート"]));
        }
        // 16-25: only the tag スカート says what they are; 16 and 17 fit Shinano
        for i in 16..=25 {
            d.push(doc(i, format!("Item {i}"), &[], &["スカート"]));
        }
        d[15].avatars = vec!["shinano".into()];
        d[16].avatars = vec!["shinano".into()];
        // 26-33: the tag Skirt without the word in the title
        for i in 26..=33 {
            d.push(doc(i, format!("Set {i}"), &[], &["Skirt"]));
        }
        for i in 34..=50 {
            d.push(doc(i, format!("Doc {i}"), &[], &["ミニスカート"]));
        }
        // 51-70: costumes
        for i in 51..=70 {
            d.push(doc(i, format!("衣装 {i}"), &[], &["衣装"]));
        }
        // 71-82: tails, four tags; 83-90 carry the tags without the words. Half spell しっぽ, half シッポ.
        for i in 71..=90 {
            let tail = if i <= 76 || (83..=86).contains(&i) { "しっぽ" } else { "シッポ" };
            let tags = [tail, "尻尾", "テール", "꼬리"];
            d.push(if i <= 82 { doc(i, format!("尻尾 しっぽ テール 꼬리 {i}"), &[format!("Tail {i}")], &tags) } else { doc(i, format!("Toy {i}"), &[], &tags) });
        }
        // 91-110: aprons titled 앞치마, which holds 치마 (skirt) only as a substring
        for i in 91..=110 {
            d.push(doc(i, format!("エプロン {i}"), &[format!("앞치마 {i}")], &["エプロン"]));
        }
        // 111-120: hats, 120 re-uploaded, so nine listed titles say hat; 121-131 tagged too
        for i in 111..=120 {
            d.push(doc(i, format!("帽子 {i}"), &[format!("Hat {i}")], &["帽子"]));
        }
        d.last_mut().unwrap().relisted = true;
        for i in 121..=131 {
            d.push(doc(i, format!("Cap {i}"), &[], &["帽子"]));
        }
        // 132-143: hoodies titled パーカー, only 132 and 133 tagged so; 144-161 tagged without the word
        for i in 132..=143 {
            d.push(doc(i, format!("パーカー {i}"), &[format!("Hoodie {i}")], if i <= 133 { &["パーカー"] } else { &[] }));
        }
        for i in 144..=161 {
            d.push(doc(i, format!("Top {i}"), &[], &["パーカー"]));
        }
        // 162-203: scarves titled マフラー; only 162-173 have a Chinese title, 围巾
        for i in 162..=203 {
            let mut scarf = doc(i, format!("マフラー {i}"), &[format!("Scarf {i}")], &["マフラー"]);
            if i <= 173 {
                scarf.titles_translated.push(format!("围巾 {i}"));
                scarf.title_languages |= 0b1000;
            }
            d.push(scarf);
        }
        // 204-243: titles in the same characters, not counted as Chinese (written by a Japanese shop, say)
        for i in 204..=243 {
            d.push(doc(i, format!("围巾 {i}"), &[], &[]));
        }
        Catalog::build(d, 0, None).unwrap()
    }

    fn of(c: &Catalog, q: &str) -> Vec<String> {
        c.equivalents(q).unwrap().to_vec()
    }

    #[test]
    fn an_equivalent_shares_most_title_matches_both_ways() {
        let c = catalogue();
        // スカート is taken; Skirt is the query's own word; 衣装 has many titles without skirt (containment);
        // ミニスカート covers too few skirts (coverage); 스커트 is on too few items
        assert_eq!(of(&c, "skirt"), ["スカート"]);
    }

    #[test]
    fn ties_and_spellings_are_settled_by_the_spelling() {
        let c = catalogue();
        // four tags tie on every count: the first three by spelling; しっぽ and シッポ fold together, and the two
        // spellings are equally common
        assert_eq!(of(&c, "tail"), ["しっぽ", "テール", "尻尾"]);
    }

    #[test]
    fn thin_evidence_and_long_queries_find_none() {
        let c = catalogue();
        // nine listed titles say hat: the re-uploaded tenth does not count
        assert!(of(&c, "hat").is_empty());
        // every hoodie title says パーカー, but only two hoodies carry the tag
        assert!(of(&c, "hoodie").is_empty());
        assert!(of(&c, "skirt skirt skirt skirt").is_empty());
    }

    #[test]
    fn equivalents_are_remembered_for_the_snapshot() {
        let c = catalogue();
        assert!(Arc::ptr_eq(&c.equivalents("skirt").unwrap(), &c.equivalents(" skirt ").unwrap()));
    }

    #[test]
    fn a_query_is_measured_on_the_items_titled_in_its_language() {
        let c = catalogue();
        // among the items with a Chinese title every マフラー title says 围巾 and every 围巾 title is a scarf; among
        // all items neither holds
        assert_eq!(of(&c, "围巾"), ["マフラー"]);
        // the script decides; a query of kanji alone reads as Chinese
        let languages = ["skirt", "스커트", "スカート", "黒髪 ロング", "围巾", "制服"].map(|q| LANGUAGES[language(q)]);
        assert_eq!(languages, ["en", "ko", "ja", "ja", "zh", "zh"]);
    }

    /// Fixed samples of what a query is also searched as, so a change to it shows up here (like
    /// `typo_reading_samples` in snapshot.rs).
    #[test]
    fn cross_language_samples() {
        let c = catalogue();
        assert_eq!(of(&c, "스커트"), ["Skirt", "スカート"]);
        // the English tag translates the Japanese query the same way
        assert_eq!(of(&c, "スカート"), ["Skirt"]);
        assert_eq!(of(&c, "앞치마"), ["エプロン"]);
        // titles match by words: 앞치마 (apron) does not hold the word 치마 (skirt)
        assert!(of(&c, "치마").is_empty());
    }
}
