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
mod items;
mod limit;
mod lookups;
mod palette;
mod params;
mod rss;
mod search;
mod search_request;
mod snapshot;
mod state;
mod text;

#[cfg(test)]
mod route_tests;
#[cfg(test)]
mod search_tests;

use std::sync::Arc;

use anyhow::Result;
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use tower_http::compression::Predicate as _;
use serde_json::json;
use sqlx::PgPool;

use account::account_write_rate_limit;
use error::ApiError;
use items::items_by_id;
use lookups::{categories, lookup, lookup_rate_limit};
use palette::palette;
use rss::rss_feed;
#[cfg(test)]
use snapshot::{Lookup, Snapshot};
use snapshot::{build_snapshot, catalog_version, reloader};
use state::AppState;

async fn build_state(pool: PgPool, admin_names: Option<Vec<String>>) -> Result<AppState> {
    let version = catalog_version(&pool).await?;
    let snap = build_snapshot(&pool, version).await?;
    let state = AppState::new(
        pool,
        snap,
        Default::default(),
        std::env::var("SITE_URL").unwrap_or_else(|_| "http://127.0.0.1:3000".into()),
        std::env::var("IMGPROXY_URL").ok().or_else(|| Some("http://127.0.0.1:3001".into())),
        account::dummy_hash(),
        Some(std::time::Duration::from_secs(10)),
    )?;
    Ok(AppState { admin_names, ..state })
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
    let admin_names = admin::admin_names();
    for name in admin::apply_admin_names(&pool, admin_names.as_deref()).await? {
        // registration refuses a listed name, so a typo or a name listed before its account exists stays unused
        eprintln!("warn: ADMIN_USERNAMES lists {name}, which no account has; unlist it, register it, then list it again");
    }
    // AI configured through environment variables: translate the taxonomy on first start
    admin::queue_localize_if_needed(&pool, &boothdex_core::settings::ai(&pool).await?).await?;
    let state = Arc::new(build_state(pool, admin_names).await?);
    tokio::spawn(reloader(state.clone()));
    tokio::spawn(gallery::flush_views(state.clone()));

    // public: catalogue lookups
    let lookups = Router::new()
        .route("/api/avatars", get(lookup(|s| &s.avatars)))
        .route("/api/authors", get(lookup(|s| &s.authors)))
        .route("/api/tags", get(lookup(|s| &s.tags)))
        .route("/api/booth-tags", get(lookup(|s| &s.booth_tags)))
        .route("/api/categories", get(categories))
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
}
