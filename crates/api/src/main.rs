//! Search API. Builds the in-memory catalog from Postgres at startup.
//! Usage: DATABASE_URL=postgres://... [BIND=0.0.0.0:3000] cargo run --release -p api

mod account;
mod admin;
mod catalog;
mod db;
mod gallery;
mod params;
mod rss;
mod text;

use std::sync::Arc;
use std::time::Instant;

use anyhow::Result;
use axum::extract::{RawQuery, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
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

/// Everything derived from the catalogue; rebuilt off to the side and swapped in atomically.
struct Snapshot {
    catalog: Catalog,
    avatars: Lookup,
    authors: Lookup,
    tags: Lookup,
    booth_tags: Lookup,
    categories: Value,
    version: i64,
    loaded_at: chrono::DateTime<chrono::Utc>,
    build_secs: f64,
}

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
    /// query text -> embedding, so paging through results doesn't call the provider again
    embed_cache: std::sync::Mutex<std::collections::HashMap<String, Vec<f32>>>,
}

impl AppState {
    fn snap(&self) -> arc_swap::Guard<Arc<Snapshot>> {
        self.snap.load()
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

async fn search(State(s): State<Arc<AppState>>, headers: axum::http::HeaderMap, RawQuery(q): RawQuery) -> Result<Response, ApiError> {
    let t0 = Instant::now();
    let p = Params::parse(q.as_deref().unwrap_or(""));
    let favourites = if p.get("v").as_deref() == Some("favourites") {
        let Some(u) = account::current_user(&s.pool, &headers).await? else {
            return Ok((StatusCode::UNAUTHORIZED, Json(json!({ "ok": false, "error": "Sign in required" }))).into_response());
        };
        Some(s.snap().catalog.docs_for(&account::favourite_ids(&s.pool, u.id).await?))
    } else {
        None
    };
    // page 1 decides whether meaning-based matches are used; the site sends sem=0 for later pages when they
    // weren't, so the result set can't change between pages if the provider answers late
    // model names in the query become a requirement for meaning-based matches and are left out of the
    // embedding, so "shinano short hair" adds short hair that fits Shinano, not other Shinano items
    let (meaning, models) = s.snap().avatars.split_names(&p.get("q").unwrap_or_default());
    let (semantic, semantic_status) = match p.get("q").filter(|q| !q.trim().is_empty()) {
        Some(_) if p.get("sem").as_deref() == Some("0") => (None, "skipped"),
        Some(_) if meaning.is_empty() => (None, "skipped"),
        Some(_) => semantic_ids(s.clone(), &meaning).await,
        None => (None, "off"),
    };
    let catalog_now = chrono::Utc::now().timestamp();
    let semantic = semantic.as_deref().map(|ids| catalog::Semantic { ids, models: &models });
    let r = tokio::task::block_in_place(|| s.snap().catalog.search(&p, catalog_now, favourites.as_ref(), semantic))?;
    let search_ms = t0.elapsed().as_secs_f64() * 1e3;
    let items = db::load_items(&s.pool, &r.ids).await?;
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
        "timing": { "searchMs": search_ms, "totalMs": t0.elapsed().as_secs_f64() * 1e3 },
    }))
    .into_response())
}

/// Items nearest to the query by meaning (the embeddings job must have run), with a status for the response:
/// "off" without an embeddings model, "unavailable" when the provider is slow or failing (words alone then).
async fn semantic_ids(s: Arc<AppState>, q: &str) -> (Option<Vec<i64>>, &'static str) {
    let Ok(c) = catalogue_core::settings::ai(&s.pool).await else { return (None, "unavailable") };
    let (Some(base), Some(key), Some(model)) = (c.base_url, c.api_key, c.embed_model) else { return (None, "off") };
    match nearest(s, q, base, key, model).await {
        Some(ids) => (Some(ids), "used"),
        None => (None, "unavailable"),
    }
}

async fn nearest(s: Arc<AppState>, q: &str, base: String, key: String, model: String) -> Option<Vec<i64>> {
    use catalogue_core::embed;
    let cache_key = format!("{model}\n{}", q.trim().to_lowercase());
    let cached = s.embed_cache.lock().unwrap().get(&cache_key).cloned();
    let vector = match cached {
        Some(v) => v,
        None => {
            let input = vec![format!("{}{}", embed::QUERY_PREFIX, q.trim())];
            // the request runs on: if it outlasts the wait below, it still fills the cache for the next page or retry
            let state = s.clone();
            let mut task = tokio::spawn(async move {
                let v = embed::embed(&state.http, &base, &key, &model, &input, 1).await?.pop().ok_or_else(|| anyhow::anyhow!("no embedding"))?;
                let mut cache = state.embed_cache.lock().unwrap();
                // shortcut: dropped wholesale when full; an LRU would keep popular queries, worth it only under real load
                if cache.len() >= 2000 {
                    cache.clear();
                }
                cache.insert(cache_key, v.clone());
                anyhow::Ok(v)
            });
            // measured on the development provider: median 0.7 s, 95th percentile 1.2 s, occasional spikes past 3 s
            match tokio::time::timeout(std::time::Duration::from_millis(2500), &mut task).await {
                Ok(Ok(Ok(v))) => v,
                Ok(Ok(Err(e))) => {
                    eprintln!("query embedding: {e:#}");
                    return None;
                }
                Ok(Err(e)) => {
                    eprintln!("query embedding task: {e}");
                    return None;
                }
                Err(_) => {
                    eprintln!("query embedding: still waiting after 2.5 s; searching by words only this time");
                    return None;
                }
            }
        }
    };
    let mut tx = s.pool.begin().await.ok()?;
    // HNSW returns at most ef_search rows; the default 40 is too few to fill a few pages
    sqlx::query("set local hnsw.ef_search = 200").execute(&mut *tx).await.ok()?;
    let rows: Vec<(i64, f64)> = sqlx::query_as(
        "select id, 1 - (embedding <=> $1::halfvec) from items where embedding is not null and status = 'ok'
         order by embedding <=> $1::halfvec limit 200",
    )
    .bind(embed::to_pg(&vector))
    .fetch_all(&mut *tx)
    .await
    .map_err(|e| eprintln!("semantic search: {e}"))
    .ok()?;
    let cutoff = semantic_cutoff(rows.first()?.1);
    Some(rows.into_iter().take_while(|r| r.1 >= cutoff).map(|r| r.0).collect())
}

/// Similarities from Qwen3-Embedding-4B sit in a narrow band (about 0.65-0.82 for the 200 nearest, even for
/// gibberish), so a fixed threshold can't separate matches from noise. Keep neighbours close to the best one,
/// and nothing when even the best is weak.
/// shortcut: calibrated by hand on a few queries for this one model; other models may need other numbers.
fn semantic_cutoff(best: f64) -> f64 {
    const FLOOR: f64 = 0.72;
    const MARGIN: f64 = 0.06;
    FLOOR.max(best - MARGIN)
}

async fn rss_feed(State(s): State<Arc<AppState>>, RawQuery(q): RawQuery) -> Result<Response, ApiError> {
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
async fn palette(State(s): State<Arc<AppState>>, RawQuery(q): RawQuery) -> Result<Json<Value>, ApiError> {
    let raw = q.unwrap_or_default();
    let p = Params::parse(&raw);
    let text = p.get("q").unwrap_or_default();
    if text.trim().is_empty() {
        return Ok(Json(json!({ "items": [], "avatars": [], "authors": [], "tags": [], "categories": [] })));
    }
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
            json!({
                "id": it["id"], "title": it["title"], "author_name": it["author_name"],
                "price_jpy": it["price_jpy"], "smart_price": it["smart_price"],
                "image": it["booth_images"].get(0).or(it["media"].get(0)),
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
    })))
}

/// Full records by id (favourites list, palette picks): `/api/items?ids=1+2+3`, at most 100.
async fn items_by_id(State(s): State<Arc<AppState>>, RawQuery(q): RawQuery) -> Result<Json<Value>, ApiError> {
    let raw = q.unwrap_or_default();
    let ids: Vec<i64> = Params::parse(&raw.replace(',', "+")).list("ids").iter().filter_map(|v| v.parse().ok()).take(100).collect();
    Ok(Json(json!({ "items": db::load_items(&s.pool, &ids).await? })))
}

fn urlencode(s: &str) -> String {
    s.bytes().map(|b| if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") }).collect()
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
    // text index on disk, one directory per catalogue version; INDEX_DIR="" keeps it in RAM
    let index_root = std::env::var("INDEX_DIR").unwrap_or_else(|_| "textindex".into());
    let text_dir = (!index_root.is_empty()).then(|| std::path::Path::new(&index_root).join(format!("v{version}")));
    let dir = text_dir.clone();
    let catalog = tokio::task::spawn_blocking(move || Catalog::build(docs, chrono::Utc::now().timestamp(), dir.as_deref())).await??;
    // drop indexes of older versions; one still mapped by the previous snapshot may refuse (Windows) and goes next time
    if let Some(current) = &text_dir
        && let Ok(entries) = std::fs::read_dir(&index_root)
    {
        for e in entries.flatten() {
            if e.path() != *current {
                let _ = std::fs::remove_dir_all(e.path());
            }
        }
    }
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
    Ok(AppState {
        pool,
        snap: arc_swap::ArcSwap::from_pointee(snap),
        reload: tokio::sync::Notify::new(),
        site_url: std::env::var("SITE_URL").unwrap_or_else(|_| "http://127.0.0.1:3000".into()),
        throttle: Default::default(),
        dummy_hash: account::dummy_hash(),
        gallery: gallery::Fetcher::new()?,
        sys: admin::SysMetrics::new(),
        http: reqwest::Client::new(),
        imgproxy_url: std::env::var("IMGPROXY_URL").ok().or_else(|| Some("http://127.0.0.1:3001".into())),
        embed_cache: Default::default(),
    })
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

    let pool = PgPool::connect(&std::env::var("DATABASE_URL")?).await?;
    catalogue_core::MIGRATOR.run(&pool).await?;
    admin::apply_admin_env(&pool).await?;
    // AI configured through environment variables: translate the taxonomy on first start
    admin::queue_localize_if_needed(&pool, &catalogue_core::settings::ai(&pool).await?).await?;
    let state = Arc::new(build_state(pool).await?);
    tokio::spawn(reloader(state.clone()));

    let app = Router::new()
        .route("/api/health", get(|| async { Json(json!({ "ok": true })) }))
        .route("/api/search", get(search))
        .route("/api/rss", get(rss_feed))
        .route("/api/palette", get(palette))
        .route("/api/items", get(items_by_id))
        .route("/api/items/{id}/gallery", get(gallery::gallery))
        .route("/api/avatars", get(lookup(|s| &s.avatars)))
        .route("/api/authors", get(lookup(|s| &s.authors)))
        .route("/api/tags", get(lookup(|s| &s.tags)))
        .route("/api/booth-tags", get(lookup(|s| &s.booth_tags)))
        .route("/api/categories", get(|State(s): State<Arc<AppState>>| async move { Json(s.snap().categories.clone()) }))
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
    axum::serve(listener, app).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn semantic_cutoff_follows_the_best_match_and_drops_weak_ones() {
        // values seen during calibration: a relevant query, and gibberish whose best neighbour is still 0.707
        assert!((super::semantic_cutoff(0.787) - 0.727).abs() < 1e-9);
        assert!(0.707 < super::semantic_cutoff(0.707));
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
