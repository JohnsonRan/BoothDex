//! Image proxy meant to sit behind Cloudflare.
//!   GET /b/{size}/{uuid}/i/{item}/{file}[?format=webp|png|jpeg]  Booth image (booth.pximg.net), re-encoded
//!   GET /v/{sha1}.webp                                         archived VRCatalogue image (delisted items)
//! Output defaults to lossy WebP. Results are cached in foyer (memory + disk); responses are immutable so
//! the CDN keeps them. Only booth.pximg.net is ever fetched, and only Booth-shaped paths with Booth's own
//! sizes are accepted, so this is not an open proxy.
//!
//! Env: BIND (127.0.0.1:3001), CACHE_DIR (./imgcache), CACHE_DISK_MB (10240), CACHE_MEM_MB (256),
//!      ARCHIVE_IMAGES (dir of <sha1>.webp files)

use std::io::Cursor;
use std::sync::atomic::{AtomicU64, Ordering};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Result, bail};
use axum::Router;
use axum::extract::{Path, Query, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use foyer::{BlockEngineConfig, DeviceBuilder, FsDeviceBuilder, HybridCache, HybridCacheBuilder};
use image::{DynamicImage, ImageFormat, ImageReader, Limits};
use tokio::sync::Semaphore;

/// Booth's own resize variants; anything else 403s upstream anyway. "orig" = original upload.
const SIZES: &[&str] = &["48x48", "72x72_a2_g5", "300x300_a2_g5", "620x620", "orig"];
const MAX_UPSTREAM_BYTES: usize = 25 * 1024 * 1024;
const IMMUTABLE: &str = "public, max-age=31536000, immutable";
/// Upstream 404s are cached (as an empty value) — Booth image paths are content-addressed and never come back
const NOT_FOUND_CACHE: &str = "public, max-age=86400";

#[derive(Clone, Copy, PartialEq, Debug)]
enum Format {
    Webp,
    Png,
    Jpeg,
}

impl Format {
    fn parse(s: Option<&str>) -> Option<Self> {
        match s {
            None | Some("webp") => Some(Self::Webp),
            Some("png") => Some(Self::Png),
            Some("jpeg" | "jpg") => Some(Self::Jpeg),
            _ => None,
        }
    }
    fn mime(self) -> &'static str {
        match self {
            Self::Webp => "image/webp",
            Self::Png => "image/png",
            Self::Jpeg => "image/jpeg",
        }
    }
}

#[derive(Default)]
struct Stats {
    requests: AtomicU64,
    /// cache misses that went to Booth (or failed trying)
    upstream: AtomicU64,
    errors: AtomicU64,
    not_found: AtomicU64,
    bytes_out: AtomicU64,
}

struct AppState {
    stats: Stats,
    mem_mb: usize,
    disk_mb: usize,
    cache: HybridCache<String, Vec<u8>>,
    client: reqwest::Client,
    transcode: Semaphore,
    archive: Option<PathBuf>,
}

fn is_uuid(s: &str) -> bool {
    s.len() == 36 && s.char_indices().all(|(i, c)| if [8, 13, 18, 23].contains(&i) { c == '-' } else { c.is_ascii_hexdigit() && !c.is_ascii_uppercase() })
}

fn is_booth_file(s: &str) -> bool {
    let Some((stem, ext)) = s.rsplit_once('.') else { return false };
    (1..=120).contains(&stem.len())
        && stem.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        && ["jpg", "jpeg", "png", "gif"].contains(&ext.to_ascii_lowercase().as_str())
}

/// Upstream URL for a validated request, or None if the path isn't Booth-shaped.
fn upstream_url(size: &str, uuid: &str, item: &str, file: &str) -> Option<String> {
    if !SIZES.contains(&size) || !is_uuid(uuid) || item.is_empty() || item.len() > 12 || !item.bytes().all(|b| b.is_ascii_digit()) || !is_booth_file(file) {
        return None;
    }
    let prefix = if size == "orig" { String::new() } else { format!("c/{size}/") };
    Some(format!("https://booth.pximg.net/{prefix}{uuid}/i/{item}/{file}"))
}

fn decode(bytes: &[u8]) -> Result<DynamicImage> {
    let mut reader = ImageReader::new(Cursor::new(bytes)).with_guessed_format()?;
    let mut limits = Limits::default();
    limits.max_image_width = Some(12_000);
    limits.max_image_height = Some(12_000);
    limits.max_alloc = Some(512 * 1024 * 1024);
    reader.limits(limits);
    Ok(reader.decode()?)
}

fn encode(img: &DynamicImage, format: Format, quality: f32) -> Result<Vec<u8>> {
    Ok(match format {
        Format::Webp => {
            // the webp crate only takes 8-bit RGB/RGBA
            let img = if img.color().has_alpha() { DynamicImage::ImageRgba8(img.to_rgba8()) } else { DynamicImage::ImageRgb8(img.to_rgb8()) };
            let enc = webp::Encoder::from_image(&img).map_err(|e| anyhow::anyhow!("webp: {e}"))?;
            enc.encode(quality).to_vec()
        }
        Format::Png => {
            let mut out = Cursor::new(Vec::new());
            img.write_to(&mut out, ImageFormat::Png)?;
            out.into_inner()
        }
        Format::Jpeg => {
            let mut out = Vec::new();
            image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, quality as u8).encode_image(&DynamicImage::ImageRgb8(img.to_rgb8()))?;
            out
        }
    })
}

async fn fetch_upstream(client: &reqwest::Client, url: &str) -> Result<Option<Vec<u8>>> {
    // one retry: transient connect errors were seen right after startup
    let mut resp = match client.get(url).send().await {
        Ok(r) => r,
        Err(_) => {
            tokio::time::sleep(Duration::from_millis(300)).await;
            // Debug keeps reqwest's source chain (TLS/DNS/connect detail) that Display drops
            client.get(url).send().await.map_err(|e| anyhow::anyhow!("{e:?}"))?
        }
    };
    // only 404 is cached as permanent; a 403 could be a transient block (e.g. a future Referer check) and must not stick
    if resp.status() == StatusCode::NOT_FOUND {
        return Ok(None);
    }
    if !resp.status().is_success() {
        bail!("upstream {} for {url}", resp.status());
    }
    if resp.content_length().is_some_and(|n| n as usize > MAX_UPSTREAM_BYTES) {
        bail!("upstream too large: {url}");
    }
    let mut body = Vec::new();
    while let Some(chunk) = resp.chunk().await? {
        body.extend_from_slice(&chunk);
        if body.len() > MAX_UPSTREAM_BYTES {
            bail!("upstream too large: {url}");
        }
    }
    Ok(Some(body))
}

fn image_response(bytes: Vec<u8>, format: Format) -> Response {
    if bytes.is_empty() {
        return (StatusCode::NOT_FOUND, [(header::CACHE_CONTROL, NOT_FOUND_CACHE)], "not found").into_response();
    }
    ([(header::CONTENT_TYPE, format.mime()), (header::CACHE_CONTROL, IMMUTABLE)], bytes).into_response()
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
    let key = format!("{size}/{uuid}/{item}/{file}:{format:?}");
    let quality = if size == "orig" { 88.0 } else { 80.0 };
    s.stats.requests.fetch_add(1, Ordering::Relaxed);
    let st = s.clone();
    let result = s
        .cache
        .get_or_fetch(&key, move || async move {
            st.stats.upstream.fetch_add(1, Ordering::Relaxed);
            let Some(raw) = fetch_upstream(&st.client, &url).await? else { return Ok::<_, anyhow::Error>(Vec::new()) };
            let _permit = st.transcode.acquire().await?;
            tokio::task::spawn_blocking(move || encode(&decode(&raw)?, format, quality)).await?
        })
        .await;
    match result {
        Ok(entry) => {
            let bytes = entry.value().clone();
            if bytes.is_empty() {
                s.stats.not_found.fetch_add(1, Ordering::Relaxed);
            }
            s.stats.bytes_out.fetch_add(bytes.len() as u64, Ordering::Relaxed);
            image_response(bytes, format)
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
        Ok(bytes) => image_response(bytes, Format::Webp),
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

fn env_num(key: &str, default: usize) -> usize {
    std::env::var(key).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

#[tokio::main]
async fn main() -> Result<()> {
    rustls::crypto::ring::default_provider().install_default().ok();
    let dir = std::env::var("CACHE_DIR").unwrap_or_else(|_| "imgcache".into());
    std::fs::create_dir_all(&dir)?;
    let (mem_mb, disk_mb) = (env_num("CACHE_MEM_MB", 256), env_num("CACHE_DISK_MB", 10_240));
    let device = FsDeviceBuilder::new(&dir).with_capacity(disk_mb * 1024 * 1024).build()?;
    let cache = HybridCacheBuilder::new()
        .memory(mem_mb * 1024 * 1024)
        .with_weighter(|k: &String, v: &Vec<u8>| k.len() + v.len())
        .storage()
        .with_engine_config(BlockEngineConfig::new(device))
        .build()
        .await?;
    let state = Arc::new(AppState {
        stats: Stats::default(),
        mem_mb,
        disk_mb,
        cache,
        client: reqwest::Client::builder().timeout(Duration::from_secs(20)).build()?,
        transcode: Semaphore::new(std::thread::available_parallelism().map_or(4, |n| n.get())),
        archive: std::env::var("ARCHIVE_IMAGES").ok().map(PathBuf::from),
    });
    let app = Router::new()
        .route("/b/{size}/{uuid}/i/{item}/{file}", get(booth))
        .route("/v/{name}", get(archived))
        .route("/stats", get(stats))
        .with_state(state.clone());
    let bind = std::env::var("BIND").unwrap_or_else(|_| "127.0.0.1:3001".into());
    println!("imgproxy listening on http://{bind}, cache {dir}");
    axum::serve(tokio::net::TcpListener::bind(&bind).await?, app).await?;
    state.cache.close().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_booth_shaped_paths_pass() {
        let uuid = "dc33a5b5-f8f7-418f-8087-393ea802b496";
        assert_eq!(
            upstream_url("300x300_a2_g5", uuid, "9064622", "7da36066-7fd0_base_resized.jpg").as_deref(),
            Some("https://booth.pximg.net/c/300x300_a2_g5/dc33a5b5-f8f7-418f-8087-393ea802b496/i/9064622/7da36066-7fd0_base_resized.jpg")
        );
        assert_eq!(upstream_url("orig", uuid, "1", "a.png").as_deref(), Some("https://booth.pximg.net/dc33a5b5-f8f7-418f-8087-393ea802b496/i/1/a.png"));
        assert!(upstream_url("999x999", uuid, "1", "a.jpg").is_none()); // unknown size
        assert!(upstream_url("orig", "not-a-uuid", "1", "a.jpg").is_none());
        assert!(upstream_url("orig", uuid, "1a", "a.jpg").is_none());
        assert!(upstream_url("orig", uuid, "1", "../../etc.jpg").is_none());
        assert!(upstream_url("orig", uuid, "1", "a.svg").is_none());
        assert!(upstream_url("orig", uuid, "1", "a.jpg?x=@evil.com").is_none());
    }

    #[test]
    fn transcodes_to_each_format() {
        let img = DynamicImage::ImageRgb8(image::RgbImage::from_fn(32, 16, |x, y| image::Rgb([x as u8 * 8, y as u8 * 16, 128])));
        let mut png = Cursor::new(Vec::new());
        img.write_to(&mut png, ImageFormat::Png).unwrap();
        let decoded = decode(png.get_ref()).unwrap();
        let webp = encode(&decoded, Format::Webp, 80.0).unwrap();
        assert_eq!(&webp[0..4], b"RIFF");
        assert_eq!(&webp[8..12], b"WEBP");
        assert_eq!(&encode(&decoded, Format::Jpeg, 90.0).unwrap()[0..2], &[0xFF, 0xD8]);
        assert_eq!(&encode(&decoded, Format::Png, 0.0).unwrap()[1..4], b"PNG");
        assert!(decode(b"not an image").is_err());
        assert_eq!(Format::parse(Some("gif")), None);
    }
}
