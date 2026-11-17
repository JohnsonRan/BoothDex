//! The items AI enrichment gave up on (`boothdex_core::ENRICH_ATTEMPTS` failures), and the retry that sets their
//! failure count back to 0. A retry also queues an enrich run, so the items are tried again without waiting for its
//! schedule; when one is already queued or running, the next run picks them up.

use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, State};
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Response};
use boothdex_core::ENRICH_ATTEMPTS;
use serde_json::{Value, json};
use sqlx::PgPool;

use super::require_admin;
use crate::{ApiError, AppState};

/// Rows the list shows; the total says how many more there are, and "retry all" covers them too.
const SHOWN: i64 = 100;

/// The worker's queue (`ENRICH_ITEMS_SELECT` in crates/worker/src/jobs.rs) without its attempt limit, with
/// `ENRICH_ATTEMPTS` bound as `$1`; the overview's count (`data_json` in admin.rs) reads it too.
pub(super) const GIVEN_UP: &str = "needs_enrichment and status = 'ok' and enrich_failures >= $1";

pub(super) async fn list(State(s): State<Arc<AppState>>, headers: HeaderMap) -> Result<Response, ApiError> {
    require_admin(&s, &headers).await?;
    let body: Value = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
        "select jsonb_build_object(
           'attempts', $1::int,
           'total', (select count(*) from items where {GIVEN_UP}),
           'items', (select coalesce(jsonb_agg(jsonb_build_object('id', id, 'title', title, 'author_name', author_name,
                                                                  'failures', enrich_failures) order by id desc), '[]')
                     from (select id, title, author_name, enrich_failures from items where {GIVEN_UP} order by id desc limit $2) i))"
    )))
    .bind(ENRICH_ATTEMPTS)
    .bind(SHOWN)
    .fetch_one(&s.pool)
    .await?;
    Ok(Json(body).into_response())
}

pub(super) async fn retry(State(s): State<Arc<AppState>>, headers: HeaderMap, Path(id): Path<i64>) -> Result<Response, ApiError> {
    let who = require_admin(&s, &headers).await?;
    let reset = reset(&s.pool, &who, Some(id)).await?;
    // 0 when the item was retried already or is no longer given up: the list is out of date, not the request wrong
    println!("admin {who} retried AI enrichment of item {id} ({reset} reset)");
    Ok(Json(json!({ "reset": reset })).into_response())
}

pub(super) async fn retry_all(State(s): State<Arc<AppState>>, headers: HeaderMap) -> Result<Response, ApiError> {
    let who = require_admin(&s, &headers).await?;
    let reset = reset(&s.pool, &who, None).await?;
    println!("admin {who} retried AI enrichment of {reset} items");
    Ok(Json(json!({ "reset": reset })).into_response())
}

/// Sets the failure count of the given-up items (one, or all) back to 0 and queues an enrich run when any was reset.
async fn reset(pool: &PgPool, who: &str, id: Option<i64>) -> Result<u64, sqlx::Error> {
    let mut tx = pool.begin().await?;
    let reset = sqlx::query(sqlx::AssertSqlSafe(format!(
        "update items set enrich_failures = 0 where {GIVEN_UP} and ($2::bigint is null or id = $2)"
    )))
    .bind(ENRICH_ATTEMPTS)
    .bind(id)
    .execute(&mut *tx)
    .await?
    .rows_affected();
    if reset > 0 {
        sqlx::query("insert into jobs (kind, params, created_by) values ('enrich', '{}', $1) on conflict do nothing")
            .bind(format!("admin:{who}"))
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    Ok(reset)
}

#[cfg(test)]
mod tests {
    use axum::extract::{Path, State};
    use axum::http::header;
    use serde_json::json;

    use super::*;
    use crate::account;

    async fn body(r: Result<Response, ApiError>) -> Value {
        let r = r.ok().expect("the admin's request succeeds");
        serde_json::from_slice(&axum::body::to_bytes(r.into_body(), 1 << 20).await.unwrap()).unwrap()
    }

    /// Items 1 and 2 are given up, 3 has failed once, 4 is enriched, 5 is delisted after three failures.
    async fn setup(pool: &PgPool) -> (Arc<AppState>, HeaderMap) {
        sqlx::raw_sql(
            "insert into items (id, title, author_name, author_uri, is_adult, status, scraped_at, needs_enrichment, enrich_failures) values
               (1, 'given up', 'shop', 'https://example.test', false, 'ok', now(), true, 3),
               (2, 'given up twice over', 'shop', 'https://example.test', false, 'ok', now(), true, 4),
               (3, 'failed once', 'shop', 'https://example.test', false, 'ok', now(), true, 1),
               (4, 'enriched', 'shop', 'https://example.test', false, 'ok', now(), false, 3),
               (5, 'delisted', 'shop', 'https://example.test', false, 'delisted', now(), true, 3);
             insert into users (id, username, email, display_name, password_hash, is_admin) values (1, 'boss', 'boss@example.test', 'Boss', 'x', true);",
        )
        .execute(pool)
        .await
        .unwrap();
        sqlx::query("insert into sessions (token_hash, user_id, expires_at) values ($1, 1, now() + interval '1 day')")
            .bind(account::token_hash("tok-boss"))
            .execute(pool)
            .await
            .unwrap();
        let mut h = HeaderMap::new();
        h.insert(header::COOKIE, "session=tok-boss".parse().unwrap());
        (account::test_state(pool.clone()), h)
    }

    async fn failures(pool: &PgPool) -> Vec<(i64, i16)> {
        sqlx::query_as("select id, enrich_failures from items order by id").fetch_all(pool).await.unwrap()
    }

    async fn enrich_jobs(pool: &PgPool) -> Vec<String> {
        sqlx::query_scalar("select created_by from jobs where kind = 'enrich'").fetch_all(pool).await.unwrap()
    }

    #[sqlx::test(migrations = "../core/migrations")]
    #[ignore = "requires PostgreSQL with pgvector"]
    async fn the_list_holds_only_listed_items_enrichment_gave_up_on(pool: PgPool) {
        let (s, h) = setup(&pool).await;
        let listed = body(list(State(s.clone()), h.clone()).await).await;
        assert_eq!(
            listed,
            json!({ "attempts": 3, "total": 2, "items": [
                { "id": 2, "title": "given up twice over", "author_name": "shop", "failures": 4 },
                { "id": 1, "title": "given up", "author_name": "shop", "failures": 3 },
            ] })
        );
        sqlx::query("update items set enrich_failures = 0").execute(&pool).await.unwrap();
        assert_eq!(body(list(State(s), h).await).await, json!({ "attempts": 3, "total": 0, "items": [] }));
    }

    #[sqlx::test(migrations = "../core/migrations")]
    #[ignore = "requires PostgreSQL with pgvector"]
    async fn a_retry_resets_only_given_up_items_and_queues_one_run(pool: PgPool) {
        let (s, h) = setup(&pool).await;
        // an item that is not given up is left alone, and nothing is queued for it
        for id in [3, 4, 5, 99] {
            assert_eq!(body(retry(State(s.clone()), h.clone(), Path(id)).await).await, json!({ "reset": 0 }), "item {id}");
        }
        assert!(enrich_jobs(&pool).await.is_empty());

        assert_eq!(body(retry(State(s.clone()), h.clone(), Path(1)).await).await, json!({ "reset": 1 }));
        assert_eq!(failures(&pool).await, [(1, 0), (2, 4), (3, 1), (4, 3), (5, 3)]);
        assert_eq!(enrich_jobs(&pool).await, ["admin:boss"]);

        // the run queued above is still waiting, so retrying the rest adds no second one
        assert_eq!(body(retry_all(State(s.clone()), h.clone()).await).await, json!({ "reset": 1 }));
        assert_eq!(failures(&pool).await, [(1, 0), (2, 0), (3, 1), (4, 3), (5, 3)]);
        assert_eq!(enrich_jobs(&pool).await, ["admin:boss"]);
        assert_eq!(body(retry_all(State(s), h).await).await, json!({ "reset": 0 }));
    }

    #[sqlx::test(migrations = "../core/migrations")]
    #[ignore = "requires PostgreSQL with pgvector"]
    async fn members_cannot_list_or_retry(pool: PgPool) {
        let (s, _) = setup(&pool).await;
        sqlx::query("update users set is_admin = false").execute(&pool).await.unwrap();
        let mut h = HeaderMap::new();
        h.insert(header::COOKIE, "session=tok-boss".parse().unwrap());
        let status = |r: Result<Response, ApiError>| r.into_response().status().as_u16();
        assert_eq!(status(list(State(s.clone()), h.clone()).await), 403);
        assert_eq!(status(retry(State(s.clone()), h.clone(), Path(1)).await), 403);
        assert_eq!(status(retry_all(State(s.clone()), h).await), 403);
        assert_eq!(status(retry_all(State(s), HeaderMap::new()).await), 401);
        assert_eq!(failures(&pool).await, [(1, 3), (2, 4), (3, 1), (4, 3), (5, 3)]);
        assert!(enrich_jobs(&pool).await.is_empty());
    }
}
