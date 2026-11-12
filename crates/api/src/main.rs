//! Search API. Builds the in-memory catalog from Postgres at startup.
//! Usage: DATABASE_URL=postgres://... [BIND=0.0.0.0:3000] cargo run --release -p api

// Measured on the 216k-item catalogue (Linux): vs glibc malloc, startup 49 s -> 20 s, text index rebuild ~45 s -> 12 s,
// steady RSS ~1.1-2 GB -> 0.4-1 GB, keyword search ~5x faster. glibc kept freed reload memory in its arenas.
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

mod account;
mod admin;
#[cfg(test)]
mod api_types;
mod catalog;
mod db;
mod error;
mod gallery;
mod limit;
mod params;
mod rss;
mod search;
mod snapshot;
mod text;

#[cfg(test)]
mod search_tests;

use std::collections::HashMap;
use std::sync::Arc;

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

use error::ApiError;
use params::Params;
use search::{EMBED_CACHE_CAP, EmbedWatch};
use snapshot::{Lookup, Snapshot, build_snapshot, catalog_version, reloader};


struct AppState {
    pool: PgPool,
    snap: arc_swap::ArcSwap<Snapshot>,
    /// wakes the reloader early (admin "reload now")
    reload: tokio::sync::Notify,
    site_url: String,
    throttle: account::LoginThrottle,
    dummy_hash: String,
    gallery: gallery::Fetcher,
    /// item opens not yet written to item_stats; see `gallery::flush_views`
    view_counts: gallery::ViewCounts,
    favourites: account::FavouriteCache,
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
    /// AI settings for query embeddings; see `ai_config`
    ai: arc_swap::ArcSwapOption<boothdex_core::settings::AiConfig>,
    /// serializes AI settings saves across database write and cache publication
    ai_save: tokio::sync::Mutex<()>,
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
            view_counts: Default::default(),
            favourites: account::FavouriteCache::new(),
            sys: admin::SysMetrics::new(),
            http,
            imgproxy_url,
            limits,
            search_permits: tokio::sync::Semaphore::new(16),
            embed_cache: moka::sync::Cache::new(EMBED_CACHE_CAP),
            embed_inflight: Default::default(),
            embed_permits: Arc::new(tokio::sync::Semaphore::new(4)),
            ai: Default::default(),
            ai_save: tokio::sync::Mutex::new(()),
        })
    }

    /// AI settings, read from the database once. They only change through the admin endpoint in this process,
    /// which stores the new value here; a lookup racing with it never overwrites that.
    async fn ai_config(&self) -> Result<Arc<boothdex_core::settings::AiConfig>, sqlx::Error> {
        if let Some(c) = self.ai.load_full() {
            return Ok(c);
        }
        let c = Arc::new(boothdex_core::settings::ai(&self.pool).await?);
        self.ai.compare_and_swap(&None::<Arc<_>>, Some(c.clone()));
        Ok(c)
    }

    fn snap(&self) -> arc_swap::Guard<Arc<Snapshot>> {
        self.snap.load()
    }

    async fn acquire_search(&self) -> std::result::Result<tokio::sync::SemaphorePermit<'_>, limit::RateLimited> {
        tokio::time::timeout(std::time::Duration::from_secs(2), self.search_permits.acquire())
            .await
            .ok()
            .and_then(std::result::Result::ok)
            .ok_or(limit::RateLimited(2))
    }
}

async fn lookup_rate_limit(
    State(s): State<Arc<AppState>>,
    limit::ClientIp(ip): limit::ClientIp,
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> Response {
    if let Err(r) = s.limits.lookup.check(ip) {
        return r.into_response();
    }
    next.run(req).await
}

/// Account changes are database writes; reading the account is not limited.
async fn account_write_rate_limit(
    State(s): State<Arc<AppState>>,
    limit::ClientIp(ip): limit::ClientIp,
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> Response {
    if req.method() != axum::http::Method::GET && let Err(r) = s.limits.account.check(ip) {
        return r.into_response();
    }
    next.run(req).await
}

async fn rss_feed(State(s): State<Arc<AppState>>, limit::ClientIp(ip): limit::ClientIp, RawQuery(q): RawQuery) -> Result<Response, ApiError> {
    s.limits.rss.check(ip)?;
    let _permit = s.acquire_search().await?;
    let user_q = q.unwrap_or_default();
    // first occurrence wins in Params, so the prefix pins ordering and size
    let p = Params::parse(&format!("sort=newest&dir=desc&page=1&pageSize=50&{user_q}"));
    let now = chrono::Utc::now().timestamp();
    let r = tokio::task::block_in_place(|| s.snap().catalog.search(&p, now, None, None, false))?;
    let items = db::load_items(&s.pool, &r.ids, false).await?;
    let link = format!("{}/?{user_q}", s.site_url);
    Ok(([(axum::http::header::CONTENT_TYPE, "application/rss+xml; charset=utf-8")], rss::render(&link, &user_q, &items)).into_response())
}

/// Ctrl+K: one query across items, avatars, authors, tags and categories.
async fn palette(State(s): State<Arc<AppState>>, limit::ClientIp(ip): limit::ClientIp, RawQuery(q): RawQuery) -> Result<Response, ApiError> {
    s.limits.palette.check(ip)?;
    let raw = q.unwrap_or_default();
    let p = Params::parse(&raw);
    let text = p.get("q").unwrap_or_default();
    if text.trim().is_empty() {
        return Ok(Json(json!({ "items": [], "avatars": [], "authors": [], "tags": [], "categories": [] })).into_response());
    }
    let _permit = s.acquire_search().await?;
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
    let r = tokio::task::block_in_place(|| s.snap().catalog.search(&item_params, now, None, None, false))?;
    let items: Vec<Value> = db::load_items(&s.pool, &r.ids, true)
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

/// Full records by id (favourites list, palette picks, the item dialog): `/api/items?ids=1+2+3`, at most 100.
async fn items_by_id(State(s): State<Arc<AppState>>, limit::ClientIp(ip): limit::ClientIp, RawQuery(q): RawQuery) -> Result<Response, ApiError> {
    let ids = item_ids(q.as_deref().unwrap_or(""));
    let limit = if ids.len() == 1 { &s.limits.item } else { &s.limits.items };
    limit.check(ip)?;
    if ids.is_empty() {
        return Ok(Json(json!({ "items": [] })).into_response());
    }
    let _permit = s.acquire_search().await?;
    let items = db::load_items(&s.pool, &ids, false).await?;
    // an id the catalogue lacks may come in later, so an answer missing one is not kept
    let whole = items.len() == ids.len();
    let body = Json(json!({ "items": items }));
    Ok(if whole { ([(axum::http::header::CACHE_CONTROL, ITEMS_CACHE)], body).into_response() } else { body.into_response() })
}

/// Records are the same for everyone, and visitors come for the search: browsers and the CDN may keep them
/// 12 hours, a price or title change included (owner). The CDN keeps them only through a cache rule (docs/DEPLOY.md).
const ITEMS_CACHE: &str = "public, max-age=43200";

/// `ids=` from a raw query, at most 100. Not `Params`: that caps lists at 10, and the web proxy re-encodes the
/// separators (`,` -> `%2C`, `+` -> `%2B`), so they're split after decoding.
fn item_ids(query: &str) -> Vec<i64> {
    let raw = query.split('&').find_map(|kv| kv.strip_prefix("ids=")).unwrap_or("");
    percent_encoding::percent_decode_str(raw).decode_utf8_lossy().split([',', '+', ' ']).filter_map(|v| v.parse().ok()).take(100).collect()
}

fn lookup(f: fn(&Snapshot) -> &Lookup) -> impl Fn(State<Arc<AppState>>, RawQuery) -> std::future::Ready<Json<Value>> + Clone {
    move |State(s), RawQuery(q)| std::future::ready(Json(f(&s.snap()).find(&Params::parse(q.as_deref().unwrap_or("")))))
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

/// Ctrl+C, or the SIGTERM of `docker stop`: as PID 1 the API would otherwise ignore it until it is killed.
async fn shutdown_signal() {
    #[cfg(unix)]
    {
        let mut term = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()).expect("SIGTERM handler");
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {}
            _ = term.recv() => {}
        }
    }
    #[cfg(not(unix))]
    let _ = tokio::signal::ctrl_c().await;
}

/// Serves until `stop` resolves, then lets open requests finish for at most `drain`: an admin watching the live
/// event stream would otherwise hold the process until it is killed, before view counts are written.
async fn serve_until(
    listener: tokio::net::TcpListener,
    app: Router,
    stop: impl Future<Output = ()> + Send + 'static,
    drain: std::time::Duration,
) -> std::io::Result<()> {
    let (stopping, stopped) = tokio::sync::oneshot::channel();
    let server = axum::serve(listener, app.into_make_service_with_connect_info::<std::net::SocketAddr>())
        .with_graceful_shutdown(async move {
            stop.await;
            let _ = stopping.send(());
        })
        .into_future();
    let mut server = std::pin::pin!(server);
    tokio::select! {
        r = &mut server => r,
        _ = stopped => match tokio::time::timeout(drain, server).await {
            Ok(r) => r,
            Err(_) => {
                eprintln!("shutdown: stopped waiting for open connections after {drain:?}");
                Ok(())
            }
        },
    }
}

/// reqwest is built without a default crypto provider: install ring once, before any client is built.
fn install_tls() {
    rustls::crypto::ring::default_provider().install_default().ok();
}

#[tokio::main]
async fn main() -> Result<()> {
    install_tls();
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
    tokio::spawn(gallery::flush_views(state.clone()));

    // public: catalogue lookups
    let lookups = Router::new()
        .route("/api/avatars", get(lookup(|s| &s.avatars)))
        .route("/api/authors", get(lookup(|s| &s.authors)))
        .route("/api/tags", get(lookup(|s| &s.tags)))
        .route("/api/booth-tags", get(lookup(|s| &s.booth_tags)))
        .route("/api/categories", get(|State(s): State<Arc<AppState>>| async move { Json(s.snap().categories.clone()) }))
        .route_layer(axum::middleware::from_fn_with_state(state.clone(), lookup_rate_limit));

    let app = Router::new()
        // public: health, search, the feed, the palette and item records
        .route("/api/health", get(|| async { Json(json!({ "ok": true })) }))
        .route("/api/search", get(search::search))
        .route("/api/rss", get(rss_feed))
        .route("/api/palette", get(palette))
        .route("/api/items", get(items_by_id))
        .route("/api/items/{id}/gallery", get(gallery::gallery))
        .merge(lookups)
        .merge(account::routes().route_layer(axum::middleware::from_fn_with_state(state.clone(), account_write_rate_limit)))
        .merge(admin::routes())
        // gzip would buffer the admin SSE stream
        .layer(tower_http::compression::CompressionLayer::new().compress_when(
            tower_http::compression::predicate::DefaultPredicate::new()
                .and(tower_http::compression::predicate::NotForContentType::const_new("text/event-stream")),
        ))
        // no credentials cross-origin: the frontend is served same-origin (Next.js rewrites /api)
        .layer(tower_http::cors::CorsLayer::new().allow_origin(tower_http::cors::Any).allow_methods([axum::http::Method::GET]))
        .with_state(state.clone());

    // hand the address over: the placeholder finishes its open requests and releases the port
    let _ = ready.send(());
    starting.await?;
    let listener = tokio::net::TcpListener::bind(&bind).await?;
    println!("listening on http://{bind}");
    // docker stop waits 10 s before it kills
    serve_until(listener, app, shutdown_signal(), std::time::Duration::from_secs(5)).await?;
    // the opens counted since the last write
    if let Err(e) = state.view_counts.flush(&state.pool).await {
        eprintln!("view counts: {e}");
    }
    println!("stopped");
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
            let mut query = super::search::nearest_query(&[0.5; boothdex_core::embed::DIMS], &models);
            assert_eq!(query.take_arguments().unwrap().unwrap().len(), if models.is_empty() { 1 } else { 2 });
            assert_eq!(query.sql().as_str().contains("$2"), !models.is_empty());
        }
    }

    #[test]
    fn queries_are_normalized_for_the_cache() {
        assert_eq!(super::search::normalize_query("  Gothic   Dress "), "gothic dress");
    }

    fn test_state() -> std::sync::Arc<super::AppState> {
        let catalog = super::catalog::Catalog::build(Vec::new(), 0, None).unwrap();
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
        super::install_tls();
        std::sync::Arc::new(
            super::AppState::new(
                sqlx::postgres::PgPoolOptions::new().connect_lazy("postgres://localhost/unused").unwrap(),
                snap,
                super::limit::Limits {
                    search: super::limit::RateLimiter::new(2.0, 3.0, 64),
                    palette: super::limit::RateLimiter::new(5.0, 2.0, 64),
                    rss: super::limit::RateLimiter::new(0.5, 2.0, 64),
                    items: super::limit::RateLimiter::new(1.0, 2.0, 64),
                    item: super::limit::RateLimiter::new(4.0, 2.0, 64),
                    gallery: super::limit::RateLimiter::new(1.0, 2.0, 64),
                    lookup: super::limit::RateLimiter::new(5.0, 2.0, 64),
                    embed_ip: super::limit::RateLimiter::new(0.25, 8.0, 64),
                    embed_global: super::limit::RateLimiter::new(2.0, 20.0, 1),
                    auth: super::limit::RateLimiter::new(0.2, 5.0, 64),
                    account: super::limit::RateLimiter::new(2.0, 2.0, 64),
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
        use axum::response::IntoResponse;

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
                super::search::search(State(state), ip, HeaderMap::new(), RawQuery(None))
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
            assert_eq!(super::palette(State(state.clone()), ip_of(ip).await, RawQuery(Some("q=dress".into()))).await.into_response().status(), StatusCode::OK);
        }
        assert_eq!(super::palette(State(state.clone()), ip_of(ip).await, RawQuery(Some("q=dress".into()))).await.into_response().status(), StatusCode::TOO_MANY_REQUESTS);

        for _ in 0..2 {
            assert!(state.limits.lookup.check(ip_of(ip).await.0).is_ok());
        }
        assert!(state.limits.lookup.check(ip_of(ip).await.0).is_err());

        for _ in 0..2 {
            assert_eq!(super::rss_feed(State(state.clone()), ip_of(ip).await, RawQuery(None)).await.into_response().status(), StatusCode::OK);
        }
        assert_eq!(super::rss_feed(State(state.clone()), ip_of(ip).await, RawQuery(None)).await.into_response().status(), StatusCode::TOO_MANY_REQUESTS);

        for _ in 0..2 {
            assert_eq!(super::items_by_id(State(state.clone()), ip_of(ip).await, RawQuery(None)).await.into_response().status(), StatusCode::OK);
        }
        assert_eq!(super::items_by_id(State(state.clone()), ip_of(ip).await, RawQuery(None)).await.into_response().status(), StatusCode::TOO_MANY_REQUESTS);

        use axum::extract::Path;
        for _ in 0..2 {
            assert_eq!(super::gallery::gallery(State(state.clone()), ip_of(ip).await, Path(999)).await.into_response().status(), StatusCode::NOT_FOUND);
        }
        assert_eq!(super::gallery::gallery(State(state.clone()), ip_of(ip).await, Path(999)).await.into_response().status(), StatusCode::TOO_MANY_REQUESTS);

        // /api/search for that same IP still succeeds with its full burst intact
        for _ in 0..3 {
            assert_eq!(call(ip).await.status(), StatusCode::OK);
        }
        assert_eq!(call(ip).await.status(), StatusCode::TOO_MANY_REQUESTS);
    }

    #[tokio::test]
    async fn search_capacity_rejections_keep_retry_after() {
        use axum::response::IntoResponse;

        let state = test_state();
        let _held = state.search_permits.acquire_many(16).await.unwrap();
        for closed in [false, true] {
            if closed {
                state.search_permits.close();
            }
            let response = state.acquire_search().await.unwrap_err().into_response();
            assert_eq!(response.status(), axum::http::StatusCode::TOO_MANY_REQUESTS);
            assert_eq!(response.headers()[axum::http::header::RETRY_AFTER], "2");
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn search_cpu_timing_excludes_the_permit_queue() {
        use axum::body::to_bytes;
        use axum::extract::{RawQuery, State};
        use axum::http::HeaderMap;
        use axum::response::IntoResponse;

        let state = test_state();
        let held = state.search_permits.acquire_many(16).await.unwrap();
        let request = super::search::search(
            State(state.clone()), super::limit::ClientIp("203.0.113.70".parse().unwrap()), HeaderMap::new(), RawQuery(None),
        );
        tokio::pin!(request);
        // Poll all the way to the exhausted semaphore before starting the controlled wait.
        assert!(futures_util::poll!(&mut request).is_pending());
        tokio::time::sleep(std::time::Duration::from_millis(30)).await;
        drop(held);
        let response = request.await.unwrap_or_else(IntoResponse::into_response);
        assert_eq!(response.status(), axum::http::StatusCode::OK);
        let body: serde_json::Value = serde_json::from_slice(&to_bytes(response.into_body(), 1024 * 1024).await.unwrap()).unwrap();
        let steps = body["trace"].as_array().unwrap();
        let ms = |step: &str| steps.iter().find(|s| s["step"] == step).unwrap()["ms"].as_f64().unwrap();
        assert!(ms("queue") >= 25.0, "{body}");
        assert!(body["timing"]["searchMs"].as_f64().unwrap() <= ms("search") + 1.0, "{body}");
        assert!(body["timing"]["totalMs"].as_f64().unwrap() >= ms("queue"), "{body}");
    }

    #[tokio::test]
    async fn auth_limiter_exhaustion_throttles_both_login_and_register() {
        use axum::extract::{Json, State};
        use axum::http::{HeaderMap, StatusCode};
        use axum::response::IntoResponse;

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
                    super::account::login(State(state.clone()), ip, HeaderMap::new(), Json(body)).await.into_response()
                }
                AuthCall::Register(body) => {
                    super::account::register(State(state.clone()), ip, HeaderMap::new(), Json(body)).await.into_response()
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
        assert!((super::search::semantic_cutoff(0.787, false) - 0.727).abs() < 1e-9);
        assert!(0.707 < super::search::semantic_cutoff(0.707, false));
        assert!(0.707 < super::search::semantic_cutoff(0.707, true));
        assert!((super::search::semantic_cutoff(0.830, true) - 0.730).abs() < 1e-9);
    }

    #[test]
    fn queries_naming_a_category_find_it() {
        let cats = serde_json::json!([
            {"slug": "hair", "en": "Hair", "ja": "髪型", "children": [
                {"slug": "hair--short", "en": "Short", "ja": "ショート"}, {"slug": "hair--other", "en": "Other"}]},
            {"slug": "clothing", "en": "Clothing", "children": [{"slug": "clothing--other", "en": "Other"}]}
        ]);
        let m = super::snapshot::category_phrases(&cats);
        for q in ["shorthair", "hairshort", "short", "ショート"] {
            assert_eq!(m.get(q).map(String::as_str), Some("hair--short"), "{q}");
        }
        assert_eq!(m.get("hair").map(String::as_str), Some("hair"));
        // a child name two parents share only counts together with its parent
        assert_eq!(m.get("other"), None);
        assert_eq!(m.get("otherclothing").map(String::as_str), Some("clothing--other"));
    }

    #[tokio::test]
    async fn shutdown_lets_requests_finish_but_not_hold_it_for_ever() {
        use std::time::{Duration, Instant};
        let app = axum::Router::new()
            .route("/slow", axum::routing::get(|| async {
                tokio::time::sleep(Duration::from_millis(300)).await;
                "done"
            }))
            .route("/hang", axum::routing::get(std::future::pending::<&'static str>));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
        let server = tokio::spawn(super::serve_until(listener, app, async { stopped.await.ok(); }, Duration::from_secs(1)));
        super::install_tls();
        let http = reqwest::Client::new();
        let slow = tokio::spawn(http.get(format!("{base}/slow")).send());
        let _hang = tokio::spawn(http.get(format!("{base}/hang")).send());
        tokio::time::sleep(Duration::from_millis(100)).await;

        let t0 = Instant::now();
        stop.send(()).unwrap();
        server.await.unwrap().unwrap();
        let took = t0.elapsed();
        assert!(took >= Duration::from_millis(900) && took < Duration::from_secs(3), "took {took:?}");
        assert_eq!(slow.await.unwrap().unwrap().text().await.unwrap(), "done");
    }

    #[tokio::test]
    async fn account_writes_are_limited_per_visitor_and_reads_are_not() {
        let state = test_state();
        let app = axum::Router::new()
            .route("/x", axum::routing::get(|| async { "ok" }).post(|| async { "ok" }))
            .route_layer(axum::middleware::from_fn_with_state(state.clone(), super::account_write_rate_limit))
            .with_state(state);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/x", listener.local_addr().unwrap());
        tokio::spawn(async move { axum::serve(listener, app).await });
        let http = reqwest::Client::new();
        let status = |r: reqwest::RequestBuilder| async move { r.send().await.unwrap().status().as_u16() };

        for _ in 0..2 {
            assert_eq!(status(http.post(&url)).await, 200);
        }
        assert_eq!(status(http.post(&url)).await, 429);
        for _ in 0..5 {
            assert_eq!(status(http.get(&url)).await, 200);
        }
        assert_eq!(status(http.post(&url).header("x-forwarded-for", "203.0.113.5")).await, 200);
    }

    /// What a client reads without a session, and when the API itself fails: the status and the JSON body.
    #[tokio::test]
    async fn missing_sessions_and_failures_keep_their_status_and_body() {
        use axum::extract::{RawQuery, State};
        use axum::http::{HeaderMap, StatusCode};
        use axum::response::IntoResponse;
        use serde_json::{Value, json};

        async fn answer(r: impl IntoResponse) -> (StatusCode, Value) {
            let r = r.into_response();
            let status = r.status();
            (status, serde_json::from_slice(&axum::body::to_bytes(r.into_body(), 1 << 20).await.unwrap()).unwrap())
        }

        let ip = super::limit::ClientIp("203.0.113.60".parse().unwrap());
        let favourites = super::search::search(State(test_state()), ip, HeaderMap::new(), RawQuery(Some("v=favourites".into()))).await;
        assert_eq!(answer(favourites).await, (StatusCode::UNAUTHORIZED, json!({ "ok": false, "error": "Sign in required" })));

        // the cause goes to the log, never to the caller
        let failure: super::ApiError = std::io::Error::other("the cause").into();
        assert_eq!(answer(failure).await, (StatusCode::INTERNAL_SERVER_ERROR, json!({ "ok": false, "error": "internal error" })));
    }

    /// The feed, the palette and item records queue for the search's permits and answer 429 when none comes.
    #[tokio::test]
    async fn the_feed_the_palette_and_item_records_wait_for_a_search_permit() {
        use axum::extract::{RawQuery, State};
        use axum::http::{StatusCode, header};
        use axum::response::IntoResponse;

        let state = test_state();
        // a closed semaphore refuses at once; a full one refuses the same way after two seconds
        state.search_permits.close();
        let ip = super::limit::ClientIp("203.0.113.70".parse().unwrap());
        let answers = [
            super::rss_feed(State(state.clone()), ip, RawQuery(None)).await.into_response(),
            super::palette(State(state.clone()), ip, RawQuery(Some("q=dress".into()))).await.into_response(),
            super::items_by_id(State(state.clone()), ip, RawQuery(Some("ids=1".into()))).await.into_response(),
        ];
        for r in answers {
            assert_eq!(r.status(), StatusCode::TOO_MANY_REQUESTS);
            assert_eq!(r.headers()[header::RETRY_AFTER], "2");
        }
    }
}
