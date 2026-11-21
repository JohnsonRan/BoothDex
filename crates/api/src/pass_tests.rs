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
        items: tiered(3.0, 7.0),
        // no record limit here: the per-record tests set one of their own
        records: RateLimiter::new(SLOW, 1e9, 64),
        rss: RateLimiter::new(SLOW, 3.0, 64),
        related: RateLimiter::new(SLOW, 3.0, 64),
        verify: RateLimiter::new(SLOW, 3.0, 64),
        ..Limits::default()
    }
}

/// Each tiered route, with the burst of its strict and of its base bucket in `limits`.
const TIERED: [(&str, usize, usize); 7] = [
    ("/api/search", 2, 4),
    ("/api/palette?q=x", 3, 5),
    ("/api/items?ids=1", 2, 6),
    ("/api/items?ids=1+2", 3, 7),
    ("/api/items/1/gallery", 1, 3),
    ("/api/tags", 2, 4),
    ("/api/guestbook", 2, 4),
];

/// Routes that keep one bucket whatever the switch: the feed and the related searches, burst 3 each.
const UNTIERED: [&str; 2] = ["/api/rss", "/api/related?q=x"];

/// A stand-in for siteverify that counts the tokens it is asked about. "pass" passes for this form, "guestbook" for
/// another and "in" for sign-in; "failing" is answered HTTP 500, "no-secret" as a wrong secret key is and "broken" as
/// Cloudflare's own failure is; a token longer than Cloudflare issues would pass, to show it is never sent; anything
/// else is refused.
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
                Some("in") => json!({ "success": true, "action": "sign-in" }),
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
    site_with(pool, settings, limits()).await
}

/// `site` with buckets of the test's own.
async fn site_with(pool: &PgPool, settings: Value, limits: Limits) -> Site {
    sqlx::query("insert into app_settings (key, value) values ('turnstile', $1) on conflict (key) do update set value = excluded.value")
        .bind(settings)
        .execute(pool)
        .await
        .unwrap();
    let (siteverify_url, asked) = siteverify_stand_in().await;
    let state = Arc::try_unwrap(account::test_state(pool.clone())).ok().unwrap();
    let state = Arc::new(AppState { siteverify_url, limits, ..state });
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

    /// The refusal of one request with `headers`, and its `Retry-After` in seconds.
    async fn refusal(&self, path: &str, headers: &[(&str, &str)]) -> (Refused, u64) {
        let r = self.send(path, headers).await;
        assert_eq!(r.status(), StatusCode::TOO_MANY_REQUESTS, "{path}");
        let wait = r.headers()["retry-after"].to_str().unwrap().parse().unwrap();
        let body: Value = r.json().await.unwrap();
        let text = |field: &str| body[field].as_str().map(str::to_owned);
        (Refused { code: text("code"), site_key: text("site_key"), retry_after: true }, wait)
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

    /// The session cookie that signing in as `a` from `ip` sets, with `token` as the widget's.
    async fn sign_in(&self, token: &str, ip: &str) -> String {
        let form = json!({ "login": "a", "password": PASSWORD, "token": token });
        let r = self.http.post(format!("{}/api/account/login", self.base)).header("x-forwarded-for", ip).json(&form).send().await.unwrap();
        assert_eq!(r.status(), StatusCode::OK, "{token}");
        r.headers()["set-cookie"].to_str().unwrap().split(';').next().unwrap().to_owned()
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

/// No shape of `/api/items` reaches an untiered bucket: exactly one id (ids that do not parse are dropped) is the item
/// record's, strict burst 2 here; none, several, duplicates and more than 100 are the list's, strict burst 3. A
/// refusal carries no Cache-Control, so neither a browser nor the CDN keeps it.
#[sqlx::test(migrations = "../core/migrations")]
#[ignore = "requires PostgreSQL with pgvector"]
async fn every_shape_of_the_item_read_is_tiered(pool: PgPool) {
    multi_thread(pool, |pool| async move {
        let site = site(&pool, strict()).await;
        let many = format!("/api/items?ids={}", (1..=101).map(|i| i.to_string()).collect::<Vec<_>>().join(","));
        let shapes = [("/api/items?ids=", 3), ("/api/items?ids=1", 2), ("/api/items?ids=1,x", 2), ("/api/items?ids=1,1", 3), ("/api/items?ids=1+2", 3), (&*many, 3)];
        for (i, (path, burst)) in shapes.into_iter().enumerate() {
            let ip = format!("192.0.2.{i}");
            assert_eq!(site.count(path, &ip, None).await, (burst, offered()), "{path}");
            let refused = site.send(path, &[("x-forwarded-for", &ip)]).await;
            assert_eq!(refused.status(), StatusCode::TOO_MANY_REQUESTS, "{path}");
            assert!(refused.headers().get("cache-control").is_none(), "{path}");
        }
    });
}

/// Request bursts above what the record tests need (8 strict, 12 today's) and a record bucket of 250.
fn record_limits() -> Limits {
    let tiered = |strict: f64, base: f64| Tiered::new((SLOW, strict), (SLOW, base));
    Limits {
        search: tiered(8.0, 12.0),
        items: tiered(8.0, 12.0),
        rss: RateLimiter::new(SLOW, 8.0, 64),
        records: RateLimiter::new(SLOW, 250.0, 64),
        ..limits()
    }
}

/// Without a pass, a search and a read of several items also pay the records they ask for, from one bucket per address
/// that both routes share; a refusal there offers the check. Request buckets still count every request, so asking for
/// fewer records buys no more requests than today. A pass, or the switch off, pays no records.
#[sqlx::test(migrations = "../core/migrations")]
#[ignore = "requires PostgreSQL with pgvector"]
async fn without_a_pass_requests_pay_their_records(pool: PgPool) {
    multi_thread(pool, |pool| async move {
        let site = site_with(&pool, strict(), record_limits()).await;
        let ids = |n: usize| format!("/api/items?ids={}", (1..=n).map(|i| i.to_string()).collect::<Vec<_>>().join(","));
        // pages of 100 records: two fit in 250, the third is refused at the records, and waits until 100 are back
        // (the 50 left refill at 0.001 a second here); pages of 50: five
        assert_eq!(site.count("/api/search?pageSize=100", "198.51.100.1", None).await, (2, offered()));
        assert_eq!(site.refusal("/api/search?pageSize=100", &[("x-forwarded-for", "198.51.100.1")]).await, (offered(), 50_000));
        // no ids pay no records: the request still passes with the records spent
        assert_eq!(site.send("/api/items?ids=", &[("x-forwarded-for", "198.51.100.1")]).await.status(), StatusCode::OK);
        assert_eq!(site.count("/api/search?pageSize=50", "198.51.100.2", None).await, (5, offered()));
        // a page of one record: the strict request bucket refuses first, after its 8
        assert_eq!(site.count("/api/search?pageSize=1", "198.51.100.3", None).await, (8, offered()));
        // one budget for both routes: after a search of 100 and a read of 100 ids, the next 100 ids are refused
        let at = |ip: &'static str| [("x-forwarded-for", ip)];
        assert_eq!(site.send("/api/search?pageSize=100", &at("198.51.100.4")).await.status(), StatusCode::OK);
        assert_eq!(site.send(&ids(100), &at("198.51.100.4")).await.status(), StatusCode::OK);
        assert_eq!(site.refusal(&ids(100), &at("198.51.100.4")).await.0, offered());
        // each id counts as asked, duplicates included; no ids pay a request token and no records
        assert_eq!(site.count(&format!("/api/items?ids={}", vec!["1"; 100].join(",")), "198.51.100.5", None).await, (2, offered()));
        assert_eq!(site.count("/api/items?ids=", "198.51.100.6", None).await, (8, offered()));
        // a pass pays no records: today's 12 requests of 100 each
        let pass = site.pass("198.51.100.7").await;
        assert_eq!(site.count("/api/search?pageSize=100", "198.51.100.7", Some(&pass)).await, (12, PLAIN));
        // nor does anyone with the switch off
        let off = site_with(&pool, json!({}), record_limits()).await;
        assert_eq!(off.count("/api/search?pageSize=100", "198.51.100.8", None).await, (12, PLAIN));
        assert_eq!(off.count(&ids(100), "198.51.100.9", None).await, (12, PLAIN));
    });
}

/// Without a pass, a feed's 50 records come from the same record bucket as the searches, and a refusal is the plain 429
/// a feed reader can wait out: it cannot pass the check. With a pass or the switch off, only the feed's own bucket
/// (burst 8 here) counts.
#[sqlx::test(migrations = "../core/migrations")]
#[ignore = "requires PostgreSQL with pgvector"]
async fn a_feed_shares_the_record_budget_and_offers_no_check(pool: PgPool) {
    multi_thread(pool, |pool| async move {
        let site = site_with(&pool, strict(), record_limits()).await;
        // five feeds of 50 fill the 250; the sixth is refused, plain
        assert_eq!(site.count("/api/rss", "198.51.100.20", None).await, (5, PLAIN));
        // a search of 100 from the same address leaves room for three feeds
        let at = [("x-forwarded-for", "198.51.100.21")];
        assert_eq!(site.send("/api/search?pageSize=100", &at).await.status(), StatusCode::OK);
        assert_eq!(site.count("/api/rss", "198.51.100.21", None).await, (3, PLAIN));
        // a pass, or the switch off: the feed's own 8
        let pass = site.pass("198.51.100.22").await;
        assert_eq!(site.count("/api/rss", "198.51.100.22", Some(&pass)).await, (8, PLAIN));
        let off = site_with(&pool, json!({}), record_limits()).await;
        assert_eq!(off.count("/api/rss", "198.51.100.23", None).await, (8, PLAIN));
    });
}

/// A feed reads no session: a session whose sign-in passed the check, which a search would admit at today's numbers once
/// the strict buckets refuse, pays a feed's records like any visitor without a pass.
#[sqlx::test(migrations = "../core/migrations")]
#[ignore = "requires PostgreSQL with pgvector"]
async fn a_feed_reads_no_session(pool: PgPool) {
    multi_thread(pool, |pool| async move {
        sqlx::query("insert into users (username, email, display_name, password_hash) values ('a', 'a@example.test', 'A', $1)")
            .bind(account::hash_password(PASSWORD.into()).await.ok().unwrap())
            .execute(&pool)
            .await
            .unwrap();
        let site = site_with(&pool, checked_sign_in(), record_limits()).await;
        let session = site.sign_in("in", "198.51.100.30").await;
        assert_eq!(site.count("/api/rss", "198.51.100.31", Some(&session)).await, (5, PLAIN));
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

const PASSWORD: &str = "0123456789";

fn checked_sign_in() -> Value {
    let mut settings = strict();
    settings["auth"] = json!(true);
    settings
}

/// A session gives today's numbers only while sign-in is Turnstile-checked too, and only if its own sign-in passed
/// that check. A sign-in let through because siteverify failed, a session from before the check was recorded (the
/// column's default), an expired or unknown session, or any session while sign-in is unchecked counts as none; a pass
/// still counts beside any session.
#[sqlx::test(migrations = "../core/migrations")]
#[ignore = "requires PostgreSQL with pgvector"]
async fn a_session_counts_only_if_its_sign_in_passed_the_check(pool: PgPool) {
    multi_thread(pool, |pool| async move {
        let user: i64 = sqlx::query_scalar("insert into users (username, email, display_name, password_hash) values ('a', 'a@example.test', 'A', $1) returning id")
            .bind(account::hash_password(PASSWORD.into()).await.ok().unwrap())
            .fetch_one(&pool)
            .await
            .unwrap();
        for (token, expiry) in [("tok-live", "1 day"), ("tok-old", "-1 day")] {
            sqlx::query("insert into sessions (token_hash, user_id, expires_at, turnstile_passed) values ($1, $2, now() + $3::interval, true)")
                .bind(account::token_hash(token))
                .bind(user)
                .bind(expiry)
                .execute(&pool)
                .await
                .unwrap();
        }
        // as a session from before the column reads
        sqlx::query("insert into sessions (token_hash, user_id, expires_at) values ($1, $2, now() + interval '1 day')")
            .bind(account::token_hash("tok-before"))
            .bind(user)
            .execute(&pool)
            .await
            .unwrap();
        let live = "settings=y; session=tok-live";

        // today's numbers; once today's bucket is empty the session is not read, so the refusal offers the check as it
        // does to any session cookie
        let site = site(&pool, checked_sign_in()).await;
        for (i, (path, _, base)) in TIERED.into_iter().enumerate() {
            assert_eq!(site.count(path, &format!("203.0.113.{i}"), Some(live)).await, (base, offered()), "{path}");
        }
        let passed = site.sign_in("in", "198.51.100.30").await;
        assert_eq!(site.count("/api/search", "203.0.113.19", Some(&passed)).await, (4, offered()));
        let unjudged = site.sign_in("failing", "198.51.100.31").await;
        for (n, cookie) in [&*unjudged, "session=tok-before", "session=tok-old", "session=tok-none"].into_iter().enumerate() {
            assert_eq!(site.count("/api/search", &format!("203.0.113.{}", 20 + n), Some(cookie)).await, (2, offered()), "{cookie}");
        }

        let site = self::site(&pool, strict()).await;
        assert_eq!(site.count("/api/search", "198.51.100.1", Some(live)).await, (2, offered()));
        let pass = site.pass("198.51.100.2").await;
        assert_eq!(site.count("/api/search", "198.51.100.2", Some(&format!("{live}; {pass}"))).await, (4, PLAIN));
    });
}

/// A made-up session cookie is read only while today's bucket admits the request: past the strict burst each refusal
/// draws from today's bucket first, so the reads per address stop at today's numbers. Either refusal offers the check.
/// A request without a session cookie draws from today's bucket only while the strict one admits it.
#[sqlx::test(migrations = "../core/migrations")]
#[ignore = "requires PostgreSQL with pgvector"]
async fn a_session_cookie_costs_reads_only_within_todays_numbers(pool: PgPool) {
    multi_thread(pool, |pool| async move {
        // today's bucket refills ten times slower than the strict one, so a refusal's wait names the bucket that answered
        let limits = Limits { search: Tiered::new((SLOW, 2.0), (SLOW / 10.0, 4.0)), ..limits() };
        let site = site_with(&pool, checked_sign_in(), limits).await;
        let made_up = [("x-forwarded-for", "203.0.113.40"), ("cookie", "session=tok-none")];
        // today's bucket (4) counts the 2 the strict bucket admits and the next 2 strict refusals; each of these reads
        // the session, finds none, and gets the strict bucket's wait
        assert_eq!(site.count_with("/api/search", &made_up).await, (2, offered()));
        let (refused, wait) = site.refusal("/api/search", &made_up).await;
        assert!(refused == offered() && wait < 2_000, "{refused:?} {wait}");
        // without a cookie the strict refusal draws from nothing, however often it is asked
        for _ in 0..6 {
            assert_eq!(site.count("/api/search", "203.0.113.41", None).await.1, offered());
        }
        // today's bucket now refuses before any read, still offering the check, with its own longer wait
        let (refused, wait) = site.refusal("/api/search", &made_up).await;
        assert!(refused == offered() && wait > 5_000, "{refused:?} {wait}");
    });
}

/// A session the API cannot read counts as none: the strict bucket's refusal stands, rather than a 500.
#[sqlx::test(migrations = "../core/migrations")]
#[ignore = "requires PostgreSQL with pgvector"]
async fn a_session_that_cannot_be_read_counts_as_none(pool: PgPool) {
    multi_thread(pool, |pool| async move {
        let site = site(&pool, checked_sign_in()).await;
        assert_eq!(site.count("/api/search", "203.0.113.1", None).await, (2, offered()));
        // the settings are already read, so the session lookup is the first read after this
        site.state.pool.close().await;
        assert_eq!(site.count("/api/search", "203.0.113.1", Some("session=tok-any")).await, (0, offered()));
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
