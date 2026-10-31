//! Opening an item: counts a view (heat), and if our copy is stale refreshes it from Booth in the same
//! request that also yields the full gallery (listing crawls store at most 4 images). This is the
//! "fetch only when someone looks" path for cold items; hot ones are kept fresh by the worker.

use std::sync::Arc;
use std::time::Duration;

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use catalogue_core::booth::{mark_delisted, store_item};
use serde_json::{Value, json};
use tokio::sync::Semaphore;

use crate::{ApiError, AppState};

const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130 Safari/537.36";

pub struct Fetcher {
    client: reqwest::Client,
    permits: Semaphore,
}

impl Fetcher {
    pub fn new() -> anyhow::Result<Self> {
        rustls::crypto::ring::default_provider().install_default().ok();
        Ok(Self { client: reqwest::Client::builder().user_agent(UA).timeout(Duration::from_secs(15)).build()?, permits: Semaphore::new(4) })
    }

    /// English endpoint, so category names match the archive. `Ok(None)` = Booth says the item is gone.
    async fn fetch(&self, id: i64) -> anyhow::Result<Option<Value>> {
        let _p = self.permits.acquire().await?;
        let r = self.client.get(format!("https://booth.pm/en/items/{id}.json")).send().await.map_err(|e| anyhow::anyhow!("{e:?}"))?;
        if r.status() == StatusCode::NOT_FOUND {
            return Ok(None);
        }
        if !r.status().is_success() {
            anyhow::bail!("booth {} for item {id}", r.status());
        }
        Ok(Some(r.json().await?))
    }
}

pub async fn gallery(
    State(s): State<Arc<AppState>>,
    crate::limit::ClientIp(ip): crate::limit::ClientIp,
    Path(id): Path<i64>,
) -> Result<Response, ApiError> {
    if let Err(r) = s.limits.gallery.check(ip) {
        return Ok(r);
    }
    if s.snap().catalog.docs_for(&[id]).is_empty() {
        return Ok((StatusCode::NOT_FOUND, Json(json!({ "ok": false, "error": "Unknown item." }))).into_response());
    }
    sqlx::query(
        "insert into item_stats (item_id, views) values ($1, 1)
         on conflict (item_id) do update set views = item_stats.views + 1, last_viewed_at = now()",
    )
    .bind(id)
    .execute(&s.pool)
    .await?;

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
