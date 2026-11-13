use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Deserialize;
use serde_json::{Value, json};

use super::require_user;
use crate::error::Refusal;
use crate::{ApiError, AppState};

pub(super) const MAX_SAVED_SEARCHES: i64 = 100;

pub(super) async fn list_searches(State(s): State<Arc<AppState>>, headers: HeaderMap) -> Result<Response, ApiError> {
    let u = require_user(&s, &headers).await?;
    let rows: Vec<(i64, String, String)> =
        sqlx::query_as("select id, name, query from saved_searches where user_id = $1 order by created_at desc").bind(u.id).fetch_all(&s.pool).await?;
    let items: Vec<Value> = rows
        .into_iter()
        .map(|(id, name, query)| json!({ "id": id, "name": name, "query": query, "rss": format!("{}/api/rss?{query}", s.site_url) }))
        .collect();
    Ok(Json(json!({ "items": items })).into_response())
}

/// A saved search's name: 1 to 100 characters once trimmed.
fn valid_search_name(name: &str) -> bool {
    (1..=100).contains(&name.trim().chars().count()) && crate::error::storable(name)
}

#[derive(Deserialize)]
pub(super) struct NewSearch {
    pub(super) name: String,
    pub(super) query: String,
}

pub(super) async fn add_search(State(s): State<Arc<AppState>>, headers: HeaderMap, Json(b): Json<NewSearch>) -> Result<Response, ApiError> {
    let u = require_user(&s, &headers).await?;
    let query = b.query.trim().trim_start_matches('?');
    if !valid_search_name(&b.name) || query.len() > 2000 || query.contains(char::is_whitespace) || !crate::error::storable(query) {
        return Err(Refusal::BadSavedSearch.into());
    }
    let id: Option<i64> = sqlx::query_scalar(
        "insert into saved_searches (user_id, name, query) select $1, $2, $3
         where (select count(*) from saved_searches where user_id = $1) < $4 returning id",
    )
    .bind(u.id)
    .bind(b.name.trim())
    .bind(query)
    .bind(MAX_SAVED_SEARCHES)
    .fetch_optional(&s.pool)
    .await?;
    match id {
        Some(id) => Ok(Json(json!({ "id": id, "rss": format!("{}/api/rss?{query}", s.site_url) })).into_response()),
        None => Err(Refusal::SavedSearchLimit.into()),
    }
}

pub(super) async fn remove_search(State(s): State<Arc<AppState>>, headers: HeaderMap, Path(id): Path<i64>) -> Result<Response, ApiError> {
    let u = require_user(&s, &headers).await?;
    sqlx::query("delete from saved_searches where id = $1 and user_id = $2").bind(id).bind(u.id).execute(&s.pool).await?;
    Ok(Json(json!({ "ok": true })).into_response())
}

#[derive(Deserialize)]
pub(super) struct SearchName {
    name: String,
}

pub(super) async fn rename_search(State(s): State<Arc<AppState>>, headers: HeaderMap, Path(id): Path<i64>, Json(b): Json<SearchName>) -> Result<Response, ApiError> {
    let u = require_user(&s, &headers).await?;
    if !valid_search_name(&b.name) {
        return Err(Refusal::BadSavedSearch.into());
    }
    sqlx::query("update saved_searches set name = $1 where id = $2 and user_id = $3").bind(b.name.trim()).bind(id).bind(u.id).execute(&s.pool).await?;
    Ok(Json(json!({ "ok": true })).into_response())
}

#[cfg(test)]
mod tests {
    use axum::http::{StatusCode, header};
    use sqlx::PgPool;

    use super::*;
    use crate::account::{test_state, token_hash};

    #[sqlx::test(migrations = "../core/migrations")]
    #[ignore = "requires PostgreSQL with pgvector"]
    async fn renaming_a_saved_search_needs_its_owner_and_a_valid_name(pool: PgPool) {
        let users: Vec<i64> = sqlx::query_scalar(
            "insert into users (username, email, display_name, password_hash)
             values ('a', 'a@example.test', 'A', 'x'), ('b', 'b@example.test', 'B', 'x') returning id",
        )
        .fetch_all(&pool)
        .await
        .unwrap();
        sqlx::query("insert into sessions (token_hash, user_id, expires_at) values ($1, $2, now() + interval '1 day')")
            .bind(token_hash("tok-a"))
            .bind(users[0])
            .execute(&pool)
            .await
            .unwrap();
        let ids: Vec<i64> = sqlx::query_scalar("insert into saved_searches (user_id, name, query) values ($1, 'mine', 'q=a'), ($2, 'theirs', 'q=b') returning id")
            .bind(users[0])
            .bind(users[1])
            .fetch_all(&pool)
            .await
            .unwrap();
        let state = test_state(pool.clone());
        let rename = |cookie: Option<&str>, id: i64, name: &str| {
            let mut h = HeaderMap::new();
            if let Some(c) = cookie {
                h.insert(header::COOKIE, c.parse().unwrap());
            }
            rename_search(State(state.clone()), h, Path(id), Json(SearchName { name: name.into() }))
        };
        let status = |r: Result<Response, ApiError>| r.into_response().status();
        let names = || sqlx::query_scalar::<_, String>("select name from saved_searches order by id").fetch_all(&pool);

        assert_eq!(status(rename(None, ids[0], "x").await), StatusCode::UNAUTHORIZED);
        assert_eq!(status(rename(Some("session=tok-a"), ids[0], "  ").await), StatusCode::BAD_REQUEST);
        assert_eq!(status(rename(Some("session=tok-a"), ids[0], &"n".repeat(101)).await), StatusCode::BAD_REQUEST);
        assert_eq!(status(rename(Some("session=tok-a"), ids[0], "  renamed ").await), StatusCode::OK);
        // another account's search is left as it was
        assert_eq!(status(rename(Some("session=tok-a"), ids[1], "stolen").await), StatusCode::OK);
        assert_eq!(names().await.unwrap(), ["renamed", "theirs"]);
    }
}
