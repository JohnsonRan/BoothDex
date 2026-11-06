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
    /// set by a job with work left over: the daemon queues the same kind again once this run has succeeded
    /// (not from inside the job, where the one-live-job-per-kind index would refuse it)
    pub again: bool,
    last_write: Option<Instant>,
}

impl<'a> Ctx<'a> {
    pub fn new(pool: &'a PgPool, booth: &'a Booth, id: i64) -> Self {
        Self { pool, booth, id, again: false, last_write: None }
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
    link_relisted(pool).await?;
    boothdex_core::bump_catalog_version(pool).await?;
    Ok(())
}

use boothdex_core::booth::{mark_delisted, store_item};

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
    match crate::ai::Ai::from_config(&boothdex_core::settings::ai(pool).await?)? {
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
    // scheduled hourly by default; without a provider there is nothing to do, which isn't a failure
    let cfg = boothdex_core::settings::ai(ctx.pool).await?;
    let Some(ai) = crate::ai::Ai::from_config(&cfg)? else {
        return Ok("skipped: AI provider not configured".into());
    };
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
        let (ai, known) = (&ai, &known);
        async move {
            let system = crate::ai::ENRICH_SYSTEM;
            let validation = match ai.json(system, &prompt).await {
                // a reply that parsed but failed validation (e.g. a misspelt category slug) gets one correction round
                Ok(v) => match crate::ai::validate_enrichment(&v, known, &it["translations"]) {
                    Err(e) => match ai.json_correction(system, &prompt, &v, &format!("{e:#}")).await {
                        Ok(v2) => crate::ai::validate_enrichment(&v2, known, &it["translations"]).map_err(|e2| e2.context(format!("after correction (first: {e:#})"))),
                        Err(err) => Err(err),
                    },
                    ok => ok,
                },
                Err(e) => Err(e),
            };
            (it, validation)
        }
    }))
    .buffer_unordered(concurrency);
    use futures_util::StreamExt;
    while let Some((it, validation)) = stream.next().await {
        let id = it["id"].as_i64().unwrap_or_default();
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
                let title = it["title"].as_str().unwrap_or_default();
                let bio = it["bio"].as_str().unwrap_or_default();
                if write_enrichment(ctx.pool, id, title, bio, &e, &linked).await? {
                    ok += 1;
                } else {
                    ctx.log(&format!("item {id}: changed while enriching, left pending")).await?;
                }
            }
            // provider-level failure: stop instead of charging it to every item in the batch
            Err(err) if err.is::<crate::ai::ProviderError>() => {
                if ok > 0 {
                    bump_catalog(ctx.pool).await?;
                }
                return Err(err.context(format!("AI provider error after {ok} enriched; no item was marked as failed")));
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
        // work through a backlog batch by batch; a localize queued meanwhile runs between batches.
        // a batch with no success stops the chain, and items failing 3 times drop out of the query
        ctx.again = sqlx::query_scalar("select exists (select 1 from items where needs_enrichment and status = 'ok' and enrich_failures < 3)")
            .fetch_one(ctx.pool)
            .await?;
    }
    // the chain ends here (backlog done, or a batch without success): embed what enrichment left without a
    // vector in one run, not after every batch, which would also embed the not-yet-enriched backlog
    if !ctx.again && cfg.embed_model.is_some() {
        sqlx::query(
            "insert into jobs (kind, params, created_by) select 'embed', '{}', 'enrich'
             where exists (select 1 from items where status = 'ok' and not needs_enrichment and embedding is null)
             on conflict do nothing",
        )
        .execute(ctx.pool)
        .await?;
    }
    Ok(format!("{ok} enriched, {failed} failed of {total}"))
}

async fn write_enrichment(pool: &PgPool, id: i64, title: &str, bio: &str, e: &crate::ai::Enrichment, linked: &[String]) -> Result<bool> {
    let mut tx = pool.begin().await?;
    let affected = sqlx::query(
        "update items set content_rating = coalesce($2, content_rating), quest = coalesce($3, quest), smart_price = $4,
                smart_price_reason = coalesce($5, smart_price_reason), translations = $6, features = $7, original_language = $6->>'original_language',
                needs_enrichment = false, enrich_failures = 0, embedding = null
         where id = $1 and title = $8 and bio = $9",
    )
    .bind(id)
    .bind(&e.content_rating)
    .bind(&e.quest)
    .bind(e.smart_price)
    .bind(&e.smart_price_reason)
    .bind(&e.translations)
    .bind(&e.features)
    .bind(title)
    .bind(bio)
    .execute(&mut *tx)
    .await?
    .rows_affected();
    if affected == 0 {
        return Ok(false);
    }
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
    Ok(true)
}

/// (id, title, booth_tags, translations.en.title, bio, input_fingerprint)
type EmbedRow = (i64, String, Vec<String>, Option<String>, String, String);

const EMBEDDING_UPDATE: &str = "update items i set embedding = v.e
    from unnest($1::bigint[], $2::halfvec[], $3::text[]) as v(id, e, fp)
    where i.id = v.id and md5(concat_ws(E'\\x1f', i.title, i.bio, i.translations->'en'->>'title', array_to_string(i.booth_tags, E'\\x1e'))) = v.fp";

/// Embeddings for semantic search: every listed item without one (new, edited, or after a model change).
/// Runs until done; requests go out `batch` items at a time, `concurrency` at once.
pub async fn embed(ctx: &mut Ctx<'_>, params: &Value) -> Result<String> {
    use boothdex_core::embed;
    let c = boothdex_core::settings::ai(ctx.pool).await?;
    let (Some(base), Some(key), Some(model)) = (c.base_url, c.api_key, c.embed_model) else {
        return Ok("skipped: embeddings model not configured".into());
    };
    let batch: usize = param(params, "batch", 64usize).clamp(1, 256);
    let concurrency: usize = param(params, "concurrency", 8usize).clamp(1, 16);
    // for trial runs
    let limit: i64 = param(params, "limit", i64::MAX);

    // vectors from another model aren't comparable: start over when the model changes
    let previous: Option<String> = sqlx::query_scalar("select value->>'model' from app_settings where key = 'embed_state'").fetch_optional(ctx.pool).await?.flatten();
    if previous.as_deref() != Some(model.as_str()) {
        if previous.is_some() {
            let n = sqlx::query("update items set embedding = null where embedding is not null").execute(ctx.pool).await?.rows_affected();
            ctx.log(&format!("model changed from {} to {model}: cleared {n} embeddings", previous.as_deref().unwrap_or("-"))).await?;
        }
        sqlx::query("insert into app_settings (key, value) values ('embed_state', $1) on conflict (key) do update set value = excluded.value, updated_at = now()")
            .bind(serde_json::json!({ "model": model }))
            .execute(ctx.pool)
            .await?;
    }

    let total: i64 = sqlx::query_scalar("select count(*) from items where status = 'ok' and embedding is null").fetch_one(ctx.pool).await?;
    let http = reqwest::Client::builder().timeout(Duration::from_secs(120)).build()?;
    // done: vectors written; sent: items sent to the provider, which bounds a trial run's `limit` even when
    // vectors are discarded because the item changed meanwhile
    let (mut done, mut sent, mut below) = (0i64, 0i64, i64::MAX);
    loop {
        // keyset paging: an item the provider can't embed is not fetched again in this run
        let rows: Vec<EmbedRow> = sqlx::query_as(
            "select id, title, booth_tags, translations->'en'->>'title', bio,
                    md5(concat_ws(E'\\x1f', title, bio, translations->'en'->>'title', array_to_string(booth_tags, E'\\x1e')))
             from items
             where status = 'ok' and embedding is null and id < $1 order by id desc limit $2",
        )
        .bind(below)
        .bind(((batch * concurrency) as i64).min(limit - sent))
        .fetch_all(ctx.pool)
        .await?;
        let Some(last) = rows.last() else { break };
        below = last.0;
        sent += rows.len() as i64;
        let chunks: Vec<Vec<(i64, String, String)>> = rows
            .chunks(batch)
            .map(|c| {
                c.iter()
                    .map(|(id, title, tags, en, bio, fp)| (*id, embed::document_text(title, en.as_deref(), tags, bio), fp.clone()))
                    .collect()
            })
            .collect();
        let results = futures_util::future::try_join_all(chunks.iter().map(|c| {
            let inputs: Vec<String> = c.iter().map(|r| r.1.clone()).collect();
            let (http, base, key, model) = (&http, &base, &key, &model);
            async move { embed::embed(http, base, key, model, &inputs, 4).await }
        }))
        .await?;
        let (mut ids, mut vecs, mut fps) = (Vec::new(), Vec::new(), Vec::new());
        for (chunk, vectors) in chunks.iter().zip(results) {
            for ((id, _, fp), v) in chunk.iter().zip(vectors) {
                ids.push(*id);
                vecs.push(embed::HalfVector::from_f32_slice(&v));
                fps.push(fp.clone());
            }
        }
        let written = sqlx::query(EMBEDDING_UPDATE)
            .bind(&ids)
            .bind(&vecs)
            .bind(&fps)
            .execute(ctx.pool)
            .await?
            .rows_affected();
        done += written as i64;
        if !ctx.progress(done, Some(total.min(limit)), &format!("embed: {done} of {}", total.min(limit)), false).await? {
            ctx.log("cancelled").await?;
            break;
        }
        if sent >= limit {
            break;
        }
    }
    // search-text cache: other models' rows are useless, stale ones and the long tail go
    let pruned = sqlx::query(
        "delete from query_embeddings where model <> $1 or last_used_at < now() - interval '90 days'
            or ctid in (select ctid from query_embeddings order by last_used_at desc offset 50000)",
    )
    .bind(&model)
    .execute(ctx.pool)
    .await?
    .rows_affected();
    if pruned > 0 {
        ctx.log(&format!("pruned {pruned} cached search embeddings")).await?;
    }
    Ok(format!("{done} items embedded with {model}"))
}

/// Points delisted items at the shop's re-upload (same shop, same title ignoring case and whitespace, newer id,
/// still listed) and clears links that no longer hold. Returns how many items changed.
async fn link_relisted(pool: &PgPool) -> Result<u64> {
    let cleared = sqlx::query(
        "update items i set relisted_as = null
         where i.relisted_as is not null
           and (i.status = 'ok' or not exists (select 1 from items o where o.id = i.relisted_as and o.status = 'ok'))",
    )
    .execute(pool)
    .await?
    .rows_affected();
    let linked = sqlx::query(
        "with norm as (select id, author_uri, status, lower(regexp_replace(title, '\\s', '', 'g')) as t from items),
              succ as (select distinct on (d.id) d.id, o.id as new_id
                       from norm d join norm o on o.author_uri = d.author_uri and o.t = d.t and o.status = 'ok' and o.id > d.id
                       where d.status = 'delisted'
                       order by d.id, o.id desc)
         update items i set relisted_as = s.new_id from succ s where i.id = s.id and i.relisted_as is distinct from s.new_id",
    )
    .execute(pool)
    .await?
    .rows_affected();
    Ok(cleared + linked)
}

pub async fn run(ctx: &mut Ctx<'_>, kind: &str, params: &Value) -> Result<String> {
    match kind {
        "embed" => embed(ctx, params).await,
        "localize" => localize(ctx, params).await,
        "sweep" => sweep(ctx, params).await,
        "incremental" => incremental(ctx, params).await,
        "details" => details(ctx, params).await,
        "refresh" => refresh(ctx, params).await,
        "enrich" => enrich(ctx, params).await,
        other => bail!("unknown job kind {other}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use boothdex_core::embed::{DIMS, HalfVector};

    #[sqlx::test(migrations = "../core/migrations")]
    #[ignore = "requires PostgreSQL with pgvector"]
    async fn binary_embedding_batch_skips_changed_items(pool: PgPool) {
        sqlx::query("insert into items (id, title, author_name, author_uri, is_adult, status, scraped_at)
                     select id, 'original', 'creator', 'https://example.test', false, 'ok', now() from generate_series(1, 3) as id")
            .execute(&pool).await.unwrap();
        let rows: Vec<(i64, String)> = sqlx::query_as(
            "select id, md5(concat_ws(E'\\x1f', title, bio, translations->'en'->>'title', array_to_string(booth_tags, E'\\x1e'))) from items order by id",
        ).fetch_all(&pool).await.unwrap();
        let (ids, fingerprints): (Vec<_>, Vec<_>) = rows.into_iter().unzip();
        sqlx::query("update items set title = 'edited during embedding' where id = 3").execute(&pool).await.unwrap();
        let values = vec![0.125; DIMS];
        let vectors = vec![HalfVector::from_f32_slice(&values); ids.len()];
        let written = sqlx::query(EMBEDDING_UPDATE).bind(&ids).bind(&vectors).bind(&fingerprints)
            .execute(&pool).await.unwrap().rows_affected();
        assert_eq!(written, 2);
        let stored: Vec<(i64, bool)> = sqlx::query_as("select id, embedding is not null from items order by id").fetch_all(&pool).await.unwrap();
        assert_eq!(stored, [(1, true), (2, true), (3, false)]);
        let same: bool = sqlx::query_scalar("select bool_and(halfvec_send(embedding) = halfvec_send($1::halfvec)) from items where embedding is not null")
            .bind(&vectors[0]).fetch_one(&pool).await.unwrap();
        assert!(same);
        let empty = sqlx::query(EMBEDDING_UPDATE).bind(Vec::<i64>::new()).bind(Vec::<HalfVector>::new()).bind(Vec::<String>::new())
            .execute(&pool).await.unwrap().rows_affected();
        assert_eq!(empty, 0);
    }

    #[sqlx::test(migrations = "../core/migrations")]
    #[ignore = "requires PostgreSQL with pgvector"]
    async fn delisted_items_point_at_their_re_upload(pool: PgPool) {
        // 10 delisted; 9 an older copy; 11 and 12 re-uploads (12 newest); 13 another shop; 14 another title
        for (id, shop, title, status) in [
            (9, "a", "Foo Bar", "ok"),
            (10, "a", "Foo Bar", "delisted"),
            (11, "a", "foo  bar", "ok"),
            (12, "a", "FOO BAR", "ok"),
            (13, "b", "Foo Bar", "ok"),
            (14, "a", "Foo Baz", "ok"),
        ] {
            sqlx::query("insert into items (id, title, author_name, author_uri, is_adult, status, scraped_at) values ($1, $3, 's', $2, false, $4, now())")
                .bind(id as i64).bind(shop).bind(title).bind(status).execute(&pool).await.unwrap();
        }
        let link = || async { sqlx::query_scalar::<_, Option<i64>>("select relisted_as from items where id = 10").fetch_one(&pool).await.unwrap() };
        assert_eq!(link_relisted(&pool).await.unwrap(), 1);
        assert_eq!(link().await, Some(12));
        assert_eq!(link_relisted(&pool).await.unwrap(), 0); // nothing changed
        sqlx::query("update items set status = 'delisted' where id = 12").execute(&pool).await.unwrap();
        link_relisted(&pool).await.unwrap();
        assert_eq!(link().await, Some(11)); // the newest copy still on sale
        sqlx::query("update items set status = 'ok' where id = 10").execute(&pool).await.unwrap();
        link_relisted(&pool).await.unwrap();
        assert_eq!(link().await, None); // listed again itself
        let stray: i64 = sqlx::query_scalar("select count(*) from items where relisted_as is not null").fetch_one(&pool).await.unwrap();
        assert_eq!(stray, 0); // delisted 12 has no newer copy: older listings are not re-uploads
    }
}
