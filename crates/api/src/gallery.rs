//! Full image gallery for one item, fetched from Booth on first request.
//! Listing crawls store at most 4 images per item; opening an item calls this to get the rest.
//! Booth is only hit for items in our catalog that lack a complete gallery, with a small concurrency cap.

use std::sync::Arc;
use std::time::Duration;

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;
use sqlx::PgPool;
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

    /// `Ok(None)` = Booth says the item is gone.
    async fn fetch(&self, id: i64) -> anyhow::Result<Option<Vec<String>>> {
        let _p = self.permits.acquire().await?;
        let r = self.client.get(format!("https://booth.pm/ja/items/{id}.json")).send().await.map_err(|e| anyhow::anyhow!("{e:?}"))?;
        if r.status() == StatusCode::NOT_FOUND {
            return Ok(None);
        }
        if !r.status().is_success() {
            anyhow::bail!("booth {} for item {id}", r.status());
        }
        let v: serde_json::Value = r.json().await?;
        Ok(Some(v["images"].as_array().into_iter().flatten().filter_map(|i| i["original"].as_str().map(str::to_owned)).collect()))
    }
}

async fn stored(pool: &PgPool, id: i64) -> Result<Option<(Vec<String>, bool)>, sqlx::Error> {
    sqlx::query_as("select images, complete from booth_images where item_id = $1").bind(id).fetch_optional(pool).await
}

pub async fn gallery(State(s): State<Arc<AppState>>, Path(id): Path<i64>) -> Result<Response, ApiError> {
    if s.snap().catalog.docs_for(&[id]).is_empty() {
        return Ok((StatusCode::NOT_FOUND, Json(json!({ "ok": false, "error": "Unknown item." }))).into_response());
    }
    let have = stored(&s.pool, id).await?;
    if let Some((images, true)) = &have {
        return Ok(Json(json!({ "images": images, "complete": true })).into_response());
    }
    match s.gallery.fetch(id).await {
        Ok(Some(images)) => {
            sqlx::query(
                "insert into booth_images (item_id, images, complete) values ($1, $2, true)
                 on conflict (item_id) do update set images = excluded.images, complete = true, fetched_at = now()",
            )
            .bind(id)
            .bind(&images)
            .execute(&s.pool)
            .await?;
            Ok(Json(json!({ "images": images, "complete": true })).into_response())
        }
        // gone from Booth, or Booth unreachable: serve what we have; the client falls back to archived media
        Ok(None) => Ok(Json(json!({ "images": have.map(|h| h.0).unwrap_or_default(), "complete": false, "gone": true })).into_response()),
        Err(e) => {
            eprintln!("gallery {id}: {e:#}");
            Ok(Json(json!({ "images": have.map(|h| h.0).unwrap_or_default(), "complete": false })).into_response())
        }
    }
}
