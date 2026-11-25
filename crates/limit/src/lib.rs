//! Client IP extraction and in-memory token-bucket rate limiting, shared by the API and imgproxy.

use std::collections::HashMap;
use std::convert::Infallible;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::sync::Mutex;
use std::time::Instant;

use axum::extract::{ConnectInfo, FromRequestParts};
use axum::http::{HeaderMap, request::Parts};

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

pub fn is_internal(ip: IpAddr) -> bool {
    match ip.to_canonical() {
        IpAddr::V4(v4) => v4.is_loopback() || v4.is_private() || v4.is_link_local() || v4.is_unspecified(),
        IpAddr::V6(v6) => {
            let s0 = v6.segments()[0];
            v6.is_loopback() || v6.is_unspecified() || (s0 & 0xfe00) == 0xfc00 || (s0 & 0xffc0) == 0xfe80
        }
    }
}

/// One address entry of a forwarding header: `1.2.3.4`, `1.2.3.4:5678`, `2001:db8::1`, `[2001:db8::1]` or
/// `[2001:db8::1]:5678`. None for anything else, a non-UTF-8 entry included.
fn parse_entry(raw: &[u8]) -> Option<IpAddr> {
    let s = std::str::from_utf8(raw).ok()?.trim();
    let ip = if let Some(rest) = s.strip_prefix('[') {
        let (ip, tail) = rest.split_once(']')?;
        if !(tail.is_empty() || tail.strip_prefix(':').is_some_and(|port| port.parse::<u16>().is_ok())) {
            return None;
        }
        IpAddr::V6(ip.parse().ok()?)
    } else if let Ok(ip) = s.parse::<IpAddr>() {
        ip
    } else {
        let (ip, port) = s.rsplit_once(':')?;
        port.parse::<u16>().ok()?;
        IpAddr::V4(ip.parse().ok()?)
    };
    Some(ip.to_canonical())
}

/// A forwarding header as the proxies wrote it: absent, or its entries, each parsed or not.
fn header_entries(headers: &HeaderMap, name: &str) -> Vec<Option<IpAddr>> {
    // per entry from the raw bytes: one byte a visitor wrote outside ASCII must not drop the entry a proxy appended
    headers.get_all(name).iter().flat_map(|v| v.as_bytes().split(|b| *b == b',')).map(parse_entry).collect()
}

/// The bucket of a request whose forwarding headers name no usable visitor address but hold something other than
/// internal addresses: such a request is not the site's own server's, so it is limited, all of them together. The
/// address is from the discard-only block (RFC 6666), which no client has.
pub const UNATTRIBUTED: IpAddr = IpAddr::V6(Ipv6Addr::new(0x100, 0, 0, 0, 0, 0, 0, 0));

/// Real visitor IP behind `web` and the front reverse proxy (Caddy, nginx or Cloudflare).
/// shortcut: assumes the front proxy supplies a sanitized `X-Forwarded-For` (see `docs/DEPLOY.md`);
/// with a public XFF hop, `CF-Connecting-IP` is only trusted when that rightmost hop is a Cloudflare range;
/// with none, `X-Real-IP` and then `CF-Connecting-IP` are the fallback. Exposing `web` on `0.0.0.0`, or
/// failing to sanitize incoming `X-Forwarded-For`, lets callers forge headers.
/// An internal answer means every forwarding header is absent or names only internal addresses: the site's own
/// server (the API exempts it from its limits) or a LAN visitor. A header holding an entry that does not parse, with
/// no usable address, gives `UNATTRIBUTED` instead of the internal peer.
pub fn client_ip(headers: &HeaderMap, peer: Option<IpAddr>) -> IpAddr {
    let peer = peer.map(|ip| ip.to_canonical()).unwrap_or(IpAddr::V4(Ipv4Addr::LOCALHOST));
    if !is_internal(peer) {
        return peer;
    }
    let xff = header_entries(headers, "x-forwarded-for");
    let real = header_entries(headers, "x-real-ip");
    let cf = header_entries(headers, "cf-connecting-ip");
    let mut public_hops = xff.iter().rev().flatten().copied().filter(|ip| !is_internal(*ip));
    let found = match public_hops.next() {
        // Cloudflare edge in front of Caddy/nginx: trust CF-Connecting-IP, not a spoofed XFF prefix
        Some(hop) if is_cloudflare(hop) => cf
            .first()
            .copied()
            .flatten()
            .filter(|ip| !is_internal(*ip))
            .or_else(|| public_hops.find(|ip| !is_cloudflare(*ip)))
            .unwrap_or(hop),
        // Caddy/nginx/cloudflared appended the visitor's real IP at the end; ignore CF-Connecting-IP and XFF prefix
        Some(hop) => hop,
        // proxy configured with only X-Real-IP or CF-Connecting-IP, or local development
        None => real.first().copied().flatten().or_else(|| cf.first().copied().flatten()).unwrap_or(peer),
    };
    let unparsed = [&xff, &real, &cf].iter().any(|entries| entries.iter().any(Option::is_none));
    if is_internal(found) && unparsed { UNATTRIBUTED } else { found }
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

    /// `(rate per second, burst)`, as given to `new`.
    pub fn numbers(&self) -> (f64, f64) {
        (self.rate, self.burst)
    }

    /// Consumes one token or returns whole seconds until one refills (`Retry-After`).
    pub fn take(&self, ip: IpAddr) -> Result<(), u64> {
        self.take_locked(ip, None)
    }

    pub fn take_global(&self) -> Result<(), u64> {
        self.take_locked(IpAddr::V4(Ipv4Addr::UNSPECIFIED), None)
    }

    #[cfg(test)]
    fn take_at(&self, ip: IpAddr, now: Instant) -> Result<(), u64> {
        self.take_locked(ip, Some(now))
    }

    /// Reads the clock after taking the bucket lock (unless a test passes `at`), so concurrent callers never write back
    /// an instant older than one another caller wrote.
    fn take_locked(&self, ip: IpAddr, at: Option<Instant>) -> Result<(), u64> {
        let mut map = self.buckets.lock().unwrap();
        let now = at.unwrap_or_else(Instant::now);
        let entry = self.entry(&mut map, bucket_key(ip), now);
        let available = self.available(entry, now);
        if available < 1.0 {
            *entry = (available, now);
            return Err(self.wait(available, 1.0));
        }
        *entry = (available - 1.0, now);
        Ok(())
    }

    /// The bucket of `key`, made full if new; at the cap, full buckets go first, then the oldest one.
    fn entry<'m>(&self, map: &'m mut HashMap<IpAddr, (f64, Instant)>, key: IpAddr, now: Instant) -> &'m mut (f64, Instant) {
        if !map.contains_key(&key) && map.len() >= self.cap {
            map.retain(|_, (tokens, at)| (*tokens + now.saturating_duration_since(*at).as_secs_f64() * self.rate) < self.burst);
            if map.len() >= self.cap
                && let Some(oldest) = map.iter().min_by_key(|(_, (_, at))| *at).map(|(k, _)| *k)
            {
                map.remove(&oldest);
            }
        }
        map.entry(key).or_insert((self.burst, now))
    }

    fn available(&self, (tokens, at): &(f64, Instant), now: Instant) -> f64 {
        (tokens + now.saturating_duration_since(*at).as_secs_f64() * self.rate).min(self.burst)
    }

    /// Whole seconds until `available` tokens become `n` (`Retry-After`).
    fn wait(&self, available: f64, n: f64) -> u64 {
        ((n - available) / self.rate).ceil().max(1.0) as u64
    }
}

/// Takes `n` tokens from each `(bucket, n)` of `draws` for `ip`, all or none, at one instant. Buckets are deduplicated
/// (summing parts for the same bucket) and locked in raw pointer-address order so concurrent callers passing buckets in
/// any order—or repeating a bucket—never deadlock. A part of 0 is left out: its bucket is neither locked nor given an
/// entry. A non-finite part (`NaN` or infinite) is refused without locking or mutating any bucket. On a refusal nothing
/// is written: no token is taken and no entry made, so a refused draw never evicts another address's bucket. The error
/// names each draw index whose bucket fell short, in `draws` order, with the whole seconds until it covers its part.
/// The clock is read once all the locks are held, so no draw writes back an instant older than one another draw wrote.
pub fn take_all(ip: IpAddr, draws: &[(&RateLimiter, f64)]) -> Result<(), Vec<(usize, u64)>> {
    let invalid: Vec<(usize, u64)> = draws.iter().enumerate().filter(|(_, (_, n))| !n.is_finite()).map(|(i, _)| (i, 1)).collect();
    if !invalid.is_empty() {
        return Err(invalid);
    }
    let key = bucket_key(ip);
    // group active draws by bucket identity and lock in ascending pointer address order to prevent AB-BA or self-deadlocks
    let mut grouped: Vec<(usize, &RateLimiter, f64, Vec<usize>)> = Vec::new();
    for (i, &(bucket, n)) in draws.iter().enumerate() {
        if n <= 0.0 {
            continue;
        }
        let addr = std::ptr::from_ref(bucket) as usize;
        if let Some(slot) = grouped.iter_mut().find(|g| g.0 == addr) {
            slot.2 += n;
            slot.3.push(i);
        } else {
            grouped.push((addr, bucket, n, vec![i]));
        }
    }
    grouped.sort_unstable_by_key(|g| g.0);
    let mut maps: Vec<_> = grouped.iter().map(|(_, bucket, _, _)| bucket.buckets.lock().unwrap()).collect();
    let now = Instant::now();
    let mut short = Vec::new();
    let mut levels = Vec::with_capacity(grouped.len());
    for ((_, bucket, n, indices), map) in grouped.iter().zip(maps.iter()) {
        // an address without a bucket has a full one
        let available = map.get(&key).map_or(bucket.burst, |entry| bucket.available(entry, now));
        if available < *n {
            let wait = bucket.wait(available, *n);
            for &i in indices {
                short.push((i, wait));
            }
        }
        levels.push(available);
    }
    if !short.is_empty() {
        short.sort_unstable_by_key(|&(i, _)| i);
        return Err(short);
    }
    for (((_, bucket, n, _), map), available) in grouped.iter().zip(maps.iter_mut()).zip(levels) {
        *bucket.entry(map, key, now) = (available - n, now);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    fn ip(s: &str) -> IpAddr {
        s.parse().unwrap()
    }

    /// Too slow to refill within a test.
    const SLOW: f64 = 0.001;

    #[test]
    fn a_draw_from_several_buckets_takes_all_or_nothing() {
        let (own, shared) = (RateLimiter::new(SLOW, 2.0, 64), RateLimiter::new(SLOW, 5.0, 64));
        let a = ip("203.0.113.1");
        assert_eq!(take_all(a, &[(&own, 1.0), (&shared, 5.0)]), Ok(()));
        // the shared bucket is empty: refused, and the own bucket keeps the token it would have given
        let refused = take_all(a, &[(&own, 1.0), (&shared, 1.0)]).unwrap_err();
        assert_eq!(refused.iter().map(|(i, _)| *i).collect::<Vec<_>>(), [1]);
        assert!(own.take(a).is_ok() && own.take(a).is_err());
        // every bucket that falls short is named, with its own wait; the wait covers the whole part
        let b = ip("203.0.113.2");
        let (slow, slower) = (RateLimiter::new(1.0, 1.0, 64), RateLimiter::new(0.5, 10.0, 64));
        assert_eq!(take_all(b, &[(&slow, 1.0), (&slower, 10.0)]), Ok(()));
        assert_eq!(take_all(b, &[(&slow, 1.0), (&slower, 4.0)]), Err(vec![(0, 1), (1, 8)]));
        // a part above a bucket's burst is never admitted, however long the bucket waits
        assert!(take_all(ip("203.0.113.3"), &[(&shared, 6.0)]).is_err());
        // duplicate bucket pointers sum their cost without self-deadlocking, and non-finite costs are refused
        let dup = RateLimiter::new(SLOW, 3.0, 64);
        assert_eq!(take_all(ip("203.0.113.4"), &[(&dup, 2.0), (&dup, 2.0)]).unwrap_err().iter().map(|(i, _)| *i).collect::<Vec<_>>(), [0, 1]);
        assert_eq!(take_all(ip("203.0.113.4"), &[(&dup, 1.0), (&dup, 2.0)]), Ok(()));
        assert!(take_all(ip("203.0.113.4"), &[(&dup, 1.0)]).is_err());
        assert_eq!(take_all(ip("203.0.113.5"), &[(&shared, f64::NAN), (&own, f64::INFINITY)]), Err(vec![(0, 1), (1, 1)]));
        // reversed bucket order across concurrent threads never deadlocks
        let a = std::sync::Arc::new(RateLimiter::new(1000.0, 1000.0, 64));
        let b = std::sync::Arc::new(RateLimiter::new(1000.0, 1000.0, 64));
        let (a1, b1) = (a.clone(), b.clone());
        let t1 = std::thread::spawn(move || {
            for _ in 0..200 {
                let _ = take_all(ip("203.0.113.6"), &[(&a1, 1.0), (&b1, 1.0)]);
            }
        });
        for _ in 0..200 {
            let _ = take_all(ip("203.0.113.6"), &[(&b, 1.0), (&a, 1.0)]);
        }
        t1.join().unwrap();
    }

    #[test]
    fn a_part_of_zero_leaves_its_bucket_alone() {
        let (own, shared) = (RateLimiter::new(SLOW, 2.0, 64), RateLimiter::new(SLOW, 5.0, 64));
        assert_eq!(take_all(ip("203.0.113.1"), &[(&own, 1.0), (&shared, 0.0)]), Ok(()));
        assert!(shared.buckets.lock().unwrap().is_empty());
    }

    /// A refused draw makes no entry, so at a bucket's cap it evicts no other address's bucket.
    #[test]
    fn a_refused_draw_leaves_other_addresses_alone() {
        let (own, shared) = (RateLimiter::new(SLOW, 3.0, 1), RateLimiter::new(SLOW, 1.0, 64));
        let (a, b) = (ip("203.0.113.1"), ip("203.0.113.2"));
        assert!(own.take(a).is_ok() && own.take(a).is_ok());
        // b's shared bucket is empty, so b's draw is refused before it touches the own bucket, which holds a alone
        assert!(shared.take(b).is_ok());
        assert!(take_all(b, &[(&own, 1.0), (&shared, 1.0)]).is_err());
        assert!(own.take(a).is_ok() && own.take(a).is_err());
    }

    #[test]
    fn concurrent_draws_never_share_tokens() {
        let (own, shared) = (RateLimiter::new(SLOW, 1000.0, 64), RateLimiter::new(SLOW, 100.0, 64));
        let admitted = std::sync::atomic::AtomicUsize::new(0);
        std::thread::scope(|scope| {
            for _ in 0..8 {
                scope.spawn(|| {
                    for _ in 0..10 {
                        if take_all(ip("203.0.113.1"), &[(&own, 1.0), (&shared, 10.0)]).is_ok() {
                            admitted.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                        }
                    }
                });
            }
        });
        // the shared bucket covers ten draws of 10; each refusal left the own bucket as it was
        assert_eq!(admitted.into_inner(), 10);
        assert_eq!(own.available(&own.buckets.lock().unwrap()[&ip("203.0.113.1")], Instant::now()).round(), 990.0);
    }

    #[test]
    fn direct_public_peer_ignores_forwarded_headers() {
        let mut h = HeaderMap::new();
        h.insert("x-forwarded-for", "1.1.1.1".parse().unwrap());
        h.insert("cf-connecting-ip", "8.8.8.8".parse().unwrap());
        assert_eq!(client_ip(&h, Some(ip("203.0.113.9"))), ip("203.0.113.9"));
    }

    /// A visitor's byte outside ASCII, or an entry with a port or in brackets, must not hide the address the proxy
    /// appended, nor let X-Real-IP stand in for it.
    #[test]
    fn every_forwarded_entry_is_read_on_its_own() {
        let with = |xff: &[u8], real: Option<&str>| {
            let mut h = HeaderMap::new();
            h.insert("x-forwarded-for", axum::http::HeaderValue::from_bytes(xff).unwrap());
            if let Some(real) = real {
                h.insert("x-real-ip", real.parse().unwrap());
            }
            client_ip(&h, Some(ip("172.18.0.5")))
        };
        assert_eq!(with(b"\xff, 203.0.113.9", None), ip("203.0.113.9"));
        assert_eq!(with("é, 203.0.113.9".as_bytes(), None), ip("203.0.113.9"));
        assert_eq!(with(b"\xff, 203.0.113.9", Some("8.8.4.4")), ip("203.0.113.9"));
        assert_eq!(with(b"203.0.113.9:1234", None), ip("203.0.113.9"));
        assert_eq!(with(b"[2001:db8::9]", None), ip("2001:db8::9"));
        assert_eq!(with(b"10.0.0.1, [2001:db8::9]:443", None), ip("2001:db8::9"));
        // nothing usable and something that is not an address: limited, never the internal peer
        for xff in [&b"\xff"[..], b"garbage", b"10.0.0.1, nope", b"203.0.113.9:99999", b"[2001:db8::9]x", b"10.0.0.1,"] {
            assert_eq!(with(xff, None), UNATTRIBUTED, "{xff:?}");
        }
        assert!(!is_internal(UNATTRIBUTED));
        let mut h = HeaderMap::new();
        h.insert("x-real-ip", axum::http::HeaderValue::from_bytes(b"\xff").unwrap());
        assert_eq!(client_ip(&h, Some(ip("127.0.0.1"))), UNATTRIBUTED);
        // internal entries only, or no header at all: the internal peer
        assert!(is_internal(with(b"10.0.0.1, 172.18.0.3", None)));
        assert!(is_internal(client_ip(&HeaderMap::new(), Some(ip("127.0.0.1")))));
    }

    /// The API exempts an internal client IP from its per-address limits as the site's own server (api limit.rs
    /// `own_server`): no header turns a public peer, plain or IPv4-mapped, into one.
    #[test]
    fn a_public_peer_is_never_internal() {
        let mut h = HeaderMap::new();
        h.insert("x-forwarded-for", "127.0.0.1, ::ffff:10.0.0.1".parse().unwrap());
        h.insert("x-real-ip", "127.0.0.1".parse().unwrap());
        h.insert("cf-connecting-ip", "::1".parse().unwrap());
        for peer in ["203.0.113.9", "::ffff:203.0.113.9", "2001:db8::9"] {
            let ip = client_ip(&h, Some(ip(peer)));
            assert!(!is_internal(ip), "{peer} -> {ip}");
        }
        // web's container, its address in either form, is internal
        assert!(is_internal(client_ip(&HeaderMap::new(), Some(ip("::ffff:172.18.0.3")))));
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

    #[test]
    fn take_reads_clock_after_acquiring_bucket_lock() {
        let l = std::sync::Arc::new(RateLimiter::new(SLOW, 5.0, 64));
        let addr = ip("203.0.113.77");
        let guard = l.buckets.lock().unwrap();
        let (ready_tx, ready_rx) = std::sync::mpsc::channel();
        let l2 = l.clone();
        let worker = std::thread::spawn(move || {
            ready_tx.send(()).unwrap();
            l2.take(addr).unwrap();
        });
        ready_rx.recv().unwrap();
        // give the worker time to reach the mutex; a pre-lock Instant::now() would be captured before this sleep ends
        std::thread::sleep(Duration::from_millis(30));
        let unlock_at = Instant::now();
        drop(guard);
        worker.join().unwrap();
        let stored_at = l.buckets.lock().unwrap()[&addr].1;
        assert!(stored_at >= unlock_at, "bucket timestamp predates lock acquisition");
    }
}
