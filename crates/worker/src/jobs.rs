//! Job implementations. Each returns a one-line summary; progress/cancel go through `Ctx`.

use std::time::{Duration, Instant};

use anyhow::{Result, bail};
use serde_json::Value;
use sqlx::PgPool;

use crate::booth::Booth;

pub struct Ctx<'a> {
    pub pool: &'a PgPool,
    pub booth: &'a Booth,
    pub id: i64,
    last_write: Option<Instant>,
}

impl<'a> Ctx<'a> {
    pub fn new(pool: &'a PgPool, booth: &'a Booth, id: i64) -> Self {
        Self { pool, booth, id, last_write: None }
    }

    /// Records progress (throttled to one write per 2 s unless `force`); returns false once cancel was requested.
    pub async fn progress(&mut self, done: i64, total: Option<i64>, message: &str, force: bool) -> Result<bool> {
        if !force && self.last_write.is_some_and(|t| t.elapsed() < Duration::from_secs(2)) {
            return Ok(true);
        }
        self.last_write = Some(Instant::now());
        let status: String = sqlx::query_scalar("update jobs set done = $2, total = $3, message = $4 where id = $1 returning status")
            .bind(self.id)
            .bind(done)
            .bind(total)
            .bind(message)
            .fetch_one(self.pool)
            .await?;
        Ok(status != "cancelling")
    }

    pub async fn log(&self, line: &str) -> Result<()> {
        let stamped = format!("{} {line}\n", chrono_now());
        // keep only the newest ~16 KB
        sqlx::query("update jobs set log = right(log || $2, 16000) where id = $1").bind(self.id).bind(stamped).execute(self.pool).await?;
        Ok(())
    }
}

fn chrono_now() -> String {
    let s = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    format!("[{:02}:{:02}:{:02}Z]", (s / 3600) % 24, (s / 60) % 60, s % 60)
}

fn param<T: serde::de::DeserializeOwned>(p: &Value, key: &str, default: T) -> T {
    p.get(key).and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or(default)
}

pub async fn bump_catalog(pool: &PgPool) -> Result<()> {
    sqlx::query("update catalog_meta set version = version + 1, changed_at = now()").execute(pool).await?;
    Ok(())
}

use catalogue_core::booth::{mark_delisted, store_item};

/// Newest-first listing walk: stores image links for every item seen, ingests unknown items, stops once caught up.
pub async fn incremental(ctx: &mut Ctx<'_>, params: &Value) -> Result<String> {
    let max_pages: u32 = param(params, "max_pages", 40);
    let tag: String = param(params, "tag", "VRChat".to_owned());
    let delay = Duration::from_millis(param(params, "delay_ms", 700));
    let (mut new_items, mut revived, mut seen) = (0i64, 0u64, 0i64);
    let mut caught_up = false;
    for page in 1..=max_pages {
        let (rows, last) = ctx.booth.listing(&tag, page).await?;
        seen += rows.len() as i64;
        let ids: Vec<i64> = rows.iter().map(|r| r.0).collect();
        let imgs: Vec<String> = rows.iter().map(|r| serde_json::to_string(&r.1).unwrap()).collect();
        sqlx::query(
            "insert into booth_images (item_id, images, complete)
             select id, array(select jsonb_array_elements_text(imgs::jsonb)), false from unnest($1::bigint[], $2::text[]) as t(id, imgs)
             on conflict (item_id) do update set images = excluded.images, fetched_at = now() where not booth_images.complete",
        )
        .bind(&ids)
        .bind(&imgs)
        .execute(ctx.pool)
        .await?;
        revived += sqlx::query("update items set status = 'ok' where id = any($1) and status = 'delisted'").bind(&ids).execute(ctx.pool).await?.rows_affected();
        let unknown: Vec<i64> = sqlx::query_scalar("select id from unnest($1::bigint[]) as t(id) where not exists (select 1 from items i where i.id = t.id)")
            .bind(&ids)
            .fetch_all(ctx.pool)
            .await?;
        for id in &unknown {
            tokio::time::sleep(delay).await;
            if let Some(v) = ctx.booth.item(*id).await? {
                store_item(ctx.pool, &v).await?;
                new_items += 1;
            }
        }
        if !ctx.progress(page as i64, Some(max_pages.min(last) as i64), &format!("page {page}: {seen} seen, {new_items} new"), false).await? {
            ctx.log("cancelled").await?;
            break;
        }
        // the first pages always get a look (new items can land slightly out of order); after that stop at a fully known page
        if page >= 3 && unknown.is_empty() {
            caught_up = true;
            break;
        }
        if page >= last {
            break;
        }
        tokio::time::sleep(delay).await;
    }
    if new_items > 0 || revived > 0 {
        bump_catalog(ctx.pool).await?;
    }
    Ok(format!("{new_items} new, {revived} relisted, {seen} seen{}", if caught_up { ", caught up" } else { "" }))
}

async fn fetch_each(ctx: &mut Ctx<'_>, ids: Vec<i64>, delay: Duration, label: &str) -> Result<String> {
    let total = ids.len() as i64;
    let (mut ok, mut gone) = (0i64, 0i64);
    for (n, id) in ids.iter().enumerate() {
        match ctx.booth.item(*id).await? {
            Some(v) => {
                store_item(ctx.pool, &v).await?;
                ok += 1;
            }
            None => {
                mark_delisted(ctx.pool, *id).await?;
                gone += 1;
            }
        }
        if !ctx.progress(n as i64 + 1, Some(total), &format!("{label}: {ok} updated, {gone} delisted"), false).await? {
            ctx.log("cancelled").await?;
            break;
        }
        tokio::time::sleep(delay).await;
    }
    if ok + gone > 0 {
        bump_catalog(ctx.pool).await?;
    }
    Ok(format!("{ok} updated, {gone} delisted of {total}"))
}

/// Items that never got Booth image links (not on the tag listing): fetch item pages.
pub async fn details(ctx: &mut Ctx<'_>, params: &Value) -> Result<String> {
    let delay = Duration::from_millis(param(params, "delay_ms", 500));
    let limit: i64 = param(params, "limit", i64::MAX);
    let ids = sqlx::query_scalar("select i.id from items i where i.status = 'ok' and not exists (select 1 from booth_images b where b.item_id = i.id) order by i.id desc limit $1")
        .bind(limit)
        .fetch_all(ctx.pool)
        .await?;
    fetch_each(ctx, ids, delay, "details").await
}

/// Re-check listed items in order of heat x staleness (see the refresh_priority view): prices, likes, tags, delistings.
pub async fn refresh(ctx: &mut Ctx<'_>, params: &Value) -> Result<String> {
    let delay = Duration::from_millis(param(params, "delay_ms", 700));
    let batch: i64 = param(params, "batch", 1500);
    let ids = sqlx::query_scalar("select id from refresh_priority order by priority desc limit $1").bind(batch).fetch_all(ctx.pool).await?;
    fetch_each(ctx, ids, delay, "refresh").await
}

pub async fn run(ctx: &mut Ctx<'_>, kind: &str, params: &Value) -> Result<String> {
    match kind {
        "incremental" => incremental(ctx, params).await,
        "details" => details(ctx, params).await,
        "refresh" => refresh(ctx, params).await,
        "enrich" => bail!("AI enrichment is not configured yet"),
        other => bail!("unknown job kind {other}"),
    }
}
