//! In-memory search index: one roaring bitmap per filter value plus numeric columns for
//! ranges and sorting. Doc index (u32) = position in `ids`. Item payloads stay in Postgres.

mod build;
mod fusion;
mod search;
#[cfg(test)]
mod expansion_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod text_tests;

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use anyhow::Result;
use roaring::RoaringBitmap;
use rustc_hash::FxHashMap;
use serde::Serialize;

use crate::expansion::Equivalents;
use crate::search_request::{ItemReference, SearchRequest, item_reference};
use crate::text::{TextIndex, TextMatches};

pub const FEATURES: &[&str] = &["quest", "vrcfury", "ma", "liltoon", "poiyomi"];
const AVATAR_CATEGORY: &str = "3D Characters";
/// UI price slider tops out at this value meaning "and above"
const PRICE_CAP: i32 = 15_000;

pub struct DocInput {
    pub id: i64,
    pub title: String,
    /// its title in English, Japanese and Korean, where that differs from `title`
    pub titles_translated: Vec<String>,
    /// its title in Chinese, searched as Chinese (text.rs): `title` itself when the listing is Chinese, else the
    /// Chinese translation
    pub title_zh: Option<String>,
    /// the listing is Chinese: `title` is searched as Chinese only, in `title_zh`
    pub title_is_zh: bool,
    /// the languages it has a title in, translated or original: bit `1 << k` for `expansion::LANGUAGES[k]`
    pub title_languages: u8,
    pub bio: String,
    pub author: String,
    pub booth_category: Option<String>,
    pub price: Option<i32>,
    pub likes: i32,
    pub published: Option<i64>,
    pub is_adult: bool,
    pub rating: Option<String>,
    pub features: Vec<String>, // includes "quest"
    pub avatars: Vec<String>,
    /// smart category slugs, parents included; bool = primary
    pub categories: Vec<(String, bool)>,
    pub booth_tags: Vec<String>,
    pub smart_tags: Vec<String>,
    pub supported: bool,
    pub gender: Option<String>,
    /// delisted and re-uploaded by the shop: left out of search, still reachable by id and in favourites
    pub relisted: bool,
}

#[derive(Default)]
pub struct Index(HashMap<String, RoaringBitmap>);

impl Index {
    pub(super) fn add(&mut self, key: &str, doc: u32) {
        self.0.entry(key.to_owned()).or_default().insert(doc);
    }
    /// OR over the given keys; unknown keys match nothing
    pub(super) fn any(&self, keys: &[String]) -> RoaringBitmap {
        keys.iter().filter_map(|k| self.0.get(k)).fold(RoaringBitmap::new(), |acc, b| acc | b)
    }
    pub fn len(&self, key: &str) -> u64 {
        self.0.get(key).map_or(0, |b| b.len())
    }
    pub(crate) fn get(&self, key: &str) -> Option<&RoaringBitmap> {
        self.0.get(key)
    }
    pub fn keys(&self) -> impl Iterator<Item = &String> {
        self.0.keys()
    }
    pub(crate) fn entries(&self) -> impl Iterator<Item = (&String, &RoaringBitmap)> {
        self.0.iter()
    }
}

pub struct Catalog {
    pub ids: Vec<i64>,
    pub(super) doc_of: FxHashMap<i64, u32>,
    pub(super) price: Vec<Option<i32>>,
    pub(super) published: Vec<i64>,
    pub(super) likes: Vec<i32>,
    pub(super) trending: Vec<f32>,
    pub(super) title_rank: Vec<u32>,
    pub(super) all: RoaringBitmap,
    pub(super) avatar_items: RoaringBitmap,
    pub(super) supported: RoaringBitmap,
    pub(super) adult_only: RoaringBitmap,
    pub(super) suggestive: RoaringBitmap,
    pub(super) relisted: RoaringBitmap,
    pub avatars: Index,
    pub authors: Index,
    pub smart_tags: Index,
    pub booth_tags: Index,
    pub(super) booth_cats: Index,
    pub(super) cats: Index,
    pub(super) cats_primary: Index,
    pub(super) features: Index,
    pub(super) gender: Index,
    pub(super) text: TextIndex,
    pub(super) equivalents: Equivalents,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum Group {
    View,
    Adult,
    Text,
    Avatars,
    Authors,
    Tags,
    BoothTags,
    SmartCat,
    BoothCat,
    Features,
    Price,
    Range,
    Gender,
}

#[derive(Serialize, Default, Debug, PartialEq)]
pub struct Facets {
    #[serde(rename = "boothCounts")]
    pub booth: BTreeMap<String, u64>,
    #[serde(rename = "smartCounts")]
    pub smart: BTreeMap<String, u64>,
    #[serde(rename = "featureCounts")]
    pub features: BTreeMap<String, u64>,
}

#[derive(Debug, PartialEq)]
pub struct SearchResult {
    pub count: u64,
    pub page: usize,
    pub total_pages: usize,
    pub page_size: usize,
    pub sort: &'static str,
    pub dir: &'static str,
    pub ids: Vec<i64>,
    pub facets: Facets,
    /// the text query matched nothing as typed and was answered by the typo-tolerant retry (see text.rs)
    pub typo: bool,
    /// BOOTH tags that translate the query (expansion.rs), when items only they found are among the results
    pub expanded_with: Vec<String>,
}

/// What a text query matches (`Catalog::text_hits`).
pub(super) struct TextHits {
    pub(super) matches: TextMatches,
    /// the BOOTH tags the query was also searched as, and the docs only they found
    pub(super) expanded: Arc<[String]>,
    pub(super) expansion: RoaringBitmap,
    /// the item the query names by its link or number (`Catalog::referenced`)
    pub(super) pinned: Option<u32>,
}

#[derive(Clone, Copy)]
pub struct Semantic<'a> {
    /// nearest first
    pub ids: &'a [i64],
    /// smart categories the query names as a whole (see snapshot.rs `category_phrases`)
    pub categories: &'a [String],
    /// model slugs named in the query
    pub models: &'a [String],
    /// the query without those names
    pub rest: &'a str,
}

impl Catalog {
    /// Whether some title, tag, author name or description has `word` as a word.
    pub fn knows_word(&self, word: &str) -> bool {
        self.text.knows(word)
    }

    /// The BOOTH tags that translate `meaning`, the query without model names (expansion.rs).
    pub(crate) fn equivalents(&self, meaning: &str) -> Result<Arc<[String]>> {
        self.equivalents.of(meaning, &self.text)
    }

    /// The folded BOOTH tags (expansion.rs), which the related searches (related.rs) read too.
    pub(crate) fn tags(&self) -> &Equivalents {
        &self.equivalents
    }

    pub(crate) fn doc(&self, id: i64) -> Option<u32> {
        self.doc_of.get(&id).copied()
    }

    /// How many items search can show: all but the re-uploaded ones.
    pub(crate) fn listed(&self) -> u64 {
        self.all.len() - self.relisted.len()
    }

    /// The listed docs `q` matches by its words or as a substring (`TextIndex::search`).
    pub(crate) fn word_hits(&self, q: &str) -> Result<RoaringBitmap> {
        Ok(self.text.search(q)?.hits - &self.relisted)
    }

    /// The listed docs whose title has every word of `q` (`TextIndex::title_hits`).
    pub(crate) fn title_hits(&self, q: &str) -> Result<RoaringBitmap> {
        Ok(self.text.title_hits(q)? - &self.relisted)
    }

    /// The docs `r`'s filters, adult setting and view let through, whatever its text.
    pub(crate) fn allowed(&self, r: &SearchRequest, now: i64, favourites: Option<&RoaringBitmap>) -> RoaringBitmap {
        self.intersect(&self.constraints(r, now, None, favourites), None)
    }

    /// The docs that fit one of `models`.
    pub(crate) fn fitting(&self, models: &[String]) -> RoaringBitmap {
        self.avatars.any(models)
    }

    /// Whether `r` finds anything by its words, without meaning-based matches or the categories its words name: its
    /// text matches, those of `rest` among the items fitting the `models` its query names, and with `expand` the
    /// matches of the tags translating it, under its filters, adult setting and view.
    pub(crate) fn finds_by_words(
        &self,
        r: &SearchRequest,
        now: i64,
        favourites: Option<&RoaringBitmap>,
        models: &[String],
        rest: &str,
        expand: Option<&str>,
    ) -> Result<bool> {
        let semantic = (!models.is_empty()).then_some(Semantic { ids: &[], categories: &[], models, rest });
        let text = self.text_hits(r, semantic, expand)?;
        Ok(!self.intersect(&self.constraints(r, now, text.as_ref(), favourites), None).is_empty())
    }

    /// The doc of the item `q` names by itself (`item_reference`): `Some(None)` for a link to an item the catalogue
    /// does not have, `None` when `q` is text (a number that is no item's id included).
    pub(crate) fn referenced(&self, q: &str) -> Option<Option<u32>> {
        let doc = |id| self.doc_of.get(&id).copied();
        match item_reference(q)? {
            ItemReference::Link(id) => Some(doc(id)),
            ItemReference::Number(id) => doc(id).map(Some),
        }
    }

    pub fn len(&self) -> usize {
        self.ids.len()
    }

    /// Approximate bitmap memory, for startup logging.
    pub fn bitmap_bytes(&self) -> usize {
        [&self.avatars, &self.authors, &self.smart_tags, &self.booth_tags, &self.booth_cats, &self.cats, &self.cats_primary, &self.features, &self.gender]
            .iter()
            .flat_map(|ix| ix.0.values())
            .map(|b| b.serialized_size())
            .sum()
    }

    pub(super) fn scan(&self, pred: impl Fn(usize) -> bool) -> RoaringBitmap {
        RoaringBitmap::from_sorted_iter((0..self.len()).filter(|&i| pred(i)).map(|i| i as u32)).expect("ascending")
    }

    /// Doc set for item ids (e.g. a user's favourites); unknown ids are skipped.
    pub fn docs_for(&self, ids: &[i64]) -> RoaringBitmap {
        ids.iter().filter_map(|id| self.doc_of.get(id)).copied().collect()
    }
}
