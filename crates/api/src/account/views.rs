use std::collections::HashMap;
use std::sync::Arc;

use axum::extract::State;
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Deserialize;
use serde_json::json;
use sqlx::PgPool;

use super::require_user;
use crate::{ApiError, AppState};

/// viewed items kept per user (the web keeps the same number in a signed-out browser)
const MAX_VIEWS: i64 = 500;
/// One item open in this many also trims the account to `MAX_VIEWS`, as every import does. In between an account can
/// hold a few more, which `list_views` never returns.
const TRIM_VIEWS_ONE_IN: u32 = 20;

pub(super) async fn list_views(State(s): State<Arc<AppState>>, headers: HeaderMap) -> Result<Response, ApiError> {
    let u = require_user(&s, &headers).await?;
    let ids: Vec<i64> = sqlx::query_scalar("select item_id from item_views where user_id = $1 order by viewed_at desc limit $2")
        .bind(u.id)
        .bind(MAX_VIEWS)
        .fetch_all(&s.pool)
        .await?;
    Ok(Json(json!({ "ids": ids })).into_response())
}

#[derive(Deserialize)]
struct ViewEntry {
    id: i64,
    /// view timestamp in epoch milliseconds (optional, missing defaults to server now())
    #[serde(default)]
    at: Option<i64>,
}

#[derive(Deserialize)]
pub(super) struct Views {
    /// newest first: one item just opened, or a signed-out browser's history merged in at sign-in
    views: Vec<ViewEntry>,
}

fn dedupe_views(entries: Vec<ViewEntry>) -> (Vec<i64>, Vec<Option<f64>>) {
    let mut seen = HashMap::new();
    let mut deduped = Vec::new();
    for entry in entries {
        let at = entry.at.filter(|&ms| ms > 0 && ms < 4_102_444_800_000);
        if let Some(&idx) = seen.get(&entry.id) {
            let (_, existing_at): &mut (i64, Option<i64>) = &mut deduped[idx];
            *existing_at = match (*existing_at, at) {
                (None, _) | (_, None) => None,
                (Some(a), Some(b)) => Some(a.max(b)),
            };
        } else {
            seen.insert(entry.id, deduped.len());
            deduped.push((entry.id, at));
        }
    }
    deduped.into_iter().map(|(id, at)| (id, at.map(|ms| ms as f64 / 1000.0))).unzip()
}

pub(super) async fn add_views(State(s): State<Arc<AppState>>, headers: HeaderMap, Json(b): Json<Views>) -> Result<Response, ApiError> {
    let u = require_user(&s, &headers).await?;
    let entries: Vec<ViewEntry> = b.views.into_iter().take(MAX_VIEWS as usize).collect();
    if entries.is_empty() {
        return Ok(Json(json!({ "ok": true })).into_response());
    }
    let (ids, ats) = dedupe_views(entries);

    // no transaction or lock, so an item open is one statement; concurrent writes can at worst leave a few rows
    // over the cap until the next trim
    // missing at defaults to now(); future timestamps are clamped to server now()
    sqlx::query(
        "insert into item_views (user_id, item_id, viewed_at)
         select $1, v.id, least(coalesce(to_timestamp(v.at_sec), now()), now())
         from unnest($2::bigint[], $3::double precision[]) as v(id, at_sec) join items i on i.id = v.id
         on conflict (user_id, item_id) do update set viewed_at = greatest(item_views.viewed_at, excluded.viewed_at)",
    )
    .bind(u.id)
    .bind(&ids)
    .bind(&ats)
    .execute(&s.pool)
    .await?;
    if ids.len() > 1 || getrandom::u32().is_ok_and(|r| r % TRIM_VIEWS_ONE_IN == 0) {
        trim_views(&s.pool, u.id).await?;
    }
    Ok(Json(json!({ "ok": true })).into_response())
}

/// Deletes the views older than the account's newest `MAX_VIEWS`. Compared by time, not by id: a view a concurrent
/// open moves up is checked again with its new time and kept, so the cap can only be exceeded, never undercut.
async fn trim_views(pool: &PgPool, user_id: i64) -> Result<(), sqlx::Error> {
    sqlx::query(
        "delete from item_views where user_id = $1 and viewed_at <
         (select viewed_at from item_views where user_id = $1 order by viewed_at desc offset $2 limit 1)",
    )
    .bind(user_id)
    .bind(MAX_VIEWS - 1)
    .execute(pool)
    .await?;
    Ok(())
}

pub(super) async fn clear_views(State(s): State<Arc<AppState>>, headers: HeaderMap) -> Result<Response, ApiError> {
    let u = require_user(&s, &headers).await?;
    sqlx::query("delete from item_views where user_id = $1").bind(u.id).execute(&s.pool).await?;
    Ok(Json(json!({ "ok": true })).into_response())
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::account::test_state;
    use crate::account::tests::{headers, ids, items, signed_in};

    #[test]
    fn views_deduplication_and_timestamps() {
        let json = r#"{"views": [{"id": 1, "at": 1000}, {"id": 2}, {"id": 1, "at": 2000}, {"id": 1, "at": 500}]}"#;
        let b: Views = serde_json::from_str(json).unwrap();
        let (ids, ats) = dedupe_views(b.views);
        assert_eq!(ids, vec![1, 2]);
        assert_eq!(ats, vec![Some(2.0), None]);
    }

    #[sqlx::test(migrations = "../core/migrations")]
    #[ignore = "requires PostgreSQL with pgvector"]
    async fn imports_trim_viewed_items_and_the_list_keeps_the_newest(pool: PgPool) {
        let user = signed_in(&pool).await;
        items(&pool, 501).await;
        let state = test_state(pool.clone());
        // newest first, item n viewed at n ms past a fixed time
        let add = |ids: Vec<i64>| {
            let views = ids.into_iter().map(|id| ViewEntry { id, at: Some(1_700_000_000_000 + id) }).collect();
            add_views(State(state.clone()), headers(), Json(Views { views }))
        };
        let count = || sqlx::query_scalar::<_, i64>("select count(*) from item_views where user_id = $1").bind(user).fetch_one(&pool);

        add((1..=300).rev().collect()).await.ok().unwrap();
        add((301..=501).rev().collect()).await.ok().unwrap();
        assert_eq!(count().await.unwrap(), 500, "an import trims to the newest MAX_VIEWS");

        // a single open is one upsert; the account may stay a row over until the next trim
        add_views(State(state.clone()), headers(), Json(Views { views: vec![ViewEntry { id: 1, at: None }] })).await.ok().unwrap();
        assert!((500..=501).contains(&count().await.unwrap()));
        let listed = ids(list_views(State(state.clone()), headers()).await.ok().unwrap()).await;
        assert_eq!(listed.len(), 500);
        assert_eq!(listed[..2], [1, 501]);
    }

    #[sqlx::test(migrations = "../core/migrations")]
    #[ignore = "requires PostgreSQL with pgvector"]
    async fn a_view_moved_up_while_trimming_is_kept(pool: PgPool) {
        let user = signed_in(&pool).await;
        items(&pool, 501).await;
        // item n viewed n seconds past a fixed time: item 1 is the one past the cap
        sqlx::query("insert into item_views (user_id, item_id, viewed_at) select $1, g, to_timestamp(1700000000 + g) from generate_series(1, 501) g")
            .bind(user)
        .execute(&pool)
        .await
        .unwrap();

        // another device opens item 1 again, on its own connection; its write is not committed yet when the trim starts
        let mut conn = <sqlx::PgConnection as sqlx::Connection>::connect_with(&pool.connect_options()).await.unwrap();
        let mut open = sqlx::Connection::begin(&mut conn).await.unwrap();
        sqlx::query("update item_views set viewed_at = now() where user_id = $1 and item_id = 1").bind(user).execute(&mut *open).await.unwrap();
        let trim = tokio::spawn({
            let pool = pool.clone();
            async move { trim_views(&pool, user).await }
        });
        // the trim waits for item 1's row
        for _ in 0..250 {
            if sqlx::query_scalar("select exists (select 1 from pg_locks where not granted)").fetch_one(&mut *open).await.unwrap() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        assert!(!trim.is_finished(), "the trim must have waited for the open");
        open.commit().await.unwrap();
        trim.await.unwrap().unwrap();

        let newest: i64 = sqlx::query_scalar("select item_id from item_views where user_id = $1 order by viewed_at desc limit 1")
            .bind(user)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(newest, 1, "the item just opened again must not be trimmed");
        let count: i64 = sqlx::query_scalar("select count(*) from item_views where user_id = $1").bind(user).fetch_one(&pool).await.unwrap();
        assert_eq!(count, 501, "the cap is only ever exceeded until the next trim");
    }
}
