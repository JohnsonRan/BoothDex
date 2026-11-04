//! Search API. Builds the in-memory catalog from Postgres at startup.
//! Usage: DATABASE_URL=postgres://... [BIND=0.0.0.0:3000] cargo run --release -p api

mod account;
mod admin;
mod catalog;
mod db;
mod gallery;
mod limit;
mod params;
mod rss;
mod text;

#[cfg(test)]
mod search_tests;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use anyhow::Result;
use axum::extract::{RawQuery, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use boothdex_core::urlencode;
use axum::{Json, Router};
use tower_http::compression::Predicate as _;
use serde_json::{Value, json};
use sqlx::PgPool;

use catalog::Catalog;
use params::Params;

/// Name lookup for sidebar pickers and the command palette.
struct Lookup(Vec<(Vec<String>, Value, u64)>);

impl Lookup {
    /// Splits model names out of a search ("shinano short hair" -> "short hair" + [shinano]); a name is a whole
    /// word (or two) equal to a model's slug or name in any language, at least 3 characters.
    fn split_names(&self, q: &str) -> (String, Vec<String>) {
        let words: Vec<&str> = q.split_whitespace().collect();
        let slug_of = |w: &str| {
            let w = w.to_lowercase();
            (w.chars().count() >= 3).then(|| self.0.iter().find(|(keys, _, _)| keys.contains(&w))).flatten().and_then(|e| e.1["slug"].as_str().map(str::to_owned))
        };
        let (mut rest, mut slugs, mut i) = (Vec::new(), Vec::new(), 0);
        while i < words.len() {
            if let Some(slug) = words.get(i + 1).and_then(|next| slug_of(&format!("{} {next}", words[i]))) {
                slugs.push(slug);
                i += 2;
            } else if let Some(slug) = slug_of(words[i]) {
                slugs.push(slug);
                i += 1;
            } else {
                rest.push(words[i]);
                i += 1;
            }
        }
        slugs.dedup();
        (rest.join(" "), slugs)
    }

    fn new(mut entries: Vec<(Vec<String>, Value, u64)>) -> Self {
        entries.retain(|e| e.2 > 0);
        for e in &mut entries {
            e.0.iter_mut().for_each(|s| *s = s.to_lowercase());
        }
        entries.sort_by(|a, b| b.2.cmp(&a.2));
        Self(entries)
    }

    fn find(&self, p: &Params) -> Value {
        let q = p.get("q").unwrap_or_default().to_lowercase();
        let offset = p.num("offset").unwrap_or(0);
        let limit = p.num("limit").unwrap_or(20).clamp(1, 100);
        let items: Vec<Value> = self
            .0
            .iter()
            .filter(|(keys, _, _)| q.is_empty() || keys.iter().any(|k| k.contains(&q)))
            .skip(offset)
            .take(limit)
            .map(|(_, v, n)| {
                let mut v = v.clone();
                v["item_count"] = json!(n);
                v
            })
            .collect();
        json!({ "items": items })
    }
}

/// Search texts that name one smart category, without spaces and lowercased ("shorthair", "ショート" -> hair--short):
/// a parent's name, a child's name when no other child shares it, or child and parent names together
/// in either order, per language.
fn category_phrases(categories: &Value) -> HashMap<String, String> {
    let norm = |s: &str| s.split_whitespace().collect::<String>().to_lowercase();
    let langs = ["en", "ja", "ko", "zh"];
    let parents = categories.as_array().map(Vec::as_slice).unwrap_or_default();
    let mut child_names: HashMap<String, Vec<String>> = HashMap::new();
    let mut out = HashMap::new();
    for p in parents {
        let slug = p["slug"].as_str().unwrap_or_default();
        for l in langs {
            let Some(pn) = p[l].as_str().map(norm).filter(|n| !n.is_empty()) else { continue };
            out.insert(pn.clone(), slug.to_owned());
            for c in p["children"].as_array().into_iter().flatten() {
                let (Some(cs), Some(cn)) = (c["slug"].as_str(), c[l].as_str().map(norm).filter(|n| !n.is_empty())) else { continue };
                out.insert(format!("{cn}{pn}"), cs.to_owned());
                out.insert(format!("{pn}{cn}"), cs.to_owned());
                child_names.entry(cn).or_default().push(cs.to_owned());
            }
        }
    }
    for (name, mut slugs) in child_names {
        slugs.dedup();
        if let [only] = slugs.as_slice() {
            out.entry(name).or_insert_with(|| only.clone());
        }
    }
    out
}

/// Everything derived from the catalogue; rebuilt off to the side and swapped in atomically.
struct Snapshot {
    catalog: Catalog,
    /// see `category_phrases`
    category_phrases: HashMap<String, String>,
    avatars: Lookup,
    authors: Lookup,
    tags: Lookup,
    booth_tags: Lookup,
    categories: Value,
    version: i64,
    loaded_at: chrono::DateTime<chrono::Utc>,
    build_secs: f64,
}

const EMBED_CACHE_CAP: u64 = 2000;
type EmbedWatch = tokio::sync::watch::Receiver<Option<Option<Vec<f32>>>>;

struct AppState {
    pool: PgPool,
    snap: arc_swap::ArcSwap<Snapshot>,
    /// wakes the reloader early (admin "reload now")
    reload: tokio::sync::Notify,
    site_url: String,
    throttle: account::LoginThrottle,
    dummy_hash: String,
    gallery: gallery::Fetcher,
    sys: admin::SysMetrics,
    http: reqwest::Client,
    imgproxy_url: Option<String>,
    /// per-IP and global rate-limit tiers across API routes and upstream embedding calls
    limits: limit::Limits,
    /// caps concurrent in-flight catalogue search / item-load CPU + DB work so floods cannot starve other routes
    search_permits: tokio::sync::Semaphore,
    /// query text -> embedding, so paging through results doesn't call the provider again
    embed_cache: moka::sync::Cache<String, Vec<f32>>,
    /// deduplicates concurrent identical uncached queries
    embed_inflight: std::sync::Mutex<HashMap<String, EmbedWatch>>,
    /// caps concurrent upstream /embeddings HTTP calls
    embed_permits: Arc<tokio::sync::Semaphore>,
}

impl AppState {
    fn new(
        pool: PgPool,
        snap: Snapshot,
        limits: limit::Limits,
        site_url: String,
        imgproxy_url: Option<String>,
        dummy_hash: String,
        http_timeout: Option<std::time::Duration>,
    ) -> Result<Self> {
        // Fetcher::new installs the rustls crypto provider required by reqwest before building the HTTP client.
        let gallery = gallery::Fetcher::new()?;
        let mut builder = reqwest::Client::builder();
        if let Some(t) = http_timeout {
            builder = builder.timeout(t);
        }
        let http = builder.build()?;
        Ok(Self {
            pool,
            snap: arc_swap::ArcSwap::from_pointee(snap),
            reload: tokio::sync::Notify::new(),
            site_url,
            throttle: Default::default(),
            dummy_hash,
            gallery,
            sys: admin::SysMetrics::new(),
            http,
            imgproxy_url,
            limits,
            search_permits: tokio::sync::Semaphore::new(16),
            embed_cache: moka::sync::Cache::new(EMBED_CACHE_CAP),
            embed_inflight: Default::default(),
            embed_permits: Arc::new(tokio::sync::Semaphore::new(4)),
        })
    }

    fn snap(&self) -> arc_swap::Guard<Arc<Snapshot>> {
        self.snap.load()
    }

    async fn acquire_search(&self) -> std::result::Result<tokio::sync::SemaphorePermit<'_>, Response> {
        tokio::time::timeout(std::time::Duration::from_secs(2), self.search_permits.acquire())
            .await
            .ok()
            .and_then(std::result::Result::ok)
            .ok_or_else(|| limit::too_many_requests(2))
    }
}

struct ApiError(anyhow::Error);

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        eprintln!("error: {:#}", self.0);
        (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "ok": false, "error": "internal error" }))).into_response()
    }
}

impl<E: Into<anyhow::Error>> From<E> for ApiError {
    fn from(e: E) -> Self {
        Self(e.into())
    }
}

/// Per-request timing, returned as `trace` so the site can show where the time went.
#[derive(Default)]
struct Trace {
    steps: Vec<Value>,
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
        self.steps.push(json!({ "step": name, "ms": (ms * 10.0).round() / 10.0 }));
        self.last = Some(now);
    }
}

enum EmbedPlan {
    Vector(Vec<f32>),
    Off,
    Skipped,
    Unavailable,
}

async fn lookup_rate_limit(
    State(s): State<Arc<AppState>>,
    limit::ClientIp(ip): limit::ClientIp,
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> Response {
    if let Err(r) = s.limits.lookup.check(ip) {
        return r;
    }
    next.run(req).await
}

async fn search(
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
                (Some(ids), "used")
            }
            None => (None, "unavailable"),
        },
        EmbedPlan::Off => (None, "off"),
        EmbedPlan::Skipped => (None, "skipped"),
        EmbedPlan::Unavailable => (None, "unavailable"),
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
    let items = db::load_items(&s.pool, &r.ids).await?;
    trace.step("items");
    Ok(Json(json!({
        "count": r.count,
        "page": r.page,
        "totalPages": r.total_pages,
        "pageSize": r.page_size,
        "sort": r.sort,
        "dir": r.dir,
        "boothCounts": r.facets.booth,
        "smartCounts": r.facets.smart,
        "featureCounts": r.facets.features,
        "items": items,
        // "used", "off" (no embeddings model or no query), "unavailable" (provider slow or failing), "skipped"
        "semantic": semantic_status,
        "trace": trace.steps,
        "timing": { "searchMs": search_ms, "totalMs": t0.elapsed().as_secs_f64() * 1e3 },
    }))
    .into_response())
}

/// Resolves the query embedding (or why there isn't one) before acquiring a search execution permit.
async fn semantic_vector(s: Arc<AppState>, ip: std::net::IpAddr, q: &str, trace: &mut Trace) -> EmbedPlan {
    let Ok(c) = boothdex_core::settings::ai(&s.pool).await else { return EmbedPlan::Unavailable };
    let (Some(base), Some(key), Some(model)) = (c.base_url, c.api_key, c.embed_model) else { return EmbedPlan::Off };
    trace.step("settings");
    match query_embedding(s, ip, q, base, key, model, trace).await {
        Some(v) => EmbedPlan::Vector(v),
        None => EmbedPlan::Unavailable,
    }
}

/// The search text the embedding is cached under: case and spacing don't change the meaning.
fn normalize_query(q: &str) -> String {
    q.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()
}

struct ClearInflight {
    state: Arc<AppState>,
    key: String,
}

impl Drop for ClearInflight {
    fn drop(&mut self) {
        self.state.embed_inflight.lock().unwrap().remove(&self.key);
    }
}

/// Embedding of a search text: process memory, then the query_embeddings table, then the provider.
async fn query_embedding(
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
async fn nearest(s: &AppState, vector: &[f32], models: &[String]) -> Option<Vec<i64>> {
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

fn nearest_query<'a>(vector: &[f32], models: &'a [String]) -> sqlx::query::QueryAs<'a, sqlx::Postgres, (i64, f64), sqlx::postgres::PgArguments> {
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
fn semantic_cutoff(best: f64, scoped: bool) -> f64 {
    const FLOOR: f64 = 0.72;
    let margin = if scoped { 0.10 } else { 0.06 };
    FLOOR.max(best - margin)
}

async fn rss_feed(State(s): State<Arc<AppState>>, limit::ClientIp(ip): limit::ClientIp, RawQuery(q): RawQuery) -> Result<Response, ApiError> {
    if let Err(r) = s.limits.rss.check(ip) {
        return Ok(r);
    }
    let _permit = match s.acquire_search().await {
        Ok(p) => p,
        Err(r) => return Ok(r),
    };
    let user_q = q.unwrap_or_default();
    // first occurrence wins in Params, so the prefix pins ordering and size
    let p = Params::parse(&format!("sort=newest&dir=desc&page=1&pageSize=50&{user_q}"));
    let now = chrono::Utc::now().timestamp();
    let r = tokio::task::block_in_place(|| s.snap().catalog.search(&p, now, None, None))?;
    let items = db::load_items(&s.pool, &r.ids).await?;
    let link = format!("{}/?{user_q}", s.site_url);
    Ok(([(axum::http::header::CONTENT_TYPE, "application/rss+xml; charset=utf-8")], rss::render(&link, &user_q, &items)).into_response())
}

/// Ctrl+K: one query across items, avatars, authors, tags and categories.
async fn palette(State(s): State<Arc<AppState>>, limit::ClientIp(ip): limit::ClientIp, RawQuery(q): RawQuery) -> Result<Response, ApiError> {
    if let Err(r) = s.limits.palette.check(ip) {
        return Ok(r);
    }
    let raw = q.unwrap_or_default();
    let p = Params::parse(&raw);
    let text = p.get("q").unwrap_or_default();
    if text.trim().is_empty() {
        return Ok(Json(json!({ "items": [], "avatars": [], "authors": [], "tags": [], "categories": [] })).into_response());
    }
    let _permit = match s.acquire_search().await {
        Ok(p) => p,
        Err(r) => return Ok(r),
    };
    let small = Params::parse(&format!("limit=5&{raw}"));
    let needle = text.to_lowercase();
    let snap = s.snap();
    let categories: Vec<Value> = snap.categories["categories"]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|p| std::iter::once(p).chain(p["children"].as_array().into_iter().flatten()))
        .filter(|c| ["en", "ja", "ko", "zh"].iter().any(|k| c[*k].as_str().is_some_and(|n| n.to_lowercase().contains(&needle))))
        .take(5)
        .map(|c| json!({ "slug": c["slug"], "en": c["en"], "ja": c["ja"], "ko": c["ko"], "zh": c["zh"] }))
        .collect();
    // items honour the caller's adult setting (default safe) like the main search
    let adult = p.get("adult").unwrap_or_else(|| "safe".into());
    let item_params = Params::parse(&format!("q={}&adult={}&sort=relevant&pageSize=8", urlencode(&text), urlencode(&adult)));
    let now = chrono::Utc::now().timestamp();
    let r = tokio::task::block_in_place(|| s.snap().catalog.search(&item_params, now, None, None))?;
    let items: Vec<Value> = db::load_items(&s.pool, &r.ids)
        .await?
        .into_iter()
        .map(|it| {
            let image = it.booth_images.as_ref().and_then(|imgs| imgs.first().cloned())
                .or_else(|| it.media.first().cloned());
            json!({
                "id": it.id, "title": it.title, "author_name": it.author_name,
                "price_jpy": it.price_jpy, "smart_price": it.smart_price,
                "image": image,
            })
        })
        .collect();
    Ok(Json(json!({
        "items": items,
        "itemCount": r.count,
        "avatars": s.snap().avatars.find(&small)["items"],
        "authors": s.snap().authors.find(&small)["items"],
        "tags": s.snap().tags.find(&small)["items"],
        "categories": categories,
    }))
    .into_response())
}

/// Full records by id (favourites list, palette picks): `/api/items?ids=1+2+3`, at most 100.
async fn items_by_id(State(s): State<Arc<AppState>>, limit::ClientIp(ip): limit::ClientIp, RawQuery(q): RawQuery) -> Result<Response, ApiError> {
    if let Err(r) = s.limits.items.check(ip) {
        return Ok(r);
    }
    let ids = item_ids(q.as_deref().unwrap_or(""));
    if ids.is_empty() {
        return Ok(Json(json!({ "items": [] })).into_response());
    }
    let _permit = match s.acquire_search().await {
        Ok(p) => p,
        Err(r) => return Ok(r),
    };
    Ok(Json(json!({ "items": db::load_items(&s.pool, &ids).await? })).into_response())
}

/// `ids=` from a raw query, at most 100. Not `Params`: that caps lists at 10, and the web proxy re-encodes the
/// separators (`,` -> `%2C`, `+` -> `%2B`), so they're split after decoding.
fn item_ids(query: &str) -> Vec<i64> {
    let raw = query.split('&').find_map(|kv| kv.strip_prefix("ids=")).unwrap_or("");
    percent_encoding::percent_decode_str(raw).decode_utf8_lossy().split([',', '+', ' ']).filter_map(|v| v.parse().ok()).take(100).collect()
}

fn lookup(f: fn(&Snapshot) -> &Lookup) -> impl Fn(State<Arc<AppState>>, RawQuery) -> std::future::Ready<Json<Value>> + Clone {
    move |State(s), RawQuery(q)| std::future::ready(Json(f(&s.snap()).find(&Params::parse(q.as_deref().unwrap_or("")))))
}

async fn catalog_version(pool: &PgPool) -> Result<i64> {
    Ok(sqlx::query_scalar("select version from catalog_meta").fetch_one(pool).await?)
}

/// Rebuilds the snapshot when the worker bumps catalog_meta.version (checked every 30 s, or on demand).
/// The old snapshot keeps serving during the rebuild, so peak memory is roughly two catalogues.
async fn reloader(state: Arc<AppState>) {
    loop {
        tokio::select! {
            _ = tokio::time::sleep(std::time::Duration::from_secs(30)) => {}
            _ = state.reload.notified() => {}
        }
        let version = match catalog_version(&state.pool).await {
            Ok(v) => v,
            Err(e) => {
                eprintln!("reload check: {e:#}");
                continue;
            }
        };
        if version == state.snap().version {
            continue;
        }
        match build_snapshot(&state.pool, version).await {
            Ok(s) => {
                println!("catalogue reloaded: version {version}, {} docs", s.catalog.len());
                state.snap.store(Arc::new(s));
            }
            Err(e) => eprintln!("catalogue reload failed, keeping version {}: {e:#}", state.snap().version),
        }
    }
}

async fn build_snapshot(pool: &PgPool, version: i64) -> Result<Snapshot> {
    let pool = pool.clone();
    let t0 = Instant::now();
    let docs = db::load_docs(&pool).await?;
    println!("loaded {} docs in {:.1}s", docs.len(), t0.elapsed().as_secs_f64());
    let t1 = Instant::now();
    // Text content selects the disk index; metadata-only reloads reuse it. INDEX_DIR="" keeps it in RAM.
    let index_root = std::env::var("INDEX_DIR").unwrap_or_else(|_| "textindex".into());
    let dir = (!index_root.is_empty()).then(|| std::path::PathBuf::from(index_root));
    let catalog = tokio::task::spawn_blocking(move || Catalog::build(docs, chrono::Utc::now().timestamp(), dir.as_deref())).await??;
    println!("built index in {:.1}s, bitmaps ~{} KB", t1.elapsed().as_secs_f64(), catalog.bitmap_bytes() / 1024);

    let avatars = sqlx::query_as::<_, (String, String, Option<String>, Option<String>, Option<String>)>("select slug, name_en, name_ja, name_ko, coalesce((select b.images[1] from booth_images b where b.item_id = a.avatar_item_id), a.image_url) from avatars a")
        .fetch_all(&pool)
        .await?
        .into_iter()
        .map(|(slug, en, ja, ko, image_url)| {
            let n = catalog.avatars.len(&slug);
            let keys = [Some(slug.clone()), Some(en.clone()), ja.clone(), ko.clone()].into_iter().flatten().collect();
            (keys, json!({ "slug": slug, "en": en, "ja": ja, "ko": ko, "image_url": image_url }), n)
        })
        .collect();
    let authors = sqlx::query_as::<_, (String, Option<String>)>("select distinct on (author_name) author_name, author_icon from items order by author_name, scraped_at desc")
        .fetch_all(&pool)
        .await?
        .into_iter()
        .map(|(name, icon)| (vec![name.clone()], json!({ "name": name, "image_url": icon }), catalog.authors.len(&name)))
        .collect();
    let tags = sqlx::query_as::<_, (i32, String, String, Option<String>, Vec<String>)>("select id, name, display, display_zh, aliases from smart_tags")
        .fetch_all(&pool)
        .await?
        .into_iter()
        .map(|(id, name, display, display_zh, aliases)| {
            let n = catalog.smart_tags.len(&name);
            let keys = aliases.iter().cloned().chain([name.clone(), display.clone()]).chain(display_zh.clone()).collect();
            (keys, json!({ "name": name, "display": display, "display_zh": display_zh, "master_id": id, "aliases": aliases }), n)
        })
        .collect();
    let booth_tags = catalog.booth_tags.keys().map(|t| (vec![t.clone()], json!({ "name": t }), catalog.booth_tags.len(t))).collect();
    let categories: Value = sqlx::query_scalar(
        "select coalesce(jsonb_agg(jsonb_build_object('slug', p.slug, 'en', p.name_en, 'ja', p.name_ja, 'ko', p.name_ko, 'zh', p.name_zh, 'mode', p.mode,
            'children', (select coalesce(jsonb_agg(jsonb_build_object('slug', c.slug, 'en', c.name_en, 'ja', c.name_ja, 'ko', c.name_ko, 'zh', c.name_zh,
                'group_en', c.group_en, 'group_ja', c.group_ja, 'group_ko', c.group_ko, 'group_zh', c.group_zh) order by c.name_en), '[]')
                from smart_categories c where c.parent_slug = p.slug)) order by p.name_en), '[]')
         from smart_categories p where p.parent_slug is null",
    )
    .fetch_one(&pool)
    .await?;

    Ok(Snapshot {
        catalog,
        category_phrases: category_phrases(&categories),
        avatars: Lookup::new(avatars),
        authors: Lookup::new(authors),
        tags: Lookup::new(tags),
        booth_tags: Lookup::new(booth_tags),
        categories: json!({ "categories": categories }),
        version,
        loaded_at: chrono::Utc::now(),
        build_secs: t0.elapsed().as_secs_f64(),
    })
}

async fn build_state(pool: PgPool) -> Result<AppState> {
    let version = catalog_version(&pool).await?;
    let snap = build_snapshot(&pool, version).await?;
    AppState::new(
        pool,
        snap,
        Default::default(),
        std::env::var("SITE_URL").unwrap_or_else(|_| "http://127.0.0.1:3000".into()),
        std::env::var("IMGPROXY_URL").ok().or_else(|| Some("http://127.0.0.1:3001".into())),
        account::dummy_hash(),
        Some(std::time::Duration::from_secs(10)),
    )
}

/// Answers every request with 503 while the catalogue loads (~20 s), so the site can say "starting" instead of
/// failing to connect. Stops when `ready` fires; the real server then binds the same address.
async fn serve_starting(bind: &str, ready: tokio::sync::oneshot::Receiver<()>) -> Result<tokio::task::JoinHandle<()>> {
    let listener = tokio::net::TcpListener::bind(bind).await?;
    let app = Router::new().fallback(|| async {
        (StatusCode::SERVICE_UNAVAILABLE, [(axum::http::header::RETRY_AFTER, "5")], Json(json!({ "ok": false, "starting": true, "error": "Starting up" })))
    });
    Ok(tokio::spawn(async move {
        let _ = axum::serve(listener, app).with_graceful_shutdown(async { ready.await.ok(); }).await;
    }))
}

#[tokio::main]
async fn main() -> Result<()> {
    let bind = std::env::var("BIND").unwrap_or_else(|_| "127.0.0.1:3000".into());
    let (ready, ready_rx) = tokio::sync::oneshot::channel();
    let starting = serve_starting(&bind, ready_rx).await?;
    println!("starting on http://{bind}");

    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(20)
        .acquire_timeout(std::time::Duration::from_secs(5))
        .connect(&std::env::var("DATABASE_URL")?)
        .await?;
    boothdex_core::MIGRATOR.run(&pool).await?;
    admin::apply_admin_env(&pool).await?;
    // AI configured through environment variables: translate the taxonomy on first start
    admin::queue_localize_if_needed(&pool, &boothdex_core::settings::ai(&pool).await?).await?;
    let state = Arc::new(build_state(pool).await?);
    tokio::spawn(reloader(state.clone()));

    let lookups = Router::new()
        .route("/api/avatars", get(lookup(|s| &s.avatars)))
        .route("/api/authors", get(lookup(|s| &s.authors)))
        .route("/api/tags", get(lookup(|s| &s.tags)))
        .route("/api/booth-tags", get(lookup(|s| &s.booth_tags)))
        .route("/api/categories", get(|State(s): State<Arc<AppState>>| async move { Json(s.snap().categories.clone()) }))
        .route_layer(axum::middleware::from_fn_with_state(state.clone(), lookup_rate_limit));

    let app = Router::new()
        .route("/api/health", get(|| async { Json(json!({ "ok": true })) }))
        .route("/api/search", get(search))
        .route("/api/rss", get(rss_feed))
        .route("/api/palette", get(palette))
        .route("/api/items", get(items_by_id))
        .route("/api/items/{id}/gallery", get(gallery::gallery))
        .merge(lookups)
        .merge(account::routes())
        .merge(admin::routes())
        // gzip would buffer the admin SSE stream
        .layer(tower_http::compression::CompressionLayer::new().compress_when(
            tower_http::compression::predicate::DefaultPredicate::new()
                .and(tower_http::compression::predicate::NotForContentType::const_new("text/event-stream")),
        ))
        // no credentials cross-origin: the frontend is served same-origin (Next.js rewrites /api)
        .layer(tower_http::cors::CorsLayer::new().allow_origin(tower_http::cors::Any).allow_methods([axum::http::Method::GET]))
        .with_state(state);

    // hand the address over: the placeholder finishes its open requests and releases the port
    let _ = ready.send(());
    starting.await?;
    let listener = tokio::net::TcpListener::bind(&bind).await?;
    println!("listening on http://{bind}");
    axum::serve(listener, app.into_make_service_with_connect_info::<std::net::SocketAddr>()).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn url_encoding_preserves_query_values() {
        let value = "衣装 x+y&z=1/#?%";
        let encoded = super::urlencode(value);
        assert_eq!(super::Params::parse(&format!("q={encoded}")).get("q").as_deref(), Some(value));
    }

    #[test]
    fn item_ids_accept_any_separator_encoding() {
        assert_eq!(super::item_ids("ids=1,2+3%2C4%2B5%206&x=7"), [1, 2, 3, 4, 5, 6]);
        assert_eq!(super::item_ids(&format!("ids={}", (1..=150).map(|i| i.to_string()).collect::<Vec<_>>().join(","))).len(), 100);
        assert!(super::item_ids("q=1").is_empty());
    }

    #[test]
    fn nearest_binds_only_parameters_used_by_each_query() {
        use sqlx::{Arguments, Execute};

        for models in [vec![], vec!["shinano".to_owned()]] {
            let mut query = super::nearest_query(&[0.5; boothdex_core::embed::DIMS], &models);
            assert_eq!(query.take_arguments().unwrap().unwrap().len(), if models.is_empty() { 1 } else { 2 });
            assert_eq!(query.sql().as_str().contains("$2"), !models.is_empty());
        }
    }

    #[test]
    fn queries_are_normalized_for_the_cache() {
        assert_eq!(super::normalize_query("  Gothic   Dress "), "gothic dress");
    }

    fn test_state() -> std::sync::Arc<super::AppState> {
        let catalog = super::Catalog::build(Vec::new(), 0, None).unwrap();
        let snap = super::Snapshot {
            catalog,
            category_phrases: Default::default(),
            avatars: super::Lookup::new(Vec::new()),
            authors: super::Lookup::new(Vec::new()),
            tags: super::Lookup::new(Vec::new()),
            booth_tags: super::Lookup::new(Vec::new()),
            categories: serde_json::json!({ "categories": [] }),
            version: 1,
            loaded_at: chrono::Utc::now(),
            build_secs: 0.0,
        };
        std::sync::Arc::new(
            super::AppState::new(
                sqlx::postgres::PgPoolOptions::new().connect_lazy("postgres://localhost/unused").unwrap(),
                snap,
                super::limit::Limits {
                    search: super::limit::RateLimiter::new(2.0, 3.0, 64),
                    palette: super::limit::RateLimiter::new(5.0, 2.0, 64),
                    rss: super::limit::RateLimiter::new(0.5, 2.0, 64),
                    items: super::limit::RateLimiter::new(1.0, 2.0, 64),
                    gallery: super::limit::RateLimiter::new(1.0, 2.0, 64),
                    lookup: super::limit::RateLimiter::new(5.0, 2.0, 64),
                    embed_ip: super::limit::RateLimiter::new(0.25, 8.0, 64),
                    embed_global: super::limit::RateLimiter::new(2.0, 20.0, 1),
                    auth: super::limit::RateLimiter::new(0.2, 5.0, 64),
                },
                "http://127.0.0.1:3000".into(),
                None,
                String::new(),
                None,
            )
            .unwrap(),
        )
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn search_allows_burst_and_throttles_one_ip_without_affecting_others() {
        use axum::extract::{FromRequestParts, RawQuery, State};
        use axum::http::{HeaderMap, Request, StatusCode, header};

        let state = test_state();

        let ip_of = |xff: &'static str| {
            let state = state.clone();
            async move {
                let (mut parts, _) = Request::builder().uri("/api/search").header("x-forwarded-for", xff).body(()).unwrap().into_parts();
                super::limit::ClientIp::from_request_parts(&mut parts, &state).await.unwrap()
            }
        };
        let call = |xff: &'static str| {
            let state = state.clone();
            async move {
                let ip = ip_of(xff).await;
                super::search(State(state), ip, HeaderMap::new(), RawQuery(None))
                    .await
                    .unwrap_or_else(axum::response::IntoResponse::into_response)
            }
        };

        for _ in 0..3 {
            assert_eq!(call("203.0.113.10").await.status(), StatusCode::OK);
        }
        let blocked = call("203.0.113.10").await;
        assert_eq!(blocked.status(), StatusCode::TOO_MANY_REQUESTS);
        assert!(blocked.headers().contains_key(header::RETRY_AFTER));
        // another visitor keeps searching unaffected
        assert_eq!(call("203.0.113.11").await.status(), StatusCode::OK);

        // exhausting palette, lookups, rss, items and gallery on a fresh IP never starves /api/search
        let ip = "203.0.113.99";
        for _ in 0..2 {
            assert_eq!(super::palette(State(state.clone()), ip_of(ip).await, RawQuery(Some("q=dress".into()))).await.ok().unwrap().status(), StatusCode::OK);
        }
        assert_eq!(super::palette(State(state.clone()), ip_of(ip).await, RawQuery(Some("q=dress".into()))).await.ok().unwrap().status(), StatusCode::TOO_MANY_REQUESTS);

        for _ in 0..2 {
            assert!(state.limits.lookup.check(ip_of(ip).await.0).is_ok());
        }
        assert!(state.limits.lookup.check(ip_of(ip).await.0).is_err());

        for _ in 0..2 {
            assert_eq!(super::rss_feed(State(state.clone()), ip_of(ip).await, RawQuery(None)).await.ok().unwrap().status(), StatusCode::OK);
        }
        assert_eq!(super::rss_feed(State(state.clone()), ip_of(ip).await, RawQuery(None)).await.ok().unwrap().status(), StatusCode::TOO_MANY_REQUESTS);

        for _ in 0..2 {
            assert_eq!(super::items_by_id(State(state.clone()), ip_of(ip).await, RawQuery(None)).await.ok().unwrap().status(), StatusCode::OK);
        }
        assert_eq!(super::items_by_id(State(state.clone()), ip_of(ip).await, RawQuery(None)).await.ok().unwrap().status(), StatusCode::TOO_MANY_REQUESTS);

        use axum::extract::Path;
        for _ in 0..2 {
            assert_eq!(super::gallery::gallery(State(state.clone()), ip_of(ip).await, Path(999)).await.ok().unwrap().status(), StatusCode::NOT_FOUND);
        }
        assert_eq!(super::gallery::gallery(State(state.clone()), ip_of(ip).await, Path(999)).await.ok().unwrap().status(), StatusCode::TOO_MANY_REQUESTS);

        // /api/search for that same IP still succeeds with its full burst intact
        for _ in 0..3 {
            assert_eq!(call(ip).await.status(), StatusCode::OK);
        }
        assert_eq!(call(ip).await.status(), StatusCode::TOO_MANY_REQUESTS);
    }

    #[tokio::test]
    async fn auth_limiter_exhaustion_throttles_both_login_and_register() {
        use axum::extract::{Json, State};
        use axum::http::{HeaderMap, StatusCode};

        let state = test_state();
        let ip = super::limit::ClientIp("203.0.113.50".parse().unwrap());

        // Exhaust the auth limiter burst for this IP
        while state.limits.auth.check(ip.0).is_ok() {}

        enum AuthCall {
            Login(super::account::Login),
            Register(super::account::Register),
        }

        let cases = [
            (
                "login",
                AuthCall::Login(super::account::Login {
                    login: "testuser".into(),
                    password: "password12345".into(),
                    keep: false,
                }),
            ),
            (
                "register",
                AuthCall::Register(super::account::Register {
                    username: "testuser".into(),
                    email: "testuser@example.com".into(),
                    display_name: "Test User".into(),
                    password: "password12345".into(),
                    keep: false,
                }),
            ),
        ];

        for (name, call) in cases {
            let res = match call {
                AuthCall::Login(body) => {
                    super::account::login(State(state.clone()), ip, HeaderMap::new(), Json(body)).await.ok().unwrap()
                }
                AuthCall::Register(body) => {
                    super::account::register(State(state.clone()), ip, HeaderMap::new(), Json(body)).await.ok().unwrap()
                }
            };
            assert_eq!(
                res.status(),
                StatusCode::TOO_MANY_REQUESTS,
                "handler {name} must return 429 when auth limiter is exhausted"
            );
        }
    }

    #[test]
    fn semantic_cutoff_follows_the_best_match_and_drops_weak_ones() {
        // values seen during calibration: a relevant query, and gibberish whose best neighbour is still 0.707
        assert!((super::semantic_cutoff(0.787, false) - 0.727).abs() < 1e-9);
        assert!(0.707 < super::semantic_cutoff(0.707, false));
        assert!(0.707 < super::semantic_cutoff(0.707, true));
        assert!((super::semantic_cutoff(0.830, true) - 0.730).abs() < 1e-9);
    }

    #[test]
    fn queries_naming_a_category_find_it() {
        let cats = serde_json::json!([
            {"slug": "hair", "en": "Hair", "ja": "髪型", "children": [
                {"slug": "hair--short", "en": "Short", "ja": "ショート"}, {"slug": "hair--other", "en": "Other"}]},
            {"slug": "clothing", "en": "Clothing", "children": [{"slug": "clothing--other", "en": "Other"}]}
        ]);
        let m = super::category_phrases(&cats);
        for q in ["shorthair", "hairshort", "short", "ショート"] {
            assert_eq!(m.get(q).map(String::as_str), Some("hair--short"), "{q}");
        }
        assert_eq!(m.get("hair").map(String::as_str), Some("hair"));
        // a child name two parents share only counts together with its parent
        assert_eq!(m.get("other"), None);
        assert_eq!(m.get("otherclothing").map(String::as_str), Some("clothing--other"));
    }
}

#[cfg(test)]
mod name_tests {
    use super::*;

    #[test]
    fn model_names_are_split_out_of_the_query() {
        let entry = |slug: &str, names: &[&str]| {
            (names.iter().map(|n| n.to_string()).chain([slug.to_string()]).collect(), json!({ "slug": slug }), 10u64)
        };
        let l = Lookup::new(vec![entry("shinano", &["Shinano", "しなの"]), entry("rusk", &["Rusk Dragon"]), entry("io", &["IO"])]);
        assert_eq!(l.split_names("Shinano short hair"), ("short hair".into(), vec!["shinano".into()]));
        assert_eq!(l.split_names("しなの ボブ"), ("ボブ".into(), vec!["shinano".into()]));
        assert_eq!(l.split_names("rusk dragon wings"), ("wings".into(), vec!["rusk".into()]));
        // too short to be taken as a name
        assert_eq!(l.split_names("io hair"), ("io hair".into(), vec![]));
        assert_eq!(l.split_names("short hair"), ("short hair".into(), vec![]));
    }
}
