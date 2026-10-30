//! Search API. Builds the in-memory catalog from Postgres at startup.
//! Usage: DATABASE_URL=postgres://... [BIND=0.0.0.0:3000] cargo run --release -p api

mod catalog;
mod db;
mod params;
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

struct AppState {
    pool: PgPool,
    catalog: Catalog,
    avatars: Lookup,
    authors: Lookup,
    tags: Lookup,
    booth_tags: Lookup,
    categories: Value,
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

async fn search(State(s): State<Arc<AppState>>, RawQuery(q): RawQuery) -> Result<Json<Value>, ApiError> {
    let t0 = Instant::now();
    let p = Params::parse(q.as_deref().unwrap_or(""));
    let catalog_now = chrono::Utc::now().timestamp();
    let r = tokio::task::block_in_place(|| s.catalog.search(&p, catalog_now))?;
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
    })))
}

fn lookup(f: fn(&AppState) -> &Lookup) -> impl Fn(State<Arc<AppState>>, RawQuery) -> std::future::Ready<Json<Value>> + Clone {
    move |State(s), RawQuery(q)| std::future::ready(Json(f(&s).find(&Params::parse(q.as_deref().unwrap_or("")))))
}

async fn build_state(pool: PgPool) -> Result<AppState> {
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

    Ok(AppState {
        pool,
        catalog,
        avatars: Lookup::new(avatars),
        authors: Lookup::new(authors),
        tags: Lookup::new(tags),
        booth_tags: Lookup::new(booth_tags),
        categories: json!({ "categories": categories }),
    })
}

#[tokio::main]
async fn main() -> Result<()> {
    let pool = PgPool::connect(&std::env::var("DATABASE_URL")?).await?;
    catalogue_core::MIGRATOR.run(&pool).await?;
    let state = Arc::new(build_state(pool).await?);

    let app = Router::new()
        .route("/api/search", get(search))
        .route("/api/avatars", get(lookup(|s| &s.avatars)))
        .route("/api/authors", get(lookup(|s| &s.authors)))
        .route("/api/tags", get(lookup(|s| &s.tags)))
        .route("/api/booth-tags", get(lookup(|s| &s.booth_tags)))
        .route("/api/categories", get(|State(s): State<Arc<AppState>>| async move { Json(s.categories.clone()) }))
        .layer(tower_http::compression::CompressionLayer::new())
        .layer(tower_http::cors::CorsLayer::permissive())
        .with_state(state);

    let bind = std::env::var("BIND").unwrap_or_else(|_| "127.0.0.1:3000".into());
    println!("listening on http://{bind}");
    axum::serve(tokio::net::TcpListener::bind(&bind).await?, app).await?;
    Ok(())
}
