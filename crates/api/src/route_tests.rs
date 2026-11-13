//! Handler tests without a database: per-route rate limits, the search permits, and the bodies of refusals.

use crate::state::test_state;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn search_allows_burst_and_throttles_one_ip_without_affecting_others() {
    use axum::extract::{FromRequestParts, RawQuery, State};
    use axum::http::{HeaderMap, Request, StatusCode, header};
    use axum::response::IntoResponse;

    let state = test_state();

    let ip_of = |xff: &'static str| {
        let state = state.clone();
        async move {
            let (mut parts, _) = Request::builder().uri("/api/search").header("x-forwarded-for", xff).body(()).unwrap().into_parts();
            super::limit::ClientIp::from_request_parts(&mut parts, &state).await.unwrap()
        }
    };
    let call = |xff: &'static str| {
        let state = state.clone();
        async move {
            let ip = ip_of(xff).await;
            super::search::search(State(state), ip, HeaderMap::new(), RawQuery(None))
                .await
                .unwrap_or_else(axum::response::IntoResponse::into_response)
        }
    };

    for _ in 0..3 {
        assert_eq!(call("203.0.113.10").await.status(), StatusCode::OK);
    }
    let blocked = call("203.0.113.10").await;
    assert_eq!(blocked.status(), StatusCode::TOO_MANY_REQUESTS);
    assert!(blocked.headers().contains_key(header::RETRY_AFTER));
    // another visitor keeps searching unaffected
    assert_eq!(call("203.0.113.11").await.status(), StatusCode::OK);

    // exhausting palette, lookups, rss, items and gallery on a fresh IP never starves /api/search
    let ip = "203.0.113.99";
    for _ in 0..2 {
        assert_eq!(super::palette(State(state.clone()), ip_of(ip).await, RawQuery(Some("q=dress".into()))).await.into_response().status(), StatusCode::OK);
    }
    assert_eq!(super::palette(State(state.clone()), ip_of(ip).await, RawQuery(Some("q=dress".into()))).await.into_response().status(), StatusCode::TOO_MANY_REQUESTS);

    for _ in 0..2 {
        assert!(state.limits.lookup.check(ip_of(ip).await.0).is_ok());
    }
    assert!(state.limits.lookup.check(ip_of(ip).await.0).is_err());

    for _ in 0..2 {
        assert_eq!(super::rss_feed(State(state.clone()), ip_of(ip).await, RawQuery(None)).await.into_response().status(), StatusCode::OK);
    }
    assert_eq!(super::rss_feed(State(state.clone()), ip_of(ip).await, RawQuery(None)).await.into_response().status(), StatusCode::TOO_MANY_REQUESTS);

    for _ in 0..2 {
        assert_eq!(super::items_by_id(State(state.clone()), ip_of(ip).await, RawQuery(None)).await.into_response().status(), StatusCode::OK);
    }
    assert_eq!(super::items_by_id(State(state.clone()), ip_of(ip).await, RawQuery(None)).await.into_response().status(), StatusCode::TOO_MANY_REQUESTS);

    use axum::extract::Path;
    for _ in 0..2 {
        assert_eq!(super::gallery::gallery(State(state.clone()), ip_of(ip).await, Path(999)).await.into_response().status(), StatusCode::NOT_FOUND);
    }
    assert_eq!(super::gallery::gallery(State(state.clone()), ip_of(ip).await, Path(999)).await.into_response().status(), StatusCode::TOO_MANY_REQUESTS);

    // /api/search for that same IP still succeeds with its full burst intact
    for _ in 0..3 {
        assert_eq!(call(ip).await.status(), StatusCode::OK);
    }
    assert_eq!(call(ip).await.status(), StatusCode::TOO_MANY_REQUESTS);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn search_cpu_timing_excludes_the_permit_queue() {
    use axum::body::to_bytes;
    use axum::extract::{RawQuery, State};
    use axum::http::HeaderMap;
    use axum::response::IntoResponse;

    let state = test_state();
    let held = state.search_permits.acquire_many(16).await.unwrap();
    let request = super::search::search(
        State(state.clone()), super::limit::ClientIp("203.0.113.70".parse().unwrap()), HeaderMap::new(), RawQuery(None),
    );
    tokio::pin!(request);
    // Poll all the way to the exhausted semaphore before starting the controlled wait.
    assert!(futures_util::poll!(&mut request).is_pending());
    tokio::time::sleep(std::time::Duration::from_millis(30)).await;
    drop(held);
    let response = request.await.unwrap_or_else(IntoResponse::into_response);
    assert_eq!(response.status(), axum::http::StatusCode::OK);
    let body: serde_json::Value = serde_json::from_slice(&to_bytes(response.into_body(), 1024 * 1024).await.unwrap()).unwrap();
    let steps = body["trace"].as_array().unwrap();
    let ms = |step: &str| steps.iter().find(|s| s["step"] == step).unwrap()["ms"].as_f64().unwrap();
    // trace steps are rounded to 0.1 ms, so a step reads at most 0.05 ms below its true length; nothing else is slack
    const ROUNDING_MS: f64 = 0.05;
    assert!(ms("queue") >= 30.0 - ROUNDING_MS, "{body}");
    assert!(body["timing"]["searchMs"].as_f64().unwrap() <= ms("search") + ROUNDING_MS, "{body}");
    assert!(body["timing"]["totalMs"].as_f64().unwrap() + ROUNDING_MS >= ms("queue"), "{body}");
}

#[tokio::test]
async fn auth_limiter_exhaustion_throttles_both_login_and_register() {
    use axum::extract::{Json, State};
    use axum::http::{HeaderMap, StatusCode};
    use axum::response::IntoResponse;

    let state = test_state();
    let ip = super::limit::ClientIp("203.0.113.50".parse().unwrap());

    // Exhaust the auth limiter burst for this IP
    while state.limits.auth.check(ip.0).is_ok() {}

    enum AuthCall {
        Login(super::account::Login),
        Register(super::account::Register),
    }

    let cases = [
        (
            "login",
            AuthCall::Login(super::account::Login {
                login: "testuser".into(),
                password: "password12345".into(),
                keep: false,
            }),
        ),
        (
            "register",
            AuthCall::Register(super::account::Register {
                username: "testuser".into(),
                email: "testuser@example.com".into(),
                display_name: "Test User".into(),
                password: "password12345".into(),
                keep: false,
            }),
        ),
    ];

    for (name, call) in cases {
        let res = match call {
            AuthCall::Login(body) => {
                super::account::login(State(state.clone()), ip, HeaderMap::new(), Json(body)).await.into_response()
            }
            AuthCall::Register(body) => {
                super::account::register(State(state.clone()), ip, HeaderMap::new(), Json(body)).await.into_response()
            }
        };
        assert_eq!(
            res.status(),
            StatusCode::TOO_MANY_REQUESTS,
            "handler {name} must return 429 when auth limiter is exhausted"
        );
    }
    // the refused sign-in printed its warning, which spent this address's one line a minute
    assert!(state.limits.auth_log.take(ip.0).is_err());
}

#[tokio::test]
async fn account_writes_are_limited_per_visitor_and_reads_are_not() {
    let state = test_state();
    let app = axum::Router::new()
        .route("/x", axum::routing::get(|| async { "ok" }).post(|| async { "ok" }))
        .route_layer(axum::middleware::from_fn_with_state(state.clone(), super::account_write_rate_limit))
        .with_state(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/x", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, app).await });
    let http = reqwest::Client::new();
    let status = |r: reqwest::RequestBuilder| async move { r.send().await.unwrap().status().as_u16() };

    for _ in 0..2 {
        assert_eq!(status(http.post(&url)).await, 200);
    }
    assert_eq!(status(http.post(&url)).await, 429);
    for _ in 0..5 {
        assert_eq!(status(http.get(&url)).await, 200);
    }
    assert_eq!(status(http.post(&url).header("x-forwarded-for", "203.0.113.5")).await, 200);
}

/// What a client reads without a session, and when the API itself fails: the status and the JSON body.
#[tokio::test]
async fn missing_sessions_and_failures_keep_their_status_and_body() {
    use axum::extract::{RawQuery, State};
    use axum::http::{HeaderMap, StatusCode};
    use axum::response::IntoResponse;
    use serde_json::{Value, json};

    async fn answer(r: impl IntoResponse) -> (StatusCode, Value) {
        let r = r.into_response();
        let status = r.status();
        (status, serde_json::from_slice(&axum::body::to_bytes(r.into_body(), 1 << 20).await.unwrap()).unwrap())
    }

    let ip = super::limit::ClientIp("203.0.113.60".parse().unwrap());
    let favourites = super::search::search(State(test_state()), ip, HeaderMap::new(), RawQuery(Some("v=favourites".into()))).await;
    assert_eq!(answer(favourites).await, (StatusCode::UNAUTHORIZED, json!({ "ok": false, "code": "sign_in_required", "error": "Sign in required" })));

    // the cause goes to the log, never to the caller
    let failure: super::ApiError = std::io::Error::other("the cause").into();
    assert_eq!(answer(failure).await, (StatusCode::INTERNAL_SERVER_ERROR, json!({ "ok": false, "error": "internal error" })));
}

/// The feed, the palette and item records queue for the search's permits and answer 429 when none comes.
#[tokio::test]
async fn the_feed_the_palette_and_item_records_wait_for_a_search_permit() {
    use axum::extract::{RawQuery, State};
    use axum::http::{StatusCode, header};
    use axum::response::IntoResponse;

    let state = test_state();
    // a closed semaphore refuses at once; a full one refuses the same way after two seconds
    state.search_permits.close();
    let ip = super::limit::ClientIp("203.0.113.70".parse().unwrap());
    let answers = [
        super::rss_feed(State(state.clone()), ip, RawQuery(None)).await.into_response(),
        super::palette(State(state.clone()), ip, RawQuery(Some("q=dress".into()))).await.into_response(),
        super::items_by_id(State(state.clone()), ip, RawQuery(Some("ids=1".into()))).await.into_response(),
    ];
    for r in answers {
        assert_eq!(r.status(), StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(r.headers()[header::RETRY_AFTER], "2");
    }
}
