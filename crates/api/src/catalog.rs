//! In-memory search index: one roaring bitmap per filter value plus numeric columns for
//! ranges and sorting. Doc index (u32) = position in `ids`. Item payloads stay in Postgres.

use std::collections::{BTreeMap, HashMap};

use anyhow::Result;
use roaring::RoaringBitmap;
use rustc_hash::FxHashMap;
use serde::Serialize;

use crate::search_request::{Adult, Range, SearchRequest, Sort, Support, View};
use crate::text::{TextDoc, TextIndex, TextMatches};

pub const FEATURES: &[&str] = &["quest", "vrcfury", "ma", "liltoon", "poiyomi"];
const AVATAR_CATEGORY: &str = "3D Characters";
/// UI price slider tops out at this value meaning "and above"
const PRICE_CAP: i32 = 15_000;

pub struct DocInput {
    pub id: i64,
    pub title: String,
    pub titles_translated: Vec<String>,
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
    fn add(&mut self, key: &str, doc: u32) {
        self.0.entry(key.to_owned()).or_default().insert(doc);
    }
    /// OR over the given keys; unknown keys match nothing
    fn any(&self, keys: &[String]) -> RoaringBitmap {
        keys.iter().filter_map(|k| self.0.get(k)).fold(RoaringBitmap::new(), |acc, b| acc | b)
    }
    pub fn len(&self, key: &str) -> u64 {
        self.0.get(key).map_or(0, |b| b.len())
    }
    pub fn keys(&self) -> impl Iterator<Item = &String> {
        self.0.keys()
    }
}

pub struct Catalog {
    pub ids: Vec<i64>,
    doc_of: FxHashMap<i64, u32>,
    price: Vec<Option<i32>>,
    published: Vec<i64>,
    likes: Vec<i32>,
    trending: Vec<f32>,
    title_rank: Vec<u32>,
    all: RoaringBitmap,
    avatar_items: RoaringBitmap,
    supported: RoaringBitmap,
    adult_only: RoaringBitmap,
    suggestive: RoaringBitmap,
    relisted: RoaringBitmap,
    pub avatars: Index,
    pub authors: Index,
    pub smart_tags: Index,
    pub booth_tags: Index,
    booth_cats: Index,
    cats: Index,
    cats_primary: Index,
    features: Index,
    gender: Index,
    text: TextIndex,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Group {
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
}

impl Catalog {
    /// Whether some title, tag or author name has `word` as a word.
    pub fn knows_word(&self, word: &str) -> bool {
        self.text.knows(word)
    }

    pub fn build(docs: Vec<DocInput>, now: i64, text_dir: Option<&std::path::Path>) -> Result<Self> {
        let n = docs.len();
        use std::hash::{DefaultHasher, Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        docs.len().hash(&mut hasher);
        for d in &docs {
            d.id.hash(&mut hasher);
            d.title.hash(&mut hasher);
            d.titles_translated.hash(&mut hasher);
            d.booth_tags.hash(&mut hasher);
            d.author.hash(&mut hasher);
            d.bio.hash(&mut hasher);
        }
        let fingerprint = hasher.finish();

        let mut c = Catalog {
            ids: Vec::with_capacity(n),
            doc_of: FxHashMap::with_capacity_and_hasher(n, Default::default()),
            price: Vec::with_capacity(n),
            published: Vec::with_capacity(n),
            likes: Vec::with_capacity(n),
            trending: Vec::with_capacity(n),
            title_rank: vec![0; n],
            all: RoaringBitmap::new(),
            avatar_items: RoaringBitmap::new(),
            supported: RoaringBitmap::new(),
            adult_only: RoaringBitmap::new(),
            suggestive: RoaringBitmap::new(),
            relisted: RoaringBitmap::new(),
            avatars: Index::default(),
            authors: Index::default(),
            smart_tags: Index::default(),
            booth_tags: Index::default(),
            booth_cats: Index::default(),
            cats: Index::default(),
            cats_primary: Index::default(),
            features: Index::default(),
            gender: Index::default(),
            text: TextIndex::build(
                docs.iter().map(|d| TextDoc {
                    title: std::iter::once(&d.title).chain(&d.titles_translated).cloned().collect::<Vec<_>>().join("\n"),
                    tags: d.booth_tags.join("\n"),
                    author: d.author.clone(),
                    bio: d.bio.clone(),
                }),
                text_dir,
                fingerprint,
            )?,
        };

        let mut by_title: Vec<(String, u32)> = Vec::with_capacity(n);
        for (i, d) in docs.into_iter().enumerate() {
            let i = i as u32;
            c.ids.push(d.id);
            c.doc_of.insert(d.id, i);
            c.price.push(d.price);
            let published = d.published.unwrap_or(0);
            c.published.push(published);
            c.likes.push(d.likes);
            // likes decayed by age (HN-style); VRCatalogue's own formula is unknown
            let age_days = ((now - published).max(0) as f32) / 86_400.0;
            c.trending.push(d.likes as f32 / (age_days + 2.0).powf(1.5));
            by_title.push((d.title.to_lowercase(), i));

            c.all.insert(i);
            if d.booth_category.as_deref() == Some(AVATAR_CATEGORY) {
                c.avatar_items.insert(i);
            }
            if d.supported {
                c.supported.insert(i);
            }
            if d.relisted {
                c.relisted.insert(i);
            }
            let rating = d.rating.as_deref();
            if d.is_adult || rating == Some("NSFW") {
                c.adult_only.insert(i);
            } else if rating == Some("Suggestive") {
                c.suggestive.insert(i);
            }
            d.avatars.iter().for_each(|a| c.avatars.add(a, i));
            c.authors.add(&d.author, i);
            d.smart_tags.iter().for_each(|t| c.smart_tags.add(t, i));
            d.booth_tags.iter().for_each(|t| c.booth_tags.add(t, i));
            if let Some(bc) = &d.booth_category {
                c.booth_cats.add(bc, i);
            }
            for (slug, primary) in &d.categories {
                c.cats.add(slug, i);
                if *primary {
                    c.cats_primary.add(slug, i);
                }
            }
            d.features.iter().for_each(|f| c.features.add(f, i));
            if let Some(g) = &d.gender {
                c.gender.add(g, i);
            }
        }
        by_title.sort_unstable();
        for (rank, (_, i)) in by_title.into_iter().enumerate() {
            c.title_rank[i as usize] = rank as u32;
        }
        Ok(c)
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

    fn scan(&self, pred: impl Fn(usize) -> bool) -> RoaringBitmap {
        RoaringBitmap::from_sorted_iter((0..self.len()).filter(|&i| pred(i)).map(|i| i as u32)).expect("ascending")
    }

    /// Doc set for item ids (e.g. a user's favourites); unknown ids are skipped.
    pub fn docs_for(&self, ids: &[i64]) -> RoaringBitmap {
        ids.iter().filter_map(|id| self.doc_of.get(id)).copied().collect()
    }

    fn constraints(&self, r: &SearchRequest, now: i64, text: Option<&RoaringBitmap>, favourites: Option<&RoaringBitmap>) -> Vec<(Group, RoaringBitmap)> {
        let mut out = Vec::new();
        out.push((
            Group::View,
            match (r.view, favourites) {
                (View::Favourites, Some(f)) => f.clone(), // favourites span both items and avatar bodies
                (View::Avatars(support), _) => &match support {
                    Support::All => self.avatar_items.clone(),
                    Support::Unsupported => &self.avatar_items - &self.supported,
                    Support::Supported => &self.avatar_items & &self.supported,
                } - &self.relisted,
                _ => &(&self.all - &self.avatar_items) - &self.relisted,
            },
        ));
        if let (View::Avatars(_), Some(g)) = (r.view, &r.gender) {
            out.push((Group::Gender, self.gender.any(std::slice::from_ref(g))));
        }
        match r.adult {
            Adult::Include => {}
            Adult::Only => out.push((Group::Adult, self.adult_only.clone())),
            Adult::Suggestive => out.push((Group::Adult, self.suggestive.clone())),
            Adult::Safe => out.push((Group::Adult, &self.all - &self.adult_only)), // "safe" keeps Suggestive, as VRCatalogue did
        }
        if let Some(t) = text {
            out.push((Group::Text, t.clone()));
        }
        let mut list = |g, ix: &Index, keys: &[String]| {
            if !keys.is_empty() {
                out.push((g, ix.any(keys)));
            }
        };
        list(Group::Avatars, &self.avatars, &r.avatars);
        list(Group::Authors, &self.authors, &r.authors);
        list(Group::Tags, &self.smart_tags, &r.tags);
        list(Group::BoothTags, &self.booth_tags, &r.btags);
        list(Group::BoothCat, &self.booth_cats, &r.bc);
        let cats = if r.scp { &self.cats_primary } else { &self.cats };
        list(Group::SmartCat, cats, &r.sc);
        // exclusions narrow the same group as their include list
        let exclude = |ix: &Index, keys: &[String]| (!keys.is_empty()).then(|| &self.all - &ix.any(keys));
        if let Some(b) = exclude(&self.smart_tags, &r.extags) {
            out.push((Group::Tags, b));
        }
        if let Some(b) = exclude(&self.booth_tags, &r.exbtags) {
            out.push((Group::BoothTags, b));
        }
        if !r.features.is_empty() {
            // features AND together
            out.push((Group::Features, r.features.iter().fold(self.all.clone(), |acc, f| acc & self.features.any(std::slice::from_ref(f)))));
        }

        if r.free {
            out.push((Group::Price, self.scan(|i| self.price[i] == Some(0))));
        } else if r.pmin > 0 || (r.pmax > 0 && r.pmax < PRICE_CAP) {
            let hi = if r.pmax > 0 && r.pmax < PRICE_CAP { r.pmax } else { i32::MAX };
            out.push((Group::Price, self.scan(|i| self.price[i].is_some_and(|v| v >= r.pmin && v <= hi))));
        }

        let day = 86_400;
        let range = r.range.map(|range| match range {
            Range::Days(n) => (now - n * day, i64::MAX),
            Range::Custom { from, to } => {
                let ts = |d: Option<chrono::NaiveDate>| d.map(|d| d.and_hms_opt(0, 0, 0).unwrap().and_utc().timestamp());
                // `to` is inclusive of the whole day
                (ts(from).unwrap_or(i64::MIN), ts(to).map_or(i64::MAX, |t| t + day - 1))
            }
        });
        if let Some((lo, hi)) = range {
            out.push((Group::Range, self.scan(|i| self.published[i] >= lo && self.published[i] <= hi)));
        }
        out
    }

    fn intersect(&self, cs: &[(Group, RoaringBitmap)], skip: Option<Group>) -> RoaringBitmap {
        cs.iter().filter(|(g, _)| Some(*g) != skip).fold(self.all.clone(), |acc, (_, b)| acc & b)
    }

    /// The text query's hits and scores, joined by the meaning-based matches when there are any.
    fn text_hits(&self, r: &SearchRequest, semantic: Option<Semantic>) -> Result<Option<TextMatches>> {
        let mut text = r.q.as_deref().map(|q| self.text.search(q)).transpose()?;
        if let (Some(TextMatches { hits, scores, .. }), Some(sem)) = (text.as_mut(), semantic) {
            // models named in the query: a meaning-based match must fit one of them
            let fits = (!sem.models.is_empty()).then(|| self.avatars.any(sem.models));
            if let Some(f) = &fits {
                // a named model also works like the avatar filter: its compatible items match even when they don't
                // spell the name (most write it in Japanese), the other words searched among them. Items matching the
                // whole query stay and score both, so they rank first.
                if sem.rest.trim().is_empty() {
                    *hits |= f;
                } else {
                    let TextMatches { hits: rest_hits, scores: rest_scores, .. } = self.text.search(sem.rest)?;
                    let rest_hits = rest_hits & f;
                    for d in &rest_hits {
                        *scores.entry(d).or_default() += rest_scores.get(&d).copied().unwrap_or(0.0);
                    }
                    *hits |= rest_hits;
                }
            }
            let docs: Vec<u32> = sem.ids.iter().filter_map(|id| self.doc_of.get(id)).copied().filter(|d| fits.as_ref().is_none_or(|f| f.contains(*d))).collect();
            let mut cat = self.cats.any(sem.categories);
            if let Some(f) = &fits {
                cat &= f;
            }
            *scores = fuse(scores, &docs, &cat);
            hits.extend(docs);
            *hits |= cat;
        }
        Ok(text)
    }

    /// How many items each of `wider` finds: requests that differ from `r` only in their filters (never in the
    /// text query), so `r`'s text and meaning-based matches hold for all of them. Counts only: no facets, no order.
    pub fn counts(&self, r: &SearchRequest, wider: &[SearchRequest], now: i64, favourites: Option<&RoaringBitmap>, semantic: Option<Semantic>) -> Result<Vec<u64>> {
        let text = self.text_hits(r, semantic)?;
        Ok(wider.iter().map(|w| self.intersect(&self.constraints(w, now, text.as_ref().map(|t| &t.hits), favourites), None).len()).collect())
    }

    /// `semantic`: item ids nearest to the query by meaning (see search.rs); they join the text hits and, when
    /// sorting by relevance, the two rankings are merged. Autocomplete and RSS skip facet counts.
    pub fn search(&self, r: &SearchRequest, now: i64, favourites: Option<&RoaringBitmap>, semantic: Option<Semantic>, count_facets: bool) -> Result<SearchResult> {
        let text = self.text_hits(r, semantic)?;
        let cs = self.constraints(r, now, text.as_ref().map(|t| &t.hits), favourites);
        let hits = self.intersect(&cs, None);

        // disjunctive facets: each group's counts ignore that group's own selection
        let mut facets = Facets::default();
        if count_facets {
            let base = self.intersect(&cs, Some(Group::BoothCat));
            facets.booth = self.booth_cats.0.iter().map(|(k, b)| (k.clone(), base.intersection_len(b))).filter(|(_, n)| *n > 0).collect();
            let base = self.intersect(&cs, Some(Group::SmartCat));
            let cats = if r.scp { &self.cats_primary } else { &self.cats };
            facets.smart = cats.0.iter().map(|(k, b)| (k.clone(), base.intersection_len(b))).filter(|(_, n)| *n > 0).collect();
            let base = self.intersect(&cs, Some(Group::Features));
            facets.features = FEATURES.iter().map(|f| (f.to_string(), base.intersection_len(self.features.0.get(*f).unwrap_or(&RoaringBitmap::new())))).collect();
        }

        let asc = r.asc;
        let (sort, scores) = match (r.sort, &text) {
            (Sort::Relevant, Some(t)) => ("relevant", Some(&t.scores)),
            (Sort::Newest, _) => ("newest", None),
            (Sort::Price, _) => ("price", None),
            (Sort::Likes, _) => ("likes", None),
            (Sort::Title, _) => ("title", None),
            _ => ("trending", None),
        };
        // every key sorts descending unless dir=asc (title defaults to A–Z)
        let key = |i: u32| -> f64 {
            let i = i as usize;
            match sort {
                "relevant" => scores.and_then(|s| s.get(&(i as u32))).copied().unwrap_or(0.0) as f64,
                "newest" => self.published[i] as f64,
                "price" => self.price[i].map_or(if asc { f64::INFINITY } else { f64::NEG_INFINITY }, f64::from),
                "likes" => self.likes[i] as f64,
                "title" => -(self.title_rank[i] as f64),
                _ => self.trending[i] as f64,
            }
        };
        let cmp = |a: &u32, b: &u32| {
            let o = key(*b).total_cmp(&key(*a));
            let o = if asc { o.reverse() } else { o };
            // equal relevance (e.g. a model's items that don't mention it by name): trending first
            let o = if sort == "relevant" { o.then_with(|| self.trending[*b as usize].total_cmp(&self.trending[*a as usize])) } else { o };
            o.then(a.cmp(b))
        };

        let page_size = r.page_size;
        let count = hits.len();
        let total_pages = (count as usize).div_ceil(page_size).max(1);
        let page = r.page.clamp(1, total_pages);
        let start = (page - 1) * page_size;
        let end = (page * page_size).min(count as usize);
        let mut docs: Vec<u32> = hits.iter().collect();
        // Partition at both page boundaries so deep pages don't sort every preceding result.
        if end < docs.len() && end > 0 {
            docs.select_nth_unstable_by(end - 1, cmp);
            docs.truncate(end);
        }
        if start > 0 {
            docs.select_nth_unstable_by(start, cmp);
        }
        let page_docs = &mut docs[start..];
        page_docs.sort_unstable_by(cmp);
        let ids = page_docs.iter().map(|&d| self.ids[d as usize]).collect();

        Ok(SearchResult {
            count,
            page,
            total_pages,
            page_size,
            sort,
            dir: if asc { "asc" } else { "desc" },
            ids,
            facets,
            typo: text.as_ref().is_some_and(|t| t.typo),
        })
    }
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

/// Reciprocal rank fusion: each list contributes 1 / (60 + rank), robust to the two scores' different scales.
/// Meaning-only matches count half: they add recall, but a word match is the stronger signal, so a semantic-only
/// item ranks with text hits around 60th place unless both lists agree on it. Items in a category the query names
/// get a little more than the best meaning-only match: below the top text hits, above meaning-only ones.
/// shortcut: category-only items tie with each other (then by load order); rank them by meaning if that shows.
fn fuse(text: &FxHashMap<u32, f32>, semantic: &[u32], category: &RoaringBitmap) -> FxHashMap<u32, f32> {
    const K: f32 = 60.0;
    let mut ranked: Vec<(u32, f32)> = text.iter().map(|(d, s)| (*d, *s)).collect();
    ranked.sort_unstable_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
    let mut out = FxHashMap::with_capacity_and_hasher(ranked.len() + semantic.len(), Default::default());
    for (rank, (d, _)) in ranked.iter().enumerate() {
        *out.entry(*d).or_default() += 1.0 / (K + rank as f32 + 1.0);
    }
    for (rank, d) in semantic.iter().enumerate() {
        *out.entry(*d).or_default() += 0.5 / (K + rank as f32 + 1.0);
    }
    for d in category {
        *out.entry(d).or_default() += 0.5 / K;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::params::Params;

    fn doc(id: i64) -> DocInput {
        DocInput {
            id,
            title: format!("item {id}"),
            titles_translated: vec![],
            bio: String::new(),
            author: "a".into(),
            booth_category: Some("3D Clothing".into()),
            price: Some(1000),
            likes: 0,
            published: Some(0),
            is_adult: false,
            rating: Some("SFW".into()),
            features: vec![],
            avatars: vec![],
            categories: vec![],
            booth_tags: vec![],
            smart_tags: vec![],
            supported: false,
            gender: None,
            relisted: false,
        }
    }

    fn ids(c: &Catalog, q: &str) -> Vec<i64> {
        let mut v = c.search_plain(&format!("{q}&pageSize=100"), 0, None).unwrap().ids;
        v.sort();
        v
    }

    fn catalog() -> Catalog {
        let mut d = (1..=6).map(doc).collect::<Vec<_>>();
        d[0].avatars = vec!["manuka".into()];
        d[0].features = vec!["ma".into(), "liltoon".into()];
        d[0].categories = vec![("clothing".into(), true), ("clothing--bikinis".into(), true)];
        d[1].avatars = vec!["shinano".into()];
        d[1].features = vec!["ma".into()];
        d[1].categories = vec![("clothing".into(), false)];
        d[1].price = Some(0);
        d[2].is_adult = true;
        d[3].rating = Some("Suggestive".into());
        d[4].rating = Some("NSFW".into());
        d[5].booth_category = Some("3D Characters".into());
        d[5].supported = true;
        d[5].gender = Some("female".into());
        Catalog::build(d, 0, None).unwrap()
    }

    #[test]
    fn metadata_reload_reuses_text_index_and_preserves_complete_results() {
        let root = std::env::temp_dir().join(format!("catalog-reuse-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let inputs = |changed: bool| {
            (1..=32).map(|id| {
                let mut d = doc(id);
                d.title = if id % 2 == 0 { "Short Hair".into() } else { "ゴシックドレス".into() };
                d.titles_translated = vec!["衣装 ヘア".into()];
                d.avatars = vec!["shinano".into()];
                d.categories = vec![("clothing".into(), true)];
                d.likes = if changed { 100 - id as i32 } else { id as i32 };
                d.price = Some(if changed && id % 3 == 0 { 0 } else { 1000 });
                d.is_adult = changed && id % 5 == 0;
                d.features = if changed && id % 4 == 0 { vec!["ma".into()] } else { vec![] };
                d
            }).collect()
        };
        let original = Catalog::build(inputs(false), 0, Some(&root)).unwrap();
        let index_dir = std::fs::read_dir(&root).unwrap().next().unwrap().unwrap().path();
        let sentinel = index_dir.join("reused");
        std::fs::write(&sentinel, "keep").unwrap();
        let cached = Catalog::build(inputs(true), 0, Some(&root)).unwrap();
        let rebuilt = Catalog::build(inputs(true), 0, None).unwrap();
        assert!(sentinel.exists(), "metadata changes must not rebuild the text index");
        let models = vec!["shinano".to_owned()];
        let categories = vec!["clothing".to_owned()];
        let semantic = Semantic { ids: &[32, 8, 2], categories: &categories, models: &models, rest: "hair" };
        let favourites = cached.docs_for(&[2, 3, 4, 5, 6, 8, 32]);
        for query in [
            "", "q=hair", "q=ヘア", "q=ゴシック", "q=hair&pageSize=3&page=2",
            "q=hair&sort=likes", "q=hair&sort=price&dir=asc", "q=hair&sort=title",
            "q=hair&adult=include&if=ma", "q=hair&free=1", "q=hair&sc=clothing", "q=hair&v=favourites",
        ] {
            let p = request(query);
            for sem in [None, Some(semantic)] {
                assert_eq!(cached.search(&p, 0, Some(&favourites), sem, true).unwrap(), rebuilt.search(&p, 0, Some(&favourites), sem, true).unwrap(), "query {query:?}");
            }
        }
        // Reusing disk files must not mutate an old snapshot still serving requests.
        assert_ne!(original.search_plain("free=1", 0, None).unwrap().count, cached.search_plain("free=1", 0, None).unwrap().count);
        drop((original, cached, rebuilt));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn text_index_identity_includes_item_ids_and_text_changes() {
        let root = std::env::temp_dir().join(format!("catalog-identity-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let mut last = None;
        for (id, title) in [(1, "Hair"), (2, "Hair"), (2, "Dress")] {
            let mut d = doc(id);
            d.title = title.into();
            let c = Catalog::build(vec![d], 0, Some(&root)).unwrap();
            let current = std::fs::read_dir(&root).unwrap().map(|e| e.unwrap().path()).find(|p| p.join("READY").exists()).unwrap();
            assert_ne!(last.as_ref(), Some(&current));
            assert_eq!(c.search_plain(&format!("q={title}"), 0, None).unwrap().ids, [id]);
            last = Some(current);
        }
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn a_named_model_matches_its_items_and_keeps_full_query_hits() {
        let mut d = (1..=4).map(doc).collect::<Vec<_>>();
        d[0].title = "しなの short hair".into(); // fits shinano, written in Japanese
        d[0].avatars = vec!["shinano".into()];
        d[1].title = "shinano short hair".into(); // spells it, not linked
        d[2].title = "long dress".into(); // fits shinano, other words
        d[2].avatars = vec!["shinano".into()];
        d[3].title = "short hair".into(); // other model
        d[3].avatars = vec!["manuka".into()];
        let c = Catalog::build(d, 0, None).unwrap();
        let models = vec!["shinano".to_owned()];
        let run = |q: &str, rest: &str| {
            let sem = Semantic { ids: &[], categories: &[], models: &models, rest };
            let mut v = c.search(&request(&format!("q={q}&sort=relevant")), 0, None, Some(sem), true).unwrap().ids;
            let first = v[0];
            v.sort();
            (first, v)
        };
        // bare name: every compatible item plus the unlinked one that spells it, which ranks first
        assert_eq!(run("shinano", ""), (2, vec![1, 2, 3]));
        // name + words: the words among compatible items, plus the old full-query hit, which ranks first
        assert_eq!(run("shinano%20short%20hair", "short hair"), (2, vec![1, 2]));
    }

    fn request(query: &str) -> SearchRequest {
        SearchRequest::from(&Params::parse(query))
    }

    impl Catalog {
        fn search_plain(&self, query: &str, now: i64, favourites: Option<&RoaringBitmap>) -> Result<SearchResult> {
            self.search(&request(query), now, favourites, None, true)
        }
    }

    #[test]
    fn fusion_rewards_agreement_and_keeps_semantic_only_hits() {
        let text: FxHashMap<u32, f32> = [(1, 9.0), (2, 5.0), (3, 1.0)].into_iter().collect();
        let f = fuse(&text, &[3, 7], &RoaringBitmap::new());
        // in both lists beats first in one list; a semantic-only hit stays below the text hits
        assert!(f[&3] > f[&1]);
        assert!(f[&7] > 0.0 && f[&7] < f[&2]);
        assert_eq!(f.len(), 4);
        // a category hit joins, ranks above meaning-only matches, and lifts a text hit inside the category
        let g = fuse(&text, &[7], &[2, 8].into_iter().collect());
        assert!(g[&8] > g[&7] && g[&8] < g[&1]);
        assert!(g[&2] > g[&1]);
    }

    #[test]
    fn re_uploaded_items_stay_out_of_search_but_not_favourites() {
        let mut d = (1..=3).map(doc).collect::<Vec<_>>();
        d[0].relisted = true;
        d[2].booth_category = Some(AVATAR_CATEGORY.into());
        d[2].supported = true;
        d[2].relisted = true;
        let c = Catalog::build(d, 0, None).unwrap();
        assert_eq!(ids(&c, ""), [2]);
        assert_eq!(ids(&c, "q=item"), [2]);
        assert!(ids(&c, "v=avatars").is_empty());
        let favs = c.docs_for(&[1, 3]);
        let mut f = c.search_plain("v=favourites&pageSize=10", 0, Some(&favs)).unwrap().ids;
        f.sort();
        assert_eq!(f, [1, 3]);
        assert_eq!(c.docs_for(&[1]).len(), 1); // still known, so its page and gallery load
    }

    #[test]
    fn adult_modes_match_vrcatalogue_rules() {
        let c = catalog();
        assert_eq!(ids(&c, ""), [1, 2, 4]); // safe keeps Suggestive, drops adult/NSFW and the avatar view
        assert_eq!(ids(&c, "adult=adult_only"), [3, 5]);
        assert_eq!(ids(&c, "adult=suggestive"), [4]);
        assert_eq!(ids(&c, "adult=include"), [1, 2, 3, 4, 5]);
    }

    #[test]
    fn group_semantics() {
        let c = catalog();
        assert_eq!(ids(&c, "avatars=manuka+shinano"), [1, 2]); // OR within group
        assert_eq!(ids(&c, "if=ma+liltoon"), [1]); // features AND
        assert_eq!(ids(&c, "sc=clothing--bikinis"), [1]);
        assert_eq!(ids(&c, "sc=clothing&scp=1"), [1]);
        assert_eq!(ids(&c, "free=1"), [2]);
        assert_eq!(ids(&c, "v=avatars&gender=female"), [6]);
        assert_eq!(ids(&c, "v=avatars&asup=unsupported"), Vec::<i64>::new());
    }

    #[test]
    fn a_search_says_when_its_words_matched_only_with_a_typo() {
        let mut ribbon = doc(1);
        ribbon.title = "Shinano Ribbon".into();
        let c = Catalog::build(vec![ribbon, doc(2)], 0, None).unwrap();
        let typo = |q: &str| c.search_plain(q, 0, None).unwrap().typo;
        assert!(typo("q=shinnano"));
        assert!(!typo("q=shinano") && !typo("q=xyzzyq") && !typo("free=1"));
    }

    #[test]
    fn an_empty_search_counts_what_each_without_group_finds() {
        let c = catalog();
        let wider = |q: &str| {
            let (r, counts) = crate::search::search_and_wider(&c, q, &request(q), 0, None, None).unwrap();
            (r.count, counts)
        };
        // each group dropped alone, in order (only the manuka item has lilToon); an unknown name changes nothing
        assert_eq!(wider("avatars=manuka&free=1&if=liltoon&without=avatars+free+if+nonsense"), (0, vec![0, 1, 0, 0]));
        assert_eq!(wider("avatars=manuka&pmin=1&pmax=500&without=pmin,pmax+avatars"), (0, vec![1, 0]));
        // the text query stays: dropping the filter finds only what the words find
        assert_eq!(wider("q=item%201&free=1&without=free+q"), (0, vec![1, 0]));
        // a search that finds something, or one without `without`, counts nothing
        assert_eq!(wider("avatars=manuka&without=avatars"), (1, vec![]));
        assert_eq!(wider("avatars=manuka&free=1"), (0, vec![]));
    }

    #[test]
    fn facets_ignore_own_group() {
        let c = catalog();
        let r = c.search_plain("if=liltoon", 0, None).unwrap();
        assert_eq!(r.count, 1);
        assert_eq!(r.facets.features["ma"], 2); // feature counts ignore the feature filter
        assert_eq!(r.facets.smart["clothing"], 1); // other groups respect it
    }

    #[test]
    fn skipping_facets_preserves_results_counts_and_order() {
        let c = catalog();
        let favourites = c.docs_for(&[1, 2, 4, 6]);
        let models = vec!["manuka".to_owned()];
        let categories = vec!["clothing".to_owned()];
        let semantic = Semantic { ids: &[4, 2], categories: &categories, models: &models, rest: "item" };
        for query in [
            "", "q=item", "q=item&pageSize=2&page=2", "q=missing", "if=liltoon", "sc=clothing&scp=1",
            "adult=include&sort=price&dir=asc", "v=avatars", "v=favourites", "free=1", "avatars=manuka",
        ] {
            let p = request(query);
            for sem in [None, Some(semantic)] {
                let mut full = c.search(&p, 0, Some(&favourites), sem, true).unwrap();
                let light = c.search(&p, 0, Some(&favourites), sem, false).unwrap();
                assert_eq!(light.facets, Facets::default());
                full.facets = Facets::default();
                assert_eq!(full, light, "query {query:?}");
            }
        }
    }

    #[test]
    fn sort_and_paging() {
        let c = catalog();
        let r = c.search_plain("adult=include&sort=price&dir=asc&pageSize=2&page=1", 0, None).unwrap();
        assert_eq!(r.ids, [2, 1]); // free first, then id order for ties
        assert_eq!(r.total_pages, 3);
        let r = c.search_plain("adult=include&page=99", 0, None).unwrap();
        assert_eq!(r.page, 1);
    }

    #[test]
    fn every_sort_pages_like_a_full_sort() {
        let docs = (1..=73).map(|id| {
            let mut d = doc(id);
            d.title = format!("item {}", id % 11);
            d.price = (id % 7 != 0).then_some((id % 9) as i32 * 100);
            d.likes = (id % 5) as i32;
            d.published = (id % 6 != 0).then_some(id % 13 * 86_400);
            d
        }).collect();
        let c = Catalog::build(docs, 2_000_000, None).unwrap();
        for query in ["", "q=item", "free=1", "q=missing"] {
            for sort in ["trending", "newest", "price", "likes", "title", "relevant"] {
                for dir in ["asc", "desc"] {
                    let base = format!("{query}&sort={sort}&dir={dir}");
                    // Fewer than 100 hits: page 1 sorts the entire set without either partition.
                    let full = c.search(&request(&format!("{base}&pageSize=100")), 0, None, None, false).unwrap();
                    for size in [1, 7, 20, 100] {
                        let pages = full.ids.len().div_ceil(size).max(1);
                        for page in [0, 1, 2, 5, pages, pages + 1, usize::MAX] {
                            let params = request(&format!("{base}&pageSize={size}&page={page}"));
                            let r = c.search(&params, 0, None, None, false).unwrap();
                            let page = page.clamp(1, pages);
                            let expected: Vec<_> = full.ids.iter().skip((page - 1) * size).take(size).copied().collect();
                            assert_eq!(r.ids, expected, "{base}, size {size}, page {page}");
                            assert_eq!((r.count, r.page, r.total_pages), (full.count, page, pages));
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn favourites_view_spans_all_kinds() {
        let c = catalog();
        let favs = c.docs_for(&[1, 6, 999]);
        let r = c.search_plain("v=favourites&adult=include", 0, Some(&favs)).unwrap();
        let mut ids = r.ids;
        ids.sort();
        assert_eq!(ids, [1, 6]);
    }
}
