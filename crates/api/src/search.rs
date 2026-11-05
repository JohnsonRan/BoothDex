//! `/api/search`: keyword and facet search over the snapshot, fused with semantic neighbours from pgvector.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Instant;

use axum::Json;
use axum::extract::{RawQuery, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;

use crate::params::Params;
use crate::{ApiError, AppState, account, catalog, db, limit};

pub(crate) const EMBED_CACHE_CAP: u64 = 2000;
pub(crate) type EmbedWatch = tokio::sync::watch::Receiver<Option<Option<Vec<f32>>>>;

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
    trace: Vec<TraceStep>,
    timing: Timing,
}

/// Whether meaning-based matches were used for this search.
#[derive(serde::Serialize, Clone, Copy)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "lowercase")]
pub(crate) enum SemanticStatus {
    Used,
    /// no embeddings model, or no text query
    Off,
    /// the provider is slow or failing
    Unavailable,
    /// not needed (sem=0 from a later page, or the query only names models)
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
}

pub(crate) async fn search(
    State(s): State<Arc<AppState>>,
    limit::ClientIp(ip): limit::ClientIp,
    headers: axum::http::HeaderMap,
    RawQuery(q): RawQuery,
) -> Result<Response, ApiError> {
    if let Err(r) = s.limits.search.check(ip) {
        return Ok(r);
    }
    let t0 = Instant::now();
    let mut trace = Trace::default();
    trace.start();
    let p = Params::parse(q.as_deref().unwrap_or(""));
    let favourites = if p.get("v").as_deref() == Some("favourites") {
        let Some(u) = account::current_user(&s.pool, &headers).await? else {
            return Ok((StatusCode::UNAUTHORIZED, Json(json!({ "ok": false, "error": "Sign in required" }))).into_response());
        };
        let f = s.snap().catalog.docs_for(&account::favourite_ids(&s.pool, u.id).await?);
        trace.step("favourites");
        Some(f)
    } else {
        None
    };
    // page 1 decides whether meaning-based matches are used; the site sends sem=0 for later pages when they
    // weren't, so the result set can't change between pages if the provider answers late
    // model names in the query become a requirement for meaning-based matches and are left out of the
    // embedding, so "shinano short hair" adds short hair that fits Shinano, not other Shinano items
    let (meaning, models) = s.snap().avatars.split_names(&p.get("q").unwrap_or_default());
    let embed_plan = match p.get("q").filter(|q| !q.trim().is_empty()) {
        Some(_) if p.get("sem").as_deref() == Some("0") => EmbedPlan::Skipped,
        Some(_) if meaning.is_empty() => EmbedPlan::Skipped,
        Some(_) => semantic_vector(s.clone(), ip, &meaning, &mut trace).await,
        None => EmbedPlan::Off,
    };
    // acquired after any external provider wait so slow embeddings cannot block keyword/filter searches
    let _permit = match s.acquire_search().await {
        Ok(p) => p,
        Err(r) => return Ok(r),
    };
    let (semantic, semantic_status) = match embed_plan {
        EmbedPlan::Vector(vector) => match nearest(&s, &vector, &models).await {
            Some(ids) => {
                trace.step("neighbours");
                (Some(ids), SemanticStatus::Used)
            }
            None => (None, SemanticStatus::Unavailable),
        },
        EmbedPlan::Off => (None, SemanticStatus::Off),
        EmbedPlan::Skipped => (None, SemanticStatus::Skipped),
        EmbedPlan::Unavailable => (None, SemanticStatus::Unavailable),
    };
    // a query that names a category ("short hair", "shorthair") also matches the items enriched into it, so
    // spellings of one meaning end up with the same results
    let categories: Vec<String> =
        s.snap().category_phrases.get(&meaning.split_whitespace().collect::<String>().to_lowercase()).cloned().into_iter().collect();
    let catalog_now = chrono::Utc::now().timestamp();
    let semantic = (semantic.is_some() || !categories.is_empty() || !models.is_empty())
        .then(|| catalog::Semantic { ids: semantic.as_deref().unwrap_or_default(), categories: &categories, models: &models, rest: &meaning });
    let r = tokio::task::block_in_place(|| s.snap().catalog.search(&p, catalog_now, favourites.as_ref(), semantic))?;
    trace.step("search");
    let search_ms = t0.elapsed().as_secs_f64() * 1e3;
    let mut items = db::load_items(&s.pool, &r.ids).await?;
    // the site's result grids ask for card-sized items; other callers keep the full records
    if p.get("fields").as_deref() == Some("card") {
        items = items.into_iter().map(db::ItemResponse::card).collect();
    }
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
        trace: trace.steps,
        timing: Timing { search_ms, total_ms: t0.elapsed().as_secs_f64() * 1e3 },
    })
    .into_response())
}

/// Resolves the query embedding (or why there isn't one) before acquiring a search execution permit.
pub(crate) async fn semantic_vector(s: Arc<AppState>, ip: std::net::IpAddr, q: &str, trace: &mut Trace) -> EmbedPlan {
    let Ok(c) = s.ai_config().await else { return EmbedPlan::Unavailable };
    let (Some(base), Some(key), Some(model)) = (c.base_url.clone(), c.api_key.clone(), c.embed_model.clone()) else { return EmbedPlan::Off };
    trace.step("settings");
    match query_embedding(s, ip, q, base, key, model, trace).await {
        Some(v) => EmbedPlan::Vector(v),
        None => EmbedPlan::Unavailable,
    }
}

/// The search text the embedding is cached under: case and spacing don't change the meaning.
pub(crate) fn normalize_query(q: &str) -> String {
    q.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()
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

/// Embedding of a search text: process memory, then the query_embeddings table, then the provider.
pub(crate) async fn query_embedding(
    s: Arc<AppState>,
    ip: std::net::IpAddr,
    q: &str,
    base: String,
    key: String,
    model: String,
    trace: &mut Trace,
) -> Option<Vec<f32>> {
    use boothdex_core::embed;
    let text = normalize_query(q);
    let cache_key = format!("{model}\n{text}");
    if let Some(v) = s.embed_cache.get(&cache_key) {
        trace.step("embed_memory");
        return Some(v);
    }
    let stored: Option<Vec<f32>> = sqlx::query_scalar("select embedding::vector::real[] from query_embeddings where model = $1 and query = $2")
        .bind(&model)
        .bind(&text)
        .fetch_optional(&s.pool)
        .await
        .map_err(|e| eprintln!("query embedding cache: {e}"))
        .ok()
        .flatten();
    if let Some(v) = stored {
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
        return Some(v);
    }
    let mut rx = {
        let mut inflight = s.embed_inflight.lock().unwrap();
        if let Some(existing) = inflight.get(&cache_key) {
            existing.clone()
        } else if let Some(v) = s.embed_cache.get(&cache_key) {
            trace.step("embed_memory");
            return Some(v);
        } else if s.limits.embed_ip.take(ip).is_err() || s.limits.embed_global.take_global().is_err() {
            // per-IP checked first so one noisy IP never drains the global uncached budget; search falls back to words
            trace.step("embed_failed");
            return None;
        } else {
            let (tx, rx) = tokio::sync::watch::channel(None);
            inflight.insert(cache_key.clone(), rx.clone());
            let input = vec![format!("{}{text}", embed::QUERY_PREFIX)];
            // the request runs on: if it outlasts the wait below, it still fills both caches for the next page or visitor
            let state = s.clone();
            let key_owned = cache_key.clone();
            tokio::spawn(async move {
                let _guard = ClearInflight { state: state.clone(), key: key_owned.clone() };
                // abandon before calling the provider if all 4 slots stay busy past the search timeout
                let Ok(Ok(_permit)) = tokio::time::timeout(std::time::Duration::from_millis(2500), state.embed_permits.acquire()).await else {
                    let _ = tx.send(Some(None));
                    return;
                };
                match embed::embed(&state.http, &base, &key, &model, &input, 1)
                    .await
                    .and_then(|mut v| v.pop().ok_or_else(|| anyhow::anyhow!("no embedding")))
                {
                    Ok(v) => {
                        state.embed_cache.insert(key_owned, v.clone());
                        let _ = tx.send(Some(Some(v.clone())));
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
                        let _ = tx.send(Some(None));
                    }
                }
            });
            rx
        }
    };
    // measured on the development provider: median 0.7 s, 95th percentile 1.2 s, occasional spikes past 3 s
    match tokio::time::timeout(std::time::Duration::from_millis(2500), async {
        rx.wait_for(Option::is_some).await.ok().and_then(|r| r.clone().flatten())
    })
    .await
    {
        Ok(Some(v)) => {
            trace.step("embed_provider");
            Some(v)
        }
        Ok(None) => {
            trace.step("embed_failed");
            None
        }
        Err(_) => {
            trace.step("embed_timeout");
            eprintln!("query embedding: still waiting after 2.5 s; searching by words only this time");
            None
        }
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
