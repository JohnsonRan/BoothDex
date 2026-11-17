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

/// Each holds a card number; a CVV or expiry after it, or a dot between groups, does not hide it.
const CARDS: &[&str] = &[
    "4111111111111111",
    "4111 1111 1111 1111",
    "pay 4111-1111-1111-1111 now",
    "５ 378282246310005",
    "4111 1111 1111 1111 123",
    "4111 1111 1111 1111 12/27",
    "4111 1111 1111 1111 12 27",
    "4111.1111.1111.1111",
    "cvv 123 4111111111111111",
];
/// A failing Luhn sum, too few or too many digits, two separators in a row, or a date and a phone number.
const NOT_CARDS: &[&str] =
    &["4111111111111112", "411111111111", "41111111111111111111", "4111  1111 1111 1111", "2026-11-17 12:00", "2026.11.17", "电话 13800138000"];

#[test]
fn card_numbers_are_found_in_runs_of_digits() {
    for card in CARDS {
        assert!(has_card_number(card), "{card}");
    }
    for text in NOT_CARDS {
        assert!(!has_card_number(text), "{text}");
    }
}

#[test]
fn entries_are_trimmed_and_bounded() {
    assert_eq!(validate("  Ann ", " hi\r\nthere\t ").unwrap(), ("Ann".into(), "hi\nthere".into()));
    assert!(validate(&"名".repeat(NICKNAME_MAX), &"字".repeat(BODY_MAX)).is_ok());
    for (nickname, body) in [(" ", "hi"), ("Ann", " "), (&*"名".repeat(NICKNAME_MAX + 1), "hi"), ("Ann", &*"字".repeat(BODY_MAX + 1)), ("A\nnn", "hi"), ("Ann", "h\0i"), ("Ann", "h\u{1b}i")] {
        assert!(matches!(validate(nickname, body), Err(Refusal::BadGuestbookEntry)), "{nickname:?} {body:?}");
    }
    assert!(matches!(validate("Ann", "4111 1111 1111 1111"), Err(Refusal::CardNumber)));
}

#[test]
fn nothing_invisible_is_taken() {
    // an emoji family keeps its zero-width joiners and a heart its variation selector
    assert!(validate("👨\u{200D}👩\u{200D}👧", "❤\u{FE0F}").is_ok());
    for c in INVISIBLE.iter().flat_map(|(a, b)| *a..=*b) {
        assert!(matches!(validate(&format!("A{c}nn"), "hi"), Err(Refusal::BadGuestbookEntry)), "{:04X}", c as u32);
        assert!(matches!(validate("Ann", &format!("h{c}i")), Err(Refusal::BadGuestbookEntry)), "{:04X}", c as u32);
    }
    for blank in ["\u{200D}", "\u{FE0F}", "\u{200D} \u{FE0F}"] {
        assert!(matches!(validate(blank, "hi"), Err(Refusal::BadGuestbookEntry)), "{blank:?}");
        assert!(matches!(validate("Ann", blank), Err(Refusal::BadGuestbookEntry)), "{blank:?}");
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
    answer(list(State(s.clone()), ip(200), Query(ListQuery { before: None })).await).await.1
}

#[sqlx::test(migrations = "../core/migrations")]
#[ignore = "requires PostgreSQL with pgvector"]
async fn closed_until_both_keys_are_set(pool: PgPool) {
    let s = state(pool.clone()).await;
    assert_eq!(page(&s).await["site_key"], Value::Null);
    let (status, body) = answer(post(State(s.clone()), ip(1), entry("Ann", "hi", "pass")).await).await;
    assert_eq!((status, body["code"].as_str()), (StatusCode::SERVICE_UNAVAILABLE, Some("verification_unavailable")));
    sqlx::query("insert into app_settings (key, value) values ('turnstile', '{\"site_key\": \"0x-site\"}')").execute(&pool).await.unwrap();
    assert_eq!(page(&s).await["site_key"], Value::Null);
    let (status, _) = answer(post(State(s.clone()), ip(1), entry("Ann", "hi", "pass")).await).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(sqlx::query_scalar::<_, i64>("select count(*) from guestbook").fetch_one(&pool).await.unwrap(), 0);
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
    let (status, body) = answer(post(State(s.clone()), ip(4), entry("Ann", "4111 1111 1111 1111", "pass")).await).await;
    assert_eq!((status, body["code"].as_str()), (StatusCode::BAD_REQUEST, Some("card_number")));
    let (status, body) = answer(post(State(s.clone()), ip(4), entry("", "hi", "pass")).await).await;
    assert_eq!((status, body["code"].as_str()), (StatusCode::BAD_REQUEST, Some("bad_guestbook_entry")));
    assert_eq!(sqlx::query_scalar::<_, i64>("select count(*) from guestbook").fetch_one(&pool).await.unwrap(), 0);

    // Cloudflare's testing secrets pass every token without naming the action, so a local run can post
    assert_eq!(answer(post(State(s.clone()), ip(7), entry("Bob", "test", "testing")).await).await.0, StatusCode::OK);
    let (status, body) = answer(post(State(s.clone()), ip(5), entry(" Ann ", "hello", "pass")).await).await;
    assert_eq!((status, &body["nickname"], &body["body"]), (StatusCode::OK, &json!("Ann"), &json!("hello")));
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
    let rest = answer(list(State(s.clone()), ip(200), Query(ListQuery { before: Some(10) })).await).await.1;
    assert_eq!((ids(&rest), &rest["more"]), ((1..10).rev().collect(), &json!(false)));
}

/// The database refuses a card number even when the API's check is skipped.
#[sqlx::test(migrations = "../core/migrations")]
#[ignore = "requires PostgreSQL with pgvector"]
async fn the_database_refuses_a_card_number(pool: PgPool) {
    let insert = |nickname: &'static str, body: &'static str| {
        sqlx::query("insert into guestbook (nickname, body) values ($1, $2)").bind(nickname).bind(body).execute(&pool)
    };
    assert!(insert("Ann", "card 4111-1111-1111-1111").await.is_err());
    assert!(insert("4111111111111111", "hi").await.is_err());
    assert!(insert("Ann", "2026-11-17 12:00, 4111111111111112, 电话 13800138000").await.is_ok());
    // the backstop agrees with the API on every case
    for t in CARDS.iter().chain(NOT_CARDS) {
        let sql: bool = sqlx::query_scalar("select has_card_number($1)").bind(t).fetch_one(&pool).await.unwrap();
        assert_eq!(sql, has_card_number(t), "{t}");
    }
}

/// Reading the list draws on the lookups' per-address bucket, so a loop cannot hold the pool.
#[sqlx::test(migrations = "../core/migrations")]
#[ignore = "requires PostgreSQL with pgvector"]
async fn reading_the_list_is_rate_limited(pool: PgPool) {
    let s = state(pool).await;
    while s.limits.lookup.check(ip(9).0).is_ok() {}
    let (status, _) = answer(list(State(s.clone()), ip(9), Query(ListQuery { before: None })).await).await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(answer(list(State(s.clone()), ip(10), Query(ListQuery { before: None })).await).await.0, StatusCode::OK);
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
