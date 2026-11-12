//! Image proxy meant to sit behind Cloudflare.
//!   GET /b/{size}/{uuid}/i/{item}/{file}[?format=avif|png|jpeg]  Booth image (booth.pximg.net), re-encoded
//!   GET /u/{size}/{user}/{file}[?format=…]                     Booth shop icon (users/{user}/icon_image), same
//!   GET /v/{sha1}.webp                                         archived VRCatalogue image (delisted items)
//! Output defaults to AVIF with full-resolution chroma (4:4:4), from libaom when built with the `libavif`
//! feature (the container images are) and from rav1e otherwise; the archived files are WebP and served as they
//! are. Results are cached in foyer (memory + disk); responses are immutable so the CDN keeps them, which is
//! why the site names the format in its URLs. Only booth.pximg.net is ever fetched, and only Booth-shaped paths
//! with Booth's own sizes are accepted, so this is not an open proxy.
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

/// AVIF quality by size, on libavif's 0..=100 scale. Both values are smaller and closer to the source than the
/// WebP served before (80 for the sized variants, 88 for the original), measured through this service on 24 Booth
/// images with SSIMULACRA2, medians:
/// - 60 for the sized variants: 10% fewer bytes and 2.8 points higher at 300 px (21 of 24 images better), 11%
///   and 3.1 at 620 px. 55 would be 23% smaller but half the thumbnails would score below their WebP.
/// - 75 for the original, the image the item dialog shows: 14% fewer bytes and 2.5 points higher (20 of 24).
///   70 would be 23% smaller for a level score.
#[cfg(feature = "libavif")]
fn avif_quality(size: &str) -> u8 {
    match size {
        "orig" => 75,
        _ => 60,
    }
}

/// AVIF quality by size for a build without libavif, on ravif's own 1..=100 scale, which is neither libaom's nor
/// the one libavif-style tools use: its 75 measured softer than the WebP this replaced. On 12 Booth images,
/// scored with SSIMULACRA2 against the WebP 80 (sized) and 88 (original) served before, rav1e's 83 is 2% smaller
/// at 300 px for a level score, and its 85 scores 3.5 points higher at 620 px for 16% more bytes and is 22%
/// smaller at the original size for about the same score.
#[cfg(not(feature = "libavif"))]
fn avif_quality(size: &str) -> u8 {
    match size {
        "620x620" | "orig" => 85,
        _ => 83,
    }
}

/// Both encoders: 1 (slowest) ..= 10. On 2 CPUs, libaom at 6 takes about 40 ms for a 300 px thumbnail and 180 ms
/// for an original; rav1e at 6 about 50 ms and 420 ms (asm, 2 threads), its 8 a quarter less for 3% larger files,
/// its 10 over a third larger.
const AVIF_SPEED: u8 = 6;
/// libaom's threads for one image; several images are encoded at once (MAX_TRANSCODES below)
#[cfg(feature = "libavif")]
const AOM_THREADS: std::ffi::c_int = 2;
/// Images libaom encodes at once, at most one per core. It holds about 60 MB for an original, and the container
/// is limited to 1 GB with a 256 MB memory cache in it: a burst of 24 originals peaked at 260 MB with 2 at once,
/// about 400 MB with 4 and 630 MB with 8 (the test's memory cache was 64 MB).
#[cfg(feature = "libavif")]
const MAX_TRANSCODES: usize = 4;

#[derive(Clone, Copy, PartialEq, Debug)]
enum Format {
    Avif,
    Png,
    Jpeg,
}

impl Format {
    fn parse(s: Option<&str>) -> Option<Self> {
        match s {
            None | Some("avif") => Some(Self::Avif),
            Some("png") => Some(Self::Png),
            Some("jpeg" | "jpg") => Some(Self::Jpeg),
            _ => None,
        }
    }
    fn mime(self) -> &'static str {
        match self {
            Self::Avif => "image/avif",
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

/// Upstream URL for a shop icon; Booth names the file after a UUID, so a new icon is a new address.
fn icon_upstream_url(size: &str, user: &str, file: &str) -> Option<String> {
    if !SIZES.contains(&size) || user.is_empty() || user.len() > 12 || !user.bytes().all(|b| b.is_ascii_digit()) || !is_booth_file(file) {
        return None;
    }
    let prefix = if size == "orig" { String::new() } else { format!("c/{size}/") };
    Some(format!("https://booth.pximg.net/{prefix}users/{user}/icon_image/{file}"))
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

/// 8-bit AVIF with full-resolution chroma (4:4:4), from libaom through the system's libavif (src/aom.c).
#[cfg(feature = "libavif")]
fn encode_avif(img: &DynamicImage, quality: u8) -> Result<Vec<u8>> {
    use std::ffi::{CStr, c_char, c_int};
    unsafe extern "C" {
        fn boothdex_avif_encode(pixels: *const u8, width: u32, height: u32, has_alpha: c_int, quality: c_int, speed: c_int, threads: c_int, out: *mut *mut u8, out_len: *mut usize) -> c_int;
        fn boothdex_avif_free(data: *mut u8);
        fn boothdex_avif_error(code: c_int) -> *const c_char;
    }
    let has_alpha = img.color().has_alpha();
    let pixels = if has_alpha { img.to_rgba8().into_raw() } else { img.to_rgb8().into_raw() };
    let (mut data, mut len) = (std::ptr::null_mut(), 0usize);
    // SAFETY: `pixels` holds width * height packed RGB or RGBA pixels and is only read; on 0 the callee has set
    // `data` and `len` to a buffer it allocated, copied here and then released with its own allocator.
    unsafe {
        let code = boothdex_avif_encode(pixels.as_ptr(), img.width(), img.height(), has_alpha.into(), quality.into(), AVIF_SPEED.into(), AOM_THREADS, &mut data, &mut len);
        if code != 0 {
            bail!("libavif: {}", CStr::from_ptr(boothdex_avif_error(code)).to_string_lossy());
        }
        let out = std::slice::from_raw_parts(data, len).to_vec();
        boothdex_avif_free(data);
        Ok(out)
    }
}

/// 8-bit AVIF from rav1e; ravif never subsamples chroma, so the output is always 4:4:4.
#[cfg(not(feature = "libavif"))]
fn encode_avif(img: &DynamicImage, quality: u8) -> Result<Vec<u8>> {
    let img = if img.color().has_alpha() { DynamicImage::ImageRgba8(img.to_rgba8()) } else { DynamicImage::ImageRgb8(img.to_rgb8()) };
    let mut out = Vec::new();
    img.write_with_encoder(image::codecs::avif::AvifEncoder::new_with_speed_quality(&mut out, AVIF_SPEED, quality))?;
    Ok(out)
}

fn encode(img: &DynamicImage, format: Format, quality: u8) -> Result<Vec<u8>> {
    Ok(match format {
        Format::Avif => encode_avif(img, quality)?,
        Format::Png => {
            let mut out = Cursor::new(Vec::new());
            img.write_to(&mut out, ImageFormat::Png)?;
            out.into_inner()
        }
        Format::Jpeg => {
            let mut out = Vec::new();
            image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, quality).encode_image(&DynamicImage::ImageRgb8(img.to_rgb8()))?;
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
    proxy(s, format!("{size}/{uuid}/{item}/{file}:{format:?}"), url, &size, format).await
}

async fn icon(
    State(s): State<Arc<AppState>>,
    Path((size, user, file)): Path<(String, String, String)>,
    Query(q): Query<FormatQuery>,
) -> Response {
    let (Some(url), Some(format)) = (icon_upstream_url(&size, &user, &file), Format::parse(q.format.as_deref())) else {
        return bad_request();
    };
    proxy(s, format!("u/{size}/{user}/{file}:{format:?}"), url, &size, format).await
}

async fn proxy(s: Arc<AppState>, key: String, url: String, size: &str, format: Format) -> Response {
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

fn env_num(key: &str, default: usize) -> usize {
    std::env::var(key).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

fn main() -> Result<()> {
    // while this is the only thread (src/aom.c says why); the environment's own setting wins
    #[cfg(feature = "libavif")]
    if std::env::var_os("MALLOC_ARENA_MAX").is_none() {
        unsafe extern "C" {
            fn boothdex_limit_malloc_arenas();
        }
        // SAFETY: sets one malloc parameter, before any other thread exists
        unsafe { boothdex_limit_malloc_arenas() };
    }
    tokio::runtime::Builder::new_multi_thread().enable_all().build()?.block_on(serve())
}

async fn serve() -> Result<()> {
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
    let transcodes = std::thread::available_parallelism().map_or(4, |n| n.get());
    #[cfg(feature = "libavif")]
    let transcodes = transcodes.min(MAX_TRANSCODES);
    let state = Arc::new(AppState {
        stats: Stats::default(),
        mem_mb,
        disk_mb,
        cache,
        client: reqwest::Client::builder().timeout(Duration::from_secs(20)).build()?,
        transcode: Semaphore::new(transcodes),
        archive: std::env::var("ARCHIVE_IMAGES").ok().map(PathBuf::from),
    });
    let app = Router::new()
        // public: the images, which the site forwards from /img
        .route("/b/{size}/{uuid}/i/{item}/{file}", get(booth))
        .route("/u/{size}/{user}/{file}", get(icon))
        .route("/v/{name}", get(archived))
        // internal: the API's admin overview reads it; the site does not forward it
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
        assert_eq!(
            icon_upstream_url("48x48", "15037164", "5fa34b7c-9f3a-4123-889f-ab80a8f3a79d_base_resized.jpg").as_deref(),
            Some("https://booth.pximg.net/c/48x48/users/15037164/icon_image/5fa34b7c-9f3a-4123-889f-ab80a8f3a79d_base_resized.jpg")
        );
        assert!(icon_upstream_url("48x48", "1x", "a.jpg").is_none());
        assert!(icon_upstream_url("999x999", "1", "a.jpg").is_none());
        assert!(icon_upstream_url("48x48", "1", "../a.jpg").is_none());
    }

    #[test]
    fn transcodes_to_each_format() {
        let img = DynamicImage::ImageRgb8(image::RgbImage::from_fn(32, 16, |x, y| image::Rgb([x as u8 * 8, y as u8 * 16, 128])));
        let mut png = Cursor::new(Vec::new());
        img.write_to(&mut png, ImageFormat::Png).unwrap();
        let decoded = decode(png.get_ref()).unwrap();
        let avif = encode(&decoded, Format::Avif, avif_quality("orig")).unwrap();
        assert_eq!(&avif[4..12], b"ftypavif");
        // av1C, third byte: monochrome, chroma_subsampling_x, chroma_subsampling_y, then the sample position.
        // All clear is colour at full chroma resolution (4:4:4).
        let av1c = avif.windows(4).position(|w| w == b"av1C").unwrap();
        assert_eq!(avif[av1c + 6] & 0b0001_1100, 0);
        assert_eq!(&encode(&decoded, Format::Jpeg, 90).unwrap()[0..2], &[0xFF, 0xD8]);
        assert_eq!(&encode(&decoded, Format::Png, 0).unwrap()[1..4], b"PNG");
        assert!(decode(b"not an image").is_err());
        assert_eq!(Format::parse(Some("gif")), None);
    }

    #[test]
    fn avif_is_the_default_and_webp_is_gone() {
        assert_eq!(Format::parse(None), Some(Format::Avif));
        assert_eq!(Format::parse(Some("avif")), Some(Format::Avif));
        assert_eq!(Format::parse(Some("webp")), None);
        assert_eq!(Format::Avif.mime(), "image/avif");
    }

    #[test]
    fn list_sizes_get_the_lighter_quality() {
        assert!(avif_quality("300x300_a2_g5") < avif_quality("orig"));
        assert_eq!(avif_quality("72x72_a2_g5"), avif_quality("300x300_a2_g5"));
        assert!(avif_quality("300x300_a2_g5") <= avif_quality("620x620") && avif_quality("620x620") <= avif_quality("orig"));
    }

    #[test]
    fn transparency_survives_avif() {
        let img = DynamicImage::ImageRgba8(image::RgbaImage::from_fn(16, 16, |x, _| image::Rgba([200, 40, 40, if x < 8 { 0 } else { 255 }])));
        let avif = encode(&img, Format::Avif, avif_quality("300x300_a2_g5")).unwrap();
        // the colour item and its alpha auxiliary item: two AV1 configurations
        assert_eq!(avif.windows(4).filter(|w| *w == b"av1C").count(), 2);
    }
}
