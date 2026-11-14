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
use crate::upstream::{
    MAX_UPSTREAM_BYTES, SIZES, UPSTREAM_TIMEOUT, asset_upstream_url, body_bound, factory_upstream_url, icon_upstream_url, open_upstream, read_body,
    upstream_url,
};

const IMMUTABLE: &str = "public, max-age=31536000, immutable";
/// Upstream 404s are cached (as an empty value) — Booth image paths are content-addressed and never come back
const NOT_FOUND_CACHE: &str = "public, max-age=86400";
/// KiB of upstream body all cache misses may hold at once: 8 bodies of the largest size accepted, 200 MiB. With the
/// libaom encodes beside it (about 60 MB each, MAX_TRANSCODES of them, encode.rs) and the 256 MB memory cache that
/// stays near 700 MB, under the container's 1 GB.
pub(crate) const BODY_BUDGET_KIB: usize = 8 * MAX_UPSTREAM_BYTES / 1024;
/// How long a cache miss waits for its share of the body budget before it is answered 503. The upstream timeout
/// runs from the request on, so half of it is left for reading the body once the budget is granted.
pub(crate) const FETCH_WAIT: Duration = Duration::from_secs(UPSTREAM_TIMEOUT.as_secs() / 2);

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
    /// KiB of upstream body, BODY_BUDGET_KIB outside tests. A cache miss reserves its body's bound after the response
    /// headers and before the body, and holds it until its transcode ends. A miss waiting for room holds its upstream
    /// connection and what the HTTP client has received past the headers (for HTTP/2 at most hyper's 2 MB stream
    /// window and 5 MB per connection), not the body.
    pub(crate) body_budget: Semaphore,
    /// How long a cache miss waits for its reservation; FETCH_WAIT outside tests
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
            let Some(resp) = open_upstream(&st.client, &url).await? else { return Ok::<_, anyhow::Error>(Vec::new()) };
            let bound = body_bound(&resp);
            // the one timeout in this closure: an `Elapsed` error below means the budget had no room in time. The bound
            // is at most MAX_UPSTREAM_BYTES, 25,600 KiB, so the cast keeps it.
            let _body = tokio::time::timeout(st.fetch_wait, st.body_budget.acquire_many(bound.div_ceil(1024) as u32)).await??;
            let raw = read_body(resp, bound).await?;
            let _permit = st.transcode.acquire().await?;
            tokio::task::spawn_blocking(move || transcode(&raw, fit_side, format, quality)).await?
        })
        .await;
    match result {
        Err(e) if e.downcast_ref::<Elapsed>().is_some() => {
            s.stats.errors.fetch_add(1, Ordering::Relaxed);
            eprintln!("{key}: busy, no room in the body budget within {:?}", s.fetch_wait);
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

    async fn test_state(budget_kib: usize, fetch_wait: Duration) -> Arc<AppState> {
        let cache = foyer::HybridCacheBuilder::new().memory(1024 * 1024).storage().build().await.unwrap();
        Arc::new(AppState {
            stats: Stats::default(),
            mem_mb: 1,
            disk_mb: 0,
            cache,
            client: crate::upstream::upstream_client().unwrap(),
            body_budget: Semaphore::new(budget_kib),
            fetch_wait,
            transcode: Semaphore::new(1),
            archive: None,
        })
    }

    /// An 8 x 8 PNG followed by `pad` zero bytes, which the decoder ignores.
    fn png(pad: usize) -> Vec<u8> {
        let mut png = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgb8(image::RgbImage::new(8, 8)).write_to(&mut png, image::ImageFormat::Png).unwrap();
        let mut png = png.into_inner();
        png.resize(png.len() + pad, 0);
        png
    }

    /// Serves `app` on loopback. Its URLs are IP literals, which the upstream client allows; no request leaves the host.
    async fn serve(app: Router) -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        base
    }

    fn miss(s: &Arc<AppState>, base: &str, n: usize) -> impl Future<Output = Response> + use<> {
        proxy(s.clone(), format!("k{n}"), format!("{base}/{n}.png"), "300x300_a2_g5", Format::Png, None)
    }

    fn assert_busy(r: &Response) {
        assert_eq!(r.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(r.headers()[header::RETRY_AFTER], FETCH_WAIT.as_secs().to_string());
        assert_eq!(r.headers()[header::CACHE_CONTROL], "no-store");
    }

    /// Loopback only. With the one transcode permit held here, two bodies of about 1 MiB fill the budget and stay
    /// reserved; the other misses find no room, are told to come back after `fetch_wait`, and that answer is not
    /// cached. A cache hit needs no room.
    #[tokio::test]
    async fn misses_past_the_body_budget_get_503() {
        let body = png(1024 * 1024);
        let budget = 2 * body.len().div_ceil(1024);
        let s = test_state(budget, Duration::from_millis(300)).await;
        let base = serve(Router::new().route("/{file}", get(move || std::future::ready(body.clone())))).await;
        let encoding = s.transcode.acquire().await.unwrap();
        let (tx, mut answers) = tokio::sync::mpsc::unbounded_channel();
        for n in 0..5 {
            let (miss, tx) = (miss(&s, &base, n), tx.clone());
            tokio::spawn(async move { tx.send(miss.await).unwrap() });
        }
        let mut next = async || tokio::time::timeout(Duration::from_secs(5), answers.recv()).await.expect("a miss got no answer").unwrap();
        for _ in 0..3 {
            assert_busy(&next().await);
        }
        drop(encoding);
        for _ in 0..2 {
            assert_eq!(next().await.status(), StatusCode::OK);
        }
        for n in 0..5 {
            assert_eq!(miss(&s, &base, n).await.status(), StatusCode::OK);
        }
        let (_budget, _encoding) = (s.body_budget.acquire_many(budget as u32).await.unwrap(), s.transcode.acquire().await.unwrap());
        let hit = tokio::time::timeout(Duration::from_secs(5), miss(&s, &base, 0)).await.expect("a cache hit waited for the budget");
        assert_eq!(hit.status(), StatusCode::OK);
    }

    /// Loopback only. A body that declares no length reserves MAX_UPSTREAM_BYTES, so with a budget of exactly that a
    /// second miss finds no room. The upstream answers HTTP/1.1 by hand, closing the connection to end the body.
    #[tokio::test]
    async fn a_body_without_a_declared_length_reserves_the_largest_size() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(async move {
            while let Ok((mut conn, _)) = listener.accept().await {
                tokio::spawn(async move {
                    let mut request = Vec::new();
                    while !request.ends_with(b"\r\n\r\n") {
                        let mut buf = [0; 1024];
                        let n = conn.read(&mut buf).await.unwrap();
                        request.extend_from_slice(&buf[..n]);
                    }
                    conn.write_all(b"HTTP/1.1 200 OK\r\nconnection: close\r\n\r\n").await.unwrap();
                    conn.write_all(&png(0)).await.unwrap();
                    conn.shutdown().await.unwrap();
                });
            }
        });
        let s = test_state(MAX_UPSTREAM_BYTES / 1024, Duration::from_millis(300)).await;
        let encoding = s.transcode.acquire().await.unwrap();
        let first = tokio::spawn(miss(&s, &base, 0));
        tokio::time::timeout(Duration::from_secs(5), async {
            while s.body_budget.available_permits() > 0 {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("the first miss did not reserve the whole budget");
        assert_busy(&tokio::time::timeout(Duration::from_secs(5), miss(&s, &base, 1)).await.unwrap());
        drop(encoding);
        assert_eq!(first.await.unwrap().status(), StatusCode::OK);
    }

    /// Loopback only. Each upstream response waits until all 20 requests have arrived, so every miss of a page of
    /// thumbnails must be in flight at once: a bound on the number of misses, such as twice the transcodes, hangs.
    #[tokio::test]
    async fn thumbnail_misses_are_not_bounded_by_count() {
        const MISSES: usize = 20;
        let s = test_state(BODY_BUDGET_KIB, FETCH_WAIT).await;
        let arrived = Arc::new(tokio::sync::Barrier::new(MISSES));
        let thumb = png(0);
        let respond = move || {
            let (arrived, thumb) = (arrived.clone(), thumb.clone());
            async move {
                arrived.wait().await;
                thumb
            }
        };
        let base = serve(Router::new().route("/{file}", get(respond))).await;
        let misses: Vec<_> = (0..MISSES).map(|n| tokio::spawn(miss(&s, &base, n))).collect();
        tokio::time::timeout(Duration::from_secs(10), async {
            for m in misses {
                assert_eq!(m.await.unwrap().status(), StatusCode::OK);
            }
        })
        .await
        .expect("the misses were not all in flight at once");
    }

    /// Loopback only, and no request reaches an upstream: each path is routed but refused by its handler's checks.
    #[tokio::test]
    async fn both_factory_routes_reach_their_handler() {
        let state = test_state(BODY_BUDGET_KIB, FETCH_WAIT).await;
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
