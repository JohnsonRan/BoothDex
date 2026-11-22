//! `/api/related`: related searches for a text query (owner, 2026-11-20), BOOTH tags near the query's meaning. Each tag
//! has a centroid, the mean embedding of the items that carry it. The tags nearest the embedding a search already
//! stored for the query are offered, minus those that respell or translate the query or one of its words, read as
//! such a translation in the visitor's language, make the same search as a nearer one, name an avatar, are spread by a
//! few shops or by everyone, seldom go with the query's own matches, or find nothing under the current filters. The
//! rules and numbers were tuned with a prototype on the development catalogue (42 sample queries, 2026-11-20). A chip
//! reads in the visitor's language when the tag dictionary has the tag in it (owner, 2026-11-21).

use std::sync::Arc;

use anyhow::Result;
use axum::Json;
use axum::extract::{RawQuery, State};
use axum::response::{IntoResponse, Response};
use boothdex_core::embed::{DIMS, HalfVector};
use futures_util::TryStreamExt;
use roaring::RoaringBitmap;
use rustc_hash::{FxHashMap, FxHashSet};
use sqlx::PgPool;

use crate::catalog::Catalog;
use crate::expansion::{Equivalents, LANGUAGES, MIN_CHARS, is_cjk};
use crate::params::Params;
use crate::search_request::{SearchRequest, View};
use crate::tag_words::TagRow;
use crate::text::{compact, fold};
use crate::{ApiError, AppState, account, limit, search};

/// A tag needs this many embedded items for a centroid; it is on at least 20 listed items (expansion.rs).
const MIN_EMBEDDED: u32 = 5;
/// Centroids this old are built again at the next reload; younger ones carry over to the new snapshot.
const REBUILD_AFTER: chrono::TimeDelta = chrono::TimeDelta::hours(6);
const SEARCHES: usize = 4;
/// Candidates checked against the catalogue: their matches under the filters, and the searches already chosen.
const VALIDATE: usize = 12;
/// More listed items per shop than this: a few shops put the tag on everything they sell (SEO).
const MAX_PER_SHOP: f64 = 6.0;
/// On a larger share of the listed items a tag says nothing about the query.
const BROAD_SHARE: f64 = 0.05;
/// From this many matches on, the query's own matches show which tags go with it:
const MIN_MATCHES: u64 = 10;
/// one carried by fewer of them (Ochiai) is near the query only in the embedding (gothic punk for jacket).
const MIN_COOCCURRENCE: f64 = 0.07;
/// Two tags whose title matches overlap this much, over the smaller set, name one thing in two languages.
const SAME_THING: f64 = 0.7;

/// What related searches read besides the catalogue.
#[derive(Default)]
pub(crate) struct Related {
    pub(crate) avatars: AvatarNames,
    pub(crate) centroids: Option<Arc<TagCentroids>>,
    pub(crate) labels: TagLabels,
}

/// The tag dictionary's rows (`booth_tag_translations`) of the tags a related search can be: those spelt as a tag on
/// enough listed items (`Equivalents::name`).
#[derive(Default)]
pub(crate) struct TagLabels {
    /// per tag, its row in `rows`
    tags: FxHashMap<String, u32>,
    rows: Vec<Label>,
    /// per form (`forms`) of a compacted value or keyword, the rows holding it: the tags that respell a query's words,
    /// found without reading every row
    spelt: FxHashMap<Box<str>, Vec<u32>>,
}

/// A tag's values in the languages of `LANGUAGES`, and its keywords.
struct Label {
    values: [Option<Box<str>>; 4],
    keywords: Box<[Box<str>]>,
}

impl Label {
    /// Its keywords count: search finds the tag's items by them as by its values (tag_words.rs).
    fn spellings(&self) -> impl Iterator<Item = &str> {
        self.values.iter().flatten().chain(self.keywords.iter()).map(|v| &**v)
    }
}

impl TagLabels {
    pub(crate) fn new(rows: &[TagRow], tags: &Equivalents) -> Self {
        let names: FxHashSet<&str> = (0..tags.len() as u32).map(|t| tags.name(t)).collect();
        let mut out = Self::default();
        for r in rows.iter().filter(|r| names.contains(r.tag.as_str())) {
            let row = out.rows.len() as u32;
            let values = std::array::from_fn(|k| r.values[k].as_deref().map(Box::from));
            let label = Label { values, keywords: r.keywords.iter().map(|k| Box::from(k.as_str())).collect() };
            for spelling in label.spellings().map(compact) {
                for form in forms(&spelling) {
                    let rows = out.spelt.entry(form.into()).or_default();
                    if rows.last() != Some(&row) {
                        rows.push(row);
                    }
                }
            }
            out.tags.insert(r.tag.clone(), row);
            out.rows.push(label);
        }
        out
    }

    fn row(&self, tag: &str) -> Option<&Label> {
        self.tags.get(tag).map(|&row| &self.rows[row as usize])
    }

    /// The tag in the language `LANGUAGES[k]`.
    fn label(&self, tag: &str, k: usize) -> Option<&str> {
        self.row(tag)?.values[k].as_deref()
    }

    /// Whether `like` accepts some value or keyword of the tag.
    fn reads_as(&self, tag: &str, like: impl Fn(&str) -> bool) -> bool {
        self.row(tag).is_some_and(|l| l.spellings().any(like))
    }

    /// The forms (`forms`) of the values in `LANGUAGES[k]` of the tags with a value or keyword that respells one of
    /// `phrases`: what that language calls them.
    fn translations<'p>(&self, k: usize, phrases: impl IntoIterator<Item = &'p str>) -> FxHashSet<String> {
        let mut out = FxHashSet::default();
        for phrase in phrases.into_iter().map(compact) {
            for &row in forms(&phrase).filter_map(|form| self.spelt.get(form)).flatten() {
                if let Some(value) = self.rows[row as usize].values[k].as_deref().map(compact) {
                    out.extend(forms(&value).map(str::to_owned));
                }
            }
        }
        out
    }
}

/// The `/api/related` response. The site's type is generated from it (see `api_types`).
#[derive(serde::Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub(crate) struct RelatedResponse {
    /// best first; empty when there is nothing to offer
    searches: Vec<RelatedSearch>,
}

#[derive(serde::Serialize, Debug, PartialEq)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub(crate) struct RelatedSearch {
    /// the tag as most items spell it
    label: String,
    /// the query that searches it: the model names of the asked query, then the tag
    q: String,
}

/// Per BOOTH tag, the mean of its items' embeddings, from one embedding model.
pub(crate) struct TagCentroids {
    /// the model of the item embeddings, as the worker records it (`app_settings` key `embed_state`)
    model: String,
    built: chrono::DateTime<chrono::Utc>,
    /// folded tags (`expansion::Equivalents` keys), and their centroids in the same order, `DIMS` floats each, unit
    /// length
    keys: Vec<String>,
    vectors: Vec<f32>,
}

impl TagCentroids {
    /// The centroids a new snapshot of `catalog` uses: `previous` while it is from the current embedding model and
    /// younger than `REBUILD_AFTER`, else built again from every item embedding (in production, 2026-11-20, about
    /// 212k of them, 435 MB as halfvec). `None` until at least one tag has enough embedded items.
    pub(crate) async fn current(pool: &PgPool, catalog: &Catalog, previous: Option<Arc<Self>>) -> Result<Option<Arc<Self>>> {
        // one view of the database: the worker clears the embeddings when the model changes, then records the model
        let mut tx = pool.begin().await?;
        sqlx::query("set transaction isolation level repeatable read, read only").execute(&mut *tx).await?;
        let model: Option<String> =
            sqlx::query_scalar("select value->>'model' from app_settings where key = 'embed_state'").fetch_optional(&mut *tx).await?.flatten();
        let Some(model) = model else { return Ok(None) };
        if let Some(p) = previous.filter(|p| p.model == model && chrono::Utc::now() - p.built < REBUILD_AFTER) {
            return Ok(Some(p));
        }
        // which tags get a centroid first, from the ids alone, so the sums take no memory for the others
        let embedded: Vec<i64> = sqlx::query_scalar("select id from items where embedding is not null").fetch_all(&mut *tx).await?;
        let mut sums = Sums::new(catalog, embedded);
        // An empty set must not carry over for six hours once the worker has embedded enough items.
        if sums.sums.is_empty() {
            return Ok(None);
        }
        let mut rows = sqlx::query_as::<_, (i64, HalfVector)>("select id, embedding from items where embedding is not null").fetch(&mut *tx);
        while let Some((id, v)) = rows.try_next().await? {
            sums.add(id, v.as_slice().iter().map(|x| x.to_f32()));
        }
        Ok(Some(Arc::new(sums.finish(model, chrono::Utc::now()))))
    }

    pub(crate) fn len(&self) -> usize {
        self.keys.len()
    }

    /// Heap memory, for the startup log.
    pub(crate) fn bytes(&self) -> usize {
        self.vectors.len() * size_of::<f32>() + self.keys.iter().map(|k| k.len() + size_of::<String>()).sum::<usize>()
    }

    /// The tags nearest to `v` by cosine similarity first (the centroids are unit length, so the dot product orders
    /// them); ties by the tag.
    fn ranked(&self, v: &[f32]) -> impl Iterator<Item = &str> {
        let mut order: Vec<(f32, usize)> = self.vectors.as_chunks::<DIMS>().0.iter().map(|c| dot(c, v)).zip(0..).collect();
        order.sort_unstable_by(|a, b| b.0.total_cmp(&a.0).then_with(|| self.keys[a.1].cmp(&self.keys[b.1])));
        order.into_iter().map(|(_, i)| self.keys[i].as_str())
    }
}

/// Centroids being summed: each item's embedding, at unit length, added to each of its tags that gets a centroid.
struct Sums<'a> {
    catalog: &'a Catalog,
    /// per tag, its row in `sums`: the tags on at least `MIN_EMBEDDED` embedded items
    rows: Vec<Option<u32>>,
    sums: Vec<f32>,
    v: Vec<f32>,
}

impl<'a> Sums<'a> {
    /// Rows for the tags that `embedded` (the ids of the embedded items) gives a centroid, so memory holds only those.
    fn new(catalog: &'a Catalog, embedded: impl IntoIterator<Item = i64>) -> Self {
        let tags = catalog.tags();
        let mut counts = vec![0u32; tags.len()];
        for d in embedded.into_iter().filter_map(|id| catalog.doc(id)) {
            tags.tags_of(d).iter().for_each(|&t| counts[t as usize] += 1);
        }
        let mut n = 0;
        let rows = counts
            .into_iter()
            .map(|c| {
                (c >= MIN_EMBEDDED).then(|| {
                    n += 1;
                    n - 1
                })
            })
            .collect();
        Self { catalog, rows, sums: vec![0.0; n as usize * DIMS], v: vec![0.0; DIMS] }
    }

    fn add(&mut self, id: i64, embedding: impl Iterator<Item = f32>) {
        let Some(tags) = self.catalog.doc(id).map(|d| self.catalog.tags().tags_of(d)).filter(|t| !t.is_empty()) else { return };
        self.v.iter_mut().zip(embedding).for_each(|(x, e)| *x = e);
        normalize(&mut self.v);
        for row in tags.iter().filter_map(|&t| self.rows[t as usize]) {
            self.sums[row as usize * DIMS..][..DIMS].iter_mut().zip(&self.v).for_each(|(s, x)| *s += x);
        }
    }

    /// The sums made unit length.
    fn finish(self, model: String, built: chrono::DateTime<chrono::Utc>) -> TagCentroids {
        let Self { catalog, rows, mut sums, .. } = self;
        let mut keys = vec![String::new(); sums.len() / DIMS];
        for (key, t) in catalog.tags().keys() {
            if let Some(row) = rows[t as usize] {
                keys[row as usize] = key.to_owned();
            }
        }
        sums.chunks_mut(DIMS).for_each(normalize);
        TagCentroids { model, built, keys, vectors: sums }
    }
}

fn normalize(v: &mut [f32]) {
    let n = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if n > 0.0 {
        v.iter_mut().for_each(|x| *x /= n);
    }
}

/// In eight lanes, which the compiler vectorizes; `DIMS` is a multiple of 8.
fn dot(a: &[f32], b: &[f32]) -> f32 {
    let mut lanes = [0.0f32; 8];
    for (x, y) in a.as_chunks::<8>().0.iter().zip(b.as_chunks::<8>().0) {
        for i in 0..8 {
            lanes[i] += x[i] * y[i];
        }
    }
    lanes.iter().sum()
}

/// Avatar names (`fold`ed) that a tag holds when it names an avatar (`Kikyo (桔梗)`, `桔梗対応`): the tag then says
/// which avatar an item fits, which the search reads from the model's name already.
#[derive(Default)]
pub(crate) struct AvatarNames {
    /// Short names are too ambiguous except as the whole tag.
    exact: FxHashSet<String>,
    /// Longer names indexed by their first two characters.
    inside: FxHashMap<(char, char), Vec<AvatarForm>>,
}

struct AvatarForm {
    chars: Vec<char>,
    whole_word: bool,
}

impl AvatarNames {
    pub(crate) fn new<'n>(names: impl IntoIterator<Item = &'n str>) -> Self {
        let mut out = Self::default();
        for name in names.into_iter().map(fold).filter(|n| !n.is_empty()) {
            let chars: Vec<char> = name.chars().collect();
            let whole_word = !chars.iter().copied().any(is_cjk);
            if chars.len() >= if whole_word { 3 } else { 2 } {
                out.inside.entry((chars[0], chars[1])).or_default().push(AvatarForm { chars, whole_word });
            } else {
                out.exact.insert(name);
            }
        }
        out
    }

    /// Whether the folded tag names an avatar.
    pub(crate) fn named_in(&self, tag: &str) -> bool {
        let chars: Vec<char> = tag.chars().collect();
        let boundary = |c: Option<&char>| c.is_none_or(|&c| !c.is_alphanumeric() || is_cjk(c));
        self.exact.contains(tag) || chars.windows(2).enumerate().any(|(i, pair)| {
            self.inside.get(&(pair[0], pair[1])).is_some_and(|names| names.iter().any(|n| {
                chars[i..].starts_with(&n.chars)
                    && (!n.whole_word || (boundary(chars[..i].last()) && boundary(chars.get(i + n.chars.len()))))
            }))
        })
    }
}

/// The same word spelt again: equal once case, width, spaces and symbols are gone, or but for a plural `s` or `es`
/// (gothic girl / gothic girls, eye / eyes, dress / dresses). Unlike `expansion::same_word`, one holding the other does
/// not count: ミニスカート is a related search for スカート.
fn respelling(a: &str, b: &str) -> bool {
    let (a, b) = (compact(a), compact(b));
    forms(&a).any(|x| forms(&b).any(|y| x == y))
}

/// A compacted word, and the word without a plural `s` or `es`: the spelling alone cannot tell which one a plural
/// added (eyes gives eyes, eye and ey; dresses gives dresses, dresse and dress).
fn forms(s: &str) -> impl Iterator<Item = &str> {
    [Some(s), s.strip_suffix('s'), s.strip_suffix("es")].into_iter().flatten()
}

/// What related searches are chosen for: a search whose query `Lookup::split_names` read as `meaning` and the models
/// `named` with their words, for a visitor reading `LANGUAGES[language]`.
pub(crate) struct Asked<'a> {
    pub(crate) req: &'a SearchRequest,
    pub(crate) meaning: &'a str,
    pub(crate) named: &'a [(String, Vec<&'a str>)],
    pub(crate) favourites: Option<&'a RoaringBitmap>,
    pub(crate) now: i64,
    pub(crate) language: Option<usize>,
}

/// Up to `SEARCHES` related searches for `asked`, whose meaning has the embedding `vector`, best first. A search is
/// labelled with the tag in the visitor's language (`labels`); its `q` uses that label when the label has two or more
/// characters and its own words find something, else the tag.
pub(crate) fn choose(catalog: &Catalog, related: &Related, centroids: &TagCentroids, vector: &[f32], asked: &Asked) -> Result<Vec<RelatedSearch>> {
    let (avatars, labels) = (&related.avatars, &related.labels);
    let tags = catalog.tags();
    let words: Vec<&str> = asked.meaning.split_whitespace().collect();
    let equivalents = catalog.equivalents(asked.meaning)?;
    let matches = catalog.word_hits(asked.meaning)?;
    // how many of the query's matches carry each tag, when they are enough to tell
    let shared = (matches.len() >= MIN_MATCHES).then(|| {
        let mut shared: FxHashMap<u32, u64> = FxHashMap::default();
        for d in &matches {
            tags.tags_of(d).iter().for_each(|&t| *shared.entry(t).or_default() += 1);
        }
        shared
    });
    let models: Vec<String> = asked.named.iter().map(|(slug, _)| slug.clone()).collect();
    let kept: Vec<&str> = asked.named.iter().flat_map(|(_, words)| words.iter().copied()).collect();
    // what a destination's matches must reach; with models named, among the items fitting them
    let mut allowed = catalog.allowed(asked.req, asked.now, asked.favourites);
    if !models.is_empty() {
        allowed &= catalog.fitting(&models);
    }
    // a destination finds something by its words: an item spelling them as a tag that the filters let through is one;
    // else their text matches, with the tags that translate them (the site drops xl=0 with new words)
    let finds = |words: &str| -> Result<bool> {
        let dest = SearchRequest { q: Some(kept.iter().copied().chain([words]).collect::<Vec<_>>().join(" ")), ..asked.req.clone() };
        let found = |expand| catalog.finds_by_words(&dest, asked.now, asked.favourites, &models, words, expand);
        Ok(catalog.booth_tags.get(words).is_some_and(|b| !b.is_disjoint(&allowed)) || found(None)? || found(Some(words))?)
    };
    let destination = |words: &str| kept.iter().copied().chain([words]).collect::<Vec<_>>().join(" ");
    // in the visitor's language, a label that respells what a tag translating the query or a word of it reads there is
    // that translation, whatever the label's tag reads elsewhere (one-piece reads 连衣裙 in Chinese, as ドレス does, which
    // reads dress; in Japanese it reads ワンピース)
    let translated = asked.language.map(|k| labels.translations(k, std::iter::once(asked.meaning).chain(words.iter().copied()))).unwrap_or_default();
    let translation = |label: &str| !translated.is_empty() && forms(&compact(label)).any(|form| translated.contains(form));
    let (mut chosen, mut chosen_titles, mut validated): (Vec<(&str, RelatedSearch)>, Vec<RoaringBitmap>, usize) = (Vec::new(), Vec::new(), 0);
    for key in centroids.ranked(vector) {
        if chosen.len() == SEARCHES || validated == VALIDATE {
            break;
        }
        // a tag no longer on enough listed items since the centroids were built
        let Some(t) = tags.id(key) else { continue };
        let items = tags.items(t) as f64;
        let rare = |shared: &FxHashMap<u32, u64>| {
            shared.get(&t).copied().unwrap_or(0) as f64 / (matches.len() as f64 * items).sqrt() < MIN_COOCCURRENCE
        };
        // cheapest first: a query can walk every centroid
        if items > MAX_PER_SHOP * f64::from(tags.shops(t)) || items > BROAD_SHARE * catalog.listed() as f64 || shared.as_ref().is_some_and(rare) {
            continue;
        }
        let name = tags.name(t);
        // a tag that is the query or a word of it, or that the dictionary translates as one, is the same search or a
        // wider one, not a related one ("黒髪 ロング" offering ロング; long hair offering ロング, which reads long)
        let repeats = |s: &str| respelling(asked.meaning, s) || words.iter().any(|w| respelling(w, s));
        let label = asked.language.and_then(|k| labels.label(name, k)).unwrap_or(name);
        // a nearer search already chosen is the same one when it reads the same, or when either tag is the other or one
        // of its values (ウェーブヘア, which reads wavy hair, before wavy hair)
        let same = |(tag, search): &(&str, RelatedSearch)| {
            respelling(&search.label, label)
                || respelling(tag, name)
                || labels.reads_as(tag, |v| respelling(v, name))
                || labels.reads_as(name, |v| respelling(v, tag))
        };
        if equivalents.iter().any(|e| fold(e) == key)
            || labels.reads_as(name, repeats)
            || avatars.named_in(key)
            || repeats(name)
            || translation(label)
            || chosen.iter().any(same)
        {
            continue;
        }
        validated += 1;
        if !finds(name)? {
            continue;
        }
        let titles = catalog.title_hits(name)?;
        let translates = |other: &RoaringBitmap| {
            let smaller = other.len().min(titles.len());
            smaller > 0 && other.intersection_len(&titles) as f64 / smaller as f64 >= SAME_THING
        };
        if chosen_titles.iter().any(translates) {
            continue;
        }
        chosen_titles.push(titles);
        // one character matches only where a segmenter leaves it a word of its own (长 for ロング); the expansion skips
        // such tags too
        let q = if label != name && compact(label).chars().count() >= MIN_CHARS && finds(label)? { destination(label) } else { destination(name) };
        chosen.push((name, RelatedSearch { label: label.to_owned(), q }));
    }
    Ok(chosen.into_iter().map(|(_, search)| search).collect())
}

/// Related searches for the search `q` asks (the same parameters), from the embedding a search already stored for its
/// meaning: never a provider request, so a query without one gets none, as does one naming an item by its link or
/// number. The results page asks once per search; a refusal hides the row there and offers no check.
pub(crate) async fn related(
    State(s): State<Arc<AppState>>,
    limit::ClientIp(ip): limit::ClientIp,
    headers: axum::http::HeaderMap,
    RawQuery(q): RawQuery,
) -> Result<Response, ApiError> {
    use limit::Check as _;
    s.limits.related.check(ip)?;
    let raw = q.as_deref().unwrap_or("");
    let p = Params::parse(raw);
    let req = SearchRequest::from(&p);
    let none = || Ok(Json(RelatedResponse { searches: Vec::new() }).into_response());
    let Some(q) = req.q.as_deref() else { return none() };
    // one snapshot for the whole request
    let snap = s.snap.load_full();
    if snap.catalog.referenced(q).is_some() {
        return none();
    }
    let typos = p.get("typo").as_deref() != Some("0");
    // the visitor's language (the site's `hl`), which labels the searches
    let language = p.get("hl").and_then(|hl| LANGUAGES.iter().position(|l| *l == hl));
    let (meaning, named, _) = snap.avatars.split_names(q, |w| !typos || snap.catalog.knows_word(w));
    let Some(centroids) = snap.related.centroids.as_ref().filter(|_| !meaning.is_empty()) else { return none() };
    let model = match s.ai_config().await {
        Ok(c) => c.embed_model.clone(),
        Err(e) => {
            eprintln!("related searches: AI settings: {e}");
            None
        }
    };
    // an admin changed the model: the centroids are from the old one until the worker has embedded the items again
    let Some(model) = model.filter(|m| *m == centroids.model) else { return none() };
    let favourites = if matches!(req.view, View::Favourites) {
        let Some(u) = account::current_user(&s.pool, &headers).await? else { return none() };
        Some(snap.catalog.docs_for(&account::favourite_ids(&s, u.id).await?))
    } else {
        None
    };
    let Some(vector) = search::stored_query_embedding(&s, &model, &meaning).await.filter(|v| v.len() == DIMS) else { return none() };
    let _permit = s.acquire_search().await?;
    let asked = Asked { req: &req, meaning: &meaning, named: &named, favourites: favourites.as_ref(), now: chrono::Utc::now().timestamp(), language };
    let searches = tokio::task::block_in_place(|| choose(&snap.catalog, &snap.related, centroids, &vector, &asked))?;
    Ok(Json(RelatedResponse { searches }).into_response())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::DocInput;

    /// Groups of docs, each `(count, title, tags, shops)`, then 400 untagged docs, so that a tag on 20 items is not on
    /// 5% of them. Doc ids count from 1 in order.
    fn catalogue(groups: &[(usize, &str, &[&str], usize)], each: impl Fn(i64, &mut DocInput)) -> Catalog {
        catalogue_with_words(groups, each, &crate::tag_words::TagWords::default())
    }

    /// `catalogue`, its text search matching the tags' translations in `words` too.
    fn catalogue_with_words(groups: &[(usize, &str, &[&str], usize)], each: impl Fn(i64, &mut DocInput), words: &crate::tag_words::TagWords) -> Catalog {
        let filler: &[(usize, &str, &[&str], usize)] = &[(400, "Filler", &[], 50)];
        let mut docs = Vec::new();
        for &(count, title, tags, shops) in groups.iter().chain(filler) {
            for i in 0..count {
                let id = docs.len() as i64 + 1;
                let mut d = DocInput {
                    id,
                    title: format!("{title} {id}"),
                    titles_translated: vec![],
                    title_zh: None,
                    title_is_zh: false,
                    title_languages: 0b0111,
                    bio: String::new(),
                    author: format!("{title} {}", i % shops),
                    booth_category: Some("3D Clothing".into()),
                    price: Some(1000),
                    likes: 0,
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
                };
                each(id, &mut d);
                docs.push(d);
            }
        }
        Catalog::build_with_tag_words(docs, words, 0, None).unwrap()
    }

    fn unit(i: usize) -> Vec<f32> {
        let mut v = vec![0.0; DIMS];
        v[i] = 1.0;
        v
    }

    /// Centroids nearest the query vector, `unit(0)`, in the order of `tags`.
    fn centroids(tags: &[&str]) -> TagCentroids {
        let mut vectors = Vec::new();
        for i in 0..tags.len() {
            let near = 0.9 - 0.01 * i as f32;
            let mut v = unit(0);
            (v[0], v[i + 1]) = (near, (1.0 - near * near).sqrt());
            vectors.extend(v);
        }
        TagCentroids { model: "m".into(), built: chrono::Utc::now(), keys: tags.iter().map(|t| fold(t)).collect(), vectors }
    }

    /// The related searches for `query`, whose words read as the `named` models are left out of its meaning.
    fn related(c: &Catalog, tags: &[&str], query: &str, named: &[(String, Vec<&str>)], favourites: Option<&RoaringBitmap>) -> Vec<(String, String)> {
        related_in(c, tags, query, named, favourites, TagLabels::default(), None)
    }

    /// `related`, for a visitor reading `language`, with the tag dictionary's rows `labels`.
    fn related_in(
        c: &Catalog,
        tags: &[&str],
        query: &str,
        named: &[(String, Vec<&str>)],
        favourites: Option<&RoaringBitmap>,
        labels: TagLabels,
        language: Option<&str>,
    ) -> Vec<(String, String)> {
        let req = SearchRequest::from(&Params::parse(query));
        let words: Vec<&str> = named.iter().flat_map(|(_, w)| w.iter().copied()).collect();
        let meaning: String = req.q.as_deref().unwrap().split_whitespace().filter(|w| !words.contains(w)).collect::<Vec<_>>().join(" ");
        let language = language.and_then(|l| LANGUAGES.iter().position(|x| *x == l));
        let asked = Asked { req: &req, meaning: &meaning, named, favourites, now: 0, language };
        let related = Related { avatars: AvatarNames::new(["Kikyo", "桔梗", "io"]), centroids: None, labels };
        choose(c, &related, &centroids(tags), &unit(0), &asked).unwrap().into_iter().map(|s| (s.label, s.q)).collect()
    }

    /// A row of the tag dictionary: the tag, its values in en, ja, ko and zh, and its keywords.
    fn row(tag: &str, values: [Option<&str>; 4], keywords: &[&str]) -> TagRow {
        TagRow { tag: tag.into(), values: values.map(|v| v.map(str::to_owned)), keywords: keywords.iter().map(|k| k.to_string()).collect() }
    }

    fn labels(searches: Vec<(String, String)>) -> Vec<String> {
        searches.into_iter().map(|(label, _)| label).collect()
    }

    /// One group per rule, then four that pass and a fifth.
    const RULES: &[(usize, &str, &[&str], usize)] = &[
        (20, "Skirts", &["skirts"], 5),
        (20, "Skirt", &["spam"], 1),
        (40, "Skirt", &["broad"], 8),
        (20, "Gothic", &["gothic punk"], 5),
        (20, "Skirt", &["adult set"], 5),
        (20, "Long Skirt", &["long skirt", "ロングスカート"], 5),
        (20, "Skirt", &["long skirts"], 5),
        (20, "Skirt", &["Kikyo (桔梗)"], 5),
        (20, "Skirt", &["pleats"], 5),
        (20, "Skirt", &["frills"], 5),
        (20, "Skirt", &["lace"], 5),
        (20, "Skirt", &["denim"], 5),
        (5, "Rare", &[], 5),
    ];
    /// The centroids, nearest first.
    const ORDER: &[&str] =
        &["skirts", "spam", "broad", "gothic punk", "adult set", "long skirt", "long skirts", "ロングスカート", "Kikyo (桔梗)", "pleats", "frills", "lace", "denim"];

    /// 101-120 (adult set) are adult; 121-140 (long skirt) fit Shinano and are titled ロングスカート too; 201-220 (frills)
    /// are free.
    fn rules() -> Catalog {
        catalogue(RULES, |id, d| {
            d.is_adult = (101..=120).contains(&id);
            if (121..=140).contains(&id) {
                d.avatars = vec!["shinano".into()];
                d.titles_translated = vec![format!("ロングスカート {id}")];
            }
            if (201..=220).contains(&id) {
                d.price = Some(0);
            }
        })
    }

    #[test]
    fn each_rule_drops_its_tag_and_four_are_offered() {
        let c = rules();
        // skirts respells the query; spam is on more than 6 items per shop; broad on more than 5% of the items; gothic
        // punk is on none of skirt's matches; adult set finds nothing in the safe view; long skirts respells a search
        // already chosen; ロングスカート names the same items as long skirt; Kikyo (桔梗) names an avatar; denim would be
        // a fifth
        assert_eq!(labels(related(&c, ORDER, "q=skirt", &[], None)), ["long skirt", "pleats", "frills", "lace"]);
        // the adult setting decides what a destination finds
        assert_eq!(labels(related(&c, ORDER, "q=skirt&adult=include", &[], None)), ["adult set", "long skirt", "pleats", "frills"]);
        // so do the filters and the view
        assert_eq!(labels(related(&c, ORDER, "q=skirt&free=1", &[], None)), ["frills"]);
        let favourites: RoaringBitmap = (241..=245).collect();
        assert_eq!(labels(related(&c, ORDER, "q=skirt&v=favourites", &[], Some(&favourites))), ["denim"]);
    }

    #[test]
    fn a_named_model_stays_in_the_query_and_narrows_the_destination() {
        let c = rules();
        let named = vec![("shinano".to_owned(), vec!["Shinano"])];
        // only the long skirts fit Shinano
        assert_eq!(related(&c, ORDER, "q=Shinano%20skirt", &named, None), [("long skirt".to_owned(), "Shinano long skirt".to_owned())]);
    }

    #[test]
    fn without_enough_matches_the_query_does_not_judge_which_tags_go_with_it() {
        let c = rules();
        // rare matches five items, too few to judge gothic punk by
        assert_eq!(labels(related(&c, &["gothic punk"], "q=rare", &[], None)), ["gothic punk"]);
        assert!(related(&c, &["gothic punk"], "q=skirt", &[], None).is_empty());
    }

    #[test]
    fn at_most_twelve_candidates_are_checked_against_the_catalogue() {
        let names: Vec<[String; 1]> = (0..12).map(|i| [format!("adult {i}")]).collect();
        let tags: Vec<[&str; 1]> = names.iter().map(|[n]| [n.as_str()]).collect();
        let mut groups: Vec<(usize, &str, &[&str], usize)> = tags.iter().map(|t| (20, "Skirt", &t[..], 5)).collect();
        groups.push((20, "Skirt", &["pleats"], 5));
        let c = catalogue(&groups, |id, d| d.is_adult = id <= 240);
        let mut order: Vec<&str> = tags.iter().map(|[t]| *t).collect();
        order.push("pleats");
        // twelve find nothing in the safe view; pleats, the thirteenth, is not reached
        assert!(related(&c, &order, "q=skirt", &[], None).is_empty());
        assert_eq!(labels(related(&c, &order, "q=skirt&adult=include", &[], None)), ["adult 0", "adult 1", "adult 2", "adult 3"]);
    }

    #[test]
    fn a_tag_translating_the_query_is_not_offered_but_a_narrower_one_is() {
        // スカート translates skirt (expansion.rs): the skirts are titled both ways
        let c = catalogue(&[(12, "スカート", &["スカート"], 4), (10, "Item", &["スカート"], 5), (20, "Skirt", &["ミニスカート"], 5)], |id, d| {
            if id <= 12 {
                d.titles_translated = vec![format!("Skirt {id}")];
            }
        });
        assert_eq!(c.equivalents("skirt").unwrap().to_vec(), ["スカート"]);
        assert_eq!(labels(related(&c, &["スカート", "ミニスカート"], "q=skirt", &[], None)), ["ミニスカート"]);
    }

    #[test]
    fn a_destination_may_find_its_items_through_the_tags_translating_it() {
        // Skirt translates to スカート (expansion.rs); the items with the word skirt are clothing, but 23-32, tagged
        // スカート only, are accessories
        let c = catalogue(&[(12, "スカート", &["スカート", "Skirt"], 4), (8, "Set", &["Skirt"], 4), (10, "Item", &["スカート"], 5)], |id, d| {
            if id <= 12 {
                d.titles_translated = vec![format!("Skirt {id}")];
            }
            if (21..=30).contains(&id) {
                d.booth_category = Some("3D Accessory".into());
            }
        });
        assert_eq!(c.equivalents("Skirt").unwrap().to_vec(), ["スカート"]);
        assert_eq!(labels(related(&c, &["Skirt"], "q=bottoms&bc=3D%20Accessory", &[], None)), ["Skirt"]);
        assert!(related(&c, &["Skirt"], "q=bottoms&bc=3D%20Avatar", &[], None).is_empty());
    }

    #[test]
    fn a_word_of_the_query_is_a_wider_search_not_a_related_one() {
        let c = catalogue(&[(20, "Black Long Hair", &["黒髪"], 5), (20, "Black Long Hair", &["ロング"], 5), (20, "Black Long Hair", &["ponytail"], 5)], |_, _| {});
        assert_eq!(labels(related(&c, &["黒髪", "ロング", "ponytail"], "q=黒髪%20ロング", &[], None)), ["ponytail"]);
    }

    #[test]
    fn a_tag_the_dictionary_translates_as_the_query_is_not_offered() {
        let c = catalogue(&[(20, "Dress", &["Dresses"], 5), (20, "Dress", &["テクスチャ"], 5), (20, "Dress", &["frills"], 5)], |_, _| {});
        let tags = ["Dresses", "テクスチャ", "frills"];
        let dictionary = || TagLabels::new(&[row("Dresses", [Some("dresses"), Some("ドレス"), Some("드레스"), Some("连衣裙")], &[]), row("テクスチャ", [Some("texture"), Some("テクスチャ"), Some("텍스처"), Some("纹理")], &["贴图"])], c.tags());
        // without the dictionary each is offered, as finding the items tagged in another language
        assert_eq!(labels(related(&c, &tags, "q=ドレス", &[], None)), tags);
        // ドレス is what Dresses says in Japanese, and 贴图 a keyword of テクスチャ
        assert_eq!(labels(related_in(&c, &tags, "q=ドレス", &[], None, dictionary(), None)), ["テクスチャ", "frills"]);
        assert_eq!(labels(related_in(&c, &tags, "q=贴图", &[], None, dictionary(), None)), ["Dresses", "frills"]);
    }

    #[test]
    fn a_tag_the_dictionary_translates_as_a_word_of_the_query_is_not_offered() {
        // ロング reads long in English, a word of "Shinano long hair"; ストレートヘア reads straight hair
        let shinano = vec![("shinano".to_owned(), vec!["Shinano"])];
        let c = catalogue(&[(20, "Hair", &["ロング"], 5), (20, "Hair", &["ストレートヘア"], 5)], |_, d| d.avatars = vec!["shinano".into()]);
        let tags = ["ロング", "ストレートヘア"];
        let dictionary = || {
            TagLabels::new(
                &[
                    row("ロング", [Some("Long"), Some("ロング"), Some("롱"), Some("长")], &[]),
                    row("ストレートヘア", [Some("Straight Hair"), Some("ストレートヘア"), Some("스트레이트 헤어"), Some("直发")], &[]),
                ],
                c.tags(),
            )
        };
        // without the dictionary ロング finds the items tagged in another language
        assert_eq!(labels(related(&c, &tags, "q=Shinano%20long%20hair", &shinano, None)), tags);
        for (hl, label) in [(None, "ストレートヘア"), (Some("en"), "Straight Hair"), (Some("ja"), "ストレートヘア"), (Some("zh"), "直发")] {
            assert_eq!(labels(related_in(&c, &tags, "q=Shinano%20long%20hair", &shinano, None, dictionary(), hl)), [label], "{hl:?}");
        }
        // the other way round: long reads ロング in Japanese, a word of "黒髪 ロング"
        let c = catalogue(&[(20, "Black Long Hair", &["long"], 5), (20, "Black Long Hair", &["ponytail"], 5)], |_, _| {});
        let dictionary = TagLabels::new(&[row("long", [Some("long"), Some("ロング"), None, None], &[])], c.tags());
        assert_eq!(labels(related_in(&c, &["long", "ponytail"], "q=黒髪%20ロング", &[], None, dictionary, Some("ja"))), ["ponytail"]);
    }

    #[test]
    fn a_label_reading_in_the_visitors_language_as_the_querys_translation_is_not_offered() {
        // ドレス reads dress in English and 连衣裙 in Chinese; one-piece reads 连衣裙 too, but ワンピース in Japanese
        let c = catalogue(&[(20, "ドレス", &["ドレス"], 5), (20, "One-piece", &["one-piece"], 5), (20, "Frills", &["frills"], 5)], |_, _| {});
        let tags = ["ドレス", "one-piece", "frills"];
        let dictionary = || {
            TagLabels::new(
                &[
                    row("ドレス", [Some("dress"), Some("ドレス"), Some("드레스"), Some("连衣裙")], &[]),
                    row("one-piece", [Some("one-piece dress"), Some("ワンピース"), Some("원피스"), Some("连衣裙")], &[]),
                    row("frills", [Some("frills"), Some("フリル"), Some("프릴"), Some("荷叶边")], &[]),
                ],
                c.tags(),
            )
        };
        for (hl, offered) in [(None, &["one-piece", "frills"][..]), (Some("en"), &["one-piece dress", "frills"]), (Some("ja"), &["ワンピース", "フリル"]), (Some("zh"), &["荷叶边"])] {
            assert_eq!(labels(related_in(&c, &tags, "q=dress", &[], None, dictionary(), hl)), offered, "{hl:?}");
        }
        // dress is a word of "gothic dress", and dresses its plural
        for query in ["q=gothic%20dress", "q=dresses"] {
            assert_eq!(labels(related_in(&c, &tags, query, &[], None, dictionary(), Some("zh"))), ["荷叶边"], "{query}");
        }
        // Dresses reads ドレス in Japanese and dresses in English, the plural of what gown reads there
        let c = catalogue(&[(20, "Dress", &["Dresses"], 5), (20, "Gown", &["gown"], 5), (20, "Frills", &["frills"], 5)], |_, _| {});
        let dictionary = TagLabels::new(&[row("Dresses", [Some("dresses"), Some("ドレス"), None, None], &[]), row("gown", [Some("dress"), Some("ガウン"), None, None], &[])], c.tags());
        assert_eq!(labels(related_in(&c, &["Dresses", "gown", "frills"], "q=ドレス", &[], None, dictionary, Some("en"))), ["frills"]);
        // a keyword translates too: テクスチャ has the keyword 贴图 and reads texture in English, where Textures, without a
        // row, is labelled with its own spelling
        let c = catalogue(&[(20, "テクスチャ", &["テクスチャ"], 5), (20, "Textures", &["Textures"], 5), (20, "Frills", &["frills"], 5)], |_, _| {});
        let dictionary = || TagLabels::new(&[row("テクスチャ", [Some("texture"), Some("テクスチャ"), None, Some("纹理")], &["贴图"])], c.tags());
        let tags = ["テクスチャ", "Textures", "frills"];
        assert_eq!(labels(related_in(&c, &tags, "q=贴图", &[], None, dictionary(), Some("en"))), ["frills"]);
        assert_eq!(labels(related_in(&c, &tags, "q=贴图", &[], None, dictionary(), None)), ["Textures", "frills"]);
    }

    #[test]
    fn a_search_reads_in_the_visitors_language_and_searches_its_label_when_that_finds_items() {
        // the text search knows frills as 荷叶边 (a dictionary word of the catalogue) but not pleats as 百褶
        let words = crate::tag_words::TagWords::new([("frills".to_owned(), vec!["frills".to_owned(), "荷叶边".to_owned()])]);
        let groups: &[(usize, &str, &[&str], usize)] = &[(20, "Skirt", &["frills"], 5), (20, "Skirt", &["pleats"], 5), (20, "Skirt", &["pleated"], 5), (20, "Skirt", &["lace"], 5)];
        let c = catalogue_with_words(groups, |_, _| {}, &words);
        let tags = ["frills", "pleats", "pleated", "lace"];
        let dictionary = || {
            TagLabels::new(
                &[
                    row("frills", [Some("frills"), Some("フリル"), Some("프릴"), Some("荷叶边")], &[]),
                    row("pleats", [Some("pleats"), Some("プリーツ"), Some("플리츠"), Some("百褶")], &[]),
                    row("pleated", [Some("pleated"), None, None, Some("百褶")], &[]),
                ],
                c.tags(),
            )
        };
        let pair = |label: &str, q: &str| (label.to_owned(), q.to_owned());
        // the label in Chinese, its words as the destination only when they find something, and no row keeps the tag;
        // pleated reads 百褶 as pleats does, so it is the same search
        assert_eq!(related_in(&c, &tags, "q=skirt", &[], None, dictionary(), Some("zh")), [pair("荷叶边", "荷叶边"), pair("百褶", "pleats"), pair("lace", "lace")]);
        // a language without a value keeps the tag, as does a request that names no language
        assert_eq!(related_in(&c, &tags, "q=skirt", &[], None, dictionary(), Some("ko"))[2], pair("pleated", "pleated"));
        assert_eq!(related_in(&c, &tags, "q=skirt", &[], None, dictionary(), None)[0], pair("frills", "frills"));
        // a model named in the query stays in the destination
        let shinano = vec![("shinano".to_owned(), vec!["Shinano"])];
        let c = catalogue_with_words(groups, |_, d| d.avatars = vec!["shinano".into()], &words);
        assert_eq!(related_in(&c, &tags, "q=Shinano%20skirt", &shinano, None, dictionary(), Some("zh"))[0], pair("荷叶边", "Shinano 荷叶边"));
    }

    #[test]
    fn a_label_of_one_character_is_shown_but_the_tag_is_searched() {
        // ロング reads 长 in Chinese and ストレートヘア 直发, and the text search knows both as words of their tags
        let words = crate::tag_words::TagWords::new([("ロング".to_owned(), vec!["长".to_owned()]), ("ストレートヘア".to_owned(), vec!["直发".to_owned()])]);
        let shinano = vec![("shinano".to_owned(), vec!["Shinano"])];
        let c = catalogue_with_words(&[(20, "Hair", &["ロング"], 5), (20, "Hair", &["ストレートヘア"], 5)], |_, d| d.avatars = vec!["shinano".into()], &words);
        let dictionary = TagLabels::new(
            &[row("ロング", [Some("Long"), Some("ロング"), Some("롱"), Some("长")], &[]), row("ストレートヘア", [Some("Straight Hair"), None, None, Some("直发")], &[])],
            c.tags(),
        );
        let pair = |label: &str, q: &str| (label.to_owned(), q.to_owned());
        assert_eq!(
            related_in(&c, &["ロング", "ストレートヘア"], "q=Shinano%20黒髪", &shinano, None, dictionary, Some("zh")),
            [pair("长", "Shinano ロング"), pair("直发", "Shinano 直发")]
        );
    }

    #[test]
    fn a_search_a_nearer_one_already_makes_is_left_out_and_the_next_takes_its_place() {
        // ウェーブヘア reads wavy hair in English; the tag wavy hair reads ウェーブヘア and 波浪发 as ウェーブヘア does. Their
        // items are titled differently, so the title rule keeps both
        let shinano = vec![("shinano".to_owned(), vec!["Shinano"])];
        let groups: &[(usize, &str, &[&str], usize)] = &[
            (20, "Straight", &["ストレートヘア"], 5),
            (20, "ウェーブヘア", &["ウェーブヘア"], 5),
            (20, "Wavy Hair", &["wavy hair"], 5),
            (20, "Ponytail", &["ponytail"], 5),
            (20, "Twin Tails", &["twin tails"], 5),
        ];
        let c = catalogue(groups, |_, d| d.avatars = vec!["shinano".into()]);
        let tags = ["ストレートヘア", "ウェーブヘア", "wavy hair", "ponytail", "twin tails"];
        let rows = |wavy: bool| {
            let mut rows = vec![
                row("ストレートヘア", [Some("Straight Hair"), Some("ストレートヘア"), None, Some("直发")], &[]),
                row("ウェーブヘア", [Some("wavy hair"), Some("ウェーブヘア"), Some("웨이브헤어"), Some("波浪发")], &[]),
            ];
            if wavy {
                rows.push(row("wavy hair", [Some("wavy hair"), Some("ウェーブヘア"), Some("웨이브 헤어"), Some("波浪发")], &[]));
            }
            rows
        };
        let query = "q=Shinano%20long%20hair";
        assert_eq!(labels(related(&c, &tags, query, &shinano, None)), ["ストレートヘア", "ウェーブヘア", "wavy hair", "ponytail"]);
        // wavy hair is ウェーブヘア's search in every language, with a row of its own or without one, and twin tails
        // takes its place
        for wavy in [true, false] {
            for (hl, first) in [(None, ["ストレートヘア", "ウェーブヘア"]), (Some("en"), ["Straight Hair", "wavy hair"]), (Some("ja"), ["ストレートヘア", "ウェーブヘア"]), (Some("zh"), ["直发", "波浪发"])] {
                let dictionary = TagLabels::new(&rows(wavy), c.tags());
                assert_eq!(labels(related_in(&c, &tags, query, &shinano, None, dictionary, hl)), [first[0], first[1], "ponytail", "twin tails"], "{hl:?} {wavy}");
            }
        }
        // pleats reads 百褶 in Chinese, the tag of the search before it
        let c = catalogue(&[(20, "Skirt", &["百褶"], 5), (20, "Skirt", &["pleats"], 5)], |_, _| {});
        let dictionary = TagLabels::new(
            &[row("百褶", [Some("pleated"), None, None, Some("百褶裙")], &[]), row("pleats", [Some("pleats"), None, None, Some("百褶")], &[])],
            c.tags(),
        );
        assert_eq!(related_in(&c, &["百褶", "pleats"], "q=skirt", &[], None, dictionary, Some("zh")), [("百褶裙".to_owned(), "百褶".to_owned())]);
        // a search left out as the same one takes none of the twelve checks against the catalogue: after wavy hair, ten
        // tags find nothing in the safe view, and the twelfth check still reaches twin tails
        let names: Vec<[String; 1]> = (0..10).map(|i| [format!("adult {i}")]).collect();
        let adult: Vec<[&str; 1]> = names.iter().map(|[n]| [n.as_str()]).collect();
        let mut groups: Vec<(usize, &str, &[&str], usize)> = vec![(20, "ウェーブヘア", &["ウェーブヘア"], 5), (20, "Wavy Hair", &["wavy hair"], 5)];
        groups.extend(adult.iter().map(|t| (20, "Hair", &t[..], 5)));
        groups.push((20, "Twin Tails", &["twin tails"], 5));
        let c = catalogue(&groups, |id, d| d.is_adult = (41..=240).contains(&id));
        let order: Vec<&str> = ["ウェーブヘア", "wavy hair"].into_iter().chain(adult.iter().map(|[t]| *t)).chain(["twin tails"]).collect();
        assert_eq!(labels(related_in(&c, &order, "q=long%20hair", &[], None, TagLabels::new(&rows(false), c.tags()), None)), ["ウェーブヘア", "twin tails"]);
    }

    #[test]
    fn the_labels_keep_only_the_tags_a_related_search_can_be() {
        // rare is on five items, too few to be offered
        let c = catalogue(&[(20, "Skirt", &["pleats"], 5), (5, "Skirt", &["rare"], 5)], |_, _| {});
        let labels = TagLabels::new(&[row("pleats", [Some("pleats"), None, None, Some("百褶")], &[]), row("rare", [Some("rare"), None, None, Some("稀有")], &[])], c.tags());
        assert_eq!((labels.label("pleats", 3), labels.label("rare", 3)), (Some("百褶"), None));
    }

    #[test]
    fn a_plural_of_a_word_ending_in_e_repeats_the_query_or_a_nearer_search() {
        // Eyes is the query's word, and Eye Textures the search eye texture makes; no title says texture, so the title
        // rule keeps every tag
        let groups: &[(usize, &str, &[&str], usize)] = &[
            (20, "Eye Set", &["eye texture set"], 5),
            (20, "Eye Iris", &["eye texture"], 5),
            (20, "Eye Pupil", &["Eye Textures"], 5),
            (20, "Eye Highlight", &["Eyes"], 5),
            (20, "Eye Lash", &["pupil"], 5),
        ];
        let c = catalogue(groups, |_, _| {});
        let tags = ["eye texture set", "eye texture", "Eye Textures", "Eyes", "pupil"];
        assert_eq!(labels(related(&c, &tags, "q=eye", &[], None)), ["eye texture set", "eye texture", "pupil"]);
        // Hoodies is the plural of the query's word, so it is not offered in any language (パーカー in Japanese)
        let c = catalogue(&[(20, "Hoodie", &["Hoodies"], 5), (20, "Hoodie", &["zip"], 5)], |_, _| {});
        let dictionary = TagLabels::new(&[row("Hoodies", [Some("hoodies"), Some("パーカー"), Some("후드티"), Some("连帽衫")], &[])], c.tags());
        assert_eq!(labels(related_in(&c, &["Hoodies", "zip"], "q=hoodie", &[], None, dictionary, Some("ja"))), ["zip"]);
    }

    #[test]
    fn respellings_are_the_same_word_but_containment_is_not() {
        let plurals = [("skirt", "Skirts"), ("gothic girl", "gothic girls"), ("dress", "Dresses"), ("eye", "Eyes"), ("eye texture", "Eye Textures"), ("hoodie", "Hoodies"), ("shoe", "shoes")];
        for (a, b) in plurals.into_iter().chain([("short hair", "ShortHair"), ("ドレス", "ﾄﾞﾚｽ")]) {
            assert!(respelling(a, b) && respelling(b, a), "{a} / {b}");
        }
        for (a, b) in [("スカート", "ミニスカート"), ("skirt", "long skirt"), ("hair", "chair")] {
            assert!(!respelling(a, b), "{a} / {b}");
        }
    }

    #[test]
    fn a_tag_names_an_avatar_by_a_cjk_name_or_a_whole_latin_name() {
        let names = AvatarNames::new(["Kikyo", "Shinano", "Alué", "桔梗", "io", "猫", "Rusk Dragon"]);
        for tag in [
            "kikyo", "kikyo (桔梗)", "桔梗対応", "io", "猫", "rusk dragon", "kikyo-dress",
            "Shinano対応", "対応Ｓｈｉｎａｎｏ", "シナノShinano", "Shinano호환", "for Rusk Dragon only", "Alué用",
        ] {
            assert!(names.named_in(&fold(tag)), "{tag}");
        }
        // Short names only as the whole tag; longer Latin names never inside another word.
        for tag in ["io hair", "猫耳", "kikyodress", "rusk", "unshinano対応", "Shinano2対応", "for Rusk Dragonfly", "brusk dragon", "éShinano", "Aluéx"] {
            assert!(!names.named_in(&fold(tag)), "{tag}");
        }
    }

    #[test]
    fn a_centroid_is_the_mean_direction_of_enough_embedded_items() {
        // 1-20 tagged a, 21-40 tagged b; 41 is re-uploaded and tagged a too
        let c = catalogue(&[(20, "A", &["a"], 5), (20, "B", &["b"], 5), (1, "Old", &["a"], 1)], |id, d| d.relisted = id == 41);
        let mut sums = Sums::new(&c, [1, 2, 3, 4, 5, 21, 22, 23, 24, 41, 999]);
        let scaled = |i, by: f32| unit(i).into_iter().map(move |x| x * by);
        // a: three items one way and two another, whatever their length; b: four, one too few; 41 and an unknown id
        // count for nothing
        for id in [1, 2, 3] {
            sums.add(id, scaled(1, 2.0));
        }
        for id in [4, 5] {
            sums.add(id, scaled(2, 5.0));
        }
        for id in [21, 22, 23, 24] {
            sums.add(id, scaled(1, 1.0));
        }
        sums.add(41, scaled(3, 1.0));
        sums.add(999, scaled(3, 1.0));
        let t = sums.finish("m".into(), chrono::Utc::now());
        assert_eq!(t.keys, ["a"]);
        let (x, y) = (3.0 / 13f32.sqrt(), 2.0 / 13f32.sqrt());
        assert!((t.vectors[1] - x).abs() < 1e-6 && (t.vectors[2] - y).abs() < 1e-6 && t.vectors[3] == 0.0);
        assert_eq!(t.vectors.len(), DIMS);
    }

    #[test]
    fn centroids_rank_by_cosine_and_then_by_tag() {
        let t = TagCentroids { model: "m".into(), built: chrono::Utc::now(), keys: vec!["c".into(), "b".into(), "a".into()], vectors: [unit(2), unit(1), unit(2)].concat() };
        let query: Vec<f32> = unit(2).iter().map(|x| x * 3.0).collect();
        assert_eq!(t.ranked(&query).collect::<Vec<_>>(), ["a", "c", "b"]);
    }

    /// A snapshot searches the tag dictionary's words and labels related searches with its values.
    #[sqlx::test(migrations = "../core/migrations")]
    #[ignore = "requires PostgreSQL with pgvector"]
    async fn a_snapshot_reads_the_tag_dictionary(pool: PgPool) {
        // 20 items carry the tag, enough to be offered as a related search
        sqlx::raw_sql(
            "insert into items (id, title, author_name, author_uri, is_adult, status, scraped_at, booth_tags)
             select i, 'Item ' || i, 'creator', 'https://example.test', false, 'ok', now(), '{テクスチャ}' from generate_series(1, 20) i;
             insert into booth_tag_translations (tag, en, ja, ko, zh, keywords, source) values
             ('テクスチャ', 'texture', 'テクスチャ', null, '纹理', '{}', 'import');",
        )
        .execute(&pool)
        .await
        .unwrap();
        let snap = crate::snapshot::build_snapshot(&pool, 1, None, None).await.unwrap();
        let texture = SearchRequest::from(&Params::parse("q=texture"));
        assert_eq!(snap.catalog.search(&texture, 0, None, None, None, false).unwrap().count, 20);
        assert_eq!(snap.related.labels.label("テクスチャ", 3), Some("纹理"));
    }

    /// Centroids carry over to the next snapshot while they are recent and from the model of the item embeddings.
    #[sqlx::test(migrations = "../core/migrations")]
    #[ignore = "requires PostgreSQL with pgvector"]
    async fn centroids_carry_over_until_the_model_changes_or_they_age(pool: PgPool) {
        let c = catalogue(&[(20, "A", &["a"], 5)], |_, _| {});
        assert!(TagCentroids::current(&pool, &c, None).await.unwrap().is_none(), "no item embedded yet");
        sqlx::query("insert into app_settings (key, value) values ('embed_state', '{\"model\": \"m\"}')").execute(&pool).await.unwrap();
        let empty = TagCentroids::current(&pool, &c, None).await.unwrap();
        assert!(empty.is_none(), "do not cache an empty set for the new model");
        sqlx::query("insert into items (id, title, author_name, author_uri, is_adult, status, scraped_at, embedding)
                     select i, 'A', 's', 'u', false, 'ok', now(), $1::halfvec from generate_series(1, 5) i")
            .bind(HalfVector::from_f32_slice(&unit(0))).execute(&pool).await.unwrap();
        let mut built = TagCentroids::current(&pool, &c, empty).await.unwrap().unwrap();
        assert_eq!(built.keys, ["a"], "first embeddings reach the next snapshot");
        assert!(Arc::ptr_eq(&built, &TagCentroids::current(&pool, &c, Some(built.clone())).await.unwrap().unwrap()));
        Arc::get_mut(&mut built).unwrap().built -= REBUILD_AFTER;
        let fresh = TagCentroids::current(&pool, &c, Some(built.clone())).await.unwrap().unwrap();
        assert!(!Arc::ptr_eq(&built, &fresh));
        sqlx::query("update app_settings set value = '{\"model\": \"n\"}' where key = 'embed_state'").execute(&pool).await.unwrap();
        assert_eq!(TagCentroids::current(&pool, &c, Some(fresh)).await.unwrap().unwrap().model, "n");
    }
}
