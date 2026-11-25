use std::sync::Arc;

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::{Value, json};
use sqlx::PgPool;

use super::*;
use crate::account;
use crate::limit::ClientIp;

#[test]
fn entries_are_trimmed_and_bounded() {
    assert_eq!(validate("  Ann ", " hi\r\nthere\t ").unwrap(), ("Ann".into(), "hi\nthere".into()));
    assert!(validate(&"名".repeat(NICKNAME_MAX), &"字".repeat(BODY_MAX)).is_ok());
    for (nickname, body) in [(" ", "hi"), ("Ann", " "), (&*"名".repeat(NICKNAME_MAX + 1), "hi"), ("Ann", &*"字".repeat(BODY_MAX + 1)), ("A\nnn", "hi"), ("A\u{2028}nn", "hi"), ("A\u{2029}nn", "hi"), ("Ann", "h\0i"), ("Ann", "h\u{1b}i")] {
        assert!(matches!(validate(nickname, body), Err(Refusal::BadGuestbookEntry)), "{nickname:?} {body:?}");
    }
}

/// The same cases as "a message keeps at most 10 lines, one blank line at a time" in
/// web/src/components/guestbook/rules.test.ts, so the form and the API agree.
#[test]
fn a_message_keeps_ten_lines_and_one_blank_line_at_a_time() {
    let kept = |body: &str| validate("Ann", body).map(|(_, body)| body);
    assert_eq!(kept("a\n\n\n\nb").unwrap(), "a\n\nb");
    assert_eq!(kept("a\r\n \r\n\t\r\n\u{3000}\nb").unwrap(), "a\n\nb");
    assert_eq!(kept("a\n\u{200D}\n\u{FE0F}\nb").unwrap(), "a\n\nb");
    assert_eq!(kept("a\u{2028}b\u{2029}\u{2029}\u{2029}c").unwrap(), "a\nb\n\nc");
    assert_eq!(kept("\n\n a \n\n").unwrap(), "a");
    // two letters and 498 line breaks are two lines and a blank one
    assert_eq!(kept(&format!("a{}b", "\n".repeat(498))).unwrap(), "a\n\nb");
    let lines = |n: usize| (1..=n).map(|i| i.to_string()).collect::<Vec<_>>().join("\n");
    assert!(kept(&lines(BODY_LINES)).is_ok());
    assert!(matches!(kept(&lines(BODY_LINES + 1)), Err(Refusal::BadGuestbookEntry)));
    // blank lines count once each run: ten lines with one blank line between each pair would be nineteen
    assert!(matches!(kept(&lines(BODY_LINES).replace('\n', "\n\n\n")), Err(Refusal::BadGuestbookEntry)));
    assert!(kept(&lines(5).replace('\n', "\n\n\n")).is_ok());
}

#[test]
fn nothing_invisible_is_taken() {
    // an emoji family keeps its zero-width joiners, a heart its variation selector and a flag its tags
    assert!(validate("👨\u{200D}👩\u{200D}👧", "❤\u{FE0F} 🏴\u{E0067}\u{E0062}\u{E0065}\u{E006E}\u{E0067}\u{E007F}").is_ok());
    // unassigned default-ignorables show nothing either, so a field made only of them would read blank
    for c in ['\u{FFF0}', '\u{FFF8}', '\u{E0080}', '\u{E00FF}', '\u{E01F0}', '\u{E0FFF}'] {
        assert!(matches!(validate(&c.to_string().repeat(5), "hi"), Err(Refusal::BadGuestbookEntry)), "{:04X}", c as u32);
    }
    for c in INVISIBLE.iter().flat_map(|(a, b)| *a..=*b) {
        assert!(matches!(validate(&format!("A{c}nn"), "hi"), Err(Refusal::BadGuestbookEntry)), "{:04X}", c as u32);
        assert!(matches!(validate("Ann", &format!("h{c}i")), Err(Refusal::BadGuestbookEntry)), "{:04X}", c as u32);
    }
    for c in SHAPING.iter().flat_map(|(a, b)| *a..=*b) {
        let blank = format!("{c} {c}");
        assert!(matches!(validate(&blank, "hi"), Err(Refusal::BadGuestbookEntry)), "{:04X}", c as u32);
        assert!(matches!(validate("Ann", &blank), Err(Refusal::BadGuestbookEntry)), "{:04X}", c as u32);
        assert!(validate(&format!("A{c}nn"), &format!("h{c}i")).is_ok(), "{:04X}", c as u32);
    }
}

/// A stand-in for siteverify: the token "pass" passes for the guestbook, "elsewhere" for another form, "unnamed" for
/// no form, "testing" as a testing secret answers (no action), and anything else fails.
async fn siteverify_stand_in() -> String {
    let app = axum::Router::new().route(
        "/",
        axum::routing::post(|Json(b): Json<Value>| async move {
            Json(match b["response"].as_str() {
                Some("pass") => json!({ "success": true, "action": "guestbook" }),
                Some("elsewhere") => json!({ "success": true, "action": "sign-in" }),
                Some("unnamed") => json!({ "success": true }),
                Some("testing") => json!({ "success": true, "metadata": { "result_with_testing_key": true } }),
                _ => json!({ "success": false, "error-codes": ["invalid-input-response"] }),
            })
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    url
}

async fn state(pool: PgPool) -> Arc<AppState> {
    let state = Arc::try_unwrap(account::test_state(pool)).ok().unwrap();
    Arc::new(AppState { siteverify_url: siteverify_stand_in().await, ..state })
}

async fn open(pool: &PgPool) {
    sqlx::query("insert into app_settings (key, value) values ('turnstile', '{\"site_key\": \"0x-site\", \"secret\": \"0x-secret\"}')")
        .execute(pool)
        .await
        .unwrap();
}

fn ip(n: u8) -> ClientIp {
    ClientIp([203, 0, 113, n].into())
}

fn entry(nickname: &str, body: &str, token: &str) -> Json<NewEntry> {
    Json(NewEntry { nickname: nickname.into(), body: body.into(), token: token.into() })
}

async fn answer(r: Result<Response, ApiError>) -> (StatusCode, Value) {
    let r = r.into_response();
    let status = r.status();
    (status, serde_json::from_slice(&axum::body::to_bytes(r.into_body(), 1 << 20).await.unwrap()).unwrap())
}

async fn page(s: &Arc<AppState>) -> Value {
    answer(list(State(s.clone()), ip(200), crate::limit::Tier::Base, Query(ListQuery { before: None })).await).await.1
}

#[sqlx::test(migrations = "../core/migrations")]
#[ignore = "requires PostgreSQL with pgvector"]
async fn closed_until_both_keys_are_set(pool: PgPool) {
    let s = state(pool.clone()).await;
    assert_eq!(page(&s).await["site_key"], Value::Null);
    let (status, body) = answer(post(State(s.clone()), ip(1), entry("Ann", "hi", "pass")).await).await;
    assert_eq!((status, body["code"].as_str()), (StatusCode::SERVICE_UNAVAILABLE, Some("verification_unavailable")));
    sqlx::query("insert into app_settings (key, value) values ('turnstile', '{\"site_key\": \"0x-site\"}')").execute(&pool).await.unwrap();
    // a fresh API reads the row, as one does after a restart; the console's save updates a running one
    let s = state(pool.clone()).await;
    assert_eq!(page(&s).await["site_key"], Value::Null);
    let (status, _) = answer(post(State(s.clone()), ip(1), entry("Ann", "hi", "pass")).await).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(sqlx::query_scalar::<_, i64>("select count(*) from guestbook").fetch_one(&pool).await.unwrap(), 0);
}

/// The list and the post read the API's copy of the settings, the one the rate limits and `/api/verify` read, so
/// all of them change together when the console saves.
#[sqlx::test(migrations = "../core/migrations")]
#[ignore = "requires PostgreSQL with pgvector"]
async fn the_guestbook_follows_the_apis_copy_of_the_settings(pool: PgPool) {
    let s = state(pool.clone()).await;
    let copy = crate::turnstile::resolve(&json!({ "site_key": "0x-site", "secret": "0x-secret" }), |_| None);
    s.turnstile.store(Some(Arc::new(copy)));
    assert_eq!(page(&s).await["site_key"], "0x-site");
    assert_eq!(answer(post(State(s.clone()), ip(1), entry("Ann", "hi", "pass")).await).await.0, StatusCode::OK);
}

#[sqlx::test(migrations = "../core/migrations")]
#[ignore = "requires PostgreSQL with pgvector"]
async fn a_post_needs_a_passing_token_for_this_form(pool: PgPool) {
    open(&pool).await;
    let s = state(pool.clone()).await;
    for (n, token) in [(1, ""), (2, "fail"), (3, "elsewhere"), (6, "unnamed")] {
        let (status, body) = answer(post(State(s.clone()), ip(n), entry("Ann", "hi", token)).await).await;
        assert_eq!((status, body["code"].as_str()), (StatusCode::FORBIDDEN, Some("verification_failed")), "{token}");
    }
    let (status, body) = answer(post(State(s.clone()), ip(4), entry("", "hi", "pass")).await).await;
    assert_eq!((status, body["code"].as_str()), (StatusCode::BAD_REQUEST, Some("bad_guestbook_entry")));
    assert_eq!(sqlx::query_scalar::<_, i64>("select count(*) from guestbook").fetch_one(&pool).await.unwrap(), 0);

    // Cloudflare's testing secrets pass every token without naming the action, so a local run can post
    assert_eq!(answer(post(State(s.clone()), ip(7), entry("Bob", "test", "testing")).await).await.0, StatusCode::OK);
    let (status, body) = answer(post(State(s.clone()), ip(5), entry(" Ann ", "hello", "pass")).await).await;
    assert_eq!((status, &body["nickname"], &body["body"], &body["status"]), (StatusCode::OK, &json!("Ann"), &json!("hello"), &json!("open")));
    let shown = page(&s).await;
    assert_eq!((&shown["entries"][0], shown["site_key"].as_str()), (&body, Some("0x-site")));
}

/// Newest first, without hidden messages, 50 to a page.
#[sqlx::test(migrations = "../core/migrations")]
#[ignore = "requires PostgreSQL with pgvector"]
async fn the_list_pages_and_leaves_out_hidden_messages(pool: PgPool) {
    sqlx::query("insert into guestbook (nickname, body, hidden_at) select 'Ann', 'm' || g, case when g = 60 then now() end from generate_series(1, 61) g")
        .execute(&pool)
        .await
        .unwrap();
    let s = state(pool.clone()).await;
    let first = page(&s).await;
    let ids = |p: &Value| p["entries"].as_array().unwrap().iter().map(|e| e["id"].as_i64().unwrap()).collect::<Vec<_>>();
    assert_eq!((ids(&first)[..2].to_vec(), ids(&first).len(), &first["more"]), (vec![61, 59], 50, &json!(true)));
    let rest = answer(list(State(s.clone()), ip(200), crate::limit::Tier::Base, Query(ListQuery { before: Some(10) })).await).await.1;
    assert_eq!((ids(&rest), &rest["more"]), ((1..10).rev().collect(), &json!(false)));
}

/// Reading the list draws on the lookups' per-address bucket, so a loop cannot hold the pool.
#[sqlx::test(migrations = "../core/migrations")]
#[ignore = "requires PostgreSQL with pgvector"]
async fn reading_the_list_is_rate_limited(pool: PgPool) {
    let s = state(pool).await;
    while s.limits.lookup.check(ip(9).0, &crate::limit::Tier::Base).await.is_ok() {}
    let (status, _) = answer(list(State(s.clone()), ip(9), crate::limit::Tier::Base, Query(ListQuery { before: None })).await).await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(answer(list(State(s.clone()), ip(10), crate::limit::Tier::Base, Query(ListQuery { before: None })).await).await.0, StatusCode::OK);
}

#[sqlx::test(migrations = "../core/migrations")]
#[ignore = "requires PostgreSQL with pgvector"]
async fn one_address_posts_three_times_in_ten_minutes(pool: PgPool) {
    open(&pool).await;
    let s = state(pool.clone()).await;
    for _ in 0..3 {
        assert_eq!(answer(post(State(s.clone()), ip(1), entry("Ann", "hi", "pass")).await).await.0, StatusCode::OK);
    }
    assert_eq!(answer(post(State(s.clone()), ip(1), entry("Ann", "hi", "pass")).await).await.0, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(answer(post(State(s.clone()), ip(2), entry("Ann", "hi", "pass")).await).await.0, StatusCode::OK);
}
