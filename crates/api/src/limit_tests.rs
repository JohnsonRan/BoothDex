use super::*;

#[tokio::test]
async fn rate_limit_response_keeps_status_retry_after_and_body() {
    let response = RateLimited::after(7).into_response();
    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(response.headers()[header::RETRY_AFTER], "7");
    let body = axum::body::to_bytes(response.into_body(), 1024).await.unwrap();
    assert_eq!(serde_json::from_slice::<serde_json::Value>(&body).unwrap(), json!({ "ok": false, "error": "Too many requests. Try again shortly." }));
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
