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
use serde_json::{Value, json};
use sqlx::PgPool;

use catalog::Catalog;
use params::Params;

/// Name lookup for sidebar pickers and the command palette.
struct Lookup(Vec<(Vec<String>, Value, u64)>);

impl Lookup {
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
    let catalog_now = chrono::Utc::now().timestamp();
    let r = tokio::task::block_in_place(|| s.snap().catalog.search(&p, catalog_now, favourites.as_ref()))?;
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
        "timing": { "searchMs": search_ms, "totalMs": t0.elapsed().as_secs_f64() * 1e3 },
    }))
    .into_response())
}

async fn rss_feed(State(s): State<Arc<AppState>>, RawQuery(q): RawQuery) -> Result<Response, ApiError> {
    let user_q = q.unwrap_or_default();
    // first occurrence wins in Params, so the prefix pins ordering and size
    let p = Params::parse(&format!("sort=newest&dir=desc&page=1&pageSize=50&{user_q}"));
    let now = chrono::Utc::now().timestamp();
    let r = tokio::task::block_in_place(|| s.snap().catalog.search(&p, now, None))?;
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
        .filter(|c| ["en", "ja", "ko"].iter().any(|k| c[*k].as_str().is_some_and(|n| n.to_lowercase().contains(&needle))))
        .take(5)
        .map(|c| json!({ "slug": c["slug"], "en": c["en"], "ja": c["ja"], "ko": c["ko"] }))
        .collect();
    // items honour the caller's adult setting (default safe) like the main search
    let adult = p.get("adult").unwrap_or_else(|| "safe".into());
    let item_params = Params::parse(&format!("q={}&adult={}&sort=relevant&pageSize=8", urlencode(&text), urlencode(&adult)));
    let now = chrono::Utc::now().timestamp();
    let r = tokio::task::block_in_place(|| s.snap().catalog.search(&item_params, now, None))?;
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
    let catalog = tokio::task::spawn_blocking(move || Catalog::build(docs, chrono::Utc::now().timestamp())).await??;
    println!("built index in {:.1}s, bitmaps ~{} KB", t1.elapsed().as_secs_f64(), catalog.bitmap_bytes() / 1024);

    let avatars = sqlx::query_as::<_, (String, String, Option<String>, Option<String>, Option<String>)>("select slug, name_en, name_ja, name_ko, image_url from avatars")
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
    let tags = sqlx::query_as::<_, (i32, String, String, Vec<String>)>("select id, name, display, aliases from smart_tags")
        .fetch_all(&pool)
        .await?
        .into_iter()
        .map(|(id, name, display, aliases)| {
            let n = catalog.smart_tags.len(&name);
            let keys = aliases.iter().cloned().chain([name.clone(), display.clone()]).collect();
            (keys, json!({ "name": name, "display": display, "master_id": id, "aliases": aliases }), n)
        })
        .collect();
    let booth_tags = catalog.booth_tags.keys().map(|t| (vec![t.clone()], json!({ "name": t }), catalog.booth_tags.len(t))).collect();
    let categories: Value = sqlx::query_scalar(
        "select coalesce(jsonb_agg(jsonb_build_object('slug', p.slug, 'en', p.name_en, 'ja', p.name_ja, 'ko', p.name_ko, 'mode', p.mode,
            'children', (select coalesce(jsonb_agg(jsonb_build_object('slug', c.slug, 'en', c.name_en, 'ja', c.name_ja, 'ko', c.name_ko,
                'group_en', c.group_en, 'group_ja', c.group_ja, 'group_ko', c.group_ko) order by c.name_en), '[]')
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
    })
}

#[tokio::main]
async fn main() -> Result<()> {
    let pool = PgPool::connect(&std::env::var("DATABASE_URL")?).await?;
    catalogue_core::MIGRATOR.run(&pool).await?;
    admin::apply_admin_env(&pool).await?;
    let state = Arc::new(build_state(pool).await?);
    tokio::spawn(reloader(state.clone()));

    let app = Router::new()
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
        .layer(tower_http::compression::CompressionLayer::new())
        // no credentials cross-origin: the frontend is served same-origin (Next.js rewrites /api)
        .layer(tower_http::cors::CorsLayer::new().allow_origin(tower_http::cors::Any).allow_methods([axum::http::Method::GET]))
        .with_state(state);

    let bind = std::env::var("BIND").unwrap_or_else(|_| "127.0.0.1:3000".into());
    println!("listening on http://{bind}");
    axum::serve(tokio::net::TcpListener::bind(&bind).await?, app).await?;
    Ok(())
}
