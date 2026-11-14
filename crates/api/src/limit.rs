//! The API's rate-limit buckets and its answer when one is empty. The client IP and the token bucket are in
//! `boothdex-limit`, which imgproxy shares.

use std::net::IpAddr;

use axum::Json;
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
pub use boothdex_limit::{ClientIp, RateLimiter, is_internal};
use serde_json::json;

#[derive(Debug)]
pub struct RateLimited(pub u64);

impl IntoResponse for RateLimited {
    fn into_response(self) -> Response {
        (
            StatusCode::TOO_MANY_REQUESTS,
            [(header::RETRY_AFTER, self.0.to_string())],
            Json(json!({ "ok": false, "error": "Too many requests. Try again shortly." })),
        )
            .into_response()
    }
}

/// `RateLimiter::take` as the API's refusal. A trait because `RateLimited` must stay in this crate: `ApiError`'s
/// blanket `From` for errors rules out a `From` for a type from another crate.
pub trait Check {
    /// Consumes one token or returns a lightweight HTTP-convertible rejection.
    fn check(&self, ip: IpAddr) -> Result<(), RateLimited>;
}

impl Check for RateLimiter {
    fn check(&self, ip: IpAddr) -> Result<(), RateLimited> {
        self.take(ip).map_err(RateLimited)
    }
}

/// Independent per-IP and global rate-limit buckets so noisy autocomplete, RSS or gallery
/// traffic can never drain `/api/search` quota.
pub struct Limits {
    /// /api/search: 2 req/s, burst 30
    pub search: RateLimiter,
    /// /api/palette: 5 req/s, burst 40 (matches 120 ms CommandPalette debounce)
    pub palette: RateLimiter,
    /// /api/rss: 0.5 req/s, burst 20
    pub rss: RateLimiter,
    /// /api/items: 1 req/s, burst 20
    pub items: RateLimiter,
    /// /api/items with one id, the item dialog's record, also fetched when a tile is pointed at: 4 req/s, burst 40
    pub item: RateLimiter,
    /// /api/items/{id}/gallery: 1 req/s, burst 20
    pub gallery: RateLimiter,
    /// /api/{avatars,authors,tags,booth-tags,categories}: 5 req/s, burst 40
    pub lookup: RateLimiter,
    /// uncached AI query embeddings: 15/min per IP (burst 8), 120/min global (burst 20)
    pub embed_ip: RateLimiter,
    pub embed_global: RateLimiter,
    /// /api/account/{login,register}: 0.2 req/s, burst 5
    pub auth: RateLimiter,
    /// the warning for a sign-in refused by `auth`: one line per address (per /64 for IPv6) a minute, so a flood cannot fill the log
    pub auth_log: RateLimiter,
    /// /api/account changes (favourites, viewed items, saved searches, settings): 2 req/s, burst 30
    pub account: RateLimiter,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            search: RateLimiter::new(2.0, 30.0, 8192),
            palette: RateLimiter::new(5.0, 40.0, 8192),
            rss: RateLimiter::new(0.5, 20.0, 8192),
            items: RateLimiter::new(1.0, 20.0, 8192),
            item: RateLimiter::new(4.0, 40.0, 8192),
            gallery: RateLimiter::new(1.0, 20.0, 8192),
            lookup: RateLimiter::new(5.0, 40.0, 8192),
            embed_ip: RateLimiter::new(0.25, 8.0, 8192),
            embed_global: RateLimiter::new(2.0, 20.0, 1),
            auth: RateLimiter::new(0.2, 5.0, 8192),
            auth_log: RateLimiter::new(1.0 / 60.0, 1.0, 8192),
            account: RateLimiter::new(2.0, 30.0, 8192),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn rate_limit_response_keeps_status_retry_after_and_body() {
        let response = RateLimited(7).into_response();
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(response.headers()[header::RETRY_AFTER], "7");
        let body = axum::body::to_bytes(response.into_body(), 1024).await.unwrap();
        assert_eq!(serde_json::from_slice::<serde_json::Value>(&body).unwrap(),
            json!({ "ok": false, "error": "Too many requests. Try again shortly." }));
    }
}
