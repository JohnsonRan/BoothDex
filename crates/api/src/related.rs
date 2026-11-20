//! `/api/related`: related searches for a text query (owner, 2026-11-20), BOOTH tags near the query's meaning. Each tag
//! has a centroid, the mean embedding of the items that carry it. The tags nearest the embedding a search already
//! stored for the query are offered, minus those that respell or translate the query, name an avatar, are spread by a
//! few shops or by everyone, seldom go with the query's own matches, or find nothing under the current filters. The
//! rules and numbers were tuned with a prototype on the development catalogue (42 sample queries, 2026-11-20).

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
use crate::params::Params;
use crate::search_request::{SearchRequest, View};
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
    /// 212k of them, 435 MB as halfvec). `None` when no item has been embedded.
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
    exact: FxHashSet<String>,
    /// ASCII names of at least 3 letters, found as a whole word
    words: FxHashSet<String>,
    /// other names of at least 2 characters, found anywhere, by their first two characters
    inside: FxHashMap<(char, char), Vec<Vec<char>>>,
}

impl AvatarNames {
    pub(crate) fn new<'n>(names: impl IntoIterator<Item = &'n str>) -> Self {
        let mut out = Self::default();
        for name in names.into_iter().map(fold).filter(|n| !n.is_empty()) {
            let chars: Vec<char> = name.chars().collect();
            if name.is_ascii() && name.len() >= 3 {
                out.words.insert(name.clone());
            } else if !name.is_ascii() && chars.len() >= 2 {
                out.inside.entry((chars[0], chars[1])).or_default().push(chars);
            }
            out.exact.insert(name);
        }
        out
    }

    /// Whether the folded tag names an avatar.
    fn named_in(&self, tag: &str) -> bool {
        let chars: Vec<char> = tag.chars().collect();
        self.exact.contains(tag)
            || tag.split(|c: char| !c.is_alphanumeric()).any(|w| self.words.contains(w))
            || (0..chars.len().saturating_sub(1))
                .any(|i| self.inside.get(&(chars[i], chars[i + 1])).is_some_and(|names| names.iter().any(|n| chars[i..].starts_with(n))))
    }
}

/// The same word spelt again: equal once case, width, spaces and symbols are gone, or but for a plural `s` or `es`
/// (gothic girl / gothic girls). Unlike `expansion::same_word`, one holding the other does not count: ミニスカート is a
/// related search for スカート.
fn respelling(a: &str, b: &str) -> bool {
    let (a, b) = (compact(a), compact(b));
    let singular = |s: &str| s.strip_suffix("es").or_else(|| s.strip_suffix('s')).unwrap_or(s).to_owned();
    a == b || singular(&a) == singular(&b) || singular(&a) == b || a == singular(&b)
}

/// What related searches are chosen for: a search whose query `Lookup::split_names` read as `meaning` and the models
/// `named` with their words.
pub(crate) struct Asked<'a> {
    pub(crate) req: &'a SearchRequest,
    pub(crate) meaning: &'a str,
    pub(crate) named: &'a [(String, Vec<&'a str>)],
    pub(crate) favourites: Option<&'a RoaringBitmap>,
    pub(crate) now: i64,
}

/// Up to `SEARCHES` related searches for `asked`, whose meaning has the embedding `vector`, best first.
pub(crate) fn choose(catalog: &Catalog, avatars: &AvatarNames, centroids: &TagCentroids, vector: &[f32], asked: &Asked) -> Result<Vec<RelatedSearch>> {
    let tags = catalog.tags();
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
    let (mut chosen, mut chosen_titles, mut validated): (Vec<RelatedSearch>, Vec<RoaringBitmap>, usize) = (Vec::new(), Vec::new(), 0);
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
        if equivalents.iter().any(|e| fold(e) == key)
            || avatars.named_in(key)
            || respelling(asked.meaning, name)
            || chosen.iter().any(|c| respelling(&c.label, name))
        {
            continue;
        }
        validated += 1;
        let q = kept.iter().copied().chain([name]).collect::<Vec<_>>().join(" ");
        // the search the chip leads to finds something by its words: an item spelling the tag so that the filters let
        // through is one; else its text matches, with the tags that translate them (the site drops xl=0 with new words)
        let dest = SearchRequest { q: Some(q.clone()), ..asked.req.clone() };
        let finds = |expand| catalog.finds_by_words(&dest, asked.now, asked.favourites, &models, name, expand);
        let spelt = catalog.booth_tags.get(name).is_some_and(|b| !b.is_disjoint(&allowed));
        if !spelt && !finds(None)? && !finds(Some(name))? {
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
        chosen.push(RelatedSearch { label: name.to_owned(), q });
    }
    Ok(chosen)
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
    let asked = Asked { req: &req, meaning: &meaning, named: &named, favourites: favourites.as_ref(), now: chrono::Utc::now().timestamp() };
    let searches = tokio::task::block_in_place(|| choose(&snap.catalog, &snap.related.avatars, centroids, &vector, &asked))?;
    Ok(Json(RelatedResponse { searches }).into_response())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::DocInput;

    /// Groups of docs, each `(count, title, tags, shops)`, then 400 untagged docs, so that a tag on 20 items is not on
    /// 5% of them. Doc ids count from 1 in order.
    fn catalogue(groups: &[(usize, &str, &[&str], usize)], each: impl Fn(i64, &mut DocInput)) -> Catalog {
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
        Catalog::build(docs, 0, None).unwrap()
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
        let req = SearchRequest::from(&Params::parse(query));
        let words: Vec<&str> = named.iter().flat_map(|(_, w)| w.iter().copied()).collect();
        let meaning: String = req.q.as_deref().unwrap().split_whitespace().filter(|w| !words.contains(w)).collect::<Vec<_>>().join(" ");
        let asked = Asked { req: &req, meaning: &meaning, named, favourites, now: 0 };
        let avatars = AvatarNames::new(["Kikyo", "桔梗", "io"]);
        choose(c, &avatars, &centroids(tags), &unit(0), &asked).unwrap().into_iter().map(|s| (s.label, s.q)).collect()
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
    fn respellings_are_the_same_word_but_containment_is_not() {
        for (a, b) in [("skirt", "Skirts"), ("gothic girl", "gothic girls"), ("dress", "Dresses"), ("short hair", "ShortHair"), ("ドレス", "ﾄﾞﾚｽ")] {
            assert!(respelling(a, b) && respelling(b, a), "{a} / {b}");
        }
        for (a, b) in [("スカート", "ミニスカート"), ("skirt", "long skirt"), ("hair", "chair")] {
            assert!(!respelling(a, b), "{a} / {b}");
        }
    }

    #[test]
    fn a_tag_names_an_avatar_by_its_whole_name_an_ascii_word_or_a_longer_name_inside() {
        let names = AvatarNames::new(["Kikyo", "桔梗", "io", "猫", "Rusk Dragon"]);
        for tag in ["kikyo", "kikyo (桔梗)", "桔梗対応", "io", "猫", "rusk dragon", "kikyo-dress"] {
            assert!(names.named_in(&fold(tag)), "{tag}");
        }
        // short names only as the whole tag; an ASCII name only as a word
        for tag in ["io hair", "猫耳", "kikyodress", "rusk"] {
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

    /// Centroids carry over to the next snapshot while they are recent and from the model of the item embeddings.
    #[sqlx::test(migrations = "../core/migrations")]
    #[ignore = "requires PostgreSQL with pgvector"]
    async fn centroids_carry_over_until_the_model_changes_or_they_age(pool: PgPool) {
        let c = catalogue(&[(20, "A", &["a"], 5)], |_, _| {});
        assert!(TagCentroids::current(&pool, &c, None).await.unwrap().is_none(), "no item embedded yet");
        sqlx::query("insert into app_settings (key, value) values ('embed_state', '{\"model\": \"m\"}')").execute(&pool).await.unwrap();
        let built = TagCentroids::current(&pool, &c, None).await.unwrap().unwrap();
        assert_eq!((built.model.as_str(), built.len()), ("m", 0));
        let again = TagCentroids::current(&pool, &c, Some(built.clone())).await.unwrap().unwrap();
        assert!(Arc::ptr_eq(&built, &again));
        let old = Arc::new(TagCentroids { built: chrono::Utc::now() - REBUILD_AFTER, keys: vec![], vectors: vec![], model: "m".into() });
        assert!(!Arc::ptr_eq(&old, &TagCentroids::current(&pool, &c, Some(old.clone())).await.unwrap().unwrap()));
        sqlx::query("update app_settings set value = '{\"model\": \"n\"}' where key = 'embed_state'").execute(&pool).await.unwrap();
        assert_eq!(TagCentroids::current(&pool, &c, Some(built)).await.unwrap().unwrap().model, "n");
    }
}
