//! The HTTP routes: request validation, the cache in front of the upstream fetch and the transcode, and the
//! operator counters.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use axum::Router;
use axum::extract::{Path, Query, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use foyer::HybridCache;
use tokio::sync::Semaphore;
use tokio::time::error::Elapsed;

use crate::encode::{Format, avif_quality, fit_side, transcode};
use crate::upstream::{SIZES, UPSTREAM_TIMEOUT, asset_upstream_url, factory_upstream_url, fetch_upstream, icon_upstream_url, upstream_url};

const IMMUTABLE: &str = "public, max-age=31536000, immutable";
/// Upstream 404s are cached (as an empty value) — Booth image paths are content-addressed and never come back
const NOT_FOUND_CACHE: &str = "public, max-age=86400";
/// How long a cache miss waits for a fetch permit before it is answered 503: as long as one upstream request of the
/// permit holder ahead of it may take. A miss that waits longer is behind a queue the permits cannot drain in time.
pub(crate) const FETCH_WAIT: Duration = UPSTREAM_TIMEOUT;

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
    /// Cache misses that may hold an upstream body at once, from the start of its download to the end of its
    /// transcode; this bounds the bodies in memory and the requests to the upstream hosts.
    pub(crate) fetches: Semaphore,
    /// How long a cache miss waits for one of `fetches`; FETCH_WAIT outside tests
    pub(crate) fetch_wait: Duration,
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
            // the one timeout in this closure: an `Elapsed` error below means no fetch permit came in time
            let _fetch = tokio::time::timeout(st.fetch_wait, st.fetches.acquire()).await??;
            let Some(raw) = fetch_upstream(&st.client, &url).await? else { return Ok::<_, anyhow::Error>(Vec::new()) };
            let _permit = st.transcode.acquire().await?;
            tokio::task::spawn_blocking(move || transcode(&raw, fit_side, format, quality)).await?
        })
        .await;
    match result {
        Err(e) if e.downcast_ref::<Elapsed>().is_some() => {
            s.stats.errors.fetch_add(1, Ordering::Relaxed);
            eprintln!("{key}: busy, no fetch permit within {:?}", s.fetch_wait);
            let retry_after = FETCH_WAIT.as_secs().to_string();
            (StatusCode::SERVICE_UNAVAILABLE, [(header::RETRY_AFTER, retry_after), (header::CACHE_CONTROL, "no-store".to_owned())], "busy").into_response()
        }
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

    async fn test_state(fetches: usize, fetch_wait: Duration) -> Arc<AppState> {
        let cache = foyer::HybridCacheBuilder::new().memory(1024 * 1024).storage().build().await.unwrap();
        Arc::new(AppState {
            stats: Stats::default(),
            mem_mb: 1,
            disk_mb: 0,
            cache,
            client: crate::upstream::upstream_client().unwrap(),
            fetches: Semaphore::new(fetches),
            fetch_wait,
            transcode: Semaphore::new(1),
            archive: None,
        })
    }

    /// An upstream on loopback that answers every `/{file}` with the same small PNG, and the count of requests it
    /// has received. Its URLs are IP literals, which the upstream client allows; no request leaves the host.
    async fn counting_upstream() -> (String, Arc<AtomicU64>) {
        let mut png = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgb8(image::RgbImage::new(8, 8)).write_to(&mut png, image::ImageFormat::Png).unwrap();
        let png = png.into_inner();
        let requests = Arc::new(AtomicU64::new(0));
        let seen = requests.clone();
        let app = Router::new().route(
            "/{file}",
            get(move || {
                seen.fetch_add(1, Ordering::SeqCst);
                let png = png.clone();
                async move { png }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        (base, requests)
    }

    fn miss(s: &Arc<AppState>, base: &str, n: usize) -> impl Future<Output = Response> + use<> {
        proxy(s.clone(), format!("k{n}"), format!("{base}/{n}.png"), "300x300_a2_g5", Format::Png, None)
    }

    async fn until(requests: &AtomicU64, n: u64) {
        tokio::time::timeout(Duration::from_secs(10), async {
            while requests.load(Ordering::SeqCst) < n {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("the upstream saw too few requests");
    }

    /// Loopback only. With the one transcode permit held here, no miss can finish, so every download the upstream
    /// sees is a body still held in memory: there must be no more than the fetch permits.
    #[tokio::test]
    async fn uncached_requests_hold_no_more_bodies_than_the_fetch_permits() {
        let s = test_state(2, Duration::from_secs(60)).await;
        let (base, requests) = counting_upstream().await;
        let encoding = s.transcode.acquire().await.unwrap();
        let misses: Vec<_> = (0..10).map(|n| tokio::spawn(miss(&s, &base, n))).collect();
        until(&requests, 2).await;
        tokio::time::sleep(Duration::from_millis(300)).await;
        assert_eq!(requests.load(Ordering::SeqCst), 2, "more bodies downloaded than fetch permits");
        drop(encoding);
        for m in misses {
            assert_eq!(m.await.unwrap().status(), StatusCode::OK);
        }
        assert_eq!(requests.load(Ordering::SeqCst), 10);
        // a cache hit takes no permit: it is served while every fetch and transcode permit is held
        let (_fetches, _encoding) = (s.fetches.acquire_many(2).await.unwrap(), s.transcode.acquire().await.unwrap());
        let hit = tokio::time::timeout(Duration::from_secs(5), miss(&s, &base, 0)).await.expect("a cache hit waited for a permit");
        assert_eq!(hit.status(), StatusCode::OK);
        assert_eq!(requests.load(Ordering::SeqCst), 10);
    }

    /// Loopback only. A miss that gets no fetch permit within `fetch_wait` is told to come back, and the answer is
    /// not cached anywhere: the same request succeeds once a permit is free.
    #[tokio::test]
    async fn a_miss_without_a_fetch_permit_in_time_gets_503() {
        let s = test_state(1, Duration::from_millis(200)).await;
        let (base, requests) = counting_upstream().await;
        let encoding = s.transcode.acquire().await.unwrap();
        let first = tokio::spawn(miss(&s, &base, 0));
        until(&requests, 1).await;
        let busy = tokio::time::timeout(Duration::from_secs(5), miss(&s, &base, 1)).await.expect("the miss waited past fetch_wait");
        assert_eq!(busy.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(busy.headers()[header::RETRY_AFTER], FETCH_WAIT.as_secs().to_string());
        assert_eq!(busy.headers()[header::CACHE_CONTROL], "no-store");
        assert_eq!(requests.load(Ordering::SeqCst), 1);
        drop(encoding);
        assert_eq!(first.await.unwrap().status(), StatusCode::OK);
        assert_eq!(miss(&s, &base, 1).await.status(), StatusCode::OK);
        assert_eq!(requests.load(Ordering::SeqCst), 2);
    }

    /// Loopback only, and no request reaches an upstream: each path is routed but refused by its handler's checks.
    #[tokio::test]
    async fn both_factory_routes_reach_their_handler() {
        let state = test_state(1, FETCH_WAIT).await;
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(async move { axum::serve(listener, router(state)).await.unwrap() });
        let uuid = "04cee5a2-f70c-49f2-b9b6-4c09a6b9bb74";
        let client = reqwest::Client::builder().no_proxy().build().unwrap();
        let status = |path: String| {
            let request = client.get(format!("{base}/{path}"));
            async move { request.send().await.unwrap().status() }
        };
        // a handler answers 400 for a path it refuses; a path no route matches gets axum's 404
        assert_eq!(status(format!("f/999x999/{uuid}/a.png")).await, StatusCode::BAD_REQUEST);
        assert_eq!(status(format!("f/{uuid}/a.jpg")).await, StatusCode::BAD_REQUEST);
        assert_eq!(status("f/a/b/c/d".to_owned()).await, StatusCode::NOT_FOUND);
    }
}
