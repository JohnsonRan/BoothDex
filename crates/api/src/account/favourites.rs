use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::extract::{Path, State};
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;

use super::require_user;
use crate::error::Refusal;
use crate::{ApiError, AppState};

/// Each account's favourite item ids, newest first, so paging through the favourites view does not read them every
/// time. A change made through this API drops them at once; one made through another API instance shows after a
/// minute.
pub struct FavouriteCache {
    ids: moka::sync::Cache<i64, Arc<[i64]>>,
    /// changes so far: ids read while one came in are not kept, they may predate it
    writes: Mutex<u64>,
}

impl FavouriteCache {
    pub fn new() -> Self {
        Self { ids: moka::sync::Cache::builder().max_capacity(10_000).time_to_live(Duration::from_secs(60)).build(), writes: Mutex::new(0) }
    }

    fn writes(&self) -> u64 {
        *self.writes.lock().unwrap()
    }

    /// After a change to the account's favourites.
    fn changed(&self, user_id: i64) {
        let mut w = self.writes.lock().unwrap();
        *w += 1;
        self.ids.invalidate(&user_id);
    }

    /// Keeps `ids`, read when `writes()` was `writes`, unless a change came in since.
    fn keep(&self, user_id: i64, ids: Arc<[i64]>, writes: u64) {
        let w = self.writes.lock().unwrap();
        if *w == writes {
            self.ids.insert(user_id, ids);
        }
    }
}

pub async fn favourite_ids(s: &AppState, user_id: i64) -> Result<Arc<[i64]>, ApiError> {
    if let Some(ids) = s.favourites.ids.get(&user_id) {
        return Ok(ids);
    }
    let writes = s.favourites.writes();
    let ids: Arc<[i64]> = sqlx::query_scalar::<_, i64>("select item_id from favourites where user_id = $1 order by created_at desc")
        .bind(user_id)
        .fetch_all(&s.pool)
        .await?
        .into();
    s.favourites.keep(user_id, ids.clone(), writes);
    Ok(ids)
}

pub(super) async fn list_favourites(State(s): State<Arc<AppState>>, headers: HeaderMap) -> Result<Response, ApiError> {
    let u = require_user(&s, &headers).await?;
    Ok(Json(json!({ "ids": &*favourite_ids(&s, u.id).await? })).into_response())
}

pub(super) async fn add_favourite(State(s): State<Arc<AppState>>, headers: HeaderMap, Path(item): Path<i64>) -> Result<Response, ApiError> {
    let u = require_user(&s, &headers).await?;
    match sqlx::query("insert into favourites (user_id, item_id) values ($1, $2) on conflict do nothing").bind(u.id).bind(item).execute(&s.pool).await {
        Ok(_) => {
            s.favourites.changed(u.id);
            Ok(Json(json!({ "ok": true })).into_response())
        }
        Err(e) if e.as_database_error().and_then(|d| d.code()).as_deref() == Some("23503") => Err(Refusal::UnknownItem.into()),
        Err(e) => Err(e.into()),
    }
}

pub(super) async fn remove_favourite(State(s): State<Arc<AppState>>, headers: HeaderMap, Path(item): Path<i64>) -> Result<Response, ApiError> {
    let u = require_user(&s, &headers).await?;
    sqlx::query("delete from favourites where user_id = $1 and item_id = $2").bind(u.id).bind(item).execute(&s.pool).await?;
    s.favourites.changed(u.id);
    Ok(Json(json!({ "ok": true })).into_response())
}

#[cfg(test)]
mod tests {
    use sqlx::PgPool;

    use super::*;
    use crate::account::test_state;
    use crate::account::tests::{headers, ids, items, signed_in};

    #[test]
    fn favourites_read_before_a_change_are_not_cached() {
        let c = FavouriteCache::new();
        let read_at = c.writes();
        c.changed(1);
        c.keep(1, Arc::from([5]), read_at);
        assert!(c.ids.get(&1).is_none());
        c.keep(1, Arc::from([5]), c.writes());
        assert_eq!(c.ids.get(&1).as_deref(), Some(&[5][..]));
        c.changed(1);
        assert!(c.ids.get(&1).is_none());
    }

    #[sqlx::test(migrations = "../core/migrations")]
    #[ignore = "requires PostgreSQL with pgvector"]
    async fn favourites_list_follows_changes_at_once(pool: PgPool) {
        let user = signed_in(&pool).await;
        items(&pool, 3).await;
        let state = test_state(pool.clone());
        let list = || async { ids(list_favourites(State(state.clone()), headers()).await.ok().unwrap()).await };

        add_favourite(State(state.clone()), headers(), Path(1)).await.ok().unwrap();
        assert_eq!(list().await, [1]);
        add_favourite(State(state.clone()), headers(), Path(2)).await.ok().unwrap();
        assert_eq!(list().await, [2, 1]);
        remove_favourite(State(state.clone()), headers(), Path(1)).await.ok().unwrap();
        assert_eq!(list().await, [2]);
        // a change that does not pass through this API is not seen until the cache expires
        sqlx::query("insert into favourites (user_id, item_id) values ($1, 3)").bind(user).execute(&pool).await.unwrap();
        assert_eq!(list().await, [2]);
    }

    #[sqlx::test(migrations = "../core/migrations")]
    #[ignore = "requires PostgreSQL with pgvector"]
    async fn item_stats_count_favourites_through_adds_removes_and_deleted_accounts(pool: PgPool) {
        let users: Vec<i64> = sqlx::query_scalar(
            "insert into users (username, email, display_name, password_hash)
             values ('a', 'a@example.test', 'A', 'x'), ('b', 'b@example.test', 'B', 'x') returning id",
        )
        .fetch_all(&pool)
        .await
        .unwrap();
        items(&pool, 2).await;
        let fav = |user: i64, item: i64| sqlx::query("insert into favourites (user_id, item_id) values ($1, $2) on conflict do nothing").bind(user).bind(item).execute(&pool);
        fav(users[0], 1).await.unwrap();
        fav(users[1], 1).await.unwrap();
        fav(users[0], 2).await.unwrap();
        // already a favourite: nothing inserted, nothing counted
        fav(users[0], 1).await.unwrap();
        sqlx::query("delete from favourites where user_id = $1 and item_id = 2").bind(users[0]).execute(&pool).await.unwrap();
        // the account's favourites go with it
        sqlx::query("delete from users where id = $1").bind(users[1]).execute(&pool).await.unwrap();

        let counts: Vec<(i64, i64)> = sqlx::query_as("select item_id, favourites from item_stats order by item_id").fetch_all(&pool).await.unwrap();
        assert_eq!(counts, [(1, 1), (2, 0)]);
        let ranked: i64 = sqlx::query_scalar("select count(*) from refresh_priority").fetch_one(&pool).await.unwrap();
        assert_eq!(ranked, 2);
    }
}
