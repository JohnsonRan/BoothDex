//! The state every handler shares: the database pool, the catalogue snapshot, rate limits, caches and clients.

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use sqlx::PgPool;

use crate::search::{EMBED_CACHE_CAP, EmbedWatch};
use crate::snapshot::Snapshot;
use crate::{account, admin, gallery, limit};

pub(crate) struct AppState {
    pub(crate) pool: PgPool,
    pub(crate) snap: arc_swap::ArcSwap<Snapshot>,
    /// wakes the reloader early (admin "reload now")
    pub(crate) reload: tokio::sync::Notify,
    pub(crate) site_url: String,
    pub(crate) throttle: account::LoginThrottle,
    pub(crate) dummy_hash: String,
    pub(crate) gallery: gallery::Fetcher,
    /// item opens not yet written to item_stats; see `gallery::flush_views`
    pub(crate) view_counts: gallery::ViewCounts,
    pub(crate) favourites: account::FavouriteCache,
    pub(crate) sys: admin::SysMetrics,
    pub(crate) http: reqwest::Client,
    pub(crate) imgproxy_url: Option<String>,
    /// per-IP and global rate-limit tiers across API routes and upstream embedding calls
    pub(crate) limits: limit::Limits,
    /// caps concurrent in-flight catalogue search / item-load CPU + DB work so floods cannot starve other routes
    pub(crate) search_permits: tokio::sync::Semaphore,
    /// query text -> embedding, so paging through results doesn't call the provider again
    pub(crate) embed_cache: moka::sync::Cache<String, Vec<f32>>,
    /// deduplicates concurrent identical uncached queries
    pub(crate) embed_inflight: std::sync::Mutex<HashMap<String, EmbedWatch>>,
    /// caps concurrent upstream /embeddings HTTP calls
    pub(crate) embed_permits: Arc<tokio::sync::Semaphore>,
    /// AI settings for query embeddings; see `ai_config`
    pub(crate) ai: arc_swap::ArcSwapOption<boothdex_core::settings::AiConfig>,
    /// serializes AI settings saves across database write and cache publication
    pub(crate) ai_save: tokio::sync::Mutex<()>,
}

impl AppState {
    pub(crate) fn new(
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
    pub(crate) async fn ai_config(&self) -> Result<Arc<boothdex_core::settings::AiConfig>, sqlx::Error> {
        if let Some(c) = self.ai.load_full() {
            return Ok(c);
        }
        let c = Arc::new(boothdex_core::settings::ai(&self.pool).await?);
        self.ai.compare_and_swap(&None::<Arc<_>>, Some(c.clone()));
        Ok(c)
    }

    pub(crate) fn snap(&self) -> arc_swap::Guard<Arc<Snapshot>> {
        self.snap.load()
    }

    pub(crate) async fn acquire_search(&self) -> std::result::Result<tokio::sync::SemaphorePermit<'_>, limit::RateLimited> {
        tokio::time::timeout(std::time::Duration::from_secs(2), self.search_permits.acquire())
            .await
            .ok()
            .and_then(std::result::Result::ok)
            .ok_or(limit::RateLimited(2))
    }
}

/// An API with an empty catalogue, tight limits and a pool that never connects, for handler tests.
#[cfg(test)]
pub(crate) fn test_state() -> std::sync::Arc<super::AppState> {
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
                auth_log: super::limit::RateLimiter::new(1.0 / 60.0, 1.0, 64),
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

#[cfg(test)]
mod tests {
    use super::test_state;

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
}
