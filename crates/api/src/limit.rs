//! The API's rate-limit buckets and its answer when one is empty. The client IP and the token bucket are in
//! `boothdex-limit`, which imgproxy shares.

use std::net::IpAddr;
use std::sync::Arc;

use axum::Json;
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
pub use boothdex_limit::{ClientIp, RateLimiter, is_internal, take_all};
use serde_json::json;

use crate::error::Refusal;
use crate::turnstile::TurnstileConfig;

pub(crate) const TOO_MANY: &str = "Too many requests. Try again shortly.";

#[derive(Debug)]
pub struct RateLimited {
    /// whole seconds until a token refills (`Retry-After`)
    pub retry_after: u64,
    /// the site key of the check that lifts this refusal (`/api/verify`, pass.rs): set when the stricter bucket of a
    /// visitor without a pass refused
    pub offer: Option<String>,
}

impl RateLimited {
    pub fn after(retry_after: u64) -> Self {
        Self { retry_after, offer: None }
    }
}

impl IntoResponse for RateLimited {
    fn into_response(self) -> Response {
        let body = match self.offer {
            Some(site_key) => json!({ "ok": false, "code": Refusal::VerificationOffered, "error": TOO_MANY, "site_key": site_key }),
            None => json!({ "ok": false, "error": TOO_MANY }),
        };
        (StatusCode::TOO_MANY_REQUESTS, [(header::RETRY_AFTER, self.retry_after.to_string())], Json(body)).into_response()
    }
}

/// Whether a request is the site's own server's, which no per-address bucket limits (owner, 2026-11-17: "我们永远不应该
/// 对服务端自身请求进行任何限速吧"): its client IP is internal. `client_ip` (crates/limit) answers an internal address only
/// for an internal peer whose forwarding headers are absent or hold only internal addresses, each entry parsed on its
/// own; an entry that does not parse sends the request to the shared `UNATTRIBUTED` bucket instead. A request from a
/// public peer is that peer, whatever its headers say. Behind the front proxy, which appends the visitor's address to
/// X-Forwarded-For (docs/DEPLOY.md), that public hop wins over internal hops, over X-Real-IP, and over
/// CF-Connecting-IP unless the hop is Cloudflare's; X-Real-IP and CF-Connecting-IP are read only when no public hop
/// exists. So an outside request is never the server's own, and a read `web` makes for a visitor carries that
/// visitor's headers (web lib/server-data.ts, `visitorHeaders`) and is limited as the visitor. What does reach here:
/// `web`'s cached lookups, a LAN visitor, local development, and every visitor when the front proxy sets no forwarding
/// header, the misconfiguration `warn_unforwarded` reports. Global caps (`embed_global`) and the embedding limit per
/// address (`embed_ip`), which bound a paid upstream, are not exempt.
pub fn own_server(ip: IpAddr) -> bool {
    is_internal(ip)
}

/// Logs, at most once a minute, a browser's request that reached the API as the site's own server's: the front proxy
/// sets no forwarding header, so no visitor is rate-limited. A browser marks its requests with `Sec-Fetch-Site` and
/// the `/api` rewrite passes it on; `web`'s own reads (Node's fetch) carry none. Local development, with no front
/// proxy, logs it as well.
pub async fn warn_unforwarded(
    axum::extract::State(s): axum::extract::State<std::sync::Arc<crate::AppState>>,
    ClientIp(ip): ClientIp,
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> Response {
    if own_server(ip) && req.headers().contains_key("sec-fetch-site") && s.limits.unforwarded_log.take_global().is_ok() {
        eprintln!("warn: a browser's request reached api with no visitor address ({ip}): set X-Forwarded-For in the front proxy (docs/DEPLOY.md); until then no visitor is rate-limited");
    }
    next.run(req).await
}

/// `RateLimiter::take` as the API's refusal, passing the site's own server (`own_server`). A trait because
/// `RateLimited` must stay in this crate: `ApiError`'s blanket `From` for errors rules out a `From` for a type from
/// another crate.
pub trait Check {
    /// Consumes one token or returns a lightweight HTTP-convertible rejection.
    fn check(&self, ip: IpAddr) -> Result<(), RateLimited>;
}

impl Check for RateLimiter {
    fn check(&self, ip: IpAddr) -> Result<(), RateLimited> {
        if own_server(ip) {
            return Ok(());
        }
        self.take(ip).map_err(RateLimited::after)
    }
}

/// Which numbers a request to a tiered bucket gets; its extractor is in pass.rs.
pub enum Tier {
    /// today's numbers: the admin's switch for stricter limits is off (or a Turnstile key is unset), or the request
    /// carries a valid pass
    Base,
    /// a visitor without a pass while the switch is on; a refusal offers the check `config` names. `session` is the
    /// request's session cookie while sign-in is Turnstile-checked too, read only when `strict` refuses
    Strict { config: Arc<TurnstileConfig>, session: Option<crate::pass::Session> },
}

/// Two per-address buckets for one group of routes: `base` holds today's numbers and counts every request, so no
/// address gets more than today's numbers, with or without a pass; `strict` counts, and refuses first, the requests
/// without a pass while the switch is on. A pass therefore draws from `base` alone: drawing from `strict` too would
/// refuse a visitor who has just passed until `strict` refilled.
///
/// A request without a pass while the switch is on is one draw, all or nothing (`take_all`): a token of `strict`, the
/// records it asks for from the shared record bucket when its route pays them (`check_records`), and a token of
/// `base`. When every part is covered, all are taken. Otherwise none is: when only `base` fell short the answer is the
/// plain 429 with `base`'s wait, as for any visitor; when `strict` or the records fell short, the check is offered
/// with the longest wait. A request with a session cookie then draws one token from `base` and only then reads the
/// session: a request the draw admits costs no database read, the reads per address are bounded by today's numbers
/// however many made-up cookies arrive, and that token stays counted. A session whose sign-in passed a Turnstile check
/// is then admitted, paying no strict token and no records; any other is refused with the check offered. When `base`
/// refuses that token, the check is offered too, with the longer wait, and the session is not read, so a checked session
/// is offered the check as well; passing it changes nothing, since a pass draws from `base` too.
pub struct Tiered {
    pub strict: RateLimiter,
    pub base: RateLimiter,
}

impl Tiered {
    /// `(rate per second, burst)` of each bucket, keyed per address as `RateLimiter` keys (IPv6 by /64).
    pub fn new(strict: (f64, f64), base: (f64, f64)) -> Self {
        Self { strict: RateLimiter::new(strict.0, strict.1, 8192), base: RateLimiter::new(base.0, base.1, 8192) }
    }

    pub async fn check(&self, ip: IpAddr, tier: &Tier) -> Result<(), RateLimited> {
        self.check_records(ip, tier, None).await
    }

    /// `check` for a route whose requests without a pass also pay records from the shared bucket: `records` is that
    /// bucket and the request's count (`Limits::records`). A route that pays none passes `None`.
    pub async fn check_records(&self, ip: IpAddr, tier: &Tier, records: Option<(&RateLimiter, usize)>) -> Result<(), RateLimited> {
        if own_server(ip) {
            return Ok(());
        }
        let Tier::Strict { config, session } = tier else { return self.base.check(ip) };
        let mut draw = vec![(&self.strict, 1.0)];
        draw.extend(records.map(|(bucket, count)| (bucket, count as f64)));
        draw.push((&self.base, 1.0));
        let base = draw.len() - 1;
        let Err(short) = take_all(ip, &draw) else { return Ok(()) };
        let retry_after = short.iter().map(|(_, wait)| *wait).max().unwrap_or(1);
        if short.iter().all(|(part, _)| *part == base) {
            return Err(RateLimited::after(retry_after));
        }
        let offer = |after| RateLimited { retry_after: after, offer: config.site_key.clone() };
        let Some(session) = session else { return Err(offer(retry_after)) };
        self.base.take(ip).map_err(|base_after| offer(retry_after.max(base_after)))?;
        if session.checked().await { Ok(()) } else { Err(offer(retry_after)) }
    }
}

/// Independent per-IP and global rate-limit buckets so noisy autocomplete, RSS or gallery
/// traffic can never drain `/api/search` quota. Tiered buckets list `strict` then today's numbers, as rate per
/// second / burst.
pub struct Limits {
    /// /api/search: 1/30, today's 2/30
    pub search: Tiered,
    /// /api/palette: 2/40, today's 5/40 (above the search field's 250 ms debounce, web search-suggest.tsx)
    pub palette: Tiered,
    /// /api/related, which the results page asks once per search: 2 req/s, burst 30, today's numbers for search. Not
    /// tiered, so a refusal offers no check: the page only hides its row of related searches
    pub related: RateLimiter,
    /// /api/rss: 0.5 req/s, burst 20; without a pass while the switch is on, each request also pays 50 `records`
    pub rss: RateLimiter,
    /// /api/items with several ids, a list's (the recent view's, other clients'): 0.5/10, today's 1/20
    pub items: Tiered,
    /// The records a visitor without a pass may read while the switch is on, shared by /api/search (its `pageSize`),
    /// /api/items with several ids (its ids) and /api/rss (its 50 items, rss.rs): 15 a second, burst 1,200 (owner,
    /// 2026-11-21, from measured browsing: REQUIREMENTS.md "Intentional designs"). The burst is at least the largest
    /// request, 100 records
    pub records: RateLimiter,
    /// /api/items with one id, the item dialog's record, also fetched when a tile is pointed at: 1/15, today's 4/40
    pub item: Tiered,
    /// /api/items/{id}/gallery: 0.5/12, today's 1/20
    pub gallery: Tiered,
    /// /api/{avatars,authors,tags,booth-tags,categories} and the guestbook list: 2/20, today's 5/40
    pub lookup: Tiered,
    /// uncached AI query embeddings: 15/min per IP (burst 8), 120/min global (burst 20)
    pub embed_ip: RateLimiter,
    pub embed_global: RateLimiter,
    /// /api/account/{login,register}: 0.2 req/s, burst 5
    pub auth: RateLimiter,
    /// the warning for a sign-in refused by `auth`: one line per address (per /64 for IPv6) a minute, so a flood cannot fill the log
    pub auth_log: RateLimiter,
    /// /api/account changes (favourites, viewed items, saved searches, settings): 2 req/s, burst 30
    pub account: RateLimiter,
    /// guestbook posts: 3 per 10 minutes (0.005 req/s, burst 3)
    pub guestbook: RateLimiter,
    /// /api/verify, each a siteverify request: 1 a minute, burst 5. A pass lasts 10 minutes, so a visitor needs about
    /// one check per 10 minutes; the burst leaves room for a refused or expired token and a few visitors behind one
    /// address, and the refill holds an address to 60 siteverify requests an hour
    pub verify: RateLimiter,
    /// `warn_unforwarded`: one line a minute for the whole API
    pub unforwarded_log: RateLimiter,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            search: Tiered::new((1.0, 30.0), (2.0, 30.0)),
            palette: Tiered::new((2.0, 40.0), (5.0, 40.0)),
            related: RateLimiter::new(2.0, 30.0, 8192),
            rss: RateLimiter::new(0.5, 20.0, 8192),
            items: Tiered::new((0.5, 10.0), (1.0, 20.0)),
            records: RateLimiter::new(15.0, 1200.0, 8192),
            item: Tiered::new((1.0, 15.0), (4.0, 40.0)),
            gallery: Tiered::new((0.5, 12.0), (1.0, 20.0)),
            lookup: Tiered::new((2.0, 20.0), (5.0, 40.0)),
            embed_ip: RateLimiter::new(0.25, 8.0, 8192),
            embed_global: RateLimiter::new(2.0, 20.0, 1),
            auth: RateLimiter::new(0.2, 5.0, 8192),
            auth_log: RateLimiter::new(1.0 / 60.0, 1.0, 8192),
            account: RateLimiter::new(2.0, 30.0, 8192),
            guestbook: RateLimiter::new(3.0 / 600.0, 3.0, 8192),
            verify: RateLimiter::new(1.0 / 60.0, 5.0, 8192),
            unforwarded_log: RateLimiter::new(1.0 / 60.0, 1.0, 1),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn rate_limit_response_keeps_status_retry_after_and_body() {
        let response = RateLimited::after(7).into_response();
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(response.headers()[header::RETRY_AFTER], "7");
        let body = axum::body::to_bytes(response.into_body(), 1024).await.unwrap();
        assert_eq!(serde_json::from_slice::<serde_json::Value>(&body).unwrap(),
            json!({ "ok": false, "error": "Too many requests. Try again shortly." }));
    }

    /// The numbers the owner approved on 2026-11-17 from a measured browsing session (strict) and today's.
    #[test]
    fn tiered_buckets_hold_the_approved_numbers() {
        let l = Limits::default();
        let numbers = |t: &Tiered| (t.strict.numbers(), t.base.numbers());
        assert_eq!(numbers(&l.search), ((1.0, 30.0), (2.0, 30.0)));
        assert_eq!(numbers(&l.palette), ((2.0, 40.0), (5.0, 40.0)));
        assert_eq!(numbers(&l.item), ((1.0, 15.0), (4.0, 40.0)));
        // the owner's numbers of 2026-11-21 for the multi-id read and for the records a visitor without a pass reads
        assert_eq!(numbers(&l.items), ((0.5, 10.0), (1.0, 20.0)));
        assert_eq!(l.records.numbers(), (15.0, 1200.0));
        // the largest request, 100 records, can always be admitted once the bucket refills
        assert!(l.records.numbers().1 >= 100.0);
        assert_eq!(numbers(&l.gallery), ((0.5, 12.0), (1.0, 20.0)));
        assert_eq!(numbers(&l.lookup), ((2.0, 20.0), (5.0, 40.0)));
    }

    fn strict() -> Tier {
        let config = crate::turnstile::resolve(&json!({ "site_key": "0x-site", "secret": "0x-secret", "strict_limits": true }), |_| None);
        Tier::Strict { config: Arc::new(config), session: None }
    }

    const SLOW: f64 = 0.001;
    const IP: IpAddr = IpAddr::V4(std::net::Ipv4Addr::new(203, 0, 113, 1));

    /// The check is offered when the strict bucket or the records fell short; when only today's bucket did, the answer
    /// is the plain 429 any visitor gets, and the strict token is not spent on a refused request.
    #[tokio::test]
    async fn only_a_strict_shortfall_offers_the_check() {
        let records = RateLimiter::new(SLOW, 150.0, 64);
        let offered = |r: Result<(), RateLimited>| r.unwrap_err().offer.is_some();
        // the strict bucket short
        let t = Tiered::new((SLOW, 1.0), (SLOW, 5.0));
        assert!(t.check(IP, &strict()).await.is_ok());
        assert!(offered(t.check(IP, &strict()).await));
        // the records short
        let t = Tiered::new((SLOW, 5.0), (SLOW, 5.0));
        assert!(t.check_records(IP, &strict(), Some((&records, 100))).await.is_ok());
        assert!(offered(t.check_records(IP, &strict(), Some((&records, 100))).await));
        // only today's bucket short: plain, and the strict token stays for the next request
        let t = Tiered::new((SLOW, 2.0), (SLOW, 1.0));
        assert!(t.check(IP, &strict()).await.is_ok());
        assert!(!offered(t.check(IP, &strict()).await));
        assert!(t.strict.take(IP).is_ok() && t.strict.take(IP).is_err());
    }
}
