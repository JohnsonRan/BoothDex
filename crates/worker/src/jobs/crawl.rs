//! BOOTH crawl jobs: incremental listing walk, missing-image details fetch, priority refresh and full sweep.

use std::time::Duration;

use anyhow::Result;
use boothdex_core::booth::{mark_delisted, store_item};
use serde_json::Value;
use sqlx::PgPool;

use super::{Ctx, bump_catalog, param};

/// Listing pages show at most 4 images, so a listing gallery never replaces a full one taken from the item page.
pub(super) const LISTING_IMAGES_UPSERT: &str = "insert into booth_images (item_id, images, complete)
             select id, array(select jsonb_array_elements_text(imgs::jsonb)), false from unnest($1::bigint[], $2::text[]) as t(id, imgs)
             on conflict (item_id) do update set images = excluded.images, fetched_at = now() where not booth_images.complete";

/// Newest-first listing walk: stores image links for every item seen, ingests unknown items, stops once caught up.
/// Bootstrapping an empty catalogue: `{"max_pages": 5000, "full": true}`; resume an interrupted one with `start_page`.
pub async fn incremental(ctx: &mut Ctx<'_>, params: &Value) -> Result<String> {
    let max_pages: u32 = param(params, "max_pages", 40);
    let start_page: u32 = param(params, "start_page", 1u32).max(1);
    let full: bool = param(params, "full", false);
    let tag: String = param(params, "tag", "VRChat".to_owned());
    let delay = Duration::from_millis(param(params, "delay_ms", 700));
    let (mut new_items, mut revived, mut seen) = (0i64, 0u64, 0i64);
    let mut caught_up = false;
    for page in start_page..=max_pages {
        let (rows, last) = ctx.booth.listing(&tag, page).await?;
        seen += rows.len() as i64;
        let ids: Vec<i64> = rows.iter().map(|r| r.0).collect();
        let imgs: Vec<String> = rows.iter().map(|r| serde_json::to_string(&r.1).unwrap()).collect();
        sqlx::query(LISTING_IMAGES_UPSERT).bind(&ids).bind(&imgs).execute(ctx.pool).await?;
        revived += sqlx::query("update items set status = 'ok' where id = any($1) and status = 'delisted'").bind(&ids).execute(ctx.pool).await?.rows_affected();
        let unknown: Vec<i64> = sqlx::query_scalar("select id from unnest($1::bigint[]) as t(id) where not exists (select 1 from items i where i.id = t.id)")
            .bind(&ids)
            .fetch_all(ctx.pool)
            .await?;
        for id in &unknown {
            tokio::time::sleep(delay).await;
            if let Some(v) = ctx.booth.http.item(*id).await? {
                store_item(ctx.pool, &v).await?;
                new_items += 1;
            }
        }
        if !ctx.progress(page as i64, Some(max_pages.min(last) as i64), &format!("page {page}: {seen} seen, {new_items} new"), false).await? {
            ctx.log("cancelled").await?;
            break;
        }
        // the first pages always get a look (new items can land slightly out of order); after that stop at a fully known page
        if !full && page >= start_page + 2 && unknown.is_empty() {
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
        match ctx.booth.http.item(*id).await? {
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

/// Full listing sweep: every page of the tag listing (60 items per request, ~3.4k requests for VRChat).
/// Refreshes listed price, image links and "still on sale" for everything cheaply; items that have
/// dropped off the listing get one item-page check to confirm whether they were delisted.
pub async fn sweep(ctx: &mut Ctx<'_>, params: &Value) -> Result<String> {
    let tag: String = param(params, "tag", "VRChat".to_owned());
    let delay = Duration::from_millis(param(params, "delay_ms", 800));
    let max_confirm: i64 = param(params, "max_confirm", 3000);
    // for trial runs; a partial walk skips the delisting check, since unseen items prove nothing
    let max_pages: u32 = param(params, "max_pages", u32::MAX);
    let started: chrono::DateTime<chrono::Utc> = sqlx::query_scalar("select now()").fetch_one(ctx.pool).await?;
    let (mut page, mut last, mut seen, mut touched) = (1u32, 1u32, 0u64, 0u64);
    loop {
        let (html, l) = ctx.booth.listing_html(&tag, page).await?;
        last = last.max(l);
        let cards = crate::booth::parse_cards(&html);
        if cards.is_empty() && page > 1 {
            break;
        }
        seen += cards.len() as u64;
        touched += store_listing_page(ctx, &html, &cards).await?;
        if !ctx.progress(page as i64, Some(last as i64), &format!("page {page}/{last}: {seen} seen"), false).await? {
            ctx.log("cancelled during listing walk; skipping delisting check").await?;
            return Ok(format!("cancelled at page {page}, {seen} seen"));
        }
        if page >= last || page >= max_pages {
            break;
        }
        page += 1;
        tokio::time::sleep(delay).await;
    }
    if page < last {
        bump_catalog(ctx.pool).await?;
        return Ok(format!("partial walk: {seen} seen on {page}/{last} pages, delisting check skipped"));
    }
    let missing = unseen_by_walk(ctx.pool, started, &tag, max_confirm).await?;
    ctx.log(&format!("listing walk done: {seen} seen, {touched} known items updated; confirming {} missing items", missing.len())).await?;
    let confirm = fetch_each(ctx, missing, delay, "sweep confirm").await?;
    bump_catalog(ctx.pool).await?;
    Ok(format!("{seen} seen on {page} pages; missing items: {confirm}"))
}

/// Stores one listing page: list prices of known items and the image links; returns how many known items it updated.
async fn store_listing_page(ctx: &Ctx<'_>, html: &str, cards: &[(i64, Option<i32>)]) -> Result<u64> {
    let ids: Vec<i64> = cards.iter().map(|c| c.0).collect();
    let prices: Vec<Option<i32>> = cards.iter().map(|c| c.1).collect();
    // a changed list price makes the AI-inferred price stale, so drop it
    let touched = sqlx::query(
        "update items i set price_jpy = coalesce(t.price, i.price_jpy), last_listed_at = now(), status = 'ok',
                smart_price = case when t.price is distinct from i.price_jpy and t.price is not null then null else i.smart_price end
         from unnest($1::bigint[], $2::int[]) as t(id, price) where i.id = t.id",
    )
    .bind(&ids)
    .bind(&prices)
    .execute(ctx.pool)
    .await?
    .rows_affected();
    let images = ctx.booth.images(html);
    let img_ids: Vec<i64> = images.iter().map(|r| r.0).collect();
    let imgs: Vec<String> = images.iter().map(|r| serde_json::to_string(&r.1).unwrap()).collect();
    sqlx::query(LISTING_IMAGES_UPSERT).bind(&img_ids).bind(&imgs).execute(ctx.pool).await?;
    Ok(touched)
}

/// Listed items carrying this tag that the walk did not see are delisting candidates.
async fn unseen_by_walk(pool: &PgPool, started: chrono::DateTime<chrono::Utc>, tag: &str, limit: i64) -> Result<Vec<i64>> {
    Ok(sqlx::query_scalar(
        "select id from items where status = 'ok' and (last_listed_at is null or last_listed_at < $1)
           and exists (select 1 from unnest(booth_tags) b where lower(b) = lower($2))
         order by id desc limit $3",
    )
    .bind(started)
    .bind(tag)
    .bind(limit)
    .fetch_all(pool)
    .await?)
}
