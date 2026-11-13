//! Image proxy meant to sit behind Cloudflare.
//!   GET /b/{size}/{uuid}/i/{item}/{file}[?format=avif|png|jpeg]  Booth image (booth.pximg.net), re-encoded
//!   GET /u/{size}/{user}/{file}[?format=…]                     Booth shop icon (users/{user}/icon_image), same
//!   GET /a/{file}[?format=…]                                   BOOTH static image (asset.booth.pm/assets), same
//!   GET /f/{size}/{uuid}/{file}[?format=…]                     pixiv Factory drawing (factory.pixiv.net), scaled down
//!                                                              here to a sized request; /f/{uuid}/{file} is "orig"
//!   GET /v/{sha1}.webp                                         archived VRCatalogue image (delisted items)
//! Output defaults to AVIF with full-resolution chroma (4:4:4), from libaom when built with the `libavif`
//! feature (the container images are) and from rav1e otherwise; the archived files are WebP and served as they
//! are. Results are cached in foyer (memory + disk); responses are immutable so the CDN keeps them, which is
//! why the site names the format in its URLs. Only booth.pximg.net, asset.booth.pm and factory.pixiv.net are ever
//! fetched, over https, and only Booth-shaped paths under fixed directories (with Booth's own sizes for pximg) are
//! accepted, so this is not an open proxy. A redirect is followed only on the same https host (src/upstream.rs), and
//! a host is connected to only at a public address (src/public_dns.rs).
//!
//! Env: BIND (127.0.0.1:3001), CACHE_DIR (./imgcache), CACHE_DISK_MB (10240), CACHE_MEM_MB (256),
//!      ARCHIVE_IMAGES (dir of <sha1>.webp files)

mod encode;
mod public_dns;
mod routes;
mod upstream;

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use foyer::{BlockEngineConfig, DeviceBuilder, FsDeviceBuilder, HybridCacheBuilder};
use tokio::sync::Semaphore;

#[cfg(feature = "libavif")]
use encode::MAX_TRANSCODES;
use routes::{AppState, Stats};
use upstream::upstream_client;

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
        client: upstream_client()?,
        transcode: Semaphore::new(transcodes),
        archive: std::env::var("ARCHIVE_IMAGES").ok().map(PathBuf::from),
    });
    let app = routes::router(state.clone());
    let bind = std::env::var("BIND").unwrap_or_else(|_| "127.0.0.1:3001".into());
    println!("imgproxy listening on http://{bind}, cache {dir}");
    axum::serve(tokio::net::TcpListener::bind(&bind).await?, app).await?;
    state.cache.close().await?;
    Ok(())
}
