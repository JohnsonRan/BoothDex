//! The two tiers through the router `main` serves, against a database and a stand-in for siteverify. The buckets here
//! hold small numbers of their own, each tier's burst a different one and refilling too slowly to matter, so a count
//! names the bucket that answered; `limit.rs` holds the approved numbers to the table.

use std::sync::atomic::{AtomicUsize, Ordering};

use reqwest::StatusCode;
use serde_json::Value;
use sqlx::PgPool;

use super::*;
use crate::limit::{Limits, RateLimiter, Tiered};

const SLOW: f64 = 0.001;

/// Runs `test` on a multi-thread runtime, over a pool of its own to the same test database: `sqlx::test` runs on a
/// current-thread one, where the search's `block_in_place` panics.
fn multi_thread<F: Future<Output = ()> + Send + 'static>(pool: PgPool, test: impl FnOnce(PgPool) -> F + Send + 'static) {
    let options = (*pool.connect_options()).clone();
    let run = std::thread::spawn(move || {
        tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap().block_on(async move {
            let pool = PgPool::connect_with(options).await.unwrap();
            test(pool.clone()).await;
            pool.close().await;
        })
    });
    if let Err(panic) = run.join() {
        std::panic::resume_unwind(panic);
    }
}

fn limits() -> Limits {
    let tiered = |strict: f64, base: f64| Tiered::new((SLOW, strict), (SLOW, base));
    Limits {
        search: tiered(2.0, 4.0),
        palette: tiered(3.0, 5.0),
        item: tiered(2.0, 6.0),
        gallery: tiered(1.0, 3.0),
        lookup: tiered(2.0, 4.0),
        items: RateLimiter::new(SLOW, 3.0, 64),
        rss: RateLimiter::new(SLOW, 3.0, 64),
        verify: RateLimiter::new(SLOW, 3.0, 64),
        ..Limits::default()
    }
}

/// Each tiered route, with the burst of its strict and of its base bucket in `limits`.
const TIERED: [(&str, usize, usize); 6] = [
    ("/api/search", 2, 4),
    ("/api/palette?q=x", 3, 5),
    ("/api/items?ids=1", 2, 6),
    ("/api/items/1/gallery", 1, 3),
    ("/api/tags", 2, 4),
    ("/api/guestbook", 2, 4),
];

/// Routes that keep one bucket whatever the switch: the multi-item read and the feed, burst 3 each.
const UNTIERED: [&str; 2] = ["/api/items?ids=1+2", "/api/rss"];

/// A stand-in for siteverify that counts the tokens it is asked about. "pass" passes for this form and "guestbook"
/// for another; "failing" is answered HTTP 500, "no-secret" as a wrong secret key is and "broken" as Cloudflare's own
/// failure is; a token longer than Cloudflare issues would pass, to show it is never sent; anything else is refused.
async fn siteverify_stand_in() -> (String, Arc<AtomicUsize>) {
    let asked = Arc::new(AtomicUsize::new(0));
    let count = asked.clone();
    let app = axum::Router::new().route(
        "/",
        axum::routing::post(move |Json(b): Json<Value>| async move {
            count.fetch_add(1, Ordering::SeqCst);
            let refused = |code: &str| json!({ "success": false, "error-codes": [code] });
            Ok(Json(match b["response"].as_str() {
                Some("pass") => json!({ "success": true, "action": ACTION }),
                Some("guestbook") => json!({ "success": true, "action": "guestbook" }),
                Some("failing") => return Err(axum::http::StatusCode::INTERNAL_SERVER_ERROR),
                Some("no-secret") => refused("invalid-input-secret"),
                Some("broken") => refused("internal-error"),
                Some(long) if long.len() > 2048 => json!({ "success": true, "action": ACTION }),
                _ => refused("invalid-input-response"),
            }))
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (url, asked)
}

struct Site {
    base: String,
    state: Arc<AppState>,
    asked: Arc<AtomicUsize>,
    http: reqwest::Client,
}

/// The API with `settings` stored as the Turnstile row before it first reads them, served on a local port.
async fn site(pool: &PgPool, settings: Value) -> Site {
    sqlx::query("insert into app_settings (key, value) values ('turnstile', $1) on conflict (key) do update set value = excluded.value")
        .bind(settings)
        .execute(pool)
        .await
        .unwrap();
    let (siteverify_url, asked) = siteverify_stand_in().await;
    let state = Arc::try_unwrap(account::test_state(pool.clone())).ok().unwrap();
    let state = Arc::new(AppState { siteverify_url, limits: limits(), ..state });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let app = crate::app(state.clone());
    tokio::spawn(async move { axum::serve(listener, app.into_make_service_with_connect_info::<std::net::SocketAddr>()).await });
    Site { base, state, asked, http: reqwest::Client::new() }
}

fn strict() -> Value {
    json!({ "site_key": "0x-site", "secret": "0x-secret", "strict_limits": true })
}

/// A refusal as the site reads it: the code (None for a plain 429), the offered site key and `Retry-After`.
#[derive(Debug, PartialEq)]
struct Refused {
    code: Option<String>,
    site_key: Option<String>,
    retry_after: bool,
}

const PLAIN: Refused = Refused { code: None, site_key: None, retry_after: true };

fn offered() -> Refused {
    Refused { code: Some("verification_offered".into()), site_key: Some("0x-site".into()), retry_after: true }
}

impl Site {
    /// How many requests to `path` from the visitor at `ip` (the address the front proxy appends), with `cookie` if
    /// any, pass before the first 429, and that refusal.
    async fn count(&self, path: &str, ip: &str, cookie: Option<&str>) -> (usize, Refused) {
        let mut headers = vec![("x-forwarded-for", ip)];
        headers.extend(cookie.map(|c| ("cookie", c)));
        self.count_with(path, &headers).await
    }

    /// A GET of `path` with the headers as given; the peer is always this machine's loopback, as `web` is.
    async fn send(&self, path: &str, headers: &[(&str, &str)]) -> reqwest::Response {
        let mut request = self.http.get(format!("{}{path}", self.base));
        for (name, value) in headers {
            request = request.header(*name, *value);
        }
        request.send().await.unwrap()
    }

    /// `count` with the request's headers as given.
    async fn count_with(&self, path: &str, headers: &[(&str, &str)]) -> (usize, Refused) {
        for passed in 0..60 {
            let r = self.send(path, headers).await;
            if r.status() != StatusCode::TOO_MANY_REQUESTS {
                assert!(r.status().is_success() || r.status() == StatusCode::NOT_FOUND, "{path}: {}", r.status());
                continue;
            }
            let retry_after = r.headers().contains_key("retry-after");
            let body: Value = r.json().await.unwrap();
            let text = |field: &str| body[field].as_str().map(str::to_owned);
            assert_eq!(text("error").as_deref(), Some("Too many requests. Try again shortly."), "{path}");
            return (passed, Refused { code: text("code"), site_key: text("site_key"), retry_after });
        }
        panic!("{path} never refused");
    }

    /// The answer to a check with `token` from `ip`: status, code and the Set-Cookie header.
    async fn verify(&self, token: &str, ip: &str, site: Option<&str>) -> (StatusCode, Option<String>, Option<String>) {
        let mut request = self.http.post(format!("{}/api/verify", self.base)).header("x-forwarded-for", ip).json(&json!({ "token": token }));
        if let Some(site) = site {
            request = request.header("sec-fetch-site", site);
        }
        let r = request.send().await.unwrap();
        let (status, cookie) = (r.status(), r.headers().get("set-cookie").map(|v| v.to_str().unwrap().to_owned()));
        let body: Value = r.json().await.unwrap();
        (status, body["code"].as_str().map(str::to_owned), cookie)
    }

    /// A pass for `ip`, earned through the endpoint.
    async fn pass(&self, ip: &str) -> String {
        let (status, _, cookie) = self.verify("pass", ip, None).await;
        assert_eq!(status, StatusCode::OK);
        cookie.unwrap().split(';').next().unwrap().to_owned()
    }
}

/// With the switch off, or on with a key unset, every route keeps today's single bucket and its plain 429, and the
/// check is closed.
#[sqlx::test(migrations = "../core/migrations")]
#[ignore = "requires PostgreSQL with pgvector"]
async fn without_the_switch_nothing_changes(pool: PgPool) {
    multi_thread(pool, |pool| async move {
        let settings = [
            json!({}),
            json!({ "site_key": "0x-site", "secret": "0x-secret" }),
            json!({ "site_key": "0x-site", "secret": "0x-secret", "auth": true }),
            json!({ "site_key": "0x-site", "strict_limits": true }),
        ];
        for (n, settings) in settings.into_iter().enumerate() {
            let site = site(&pool, settings.clone()).await;
            for (i, (path, _, base)) in TIERED.into_iter().enumerate() {
                assert_eq!(site.count(path, &format!("203.0.113.{}", 10 * n + i), None).await, (base, PLAIN), "{settings} {path}");
            }
            assert_eq!(site.verify("pass", "203.0.113.99", None).await, (StatusCode::SERVICE_UNAVAILABLE, Some("verification_unavailable".into()), None), "{settings}");
            assert_eq!(site.asked.load(Ordering::SeqCst), 0, "{settings}");
        }
    });
}

/// With the switch on, a visitor without a pass gets the strict bucket and a 429 that offers the check; the routes
/// left out keep their single bucket and its plain 429.
#[sqlx::test(migrations = "../core/migrations")]
#[ignore = "requires PostgreSQL with pgvector"]
async fn without_a_pass_the_strict_bucket_refuses_and_offers_the_check(pool: PgPool) {
    multi_thread(pool, |pool| async move {
        let site = site(&pool, strict()).await;
        // an address per route: the tags and the guestbook share the lookup bucket
        for (i, (path, strict, _)) in TIERED.into_iter().enumerate() {
            assert_eq!(site.count(path, &format!("203.0.113.{i}"), None).await, (strict, offered()), "{path}");
        }
        for path in UNTIERED {
            assert_eq!(site.count(path, "203.0.113.1", None).await, (3, PLAIN), "{path}");
        }
    });
}

/// A pass gives today's bucket back, to every address of its range, and only that bucket: the requests made without
/// it were counted there too, so no address gets more than today's numbers.
#[sqlx::test(migrations = "../core/migrations")]
#[ignore = "requires PostgreSQL with pgvector"]
async fn a_pass_gives_todays_numbers_and_never_more(pool: PgPool) {
    multi_thread(pool, |pool| async move {
        let site = site(&pool, strict()).await;
        let (status, code, cookie) = site.verify("pass", "203.0.113.5", None).await;
        assert_eq!((status, code), (StatusCode::OK, None));
        let cookie = cookie.unwrap();
        for attribute in ["; Path=/;", "; HttpOnly", "; SameSite=Lax", "; Max-Age=600"] {
            assert!(cookie.contains(attribute), "{cookie}");
        }
        let pass = cookie.split(';').next().unwrap();
        assert!(pass.starts_with("pass=203.0.113.0/24."), "{pass}");
        for (i, (path, _, base)) in TIERED.into_iter().enumerate() {
            assert_eq!(site.count(path, &format!("203.0.113.{}", 5 + i), Some(pass)).await, (base, PLAIN), "{path}");
        }

        // another address of the same /24, with other cookies around the pass
        let around = format!("session=x; {pass}; settings=y");
        assert_eq!(site.count("/api/search", "203.0.113.200", Some(&around)).await, (4, PLAIN));
        // an IPv6 pass covers its /64
        let v6 = site.pass("2001:db8:1:2::5").await;
        assert_eq!(site.count("/api/tags", "2001:db8:1:2:ffff::1", Some(&v6)).await, (4, PLAIN));

        // without a pass first: the strict bucket's two count against the four of today's
        let ip = "198.51.100.7";
        assert_eq!(site.count("/api/search", ip, None).await, (2, offered()));
        let pass = site.pass(ip).await;
        assert_eq!(site.count("/api/search", ip, Some(&pass)).await, (2, PLAIN));
    });
}

/// The site's own server is never rate-limited (limit.rs `own_server`): a request the API attributes to an internal
/// address passes every bucket, tiered or not, with the switch on or off. Internal addresses written into a request's
/// headers make nothing internal: behind the front proxy the appended public address wins (a public peer's headers
/// are never read: `a_public_peer_is_never_internal` in crates/limit).
#[sqlx::test(migrations = "../core/migrations")]
#[ignore = "requires PostgreSQL with pgvector"]
async fn the_sites_own_server_is_never_limited(pool: PgPool) {
    multi_thread(pool, |pool| async move {
        for settings in [strict(), json!({})] {
            let site = site(&pool, settings.clone()).await;
            // web's own reads carry no visitor's headers; a hop list of internal addresses is still web's
            for headers in [&[][..], &[("x-forwarded-for", "10.0.0.5, 172.18.0.3")][..]] {
                for path in ["/api/tags", "/api/search", "/api/rss", "/api/items?ids=1+2"] {
                    for _ in 0..20 {
                        let r = site.send(path, headers).await;
                        assert_ne!(r.status(), StatusCode::TOO_MANY_REQUESTS, "{settings} {path} {headers:?}");
                    }
                }
            }
        }

        // what an outside request can write into its headers, after the front proxy appended its real address
        let site = site(&pool, strict()).await;
        let spoofed = |real: &'static str| {
            [("x-forwarded-for", format!("127.0.0.1, ::ffff:127.0.0.1, 10.0.0.1, {real}")), ("x-real-ip", "127.0.0.1".into()), ("cf-connecting-ip", "::1".into())]
        };
        let as_refs = |h: &[(&'static str, String); 3]| h.iter().map(|(k, v)| (*k, v.clone())).collect::<Vec<_>>();
        let headers = as_refs(&spoofed("203.0.113.9"));
        let headers: Vec<(&str, &str)> = headers.iter().map(|(k, v)| (*k, v.as_str())).collect();
        assert_eq!(site.count_with("/api/palette?q=x", &headers).await, (3, offered()));
        assert_eq!(site.count_with("/api/rss", &headers).await, (3, PLAIN));
        // a public address in its IPv4-mapped IPv6 form is that public address
        let mapped = as_refs(&spoofed("::ffff:198.51.100.9"));
        let mapped: Vec<(&str, &str)> = mapped.iter().map(|(k, v)| (*k, v.as_str())).collect();
        assert_eq!(site.count_with("/api/tags", &mapped).await, (2, offered()));
    });
}

/// A forwarding header the server would never send (a byte outside ASCII, an entry that is not an address) makes a
/// request a visitor's: the appended address when there is one, else one shared bucket, and never unlimited.
#[sqlx::test(migrations = "../core/migrations")]
#[ignore = "requires PostgreSQL with pgvector"]
async fn a_visitor_cannot_pass_as_the_server(pool: PgPool) {
    multi_thread(pool, |pool| async move {
        let site = site(&pool, strict()).await;
        let behind_nginx = [("x-forwarded-for", "é, 203.0.113.9"), ("x-real-ip", "8.8.4.4")];
        assert_eq!(site.count_with("/api/search", &behind_nginx).await, (2, offered()));
        assert_eq!(site.count_with("/api/rss", &behind_nginx).await, (3, PLAIN));
        // the same visitor, its address written with a port
        assert_eq!(site.count_with("/api/search", &[("x-forwarded-for", "203.0.113.9:1234")]).await, (0, offered()));
        // nothing usable: the one shared bucket
        assert_eq!(site.count_with("/api/tags", &[("x-forwarded-for", "é")]).await, (2, offered()));
        assert_eq!(site.count_with("/api/guestbook", &[("x-forwarded-for", "garbage, 10.0.0.1")]).await, (0, offered()));
        // the check's own limit holds too, and siteverify is asked no more than it allows
        for _ in 0..3 {
            assert_eq!(site.verify("nope", "é, 198.51.100.50", None).await.0, StatusCode::FORBIDDEN);
        }
        assert_eq!(site.verify("nope", "é, 198.51.100.50", None).await.0, StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(site.asked.load(Ordering::SeqCst), 3);
    });
}

/// A browser's request that reaches the API as the server's own (no forwarding header: a front proxy that sets none)
/// is reported once a minute, so the operator learns that no visitor is limited; `web`'s own reads are not.
#[sqlx::test(migrations = "../core/migrations")]
#[ignore = "requires PostgreSQL with pgvector"]
async fn a_browser_without_a_visitor_address_is_reported(pool: PgPool) {
    multi_thread(pool, |pool| async move {
        let site = site(&pool, json!({})).await;
        site.send("/api/health", &[("sec-fetch-mode", "cors")]).await;
        site.send("/api/health", &[("x-forwarded-for", "203.0.113.9"), ("sec-fetch-site", "same-origin")]).await;
        assert!(site.state.limits.unforwarded_log.take_global().is_ok(), "reported without cause");
        let site = self::site(&pool, json!({})).await;
        site.send("/api/health", &[("sec-fetch-site", "same-origin")]).await;
        assert!(site.state.limits.unforwarded_log.take_global().is_err(), "not reported");
    });
}

/// A pass that is forged, signed by another process, expired, from another range or not a pass at all counts as
/// none.
#[sqlx::test(migrations = "../core/migrations")]
#[ignore = "requires PostgreSQL with pgvector"]
async fn a_bad_pass_counts_as_none(pool: PgPool) {
    multi_thread(pool, |pool| async move {
        let site = site(&pool, strict()).await;
        let ip: IpAddr = "203.0.113.5".parse().unwrap();
        let key = &site.state.pass_key;
        let good = key.issue(ip, now() + TTL);
        let flipped = format!("{}{}", &good[..good.len() - 1], if good.ends_with('0') { '1' } else { '0' });
        let (range_part, rest) = good.split_once(".0/24").unwrap();
        assert_eq!(range_part, "203.0.113");
        let bad = [
            // a signature changed, or kept for a range or an expiry of the visitor's choosing
            flipped,
            format!("198.51.100.0/24{rest}"),
            good.replacen(&format!(".{}.", now() + TTL), &format!(".{}.", now() + 10 * TTL), 1),
            // another process's key, as after a restart
            PassKey::new().unwrap().issue(ip, now() + TTL),
            // expired
            key.issue(ip, now() - 1),
            key.issue(ip, now()),
            // not a pass
            String::new(),
            "garbage".into(),
            "203.0.113.0/24.soon.00".into(),
            format!("{}0", good),
            format!("{}é", &good[..good.len() - 2]),
        ];
        for (n, value) in bad.iter().enumerate() {
            let cookie = format!("pass={value}");
            assert_eq!(site.count("/api/search", &format!("203.0.113.{}", 10 + n), Some(&cookie)).await, (2, offered()), "{value}");
        }
        // a good pass, from an address outside its range
        assert_eq!(site.count("/api/search", "203.0.114.5", Some(&format!("pass={good}"))).await, (2, offered()));
        assert_eq!(site.count("/api/search", "203.0.113.5", Some(&format!("pass={good}"))).await, (4, PLAIN));
    });
}

/// What `/api/verify` gives for each answer siteverify can make, its own limit, and the cross-site guard.
#[sqlx::test(migrations = "../core/migrations")]
#[ignore = "requires PostgreSQL with pgvector"]
async fn only_a_passed_check_earns_a_pass(pool: PgPool) {
    multi_thread(pool, |pool| async move {
        let site = site(&pool, strict()).await;
        let refused = (StatusCode::FORBIDDEN, Some("verification_failed".into()), None);
        let unjudged = (StatusCode::SERVICE_UNAVAILABLE, Some("verification_unavailable".into()), None);
        // each from an address of its own, so the endpoint's limit never answers for the check
        for (n, (token, answer)) in [("nope", &refused), ("", &refused), ("guestbook", &refused), ("failing", &unjudged), ("no-secret", &unjudged), ("broken", &unjudged)]
            .into_iter()
            .enumerate()
        {
            let ip = format!("198.51.100.{n}");
            assert_eq!(&site.verify(token, &ip, None).await, answer, "{token}");
            // no pass: still the strict bucket
            assert_eq!(site.count("/api/search", &ip, None).await, (2, offered()), "{token}");
        }
        assert_eq!(site.asked.load(Ordering::SeqCst), 6);

        // a token longer than Cloudflare issues, and a check from another site, are refused without asking siteverify
        assert_eq!(site.verify(&"x".repeat(2049), "198.51.100.20", None).await, refused);
        let cross = (StatusCode::FORBIDDEN, Some("cross_site_request".into()), None);
        assert_eq!(site.verify("pass", "198.51.100.21", Some("cross-site")).await, cross);
        assert_eq!(site.asked.load(Ordering::SeqCst), 6);

        // three checks per address, then a plain 429 that asks siteverify nothing
        for _ in 0..3 {
            assert_eq!(site.verify("nope", "198.51.100.30", None).await, refused);
        }
        let (status, code, cookie) = site.verify("pass", "198.51.100.30", None).await;
        assert_eq!((status, code, cookie), (StatusCode::TOO_MANY_REQUESTS, None, None));
        assert_eq!(site.asked.load(Ordering::SeqCst), 9);

        // siteverify not reached at all
        let closed = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let unreachable = format!("http://{}/", closed.local_addr().unwrap());
        drop(closed);
        let s = Arc::try_unwrap(account::test_state(pool.clone())).ok().unwrap();
        let s = Arc::new(AppState { siteverify_url: unreachable, ..s });
        let r = verify(State(s), ClientIp("198.51.100.40".parse().unwrap()), Json(Verify { token: "pass".into() })).await;
        let r = r.into_response();
        assert_eq!(r.status(), axum::http::StatusCode::SERVICE_UNAVAILABLE);
        assert!(!r.headers().contains_key(header::SET_COOKIE));
    });
}

#[test]
fn a_signature_has_one_spelling() {
    let sig = "0123456789abcdef".repeat(4);
    assert_eq!(unhex(&sig).map(|b| b.len()), Some(32));
    for bad in [sig.to_uppercase(), format!("+{}", &sig[1..]), sig[2..].to_owned(), format!("{sig}00"), format!("{}g", &sig[1..])] {
        assert_eq!(unhex(&bad), None, "{bad}");
    }
}

#[test]
fn a_pass_names_its_range() {
    let key = PassKey::new().unwrap();
    for (ip, range_text) in [
        ("203.0.113.77", "203.0.113.0/24"),
        ("::ffff:203.0.113.77", "203.0.113.0/24"),
        ("2001:db8:1:2:3:4:5:6", "2001:db8:1:2::/64"),
    ] {
        let ip: IpAddr = ip.parse().unwrap();
        assert_eq!(range(ip), range_text);
        assert!(key.issue(ip, 100).starts_with(&format!("{range_text}.100.")));
        assert!(key.valid(&key.issue(ip, 100), ip, 99));
        assert!(!key.valid(&key.issue(ip, 100), ip, 100));
    }
}
