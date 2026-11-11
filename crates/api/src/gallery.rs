//! Opening an item: counts a view (heat), and if our copy is stale refreshes it from Booth in the same
//! request that also yields the full gallery (listing crawls store at most 4 images). This is the
//! "fetch only when someone looks" path for cold items; hot ones are kept fresh by the worker.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use boothdex_core::booth::{mark_delisted, store_item};
use boothdex_core::booth_client::BoothClient;
use serde_json::{Value, json};
use sqlx::PgPool;
use tokio::sync::Semaphore;

use crate::{ApiError, AppState};

const FLUSH_EVERY: Duration = Duration::from_secs(10);

/// Item opens counted in memory and written together every `FLUSH_EVERY`: a popular item is one row update
/// per interval instead of one per visitor.
#[derive(Default)]
pub struct ViewCounts(Mutex<HashMap<i64, i64>>);

impl ViewCounts {
    fn add(&self, id: i64) {
        *self.0.lock().unwrap().entry(id).or_default() += 1;
    }

    /// Writes the counts so far in one statement; if that fails they are kept for the next try.
    pub async fn flush(&self, pool: &PgPool) -> Result<(), sqlx::Error> {
        let mut counts: Vec<(i64, i64)> = std::mem::take(&mut *self.0.lock().unwrap()).into_iter().collect();
        if counts.is_empty() {
            return Ok(());
        }
        // rows are locked in id order, so API instances flushing at the same time cannot deadlock
        counts.sort_unstable();
        let (ids, views): (Vec<i64>, Vec<i64>) = counts.iter().copied().unzip();
        let r = sqlx::query(
            "insert into item_stats (item_id, views)
             select v.id, v.n from unnest($1::bigint[], $2::bigint[]) as v(id, n) join items i on i.id = v.id order by v.id
             on conflict (item_id) do update set views = item_stats.views + excluded.views, last_viewed_at = now()",
        )
        .bind(&ids)
        .bind(&views)
        .execute(pool)
        .await;
        if r.is_err() {
            let mut map = self.0.lock().unwrap();
            for (id, n) in counts {
                *map.entry(id).or_default() += n;
            }
        }
        r.map(|_| ())
    }
}

/// Writes the counts every `FLUSH_EVERY`; `main` writes the rest when the API stops.
pub async fn flush_views(s: Arc<AppState>) {
    loop {
        tokio::time::sleep(FLUSH_EVERY).await;
        if let Err(e) = s.view_counts.flush(&s.pool).await {
            eprintln!("view counts: {e}");
        }
    }
}

pub struct Fetcher {
    booth: BoothClient,
    permits: Semaphore,
}

impl Fetcher {
    pub fn new() -> anyhow::Result<Self> {
        // a visitor is waiting: one try, and stored data is served if Booth is busy
        Ok(Self { booth: BoothClient::new(1, Duration::from_secs(15))?, permits: Semaphore::new(4) })
    }

    /// `Ok(None)` = Booth says the item is gone.
    async fn fetch(&self, id: i64) -> anyhow::Result<Option<Value>> {
        let _p = self.permits.acquire().await?;
        self.booth.item(id).await
    }
}

pub async fn gallery(
    State(s): State<Arc<AppState>>,
    crate::limit::ClientIp(ip): crate::limit::ClientIp,
    Path(id): Path<i64>,
) -> Result<Response, ApiError> {
    if let Err(r) = s.limits.gallery.check(ip) {
        return Ok(r.into_response());
    }
    if s.snap().catalog.docs_for(&[id]).is_empty() {
        return Ok((StatusCode::NOT_FOUND, Json(json!({ "ok": false, "error": "Unknown item." }))).into_response());
    }
    s.view_counts.add(id);

    let (images, complete, fresh, listed): (Option<Vec<String>>, Option<bool>, bool, bool) = // a copy younger than 3 days is served as is
    sqlx::query_as(
        "select b.images, b.complete, i.scraped_at > now() - interval '3 days', i.status = 'ok'
         from items i left join booth_images b on b.item_id = i.id where i.id = $1",
    )
    .bind(id)
    .fetch_one(&s.pool)
    .await?;
    let stored = images.clone().unwrap_or_default();
    // delisted items keep their archived images; nothing to fetch
    if !listed || (complete == Some(true) && fresh) {
        return Ok(Json(json!({ "images": stored, "complete": complete == Some(true), "refreshed": false })).into_response());
    }
    match s.gallery.fetch(id).await {
        Ok(Some(v)) => {
            store_item(&s.pool, &v).await?;
            let images: Vec<String> = sqlx::query_scalar("select images from booth_images where item_id = $1").bind(id).fetch_one(&s.pool).await?;
            Ok(Json(json!({ "images": images, "complete": true, "refreshed": true })).into_response())
        }
        Ok(None) => {
            mark_delisted(&s.pool, id).await?;
            Ok(Json(json!({ "images": stored, "complete": false, "gone": true })).into_response())
        }
        // Booth unreachable: serve what we have
        Err(e) => {
            eprintln!("gallery {id}: {e:#}");
            Ok(Json(json!({ "images": stored, "complete": complete == Some(true), "refreshed": false })).into_response())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[sqlx::test(migrations = "../core/migrations")]
    #[ignore = "requires PostgreSQL with pgvector"]
    async fn view_counts_add_up_across_flushes_and_skip_unknown_items(pool: PgPool) {
        sqlx::query("insert into items (id, title, author_name, author_uri, is_adult, status, scraped_at) values (1, 't', 'c', 'https://example.test', false, 'ok', now())")
            .execute(&pool)
            .await
            .unwrap();
        let counts = ViewCounts::default();
        for id in [1, 1, 1, 999] {
            counts.add(id);
        }
        counts.flush(&pool).await.unwrap();
        counts.add(1);
        counts.add(1);
        counts.flush(&pool).await.unwrap();
        counts.flush(&pool).await.unwrap();
        let views: i64 = sqlx::query_scalar("select views from item_stats where item_id = 1").fetch_one(&pool).await.unwrap();
        assert_eq!(views, 5);
        assert!(counts.0.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn view_counts_survive_a_failed_flush() {
        let pool = sqlx::postgres::PgPoolOptions::new().acquire_timeout(Duration::from_secs(1)).connect_lazy("postgres://127.0.0.1:1/none").unwrap();
        let counts = ViewCounts::default();
        counts.add(1);
        counts.add(1);
        assert!(counts.flush(&pool).await.is_err());
        assert_eq!(counts.0.lock().unwrap()[&1], 2);
    }
}
