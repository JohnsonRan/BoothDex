//! Guestbook moderation: every message, hidden ones included, newest first; hide, show again or delete one, and
//! close one as completed or as not planned, or reopen it, the way an issue is closed.

use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use serde_json::{Value, json};

use super::require_admin;
use crate::error::Refusal;
use crate::guestbook::GuestbookStatus;
use crate::{ApiError, AppState};

const PAGE: i64 = 50;

#[derive(Deserialize)]
pub(super) struct ListQuery {
    before: Option<i64>,
}

pub(super) async fn list(State(s): State<Arc<AppState>>, headers: HeaderMap, Query(q): Query<ListQuery>) -> Result<Response, ApiError> {
    require_admin(&s, &headers).await?;
    let mut entries: Vec<Value> = sqlx::query_scalar(
        "select jsonb_build_object('id', id, 'nickname', nickname, 'body', body, 'status', status, 'created_at', created_at, 'hidden_at', hidden_at)
         from guestbook where $1::bigint is null or id < $1 order by id desc limit $2",
    )
    .bind(q.before)
    .bind(PAGE + 1)
    .fetch_all(&s.pool)
    .await?;
    let more = entries.len() as i64 > PAGE;
    entries.truncate(PAGE as usize);
    Ok(Json(json!({ "entries": entries, "more": more })).into_response())
}

/// What to change on a message; a field left out stays as it is.
#[derive(Default, Deserialize)]
pub(super) struct Moderation {
    hidden: Option<bool>,
    status: Option<GuestbookStatus>,
}

pub(super) async fn patch(
    State(s): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(b): Json<Moderation>,
) -> Result<Response, ApiError> {
    let who = require_admin(&s, &headers).await?;
    let changed: Option<(Option<chrono::DateTime<chrono::Utc>>, GuestbookStatus)> = sqlx::query_as(
        "update guestbook set
           hidden_at = case when $2 is null then hidden_at when $2 then coalesce(hidden_at, now()) end,
           status = coalesce($3, status)
         where id = $1 returning hidden_at, status",
    )
    .bind(id)
    .bind(b.hidden)
    .bind(b.status)
    .fetch_optional(&s.pool)
    .await?;
    let Some((hidden_at, status)) = changed else { return Err(Refusal::NoSuchEntry.into()) };
    if let Some(hidden) = b.hidden {
        println!("admin {who} {} guestbook entry {id}", if hidden { "hid" } else { "showed" });
    }
    if let Some(status) = b.status {
        println!("admin {who} set guestbook entry {id} to {status:?}");
    }
    Ok(Json(json!({ "id": id, "hidden_at": hidden_at, "status": status })).into_response())
}

pub(super) async fn delete(State(s): State<Arc<AppState>>, headers: HeaderMap, Path(id): Path<i64>) -> Result<Response, ApiError> {
    let who = require_admin(&s, &headers).await?;
    let gone = sqlx::query("delete from guestbook where id = $1").bind(id).execute(&s.pool).await?.rows_affected();
    if gone == 0 {
        return Err(Refusal::NoSuchEntry.into());
    }
    println!("admin {who} deleted guestbook entry {id}");
    Ok(Json(json!({ "ok": true })).into_response())
}

#[cfg(test)]
mod tests {
    use axum::extract::{Path, Query, State};
    use axum::http::{StatusCode, header};

    use super::*;
    use crate::account;

    /// The headers of a signed-in admin.
    async fn boss(pool: &sqlx::PgPool) -> HeaderMap {
        let boss: i64 = sqlx::query_scalar("insert into users (username, email, display_name, password_hash, is_admin) values ('boss', 'boss@example.test', 'Boss', 'x', true) returning id")
            .fetch_one(pool)
            .await
            .unwrap();
        sqlx::query("insert into sessions (token_hash, user_id, expires_at) values ($1, $2, now() + interval '1 day')")
            .bind(account::token_hash("tok-boss"))
            .bind(boss)
            .execute(pool)
            .await
            .unwrap();
        let mut h = HeaderMap::new();
        h.insert(header::COOKIE, "session=tok-boss".parse().unwrap());
        h
    }

    #[sqlx::test(migrations = "../core/migrations")]
    #[ignore = "requires PostgreSQL with pgvector"]
    async fn an_admin_hides_shows_and_deletes(pool: sqlx::PgPool) {
        let h = boss(&pool).await;
        let id: i64 = sqlx::query_scalar("insert into guestbook (nickname, body) values ('Ann', 'hi') returning id").fetch_one(&pool).await.unwrap();
        let s = account::test_state(pool.clone());
        let status = |r: Result<Response, ApiError>| r.into_response().status();
        let hidden = || sqlx::query_scalar::<_, bool>("select hidden_at is not null from guestbook where id = $1").bind(id).fetch_one(&pool);

        assert_eq!(status(patch(State(s.clone()), h.clone(), Path(id), Json(Moderation { hidden: Some(true), ..Default::default() })).await), StatusCode::OK);
        assert!(hidden().await.unwrap());
        let listed = list(State(s.clone()), h.clone(), Query(ListQuery { before: None })).await.ok().unwrap();
        let listed: Value = serde_json::from_slice(&axum::body::to_bytes(listed.into_body(), 1 << 20).await.unwrap()).unwrap();
        assert!(listed["entries"][0]["hidden_at"].is_string(), "{listed}");
        assert_eq!(status(patch(State(s.clone()), h.clone(), Path(id), Json(Moderation { hidden: Some(false), ..Default::default() })).await), StatusCode::OK);
        assert!(!hidden().await.unwrap());
        assert_eq!(status(delete(State(s.clone()), h.clone(), Path(id)).await), StatusCode::OK);
        assert_eq!(status(delete(State(s.clone()), h.clone(), Path(id)).await), StatusCode::NOT_FOUND);
        assert_eq!(status(patch(State(s.clone()), h.clone(), Path(id), Json(Moderation { hidden: Some(true), ..Default::default() })).await), StatusCode::NOT_FOUND);
    }

    /// Closing and reopening leave a hidden message hidden, and hiding leaves a closed one closed.
    #[sqlx::test(migrations = "../core/migrations")]
    #[ignore = "requires PostgreSQL with pgvector"]
    async fn an_admin_closes_and_reopens(pool: sqlx::PgPool) {
        let h = boss(&pool).await;
        let id: i64 = sqlx::query_scalar("insert into guestbook (nickname, body) values ('Ann', 'hi') returning id").fetch_one(&pool).await.unwrap();
        let s = account::test_state(pool.clone());
        let stored = || sqlx::query_as::<_, (String, bool)>("select status, hidden_at is not null from guestbook where id = $1").bind(id).fetch_one(&pool);
        assert_eq!(stored().await.unwrap(), ("open".into(), false));

        for (status, text) in [(GuestbookStatus::Completed, "completed"), (GuestbookStatus::NotPlanned, "not_planned"), (GuestbookStatus::Open, "open")] {
            let r = patch(State(s.clone()), h.clone(), Path(id), Json(Moderation { status: Some(status), ..Default::default() })).await.ok().unwrap();
            let r: Value = serde_json::from_slice(&axum::body::to_bytes(r.into_body(), 1 << 20).await.unwrap()).unwrap();
            assert_eq!((&r["status"], &r["hidden_at"]), (&json!(text), &Value::Null));
            assert_eq!(stored().await.unwrap(), (text.into(), false));
        }

        let change = |m: Moderation| patch(State(s.clone()), h.clone(), Path(id), Json(m));
        assert!(change(Moderation { hidden: Some(true), ..Default::default() }).await.is_ok());
        assert!(change(Moderation { status: Some(GuestbookStatus::NotPlanned), ..Default::default() }).await.is_ok());
        assert_eq!(stored().await.unwrap(), ("not_planned".into(), true));
        assert!(change(Moderation { hidden: Some(false), ..Default::default() }).await.is_ok());
        assert_eq!(stored().await.unwrap(), ("not_planned".into(), false));
        let listed = list(State(s.clone()), h.clone(), Query(ListQuery { before: None })).await.ok().unwrap();
        let listed: Value = serde_json::from_slice(&axum::body::to_bytes(listed.into_body(), 1 << 20).await.unwrap()).unwrap();
        assert_eq!(listed["entries"][0]["status"], "not_planned");

        // a state the site does not know is refused where the request is read, and the table's check backs it
        assert!(serde_json::from_value::<Moderation>(json!({ "status": "closed" })).is_err());
        assert!(sqlx::query("update guestbook set status = 'closed' where id = $1").bind(id).execute(&pool).await.is_err());
    }
}
