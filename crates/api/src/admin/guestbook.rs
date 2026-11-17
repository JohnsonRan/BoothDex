//! Guestbook moderation: every message, hidden ones included, newest first; hide, show again or delete one.

use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use serde_json::{Value, json};

use super::require_admin;
use crate::error::Refusal;
use crate::{ApiError, AppState};

const PAGE: i64 = 50;

#[derive(Deserialize)]
pub(super) struct ListQuery {
    before: Option<i64>,
}

pub(super) async fn list(State(s): State<Arc<AppState>>, headers: HeaderMap, Query(q): Query<ListQuery>) -> Result<Response, ApiError> {
    require_admin(&s, &headers).await?;
    let mut entries: Vec<Value> = sqlx::query_scalar(
        "select jsonb_build_object('id', id, 'nickname', nickname, 'body', body, 'created_at', created_at, 'hidden_at', hidden_at)
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

#[derive(Deserialize)]
pub(super) struct Moderation {
    hidden: bool,
}

pub(super) async fn patch(
    State(s): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(b): Json<Moderation>,
) -> Result<Response, ApiError> {
    let who = require_admin(&s, &headers).await?;
    let hidden_at: Option<Option<chrono::DateTime<chrono::Utc>>> = sqlx::query_scalar(
        "update guestbook set hidden_at = case when $2 then coalesce(hidden_at, now()) end where id = $1 returning hidden_at",
    )
    .bind(id)
    .bind(b.hidden)
    .fetch_optional(&s.pool)
    .await?;
    let Some(hidden_at) = hidden_at else { return Err(Refusal::NoSuchEntry.into()) };
    println!("admin {who} {} guestbook entry {id}", if b.hidden { "hid" } else { "showed" });
    Ok(Json(json!({ "id": id, "hidden_at": hidden_at })).into_response())
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

    #[sqlx::test(migrations = "../core/migrations")]
    #[ignore = "requires PostgreSQL with pgvector"]
    async fn an_admin_hides_shows_and_deletes(pool: sqlx::PgPool) {
        let boss: i64 = sqlx::query_scalar("insert into users (username, email, display_name, password_hash, is_admin) values ('boss', 'boss@example.test', 'Boss', 'x', true) returning id")
            .fetch_one(&pool)
            .await
            .unwrap();
        sqlx::query("insert into sessions (token_hash, user_id, expires_at) values ($1, $2, now() + interval '1 day')")
            .bind(account::token_hash("tok-boss"))
            .bind(boss)
            .execute(&pool)
            .await
            .unwrap();
        let mut h = HeaderMap::new();
        h.insert(header::COOKIE, "session=tok-boss".parse().unwrap());
        let id: i64 = sqlx::query_scalar("insert into guestbook (nickname, body) values ('Ann', 'hi') returning id").fetch_one(&pool).await.unwrap();
        let s = account::test_state(pool.clone());
        let status = |r: Result<Response, ApiError>| r.into_response().status();
        let hidden = || sqlx::query_scalar::<_, bool>("select hidden_at is not null from guestbook where id = $1").bind(id).fetch_one(&pool);

        assert_eq!(status(patch(State(s.clone()), h.clone(), Path(id), Json(Moderation { hidden: true })).await), StatusCode::OK);
        assert!(hidden().await.unwrap());
        let listed = list(State(s.clone()), h.clone(), Query(ListQuery { before: None })).await.ok().unwrap();
        let listed: Value = serde_json::from_slice(&axum::body::to_bytes(listed.into_body(), 1 << 20).await.unwrap()).unwrap();
        assert!(listed["entries"][0]["hidden_at"].is_string(), "{listed}");
        assert_eq!(status(patch(State(s.clone()), h.clone(), Path(id), Json(Moderation { hidden: false })).await), StatusCode::OK);
        assert!(!hidden().await.unwrap());
        assert_eq!(status(delete(State(s.clone()), h.clone(), Path(id)).await), StatusCode::OK);
        assert_eq!(status(delete(State(s.clone()), h.clone(), Path(id)).await), StatusCode::NOT_FOUND);
        assert_eq!(status(patch(State(s.clone()), h.clone(), Path(id), Json(Moderation { hidden: true })).await), StatusCode::NOT_FOUND);
    }
}
