//! `/api/search`: keyword and facet search over the snapshot, fused with semantic neighbours from pgvector.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Instant;

use axum::Json;
use axum::extract::{RawQuery, State};
use axum::response::{IntoResponse, Response};

use crate::error::Refusal;
use crate::params::Params;
use crate::search_request::{SearchRequest, View, query_without, without_groups};
use crate::{ApiError, AppState, account, catalog, db, limit};

pub(crate) const EMBED_CACHE_CAP: u64 = 2000;
pub(crate) type EmbedWatch = tokio::sync::watch::Receiver<Embedding>;

/// Where a query embedding's provider request stands; the searches waiting for it watch this.
#[derive(Clone)]
pub(crate) enum Embedding {
    /// waiting for one of the provider slots (`embed_permits`)
    Queued,
    /// the provider has the request
    Asking,
    /// the vector, or none: the request failed or no slot came free in time
    Done(Option<Vec<f32>>),
}

/// The `/api/search` response. The site's `SearchResult` type is generated from this (see `api_types`).
#[derive(serde::Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub(crate) struct SearchResponse {
    count: u64,
    page: usize,
    total_pages: usize,
    page_size: usize,
    sort: &'static str,
    dir: &'static str,
    booth_counts: BTreeMap<String, u64>,
    smart_counts: BTreeMap<String, u64>,
    feature_counts: BTreeMap<String, u64>,
    items: Vec<db::ItemResponse>,
    semantic: SemanticStatus,
    /// what the search read the query's words as: the site marks those tokens
    recognized: Vec<Recognized>,
    trace: Vec<TraceStep>,
    timing: Timing,
    /// for a search that finds nothing, asked with `without=`: what it finds without each group, in order
    wider_counts: Vec<u64>,
    /// the query's words matched nothing as typed, so its word matches allow one typo per long word; or a word was
    /// read as the model whose name it misspells
    typo_tolerant: bool,
    /// a word was read as the model whose name it misspells; `typo=0` asks for the words as typed
    name_typo: bool,
    /// BOOTH tags that translate the query, searched too, when items only they found are among the results;
    /// `xl=0` asks for the words alone
    expanded_with: Vec<String>,
}

/// Query words the search took as a model (matching what fits it) or a smart category (matching what is in it):
/// the models in query order, then the category.
#[derive(serde::Serialize, Debug, PartialEq)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub(crate) struct Recognized {
    kind: RecognizedKind,
    slug: String,
    /// as typed
    words: Vec<String>,
    /// the words misspell the model's name, which no item has as a word
    typo: bool,
}

#[derive(serde::Serialize, Debug, PartialEq)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "lowercase")]
pub(crate) enum RecognizedKind {
    Model,
    Category,
}

/// `models` and `typos`: as `split_names` returns them; `category`: the smart category `meaning` names as a whole,
/// all its words.
fn recognized(models: &[(String, Vec<&str>)], typos: &[&str], category: Option<&String>, meaning: &str) -> Vec<Recognized> {
    let entry = |kind, slug: &String, words: Vec<&str>, typo| Recognized { kind, slug: slug.clone(), words: words.into_iter().map(str::to_owned).collect(), typo };
    let models = models.iter().map(|(slug, words)| entry(RecognizedKind::Model, slug, words.clone(), words.iter().all(|w| typos.contains(w))));
    models.chain(category.map(|c| entry(RecognizedKind::Category, c, meaning.split_whitespace().collect(), false))).collect()
}

/// Whether meaning-based matches were used for this search.
#[derive(serde::Serialize, Clone, Copy)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "lowercase")]
pub(crate) enum SemanticStatus {
    Used,
    /// no embeddings model, or no text query
    Off,
    /// the provider failed, an embedding budget refused the request, or no provider slot came free in time
    Unavailable,
    /// the provider is still working on the query's embedding after the search's wait; the request runs on and fills
    /// the caches, so the same search a moment later uses it
    Pending,
    /// not needed (sem=0 from a later page, the query only names models, or it names an item by its link or number)
    Skipped,
}

#[derive(serde::Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub(crate) struct TraceStep {
    step: &'static str,
    ms: f64,
}

#[derive(serde::Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub(crate) struct Timing {
    // Catalogue CPU work only; embedding, permit wait and item hydration are separate trace steps.
    search_ms: f64,
    total_ms: f64,
}

/// Per-request timing, returned as `trace` so the site can show where the time went.
#[derive(Default)]
pub(crate) struct Trace {
    steps: Vec<TraceStep>,
    last: Option<Instant>,
}

impl Trace {
    fn start(&mut self) {
        self.last = Some(Instant::now());
    }
    /// Records the time since `start` or the previous step.
    fn step(&mut self, name: &'static str) {
        let now = Instant::now();
        let ms = self.last.map_or(0.0, |t| now.duration_since(t).as_secs_f64() * 1e3);
        self.steps.push(TraceStep { step: name, ms: (ms * 10.0).round() / 10.0 });
        self.last = Some(now);
    }
}

pub(crate) enum EmbedPlan {
    Vector(Vec<f32>),
    Off,
    Skipped,
    Unavailable,
    Pending,
}

pub(crate) async fn search(
    State(s): State<Arc<AppState>>,
    limit::ClientIp(ip): limit::ClientIp,
    tier: limit::Tier,
    headers: axum::http::HeaderMap,
    RawQuery(q): RawQuery,
) -> Result<Response, ApiError> {
    let raw = q.as_deref().unwrap_or("");
    let p = Params::parse(raw);
    let req = SearchRequest::from(&p);
    // without a pass the page's records count too, as many as asked for, before any work for them
    s.limits.search.check_records(ip, &tier, Some((&s.limits.records, req.page_size))).await?;
    let t0 = Instant::now();
    let mut trace = Trace::default();
    trace.start();
    // one snapshot for the whole request, so a reload during it cannot mix two catalogues
    let snap = s.snap.load_full();
    let favourites = if matches!(req.view, View::Favourites) {
        let u = account::current_user(&s.pool, &headers).await?.ok_or(Refusal::SignInRequired)?;
        let f = snap.catalog.docs_for(&account::favourite_ids(&s, u.id).await?);
        trace.step("favourites");
        Some(f)
    } else {
        None
    };
    // page 1 decides whether meaning-based matches are used; the site sends sem=0 for later pages when they
    // weren't, so the result set can't change between pages if the provider answers late
    // model names in the query become a requirement for meaning-based matches and are left out of the
    // embedding, so "shinano short hair" adds short hair that fits Shinano, not other Shinano items
    let q = req.q.as_deref().unwrap_or_default();
    // a BOOTH item link, or the number of an item, names that item (catalog.rs): it is neither embedded nor searched
    // as the tags that translate it
    let names_item = snap.catalog.referenced(q).is_some();
    // typo=0 (the site's "search instead for"): every word counts as known, so none is read as a misspelt model
    let typos = p.get("typo").as_deref() != Some("0");
    let (meaning, named, misspelt) = snap.avatars.split_names(q, |w| !typos || snap.catalog.knows_word(w));
    let name_typo = !misspelt.is_empty();
    let models: Vec<String> = named.iter().map(|(slug, _)| slug.clone()).collect();
    trace.step("prepare");
    let embed_plan = match req.q {
        Some(_) if p.get("sem").as_deref() == Some("0") => EmbedPlan::Skipped,
        Some(_) if meaning.is_empty() || names_item => EmbedPlan::Skipped,
        Some(_) => semantic_vector(s.clone(), ip, &meaning, &mut trace).await,
        None => EmbedPlan::Off,
    };
    // acquired after any external provider wait so slow embeddings cannot block keyword/filter searches
    let _permit = s.acquire_search().await?;
    trace.step("queue");
    let (semantic, semantic_status) = match embed_plan {
        EmbedPlan::Vector(vector) => {
            let ids = nearest(&s, &vector, &models).await;
            trace.step("neighbours");
            match ids {
                Some(ids) => (Some(ids), SemanticStatus::Used),
                None => (None, SemanticStatus::Unavailable),
            }
        },
        EmbedPlan::Off => (None, SemanticStatus::Off),
        EmbedPlan::Skipped => (None, SemanticStatus::Skipped),
        EmbedPlan::Unavailable => (None, SemanticStatus::Unavailable),
        EmbedPlan::Pending => (None, SemanticStatus::Pending),
    };
    // a query that names a category ("short hair", "shorthair") also matches the items enriched into it, so
    // spellings of one meaning end up with the same results
    let categories: Vec<String> =
        snap.category_phrases.get(&crate::text::fold(&meaning.split_whitespace().collect::<String>())).cloned().into_iter().collect();
    let recognized = recognized(&named, &misspelt, categories.first(), &meaning);
    let catalog_now = chrono::Utc::now().timestamp();
    let semantic = (semantic.is_some() || !categories.is_empty() || !models.is_empty())
        .then(|| catalog::Semantic { ids: semantic.as_deref().unwrap_or_default(), categories: &categories, models: &models, rest: &meaning });
    // xl=0 (the site's "without translated matches"): the words are not also searched as the tags that translate them
    let expand = (!names_item && p.get("xl").as_deref() != Some("0")).then_some(meaning.as_str());
    let search_start = Instant::now();
    let (r, wider_counts) =
        tokio::task::block_in_place(|| search_and_wider(&snap.catalog, raw, &req, catalog_now, favourites.as_ref(), semantic, expand))?;
    let search_ms = search_start.elapsed().as_secs_f64() * 1e3;
    trace.step("search");
    let items = db::load_items(&s.pool, &r.ids, p.get("fields").as_deref() == Some("card")).await?;
    trace.step("items");
    Ok(Json(SearchResponse {
        count: r.count,
        page: r.page,
        total_pages: r.total_pages,
        page_size: r.page_size,
        sort: r.sort,
        dir: r.dir,
        booth_counts: r.facets.booth,
        smart_counts: r.facets.smart,
        feature_counts: r.facets.features,
        items,
        semantic: semantic_status,
        recognized,
        trace: trace.steps,
        timing: Timing { search_ms, total_ms: t0.elapsed().as_secs_f64() * 1e3 },
        wider_counts,
        typo_tolerant: r.typo || name_typo,
        name_typo,
        expanded_with: r.expanded_with,
    })
    .into_response())
}

/// The search, and for one that finds nothing, what it finds without each `without=` group of `raw`: from this
/// request's own text and meaning-based matches, so the site learns which one clause to drop at no extra request.
pub(crate) fn search_and_wider(
    catalog: &catalog::Catalog,
    raw: &str,
    req: &SearchRequest,
    now: i64,
    favourites: Option<&roaring::RoaringBitmap>,
    semantic: Option<catalog::Semantic>,
    expand: Option<&str>,
) -> anyhow::Result<(catalog::SearchResult, Vec<u64>)> {
    let r = catalog.search(req, now, favourites, semantic, expand, true)?;
    if r.count > 0 {
        return Ok((r, Vec::new()));
    }
    let wider: Vec<SearchRequest> = without_groups(raw).iter().map(|g| SearchRequest::from(&Params::parse(&query_without(raw, g)))).collect();
    let counts = if wider.is_empty() { Vec::new() } else { catalog.counts(req, &wider, now, favourites, semantic, expand)? };
    Ok((r, counts))
}

/// Resolves the query embedding (or why there isn't one) before acquiring a search execution permit.
pub(crate) async fn semantic_vector(s: Arc<AppState>, ip: std::net::IpAddr, q: &str, trace: &mut Trace) -> EmbedPlan {
    let config = s.ai_config().await;
    trace.step("settings");
    let Ok(c) = config else { return EmbedPlan::Unavailable };
    let (Some(base_url), Some(api_key), Some(model)) = (c.base_url.clone(), c.api_key.clone(), c.embed_model.clone()) else { return EmbedPlan::Off };
    query_embedding(s, ip, q, EmbedProvider { base_url, api_key, model }, trace).await
}

/// The search text the embedding is cached under: case and spacing don't change the meaning.
pub(crate) fn normalize_query(q: &str) -> String {
    // a NUL breaks words like a space: query_embeddings cannot store it
    q.split(|c: char| c.is_whitespace() || c == '\0').filter(|w| !w.is_empty()).collect::<Vec<_>>().join(" ").to_lowercase()
}

pub(crate) struct ClearInflight {
    state: Arc<AppState>,
    key: String,
}

impl Drop for ClearInflight {
    fn drop(&mut self) {
        self.state.embed_inflight.lock().unwrap().remove(&self.key);
    }
}

/// The provider an embedding request goes to. It has no `Debug` impl so the key cannot reach a log.
pub(crate) struct EmbedProvider {
    pub(crate) base_url: String,
    pub(crate) api_key: String,
    pub(crate) model: String,
}

fn embed_cache_key(model: &str, text: &str) -> String {
    format!("{model}\n{text}")
}

async fn stored_embedding(pool: &sqlx::PgPool, model: &str, text: &str) -> Option<Vec<f32>> {
    sqlx::query_scalar("select embedding::vector::real[] from query_embeddings where model = $1 and query = $2")
        .bind(model)
        .bind(text)
        .fetch_optional(pool)
        .await
        .map_err(|e| eprintln!("query embedding cache: {e}"))
        .ok()
        .flatten()
}

/// The embedding a search stored for `q` (process memory, then the query_embeddings table), read only: it counts no
/// use and never asks the provider.
pub(crate) async fn stored_query_embedding(s: &AppState, model: &str, q: &str) -> Option<Vec<f32>> {
    let text = normalize_query(q);
    match s.embed_cache.get(&embed_cache_key(model, &text)) {
        Some(v) => Some(v),
        None => stored_embedding(&s.pool, model, &text).await,
    }
}

/// Embedding of a search text: process memory, then the query_embeddings table, then the provider. `Vector`,
/// `Unavailable`, or `Pending` when the provider request outlasts the wait.
pub(crate) async fn query_embedding(
    s: Arc<AppState>,
    ip: std::net::IpAddr,
    q: &str,
    provider: EmbedProvider,
    trace: &mut Trace,
) -> EmbedPlan {
    use boothdex_core::embed;
    let EmbedProvider { base_url: base, api_key: key, model } = provider;
    let text = normalize_query(q);
    let cache_key = embed_cache_key(&model, &text);
    if let Some(v) = s.embed_cache.get(&cache_key) {
        trace.step("embed_memory");
        return EmbedPlan::Vector(v);
    }
    if let Some(v) = stored_embedding(&s.pool, &model, &text).await {
        trace.step("embed_db");
        s.embed_cache.insert(cache_key, v.clone());
        // usage bookkeeping for pruning; the search doesn't wait for it
        let pool = s.pool.clone();
        tokio::spawn(async move {
            let _ = sqlx::query("update query_embeddings set hits = hits + 1, last_used_at = now() where model = $1 and query = $2")
                .bind(&model)
                .bind(&text)
                .execute(&pool)
                .await;
        });
        return EmbedPlan::Vector(v);
    }
    let mut rx = {
        let mut inflight = s.embed_inflight.lock().unwrap();
        if let Some(existing) = inflight.get(&cache_key) {
            existing.clone()
        } else if let Some(v) = s.embed_cache.get(&cache_key) {
            trace.step("embed_memory");
            return EmbedPlan::Vector(v);
        } else if s.limits.embed_ip.take(ip).is_err() || s.limits.embed_global.take_global().is_err() {
            // per-IP checked first so one noisy IP never drains the global uncached budget; search falls back to words
            trace.step("embed_failed");
            return EmbedPlan::Unavailable;
        } else {
            let (tx, rx) = tokio::sync::watch::channel(Embedding::Queued);
            inflight.insert(cache_key.clone(), rx.clone());
            let input = vec![format!("{}{text}", embed::QUERY_PREFIX)];
            // the request runs on: if it outlasts the wait below, it still fills both caches for the next page or visitor
            let state = s.clone();
            let key_owned = cache_key.clone();
            tokio::spawn(async move {
                let _guard = ClearInflight { state: state.clone(), key: key_owned.clone() };
                // abandon before calling the provider if all 4 slots stay busy past the search timeout
                let Ok(Ok(_permit)) = tokio::time::timeout(std::time::Duration::from_millis(2500), state.embed_permits.acquire()).await else {
                    let _ = tx.send(Embedding::Done(None));
                    return;
                };
                let _ = tx.send(Embedding::Asking);
                match embed::embed(&state.http, &base, &key, &model, &input, 1)
                    .await
                    .and_then(|mut v| v.pop().ok_or_else(|| anyhow::anyhow!("no embedding")))
                {
                    Ok(v) => {
                        state.embed_cache.insert(key_owned, v.clone());
                        let _ = tx.send(Embedding::Done(Some(v.clone())));
                        if let Err(e) = sqlx::query(
                            "insert into query_embeddings (model, query, embedding) values ($1, $2, $3::halfvec) on conflict (model, query) do nothing",
                        )
                        .bind(&model)
                        .bind(&text)
                        .bind(embed::HalfVector::from_f32_slice(&v))
                        .execute(&state.pool)
                        .await
                        {
                            eprintln!("query embedding store: {e}");
                        }
                    }
                    Err(e) => {
                        eprintln!("query embedding: {e:#}");
                        let _ = tx.send(Embedding::Done(None));
                    }
                }
            });
            rx
        }
    };
    // measured on the development provider: median 0.7 s, 95th percentile 1.2 s, occasional spikes past 3 s
    let done = tokio::time::timeout(std::time::Duration::from_millis(2500), async {
        let done = rx.wait_for(|e| matches!(e, Embedding::Done(_))).await.ok()?;
        match &*done {
            Embedding::Done(v) => v.clone(),
            _ => None,
        }
    })
    .await;
    match done {
        Ok(Some(v)) => {
            trace.step("embed_provider");
            EmbedPlan::Vector(v)
        }
        Ok(None) => {
            trace.step("embed_failed");
            EmbedPlan::Unavailable
        }
        Err(_) => {
            let (step, plan) = plan_on_timeout(&rx.borrow());
            trace.step(step);
            if step == "embed_timeout" {
                eprintln!("query embedding: still waiting after 2.5 s; searching by words only this time");
            }
            plan
        }
    }
}

/// Decides the embed plan when `wait_for` times out: if the embedding finished right as the deadline fired, use it
/// instead of discarding the vector and showing an unavailable note.
fn plan_on_timeout(state: &Embedding) -> (&'static str, EmbedPlan) {
    match state {
        Embedding::Done(Some(v)) => ("embed_provider", EmbedPlan::Vector(v.clone())),
        Embedding::Done(None) => ("embed_failed", EmbedPlan::Unavailable),
        // with the provider, the request fills both caches for the same search a moment later; still waiting for a
        // slot, it gives up at about this time and nothing comes
        Embedding::Asking => ("embed_timeout", EmbedPlan::Pending),
        Embedding::Queued => ("embed_timeout", EmbedPlan::Unavailable),
    }
}

/// Listed items nearest to the vector, cut where they stop resembling the best match. With models named,
/// only items fitting one of them: searched among those directly, as the 200 nearest of the whole catalogue
/// hold only a few that fit.
pub(crate) async fn nearest(s: &AppState, vector: &[f32], models: &[String]) -> Option<Vec<i64>> {
    let mut tx = s.pool.begin().await.ok()?;
    // HNSW returns at most ef_search rows; the default 40 is too few to fill a few pages
    sqlx::query("set local hnsw.ef_search = 200").execute(&mut *tx).await.ok()?;
    if !models.is_empty() {
        // iterative scan: the index keeps going until enough fitting items turn up instead of filtering its
        // first 200 candidates (Shinano: ~50 ms, vs ~300 ms for an exact scan of its 17k items)
        sqlx::query("set local hnsw.iterative_scan = relaxed_order").execute(&mut *tx).await.ok()?;
    }
    let mut rows: Vec<(i64, f64)> = nearest_query(vector, models)
        .fetch_all(&mut *tx)
        .await
        .map_err(|e| eprintln!("semantic search: {e}"))
        .ok()?;
    // relaxed order comes back almost sorted
    rows.sort_by(|a, b| b.1.total_cmp(&a.1));
    let cutoff = semantic_cutoff(rows.first()?.1, !models.is_empty());
    Some(rows.into_iter().take_while(|r| r.1 >= cutoff).map(|r| r.0).collect())
}

pub(crate) fn nearest_query<'a>(vector: &[f32], models: &'a [String]) -> sqlx::query::QueryAs<'a, sqlx::Postgres, (i64, f64), sqlx::postgres::PgArguments> {
    let sql = if models.is_empty() {
        "select id, 1 - (embedding <=> $1::halfvec) from items where embedding is not null and status = 'ok'
         order by embedding <=> $1::halfvec limit 200"
    } else {
        "select id, 1 - (embedding <=> $1::halfvec) from items where embedding is not null and status = 'ok'
           and id in (select item_id from item_avatars where avatar_slug = any($2))
         order by embedding <=> $1::halfvec limit 400"
    };
    let query = sqlx::query_as(sql).bind(boothdex_core::embed::HalfVector::from_f32_slice(vector));
    if models.is_empty() { query } else { query.bind(models) }
}

/// Similarities from Qwen3-Embedding-4B sit in a narrow band (about 0.65-0.82 for the 200 nearest, even for
/// gibberish), so a fixed threshold can't separate matches from noise. Keep neighbours close to the best one,
/// and nothing when even the best is weak.
/// shortcut: calibrated by hand on a few queries for this one model; other models may need other numbers.
/// Among one model's items the margin is wider: sampled on Shinano, "short hair" neighbours down to 0.72 were
/// ~90% hair items, below that hair pins and clothes take over.
pub(crate) fn semantic_cutoff(best: f64, scoped: bool) -> f64 {
    const FLOOR: f64 = 0.72;
    let margin = if scoped { 0.10 } else { 0.06 };
    FLOOR.max(best - margin)
}

#[cfg(test)]
mod recognized_tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn models_then_the_category_with_their_words() {
        let named = vec![("mao".to_owned(), vec!["Mao"]), ("shinano".to_owned(), vec!["shinnano"])];
        let r = serde_json::to_value(recognized(&named, &["shinnano"], Some(&"hair--short".to_owned()), "short hair")).unwrap();
        assert_eq!(
            r,
            json!([
                { "kind": "model", "slug": "mao", "words": ["Mao"], "typo": false },
                { "kind": "model", "slug": "shinano", "words": ["shinnano"], "typo": true },
                { "kind": "category", "slug": "hair--short", "words": ["short", "hair"], "typo": false },
            ])
        );
        assert_eq!(recognized(&[], &[], None, "dress"), vec![]);
    }

    #[test]
    fn timeout_keeps_a_vector_that_landed_at_the_deadline() {
        assert!(matches!(plan_on_timeout(&Embedding::Done(Some(vec![1.0]))), ("embed_provider", EmbedPlan::Vector(v)) if v == [1.0]));
        assert!(matches!(plan_on_timeout(&Embedding::Done(None)), ("embed_failed", EmbedPlan::Unavailable)));
        assert!(matches!(plan_on_timeout(&Embedding::Asking), ("embed_timeout", EmbedPlan::Pending)));
        assert!(matches!(plan_on_timeout(&Embedding::Queued), ("embed_timeout", EmbedPlan::Unavailable)));
    }
}
