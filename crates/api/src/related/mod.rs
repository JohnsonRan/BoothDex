//! `/api/related`: related searches for a text query (owner, 2026-11-20), BOOTH tags near the query's meaning. Each tag
//! has a centroid, the mean embedding of the items that carry it. The tags nearest the embedding a search already
//! stored for the query are offered, minus those that respell or translate the query or one of its words, read as
//! such a translation in the visitor's language, make the same search as a nearer one, name an avatar, are spread by a
//! few shops or by everyone, seldom go with the query's own matches, or find nothing under the current filters. The
//! rules and numbers were tuned with a prototype on the development catalogue (42 sample queries, 2026-11-20). A chip
//! reads in the visitor's language when the tag dictionary has the tag in it (owner, 2026-11-21).

mod centroids;
mod labels;
#[cfg(test)]
mod labels_tests;
#[cfg(test)]
mod tests;

pub(crate) use centroids::TagCentroids;
pub(crate) use labels::{AvatarNames, TagLabels};

use std::sync::Arc;

use anyhow::Result;
use axum::Json;
use axum::extract::{RawQuery, State};
use axum::response::{IntoResponse, Response};
use boothdex_core::embed::DIMS;
use roaring::RoaringBitmap;
use rustc_hash::FxHashMap;

use self::labels::{forms, respelling};
use crate::catalog::Catalog;
use crate::expansion::{LANGUAGES, MIN_CHARS};
use crate::params::Params;
use crate::search_request::{SearchRequest, View};
use crate::text::{compact, fold};
use crate::{ApiError, AppState, account, limit, search};

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
    let translated = asked.language.map(|k| labels.translations(k, std::iter::once(asked.meaning).chain(words.iter().copied()))).unwrap_or_default();
    let translation = |label: &str| !translated.is_empty() && forms(&compact(label)).any(|form| translated.contains(form));
    let (mut chosen, mut chosen_titles, mut validated): (Vec<(&str, RelatedSearch)>, Vec<RoaringBitmap>, usize) = (Vec::new(), Vec::new(), 0);
    for key in centroids.ranked(vector) {
        if chosen.len() == SEARCHES || validated == VALIDATE {
            break;
        }
        let Some(t) = tags.id(key) else { continue };
        let items = tags.items(t) as f64;
        let rare = |shared: &FxHashMap<u32, u64>| shared.get(&t).copied().unwrap_or(0) as f64 / (matches.len() as f64 * items).sqrt() < MIN_COOCCURRENCE;
        if items > MAX_PER_SHOP * f64::from(tags.shops(t)) || items > BROAD_SHARE * catalog.listed() as f64 || shared.as_ref().is_some_and(rare) {
            continue;
        }
        let name = tags.name(t);
        let repeats = |s: &str| respelling(asked.meaning, s) || words.iter().any(|w| respelling(w, s));
        let label = asked.language.and_then(|k| labels.label(name, k)).unwrap_or(name);
        let same = |(tag, search): &(&str, RelatedSearch)| {
            respelling(&search.label, label)
                || respelling(tag, name)
                || labels.reads_as(tag, |v| respelling(v, name))
                || labels.reads_as(name, |v| respelling(v, tag))
        };
        if equivalents.iter().any(|e| fold(e) == key)
            || labels.reads_as(name, repeats)
            || avatars.named_in(key)
            || (label != name && avatars.named_in(&fold(label)))
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
    let snap = s.snap.load_full();
    if snap.catalog.referenced(q).is_some() {
        return none();
    }
    let typos = p.get("typo").as_deref() != Some("0");
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
    let Some(model) = model.filter(|m| *m == centroids.model()) else { return none() };
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
