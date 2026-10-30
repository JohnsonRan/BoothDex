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
        let ids: Vec<i64> = cards.iter().map(|c| c.0).collect();
        let prices: Vec<Option<i32>> = cards.iter().map(|c| c.1).collect();
        // a changed list price makes the AI-inferred price stale, so drop it
        touched += sqlx::query(
            "update items i set price_jpy = coalesce(t.price, i.price_jpy), last_listed_at = now(), status = 'ok',
                    smart_price = case when t.price is distinct from i.price_jpy and t.price is not null then null else i.smart_price end
             from unnest($1::bigint[], $2::int[]) as t(id, price) where i.id = t.id",
        )
        .bind(&ids)
        .bind(&prices)
        .execute(ctx.pool)
        .await?
        .rows_affected();
        let images = ctx.booth.images(&html);
        let img_ids: Vec<i64> = images.iter().map(|r| r.0).collect();
        let imgs: Vec<String> = images.iter().map(|r| serde_json::to_string(&r.1).unwrap()).collect();
        sqlx::query(
            "insert into booth_images (item_id, images, complete)
             select id, array(select jsonb_array_elements_text(imgs::jsonb)), false from unnest($1::bigint[], $2::text[]) as t(id, imgs)
             on conflict (item_id) do update set images = excluded.images, fetched_at = now() where not booth_images.complete",
        )
        .bind(&img_ids)
        .bind(&imgs)
        .execute(ctx.pool)
        .await?;
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
    // listed items carrying this tag that the walk did not see are delisting candidates
    let missing: Vec<i64> = sqlx::query_scalar(
        "select id from items where status = 'ok' and (last_listed_at is null or last_listed_at < $1)
           and exists (select 1 from unnest(booth_tags) b where lower(b) = lower($2))
         order by id desc limit $3",
    )
    .bind(started)
    .bind(&tag)
    .bind(max_confirm)
    .fetch_all(ctx.pool)
    .await?;
    ctx.log(&format!("listing walk done: {seen} seen, {touched} known items updated; confirming {} missing items", missing.len())).await?;
    let confirm = fetch_each(ctx, missing, delay, "sweep confirm").await?;
    bump_catalog(ctx.pool).await?;
    Ok(format!("{seen} seen on {page} pages; missing items: {confirm}"))
}

async fn ai_client(pool: &PgPool) -> Result<crate::ai::Ai> {
    match crate::ai::Ai::from_config(&catalogue_core::settings::ai(pool).await?)? {
        Some(ai) => Ok(ai),
        None => bail!("AI provider not configured (set API address, model and key in the admin console)"),
    }
}

/// Translates the taxonomy (category names, category groups, smart tags) into Simplified Chinese.
/// Only fills missing names unless params.force is set, so it is cheap to rerun.
pub async fn localize(ctx: &mut Ctx<'_>, params: &Value) -> Result<String> {
    let ai = ai_client(ctx.pool).await?;
    let force: bool = param(params, "force", false);
    // (table, key column, English column, Chinese column)
    let targets = [
        ("smart_categories", "slug", "name_en", "name_zh"),
        ("smart_categories", "slug", "group_en", "group_zh"),
        ("smart_tags", "id::text", "display", "display_zh"),
    ];
    let mut work: Vec<(usize, String, String)> = Vec::new();
    for (t, (table, key, en, zh)) in targets.iter().enumerate() {
        // static identifiers from the list above, no user input
        let sql = format!("select {key}, {en} from {table} where {en} is not null and ({zh} is null or $1)");
        let rows: Vec<(String, String)> = sqlx::query_as(sqlx::AssertSqlSafe(sql)).bind(force).fetch_all(ctx.pool).await?;
        work.extend(rows.into_iter().map(|(k, v)| (t, k, v)));
    }
    if work.is_empty() {
        return Ok("nothing to translate".into());
    }
    // unique English strings, translated in batches of 60
    let mut texts: Vec<String> = work.iter().map(|w| w.2.clone()).collect();
    texts.sort();
    texts.dedup();
    let mut zh: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    for (n, batch) in texts.chunks(60).enumerate() {
        let input: serde_json::Map<String, Value> = batch.iter().map(|s| (s.clone(), Value::String(s.clone()))).collect();
        let out = ai.json(crate::ai::LOCALIZE_SYSTEM, &Value::Object(input).to_string()).await?;
        for s in batch {
            if let Some(t) = out[s.as_str()].as_str().map(str::trim).filter(|t| !t.is_empty() && t.chars().count() <= 60) {
                zh.insert(s.clone(), t.to_owned());
            }
        }
        if !ctx.progress((n + 1) as i64 * 60, Some(texts.len() as i64), &format!("translated {} of {}", zh.len(), texts.len()), false).await? {
            break;
        }
    }
    let mut written = 0u64;
    for (t, key, en) in &work {
        let Some(value) = zh.get(en) else { continue };
        let (table, key_col, _, zh_col) = targets[*t];
        let sql = format!("update {table} set {zh_col} = $1 where {key_col} = $2");
        written += sqlx::query(sqlx::AssertSqlSafe(sql)).bind(value).bind(key).execute(ctx.pool).await?.rows_affected();
    }
    bump_catalog(ctx.pool).await?;
    Ok(format!("{written} names translated to Chinese ({} unique strings)", zh.len()))
}

/// AI enrichment for items our crawler ingested (needs_enrichment): categories, rating, Quest, smart price,
/// translations and features via the model; avatar links by name matching (see avatars.rs).
pub async fn enrich(ctx: &mut Ctx<'_>, params: &Value) -> Result<String> {
    let ai = ai_client(ctx.pool).await?;
    let batch: i64 = param(params, "batch", 200);
    let concurrency: usize = param(params, "concurrency", 4usize).clamp(1, 16);

    let cats: Vec<(String, String, Option<String>)> =
        sqlx::query_as("select c.slug, c.name_en, p.name_en from smart_categories c left join smart_categories p on p.slug = c.parent_slug order by c.slug")
            .fetch_all(ctx.pool)
            .await?;
    // children only: a parent is implied by its child (the API adds parents itself)
    let leaf: Vec<&(String, String, Option<String>)> = cats.iter().filter(|c| c.2.is_some()).collect();
    let known: Vec<String> = leaf.iter().map(|c| c.0.clone()).collect();
    let category_list = leaf.iter().map(|(slug, en, parent)| format!("{slug}: {} / {en}", parent.as_deref().unwrap_or_default())).collect::<Vec<_>>().join("\n");
    let avatars: Vec<(String, String, Option<String>, Option<String>)> = sqlx::query_as("select slug, name_en, name_ja, name_ko from avatars").fetch_all(ctx.pool).await?;
    let matcher = crate::avatars::Matcher::new(avatars.into_iter().map(|(s, en, ja, ko)| (s, [Some(en), ja, ko].into_iter().flatten().collect())));

    let items: Vec<Value> = sqlx::query_scalar(
        "select jsonb_build_object('id', id, 'title', title, 'bio', bio, 'booth_category', booth_category, 'price_jpy', price_jpy,
                'booth_tags', booth_tags, 'variations', variations, 'translations', translations)
         from items where needs_enrichment and status = 'ok' and enrich_failures < 3 order by id desc limit $1",
    )
    .bind(batch)
    .fetch_all(ctx.pool)
    .await?;
    let total = items.len() as i64;
    let (mut ok, mut failed) = (0i64, 0i64);
    let mut stream = futures_util::stream::iter(items.into_iter().map(|it| {
        let prompt = crate::ai::enrich_prompt(&it, &category_list);
        let ai = &ai;
        async move { (it, ai.json(crate::ai::ENRICH_SYSTEM, &prompt).await) }
    }))
    .buffer_unordered(concurrency);
    use futures_util::StreamExt;
    while let Some((it, reply)) = stream.next().await {
        let id = it["id"].as_i64().unwrap_or_default();
        let bio_len = it["bio"].as_str().unwrap_or_default().chars().count();
        let bio_truncated = bio_len > 2500;
        let validation = match reply {
            Ok(v) => crate::ai::validate_enrichment(&v, &known, bio_truncated, &it["translations"]),
            Err(e) => Err(e),
        };
        match validation {
            Ok(e) => {
                let variation_names: Vec<&str> = it["variations"].as_array().into_iter().flatten().filter_map(|v| v["name"].as_str()).collect();
                let match_text = format!(
                    "{} {} {}",
                    it["title"].as_str().unwrap_or_default(),
                    it["booth_tags"].as_array().map(|t| t.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(" ")).unwrap_or_default(),
                    variation_names.join(" ")
                );
                let linked = matcher.find(&match_text);
                write_enrichment(ctx.pool, id, &e, &linked).await?;
                ok += 1;
            }
            Err(err) => {
                failed += 1;
                sqlx::query("update items set enrich_failures = enrich_failures + 1 where id = $1")
                    .bind(id)
                    .execute(ctx.pool)
                    .await?;
                ctx.log(&format!("item {id}: {err:#}")).await?;
            }
        }
        if !ctx.progress(ok + failed, Some(total), &format!("enrich: {ok} done, {failed} failed"), false).await? {
            ctx.log("cancelled").await?;
            break;
        }
    }
    if ok > 0 {
        bump_catalog(ctx.pool).await?;
    }
    Ok(format!("{ok} enriched, {failed} failed of {total}"))
}

async fn write_enrichment(pool: &PgPool, id: i64, e: &crate::ai::Enrichment, linked: &[String]) -> Result<()> {
    let mut tx = pool.begin().await?;
    sqlx::query(
        "update items set content_rating = coalesce($2, content_rating), quest = coalesce($3, quest), smart_price = $4,
                smart_price_reason = coalesce($5, smart_price_reason), translations = $6, features = $7, original_language = $6->>'original_language',
                needs_enrichment = false, enrich_failures = 0
         where id = $1",
    )
    .bind(id)
    .bind(&e.content_rating)
    .bind(&e.quest)
    .bind(e.smart_price)
    .bind(&e.smart_price_reason)
    .bind(&e.translations)
    .bind(&e.features)
    .execute(&mut *tx)
    .await?;
    if !e.categories.is_empty() {
        sqlx::query("delete from item_categories where item_id = $1").bind(id).execute(&mut *tx).await?;
        for (i, slug) in e.categories.iter().enumerate() {
            sqlx::query("insert into item_categories (item_id, category_slug, is_primary) values ($1, $2, $3) on conflict do nothing")
                .bind(id)
                .bind(slug)
                .bind(i == 0)
                .execute(&mut *tx)
                .await?;
        }
    }
    sqlx::query("delete from item_avatars where item_id = $1").bind(id).execute(&mut *tx).await?;
    sqlx::query("insert into item_avatars (item_id, avatar_slug) select $1, unnest($2::text[]) on conflict do nothing")
        .bind(id)
        .bind(linked)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}

pub async fn run(ctx: &mut Ctx<'_>, kind: &str, params: &Value) -> Result<String> {
    match kind {
        "localize" => localize(ctx, params).await,
        "sweep" => sweep(ctx, params).await,
        "incremental" => incremental(ctx, params).await,
        "details" => details(ctx, params).await,
        "refresh" => refresh(ctx, params).await,
        "enrich" => enrich(ctx, params).await,
        other => bail!("unknown job kind {other}"),
    }
}
