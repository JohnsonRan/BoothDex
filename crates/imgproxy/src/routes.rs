//! The HTTP routes: request validation, the cache in front of the upstream fetch and the transcode, and the
//! operator counters.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use axum::Router;
use axum::extract::{Path, Query, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use foyer::HybridCache;
use tokio::sync::Semaphore;

use crate::encode::{Format, avif_quality, fit_side, transcode};
use crate::upstream::{SIZES, asset_upstream_url, factory_upstream_url, fetch_upstream, icon_upstream_url, upstream_url};

const IMMUTABLE: &str = "public, max-age=31536000, immutable";
/// Upstream 404s are cached (as an empty value) — Booth image paths are content-addressed and never come back
const NOT_FOUND_CACHE: &str = "public, max-age=86400";

#[derive(Default)]
pub(crate) struct Stats {
    requests: AtomicU64,
    /// cache misses that went to Booth (or failed trying)
    upstream: AtomicU64,
    errors: AtomicU64,
    not_found: AtomicU64,
    bytes_out: AtomicU64,
}

pub(crate) struct AppState {
    pub(crate) stats: Stats,
    pub(crate) mem_mb: usize,
    pub(crate) disk_mb: usize,
    pub(crate) cache: HybridCache<String, Vec<u8>>,
    pub(crate) client: reqwest::Client,
    pub(crate) transcode: Semaphore,
    pub(crate) archive: Option<PathBuf>,
}

fn image_response(bytes: Vec<u8>, mime: &'static str) -> Response {
    if bytes.is_empty() {
        return (StatusCode::NOT_FOUND, [(header::CACHE_CONTROL, NOT_FOUND_CACHE)], "not found").into_response();
    }
    ([(header::CONTENT_TYPE, mime), (header::CACHE_CONTROL, IMMUTABLE)], bytes).into_response()
}

fn bad_request() -> Response {
    (StatusCode::BAD_REQUEST, "bad image path").into_response()
}

#[derive(serde::Deserialize)]
struct FormatQuery {
    format: Option<String>,
}

async fn booth(
    State(s): State<Arc<AppState>>,
    Path((size, uuid, item, file)): Path<(String, String, String, String)>,
    Query(q): Query<FormatQuery>,
) -> Response {
    let (Some(url), Some(format)) = (upstream_url(&size, &uuid, &item, &file), Format::parse(q.format.as_deref())) else {
        return bad_request();
    };
    proxy(s, format!("{size}/{uuid}/{item}/{file}:{format:?}"), url, &size, format, None).await
}

async fn icon(
    State(s): State<Arc<AppState>>,
    Path((size, user, file)): Path<(String, String, String)>,
    Query(q): Query<FormatQuery>,
) -> Response {
    let (Some(url), Some(format)) = (icon_upstream_url(&size, &user, &file), Format::parse(q.format.as_deref())) else {
        return bad_request();
    };
    proxy(s, format!("u/{size}/{user}/{file}:{format:?}"), url, &size, format, None).await
}

async fn asset(State(s): State<Arc<AppState>>, Path(file): Path<String>, Query(q): Query<FormatQuery>) -> Response {
    let (Some(url), Some(format)) = (asset_upstream_url(&file), Format::parse(q.format.as_deref())) else {
        return bad_request();
    };
    // a sized variant's quality: the one asset in use, the placeholder icon, is 150 px
    proxy(s, format!("a/{file}:{format:?}"), url, "asset", format, None).await
}

/// The original drawing: the address the site used before it named a size, kept for pages and caches that hold it.
async fn factory(
    State(s): State<Arc<AppState>>,
    Path((uuid, file)): Path<(String, String)>,
    query: Query<FormatQuery>,
) -> Response {
    factory_sized(State(s), Path(("orig".to_owned(), uuid, file)), query).await
}

async fn factory_sized(
    State(s): State<Arc<AppState>>,
    Path((size, uuid, file)): Path<(String, String, String)>,
    Query(q): Query<FormatQuery>,
) -> Response {
    let (Some((key, url, side)), Some(format)) = (factory_request(&size, &uuid, &file), Format::parse(q.format.as_deref())) else {
        return bad_request();
    };
    proxy(s, format!("{key}:{format:?}"), url, &size, format, side).await
}

/// Cache key (without the format), upstream URL and the square to fit into for a pixiv Factory drawing, or None when
/// the path is not one. pixiv Factory has no resize variants, so imgproxy scales the original down itself. The
/// original keeps the key it had before sizes existed, so the cached copies stay valid.
fn factory_request(size: &str, uuid: &str, file: &str) -> Option<(String, String, Option<u32>)> {
    if !SIZES.contains(&size) {
        return None;
    }
    let url = factory_upstream_url(uuid, file)?;
    let key = if size == "orig" { format!("f/{uuid}/{file}") } else { format!("f/{size}/{uuid}/{file}") };
    Some((key, url, fit_side(size)))
}

/// `fit_side`: the square the decoded image is scaled down to before encoding, for an upstream without the size.
async fn proxy(s: Arc<AppState>, key: String, url: String, size: &str, format: Format, fit_side: Option<u32>) -> Response {
    let quality = match format {
        Format::Avif => avif_quality(size),
        _ if size == "orig" => 88,
        _ => 80,
    };
    s.stats.requests.fetch_add(1, Ordering::Relaxed);
    let st = s.clone();
    let result = s
        .cache
        .get_or_fetch(&key, move || async move {
            st.stats.upstream.fetch_add(1, Ordering::Relaxed);
            let Some(raw) = fetch_upstream(&st.client, &url).await? else { return Ok::<_, anyhow::Error>(Vec::new()) };
            let _permit = st.transcode.acquire().await?;
            tokio::task::spawn_blocking(move || transcode(&raw, fit_side, format, quality)).await?
        })
        .await;
    match result {
        Ok(entry) => {
            let bytes = entry.value().clone();
            if bytes.is_empty() {
                s.stats.not_found.fetch_add(1, Ordering::Relaxed);
            }
            s.stats.bytes_out.fetch_add(bytes.len() as u64, Ordering::Relaxed);
            image_response(bytes, format.mime())
        }
        Err(e) => {
            s.stats.errors.fetch_add(1, Ordering::Relaxed);
            eprintln!("{key}: {e:#}");
            (StatusCode::BAD_GATEWAY, [(header::CACHE_CONTROL, "no-store")], "upstream error").into_response()
        }
    }
}

async fn archived(State(s): State<Arc<AppState>>, Path(name): Path<String>) -> Response {
    let Some(sha1) = name.strip_suffix(".webp").filter(|h| h.len() == 40 && h.bytes().all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())) else {
        return bad_request();
    };
    let Some(dir) = &s.archive else { return (StatusCode::NOT_FOUND, "archive not configured").into_response() };
    match tokio::fs::read(dir.join(format!("{sha1}.webp"))).await {
        Ok(bytes) => image_response(bytes, "image/webp"),
        Err(_) => (StatusCode::NOT_FOUND, [(header::CACHE_CONTROL, NOT_FOUND_CACHE)], "not found").into_response(),
    }
}

/// Counters since start, for the admin console.
async fn stats(State(s): State<Arc<AppState>>) -> axum::Json<serde_json::Value> {
    let g = |a: &AtomicU64| a.load(Ordering::Relaxed);
    let (req, up) = (g(&s.stats.requests), g(&s.stats.upstream));
    axum::Json(serde_json::json!({
        "reachable": true,
        "requests": req, "upstream_fetches": up, "hit_rate": if req > 0 { 1.0 - up as f64 / req as f64 } else { 0.0 },
        "errors": g(&s.stats.errors), "not_found": g(&s.stats.not_found), "bytes_out": g(&s.stats.bytes_out),
        "memory_cache_mb": s.mem_mb, "disk_cache_mb": s.disk_mb,
    }))
}

pub(crate) fn router(state: Arc<AppState>) -> Router {
    Router::new()
        // public: the images, which the site forwards from /img
        .route("/b/{size}/{uuid}/i/{item}/{file}", get(booth))
        .route("/u/{size}/{user}/{file}", get(icon))
        .route("/a/{file}", get(asset))
        .route("/f/{uuid}/{file}", get(factory))
        .route("/f/{size}/{uuid}/{file}", get(factory_sized))
        .route("/v/{name}", get(archived))
        // internal: the API's admin overview reads it; the site does not forward it
        .route("/stats", get(stats))
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_sized_factory_request_is_scaled_and_the_original_keeps_its_key() {
        let (uuid, file) = ("04cee5a2-f70c-49f2-b9b6-4c09a6b9bb74", "canvas_94032c5836.png");
        let url = factory_upstream_url(uuid, file).unwrap();
        assert_eq!(factory_request("300x300_a2_g5", uuid, file), Some((format!("f/300x300_a2_g5/{uuid}/{file}"), url.clone(), Some(300))));
        assert_eq!(factory_request("orig", uuid, file), Some((format!("f/{uuid}/{file}"), url, None)));
        assert_eq!(factory_request("999x999", uuid, file), None);
        assert_eq!(factory_request("300x300_a2_g5", uuid, "a.jpg"), None);
    }
}
