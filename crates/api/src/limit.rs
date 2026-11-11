//! Client IP extraction and in-memory token-bucket rate limiting for public endpoints.

use std::collections::HashMap;
use std::convert::Infallible;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::sync::Mutex;
use std::time::Instant;

use axum::Json;
use axum::extract::{ConnectInfo, FromRequestParts};
use axum::http::{HeaderMap, StatusCode, header, request::Parts};
use axum::response::{IntoResponse, Response};
use serde_json::json;

/// Published Cloudflare edge ranges (https://www.cloudflare.com/ips-v4, /ips-v6).
/// shortcut: static list; refresh if Cloudflare announces new edge prefixes.
const CF_V4: &[(u32, u32)] = &[
    (0xad_f5_30_00, 20), // 173.245.48.0/20
    (0x67_15_f4_00, 22), // 103.21.244.0/22
    (0x67_16_c8_00, 22), // 103.22.200.0/22
    (0x67_1f_04_00, 22), // 103.31.4.0/22
    (0x8d_65_40_00, 18), // 141.101.64.0/18
    (0x6c_a2_c0_00, 18), // 108.162.192.0/18
    (0xbe_5d_f0_00, 20), // 190.93.240.0/20
    (0xbc_72_60_00, 20), // 188.114.96.0/20
    (0xc5_ea_f0_00, 22), // 197.234.240.0/22
    (0xc6_29_80_00, 17), // 198.41.128.0/17
    (0xa2_9e_00_00, 15), // 162.158.0.0/15
    (0x68_10_00_00, 13), // 104.16.0.0/13
    (0x68_18_00_00, 14), // 104.24.0.0/14
    (0xac_40_00_00, 13), // 172.64.0.0/13
    (0x83_00_48_00, 22), // 131.0.72.0/22
];

const CF_V6: &[(u32, u32)] = &[
    (0x2400_cb00, 32), // 2400:cb00::/32
    (0x2606_4700, 32), // 2606:4700::/32
    (0x2803_f800, 32), // 2803:f800::/32
    (0x2405_b500, 32), // 2405:b500::/32
    (0x2405_8100, 32), // 2405:8100::/32
    (0x2a06_98c0, 29), // 2a06:98c0::/29
    (0x2c0f_f248, 32), // 2c0f:f248::/32
];

fn in_prefix(addr: u32, &(net, len): &(u32, u32)) -> bool {
    let mask = u32::MAX << (32 - len);
    (addr & mask) == (net & mask)
}

fn is_cloudflare(ip: IpAddr) -> bool {
    match ip.to_canonical() {
        IpAddr::V4(v4) => CF_V4.iter().any(|p| in_prefix(u32::from(v4), p)),
        IpAddr::V6(v6) => CF_V6.iter().any(|p| in_prefix((u128::from(v6) >> 96) as u32, p)),
    }
}

fn is_internal(ip: IpAddr) -> bool {
    match ip.to_canonical() {
        IpAddr::V4(v4) => v4.is_loopback() || v4.is_private() || v4.is_link_local() || v4.is_unspecified(),
        IpAddr::V6(v6) => {
            let s0 = v6.segments()[0];
            v6.is_loopback() || v6.is_unspecified() || (s0 & 0xfe00) == 0xfc00 || (s0 & 0xffc0) == 0xfe80
        }
    }
}

fn parse_ip(s: &str) -> Option<IpAddr> {
    s.trim().parse::<IpAddr>().ok().map(|ip| ip.to_canonical())
}

fn header_ip(headers: &HeaderMap, name: &str) -> Option<IpAddr> {
    headers.get(name).and_then(|v| v.to_str().ok()).and_then(parse_ip)
}

/// Real visitor IP behind `web` and the front reverse proxy (Caddy, nginx or Cloudflare).
/// shortcut: assumes the front proxy supplies a sanitized `X-Forwarded-For` (see `docs/DEPLOY.md`);
/// with a public XFF hop, `CF-Connecting-IP` is only trusted when that rightmost hop is a Cloudflare range;
/// with none, `X-Real-IP` and then `CF-Connecting-IP` are the fallback. Exposing `web` on `0.0.0.0`, or
/// failing to sanitize incoming `X-Forwarded-For`, lets callers forge headers.
pub fn client_ip(headers: &HeaderMap, peer: Option<IpAddr>) -> IpAddr {
    let peer = peer.map(|ip| ip.to_canonical()).unwrap_or(IpAddr::V4(Ipv4Addr::LOCALHOST));
    if !is_internal(peer) {
        return peer;
    }
    let xff: Vec<IpAddr> = headers
        .get_all("x-forwarded-for")
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(','))
        .filter_map(parse_ip)
        .collect();
    let mut public_hops = xff.into_iter().rev().filter(|ip| !is_internal(*ip));
    match public_hops.next() {
        // Cloudflare edge in front of Caddy/nginx: trust CF-Connecting-IP, not a spoofed XFF prefix
        Some(hop) if is_cloudflare(hop) => header_ip(headers, "cf-connecting-ip")
            .filter(|ip| !is_internal(*ip))
            .or_else(|| public_hops.find(|ip| !is_cloudflare(*ip)))
            .unwrap_or(hop),
        // Caddy/nginx/cloudflared appended the visitor's real IP at the end; ignore CF-Connecting-IP and XFF prefix
        Some(hop) => hop,
        // proxy configured with only X-Real-IP or CF-Connecting-IP, or local development
        None => header_ip(headers, "x-real-ip")
            .or_else(|| header_ip(headers, "cf-connecting-ip"))
            .unwrap_or(peer),
    }
}

/// Groups an IPv6 `/64` allocation into one bucket so subnet address rotation cannot bypass limits.
fn bucket_key(ip: IpAddr) -> IpAddr {
    match ip.to_canonical() {
        IpAddr::V4(v4) => IpAddr::V4(v4),
        IpAddr::V6(v6) => IpAddr::V6(Ipv6Addr::from(u128::from(v6) & !u128::from(u64::MAX))),
    }
}

#[derive(Clone, Copy)]
pub struct ClientIp(pub IpAddr);

impl<S: Send + Sync> FromRequestParts<S> for ClientIp {
    type Rejection = Infallible;

    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, Self::Rejection> {
        let peer = parts.extensions.get::<ConnectInfo<SocketAddr>>().map(|c| c.0.ip());
        Ok(Self(client_ip(&parts.headers, peer)))
    }
}

/// Token bucket keyed by client IP (`/32` IPv4, `/64` IPv6), with bounded state size.
pub struct RateLimiter {
    rate: f64,
    burst: f64,
    cap: usize,
    buckets: Mutex<HashMap<IpAddr, (f64, Instant)>>,
}

impl RateLimiter {
    pub fn new(rate: f64, burst: f64, cap: usize) -> Self {
        Self {
            rate,
            burst,
            cap: cap.max(1),
            buckets: Mutex::new(HashMap::new()),
        }
    }

    /// Consumes one token or returns whole seconds until one refills (`Retry-After`).
    pub fn take(&self, ip: IpAddr) -> Result<(), u64> {
        self.take_at(ip, Instant::now())
    }

    /// Consumes one token or returns a lightweight HTTP-convertible rejection.
    pub fn check(&self, ip: IpAddr) -> Result<(), RateLimited> {
        self.take(ip).map_err(RateLimited)
    }

    pub fn take_global(&self) -> Result<(), u64> {
        self.take_at(IpAddr::V4(Ipv4Addr::UNSPECIFIED), Instant::now())
    }

    fn take_at(&self, ip: IpAddr, now: Instant) -> Result<(), u64> {
        let key = bucket_key(ip);
        let mut map = self.buckets.lock().unwrap();
        if !map.contains_key(&key) && map.len() >= self.cap {
            map.retain(|_, (tokens, at)| (*tokens + now.saturating_duration_since(*at).as_secs_f64() * self.rate) < self.burst);
            if map.len() >= self.cap
                && let Some(oldest) = map.iter().min_by_key(|(_, (_, at))| *at).map(|(k, _)| *k)
            {
                map.remove(&oldest);
            }
        }
        let (tokens, at) = map.entry(key).or_insert((self.burst, now));
        let available = (*tokens + now.saturating_duration_since(*at).as_secs_f64() * self.rate).min(self.burst);
        if available < 1.0 {
            *tokens = available;
            *at = now;
            return Err(((1.0 - available) / self.rate).ceil().max(1.0) as u64);
        }
        *tokens = available - 1.0;
        *at = now;
        Ok(())
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
            account: RateLimiter::new(2.0, 30.0, 8192),
        }
    }
}

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

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    fn ip(s: &str) -> IpAddr {
        s.parse().unwrap()
    }

    #[tokio::test]
    async fn rate_limit_response_keeps_status_retry_after_and_body() {
        let response = RateLimited(7).into_response();
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(response.headers()[header::RETRY_AFTER], "7");
        let body = axum::body::to_bytes(response.into_body(), 1024).await.unwrap();
        assert_eq!(serde_json::from_slice::<serde_json::Value>(&body).unwrap(),
            json!({ "ok": false, "error": "Too many requests. Try again shortly." }));
    }

    #[test]
    fn direct_public_peer_ignores_forwarded_headers() {
        let mut h = HeaderMap::new();
        h.insert("x-forwarded-for", "1.1.1.1".parse().unwrap());
        h.insert("cf-connecting-ip", "8.8.8.8".parse().unwrap());
        assert_eq!(client_ip(&h, Some(ip("203.0.113.9"))), ip("203.0.113.9"));
    }

    #[test]
    fn reverse_proxy_uses_rightmost_public_hop_and_ignores_spoofed_headers() {
        let mut h = HeaderMap::new();
        // attacker sends fake XFF and fake CF-Connecting-IP; Caddy appends the real IP
        h.append("x-forwarded-for", "1.1.1.1".parse().unwrap());
        h.append("x-forwarded-for", "198.51.100.7, 203.0.113.42, 172.18.0.3".parse().unwrap());
        h.insert("cf-connecting-ip", "8.8.8.8".parse().unwrap());
        assert_eq!(client_ip(&h, Some(ip("172.18.0.5"))), ip("203.0.113.42"));
    }

    #[test]
    fn cloudflare_hop_trusts_cf_connecting_ip() {
        let mut h = HeaderMap::new();
        h.insert("x-forwarded-for", "1.1.1.1, 203.0.113.42, 172.68.10.5".parse().unwrap());
        h.insert("cf-connecting-ip", "203.0.113.42".parse().unwrap());
        assert_eq!(client_ip(&h, Some(ip("127.0.0.1"))), ip("203.0.113.42"));
    }

    #[test]
    fn fallback_to_real_ip_when_no_public_forwarded_hop() {
        let mut h = HeaderMap::new();
        // internal peer, only private XFF hops or header missing
        h.insert("x-forwarded-for", "10.0.0.1, 192.168.1.1, 127.0.0.1".parse().unwrap());
        h.insert("x-real-ip", "203.0.113.55".parse().unwrap());
        assert_eq!(client_ip(&h, Some(ip("172.18.0.2"))), ip("203.0.113.55"));

        // absent XFF header
        let mut h2 = HeaderMap::new();
        h2.insert("x-real-ip", "203.0.113.56".parse().unwrap());
        assert_eq!(client_ip(&h2, Some(ip("127.0.0.1"))), ip("203.0.113.56"));
    }

    #[test]
    fn token_bucket_refills_groups_ipv6_subnets_and_caps_memory() {
        let l = RateLimiter::new(2.0, 3.0, 2);
        let t0 = Instant::now();
        let a1 = ip("2001:db8:abcd:1234::1");
        let a2 = ip("2001:db8:abcd:1234:ffff:ffff:ffff:ffff");
        assert!(l.take_at(a1, t0).is_ok());
        assert!(l.take_at(a2, t0).is_ok()); // same /64 shares the bucket
        assert!(l.take_at(a1, t0).is_ok());
        assert_eq!(l.take_at(a2, t0), Err(1));
        assert!(l.take_at(a1, t0 + Duration::from_millis(500)).is_ok());

        // cap = 2: inserting more active subnets never grows past cap
        assert!(l.take_at(ip("198.51.100.1"), t0 + Duration::from_millis(600)).is_ok());
        assert!(l.take_at(ip("198.51.100.2"), t0 + Duration::from_millis(700)).is_ok());
        assert!(l.buckets.lock().unwrap().len() <= 2);
    }
}
