//! Job implementations. Each returns a one-line summary; progress/cancel go through `Ctx`.

use std::time::{Duration, Instant};

use anyhow::{Result, bail};
use serde_json::Value;
use sqlx::PgPool;

use crate::booth::Booth;
use boothdex_core::jobs::JobKind;

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

/// Listing pages show at most 4 images, so a listing gallery never replaces a full one taken from the item page.
const LISTING_IMAGES_UPSERT: &str = "insert into booth_images (item_id, images, complete)
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
        sqlx::query(LISTING_IMAGES_UPSERT)
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

/// Listed items a BOOTH tag must be on before `localize` gives it a dictionary row: production had 31,775 such tags,
/// 95% of tag uses (2026-11-20).
const TAG_MIN_ITEMS: i64 = 5;
/// Tags per request.
const TAG_BATCH: usize = 40;
/// Runs a tag may get no valid value in before `localize` stops asking for it (`booth_tag_failures`), so a tag the model
/// always fails on is not paid for every day.
const TAG_MAX_FAILURES: i32 = 3;

/// Translates the taxonomy (category names, category groups, smart tags) into Simplified Chinese, and gives each BOOTH
/// tag on `TAG_MIN_ITEMS` listed items without a dictionary row (`booth_tag_translations`) one in four languages.
/// Only fills what is missing, so it is cheap to rerun. params.force translates every taxonomy name again and leaves the
/// dictionary alone, so no run overwrites an imported or corrected row (owner ruling 22, 2026-11-21).
/// params.max_tags (default 400) caps the tags one run asks for: a dictionary without an import fills over several runs
/// instead of holding the AI lane, which enrichment shares, for hours.
pub async fn localize(ctx: &mut Ctx<'_>, params: &Value) -> Result<String> {
    // scheduled daily; without a provider there is nothing to do, which isn't a failure
    let Some(ai) = crate::ai::Ai::from_config(&boothdex_core::settings::ai(ctx.pool).await?)? else {
        return Ok("skipped: AI provider not configured".into());
    };
    let force: bool = param(params, "force", false);
    let (written, unique, finished) = localize_names(ctx, &ai, force).await?;
    if written > 0 {
        bump_catalog(ctx.pool).await?;
    }
    let names = format!("{written} names translated to Chinese ({unique} unique strings)");
    if force || !finished {
        return Ok(names);
    }
    let tags = localize_tags(ctx, &ai, param(params, "max_tags", 400i64)).await?;
    Ok(format!("{names}; {tags}"))
}

/// The taxonomy part of `localize`: (rows written, unique strings translated, whether it ran to the end).
async fn localize_names(ctx: &mut Ctx<'_>, ai: &crate::ai::Ai, force: bool) -> Result<(u64, usize, bool)> {
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
    // unique English strings, translated in batches of 60
    let mut texts: Vec<String> = work.iter().map(|w| w.2.clone()).collect();
    texts.sort();
    texts.dedup();
    let mut zh: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    let mut finished = true;
    for (n, batch) in texts.chunks(60).enumerate() {
        let input: serde_json::Map<String, Value> = batch.iter().map(|s| (s.clone(), Value::String(s.clone()))).collect();
        let out = ai.json(crate::ai::LOCALIZE_SYSTEM, &Value::Object(input).to_string()).await?;
        for s in batch {
            if let Some(t) = out[s.as_str()].as_str().map(str::trim).filter(|t| !t.is_empty() && t.chars().count() <= 60) {
                zh.insert(s.clone(), t.to_owned());
            }
        }
        if !ctx.progress((n + 1) as i64 * 60, Some(texts.len() as i64), &format!("translated {} of {}", zh.len(), texts.len()), false).await? {
            finished = false;
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
    Ok((written, zh.len(), finished))
}

/// The BOOTH tags on at least $2 listed items without a dictionary row and without $3 failed runs, most used first, at
/// most $1; each with up to 3 titles of listed items that carry it, most liked first.
const MISSING_TAGS_SELECT: &str = "with counts as (
           select t.tag, count(*) as n
             from items i cross join lateral (select distinct unnest(i.booth_tags) as tag) t
            where i.status = 'ok'
            group by t.tag
           having count(*) >= $2
         ), missing as (
           select c.tag, c.n from counts c
            where c.tag <> '' and not exists (select 1 from booth_tag_translations b where b.tag = c.tag)
              and not exists (select 1 from booth_tag_failures f where f.tag = c.tag and f.failures >= $3)
            order by c.n desc, c.tag limit $1
         ), ranked as (
           select m.tag, m.n, i.title, row_number() over (partition by m.tag order by i.likes desc, i.id desc) as r
             from missing m join (select id, title, likes, unnest(booth_tags) as tag from items where status = 'ok') i on i.tag = m.tag
         )
         select tag, array_agg(title order by r) from ranked where r <= 3 group by tag, n order by n desc, tag";

/// Adds the rows the job made; a tag that got a row meanwhile (an import) keeps it. A language left out is null. $6
/// holds each row's keywords joined by U+001F.
const TAG_ROWS_INSERT: &str = "insert into booth_tag_translations (tag, en, ja, ko, zh, keywords, source)
         select t.tag, t.en, t.ja, t.ko, t.zh, coalesce(string_to_array(nullif(t.k, ''), chr(31)), '{}'), 'localize'
           from unnest($1::text[], $2::text[], $3::text[], $4::text[], $5::text[], $6::text[]) as t(tag, en, ja, ko, zh, k)
         on conflict (tag) do nothing";

/// Counts one more failed run for each tag.
const TAG_FAILURES_UPSERT: &str = "insert into booth_tag_failures (tag, failures) select unnest($1::text[]), 1
         on conflict (tag) do update set failures = booth_tag_failures.failures + 1";

/// A tag with its values in `ai::TAG_LANGUAGES` order, `None` for a language left out.
type TagRow = (String, [Option<String>; 4]);
/// A tag with titles of listed items that carry it.
type TagAsk = (String, Vec<String>);

async fn insert_tag_rows(pool: &PgPool, rows: &[TagRow]) -> Result<u64> {
    if rows.is_empty() {
        return Ok(0);
    }
    let column = |k: usize| rows.iter().map(|r| r.1[k].clone()).collect::<Vec<_>>();
    let tags: Vec<&str> = rows.iter().map(|r| r.0.as_str()).collect();
    let keywords: Vec<String> = rows.iter().map(|r| r.1[3].as_deref().map(crate::ai::tag_keywords).unwrap_or_default().join("\u{1f}")).collect();
    let insert = sqlx::query(TAG_ROWS_INSERT).bind(&tags).bind(column(0)).bind(column(1)).bind(column(2)).bind(column(3)).bind(&keywords);
    Ok(insert.execute(pool).await?.rows_affected())
}

/// What the tag part of one `localize` run did.
#[derive(Default)]
struct TagTally {
    added: u64,
    /// rows added with a language left out
    partial: usize,
    /// tags without any valid value, asked again by a later run
    failed: usize,
}

/// The tag part of `localize`. A tag that is an avatar's name in any language gets its romaji in English and the tag as
/// written in the other languages, without a request. The others are asked in batches of `TAG_BATCH`; a value that
/// fails `ai::tag_values` is left out and logged, and a tag with no valid value gets no row and one more failed run.
async fn localize_tags(ctx: &mut Ctx<'_>, ai: &crate::ai::Ai, max: i64) -> Result<String> {
    let missing: Vec<TagAsk> = sqlx::query_as(MISSING_TAGS_SELECT).bind(max).bind(TAG_MIN_ITEMS).bind(TAG_MAX_FAILURES).fetch_all(ctx.pool).await?;
    if missing.is_empty() {
        return Ok("no new tags".into());
    }
    let avatars = avatar_kit(ctx.pool).await?;
    let romaji = |tag: &str| avatars.romaji.get(&tag.trim().to_lowercase()).cloned();
    let (named, asked): (Vec<TagAsk>, Vec<TagAsk>) = missing.into_iter().partition(|(tag, _)| romaji(tag).is_some());
    let named: Vec<TagRow> = named.into_iter().map(|(tag, _)| (tag.clone(), [romaji(&tag), Some(tag.clone()), Some(tag.clone()), Some(tag)])).collect();
    let mut tally = TagTally { added: insert_tag_rows(ctx.pool, &named).await?, ..Default::default() };
    let outcome = ask_tags(ctx, ai, &avatars, asked, &mut tally).await;
    if tally.added > 0 {
        bump_catalog(ctx.pool).await?;
    }
    outcome.map_err(|e| e.context(format!("after {} tags were added", tally.added)))?;
    Ok(format!(
        "{} tags added to the dictionary ({} avatar names, {} with a language left empty), {} without a valid translation",
        tally.added,
        named.len(),
        tally.partial,
        tally.failed
    ))
}

/// Asks for `asked` in batches of `TAG_BATCH`, with the reply held to `ai::tag_schema` until the provider refuses that
/// format; from then on, for the rest of the run, as a plain JSON object.
async fn ask_tags(ctx: &mut Ctx<'_>, ai: &crate::ai::Ai, avatars: &AvatarKit, asked: Vec<TagAsk>, tally: &mut TagTally) -> Result<()> {
    let total = asked.len();
    let mut schema = true;
    for (n, batch) in asked.chunks(TAG_BATCH).enumerate() {
        let reply = ask_batch(ctx, ai, avatars, batch, &mut schema).await?;
        let (mut rows, mut failed) = (Vec::new(), Vec::new());
        for (i, (tag, titles)) in batch.iter().enumerate() {
            let values = crate::ai::tag_values(&reply[crate::ai::tag_id(i)], tag, titles, &avatars.held_names(tag));
            let left_out: Vec<String> = crate::ai::TAG_LANGUAGES.iter().zip(&values).filter_map(|(lang, v)| v.as_ref().err().map(|why| format!("{lang} {why}"))).collect();
            if left_out.len() == values.len() {
                let why = if values.iter().all(|v| v.as_ref().is_err_and(|why| why == "no value")) { "no answer".to_owned() } else { left_out.join("; ") };
                ctx.log(&format!("tag {tag}: no valid translation ({why}); a later run asks again")).await?;
                failed.push(tag.clone());
                continue;
            }
            if !left_out.is_empty() {
                ctx.log(&format!("tag {tag}: left empty: {}", left_out.join("; "))).await?;
                tally.partial += 1;
            }
            rows.push((tag.clone(), values.map(std::result::Result::ok)));
        }
        tally.added += insert_tag_rows(ctx.pool, &rows).await?;
        tally.failed += failed.len();
        if !failed.is_empty() {
            sqlx::query(TAG_FAILURES_UPSERT).bind(&failed).execute(ctx.pool).await?;
        }
        let done = ((n + 1) * TAG_BATCH).min(total);
        if !ctx.progress(done as i64, Some(total as i64), &format!("tags: {} added, {} without a valid translation", tally.added, tally.failed), false).await? {
            return ctx.log("cancelled").await;
        }
    }
    Ok(())
}

/// One request for `batch`; returns the reply, keyed by `ai::tag_id`. A provider error stops the job; an unreadable
/// reply is logged and counts as no answer. A provider that refuses the json_schema format gets the batch again as a
/// plain JSON request, and `schema` turns off.
async fn ask_batch(ctx: &Ctx<'_>, ai: &crate::ai::Ai, avatars: &AvatarKit, batch: &[TagAsk], schema: &mut bool) -> Result<Value> {
    let mut slugs: Vec<String> = batch.iter().flat_map(|(tag, _)| avatars.matcher.find(tag)).collect();
    slugs.sort();
    slugs.dedup();
    let names: Vec<&crate::ai::AvatarName> = slugs.iter().filter_map(|s| avatars.names.get(s)).collect();
    let user = crate::ai::tag_prompt(batch, &names);
    let mut reply = None;
    if *schema {
        reply = ai.json_schema(crate::ai::TAG_SYSTEM, &user, crate::ai::tag_schema(batch.len())).await.transpose();
        if reply.is_none() {
            ctx.log("the provider refused the json_schema reply format; the rest of this run asks for a plain JSON object").await?;
            *schema = false;
        }
    }
    let reply = match reply {
        Some(reply) => reply,
        None => ai.json(crate::ai::TAG_SYSTEM, &user).await,
    };
    match reply {
        Ok(v) => Ok(v),
        Err(err) if err.is::<crate::ai::ProviderError>() => Err(err),
        Err(err) => {
            ctx.log(&format!("{} tags from {}: {err:#}", batch.len(), batch[0].0)).await?;
            Ok(Value::Null)
        }
    }
}

/// The avatars table as the AI jobs read it.
struct AvatarKit {
    /// finds the avatars a text names (avatars.rs)
    matcher: crate::avatars::Matcher,
    /// each avatar's names, by slug
    names: std::collections::HashMap<String, crate::ai::AvatarName>,
    /// each avatar's names in every language, by slug
    forms: std::collections::HashMap<String, Vec<String>>,
    /// every name in any language, trimmed and lowercase -> the avatar's romaji
    romaji: std::collections::HashMap<String, String>,
}

impl AvatarKit {
    /// The avatar names `tag` holds: for each avatar the matcher finds in it, the names it writes and the avatar's
    /// Japanese name and romaji, each NFKC and lowercase.
    fn held_names(&self, tag: &str) -> Vec<crate::ai::HeldName> {
        let folded = crate::avatars::norm(tag);
        self.matcher
            .find(tag)
            .iter()
            .filter_map(|slug| {
                let name = self.names.get(slug)?;
                let written = self.forms.get(slug)?.iter().map(|f| crate::avatars::norm(f)).filter(|f| folded.contains(f.as_str()));
                let mut forms: Vec<String> = written.chain(name.ja.iter().chain([&name.romaji]).map(|f| crate::avatars::norm(f))).collect();
                forms.dedup();
                Some(crate::ai::HeldName(forms))
            })
            .collect()
    }
}

async fn avatar_kit(pool: &PgPool) -> Result<AvatarKit> {
    let avatars: Vec<(String, String, Option<String>, Option<String>)> = sqlx::query_as("select slug, name_en, name_ja, name_ko from avatars").fetch_all(pool).await?;
    let forms: std::collections::HashMap<String, Vec<String>> =
        avatars.iter().map(|(s, en, ja, ko)| (s.clone(), [Some(en), ja.as_ref(), ko.as_ref()].into_iter().flatten().cloned().collect())).collect();
    let matcher = crate::avatars::Matcher::new(forms.iter().map(|(s, f)| (s.clone(), f.clone())));
    let romaji = avatars
        .iter()
        .flat_map(|(_, en, ja, ko)| [Some(en), ja.as_ref(), ko.as_ref()].into_iter().flatten().map(move |n| (n.trim().to_lowercase(), en.clone())))
        .collect();
    let names = avatars.into_iter().map(|(slug, romaji, ja, _)| (slug, crate::ai::AvatarName { ja, romaji })).collect();
    Ok(AvatarKit { matcher, names, forms, romaji })
}

const ENRICH_ITEMS_SELECT: &str = "select id, title, bio, booth_category, price_jpy, booth_tags, variations, translations
         from items where needs_enrichment and status = 'ok' and enrich_failures < $2 order by id desc limit $1";

/// What one enrich batch shares: the category list for the prompt, the slugs a reply may use and the avatars.
struct EnrichKit {
    category_list: String,
    known: Vec<String>,
    avatars: AvatarKit,
}

async fn enrich_kit(pool: &PgPool) -> Result<EnrichKit> {
    let cats: Vec<(String, String, Option<String>)> =
        sqlx::query_as("select c.slug, c.name_en, p.name_en from smart_categories c left join smart_categories p on p.slug = c.parent_slug order by c.slug")
            .fetch_all(pool)
            .await?;
    // children only: a parent is implied by its child (the API adds parents itself)
    let leaf: Vec<&(String, String, Option<String>)> = cats.iter().filter(|c| c.2.is_some()).collect();
    let known = leaf.iter().map(|c| c.0.clone()).collect();
    let category_list = leaf.iter().map(|(slug, en, parent)| format!("{slug}: {} / {en}", parent.as_deref().unwrap_or_default())).collect::<Vec<_>>().join("\n");
    Ok(EnrichKit { category_list, known, avatars: avatar_kit(pool).await? })
}

/// One model call for one item; a reply that parsed but failed validation (e.g. a misspelt category slug) gets one correction round.
/// `linked`: the slugs of the avatars the matcher found in the listing.
async fn enrich_reply(ai: &crate::ai::Ai, kit: &EnrichKit, item: &crate::ai::EnrichItem, linked: &[String]) -> Result<crate::ai::Enrichment> {
    let system = crate::ai::ENRICH_SYSTEM;
    let names: Vec<&crate::ai::AvatarName> = linked.iter().filter_map(|slug| kit.avatars.names.get(slug)).collect();
    let prompt = crate::ai::enrich_prompt(item, &kit.category_list, &names);
    let v = ai.json(system, &prompt).await?;
    match crate::ai::validate_enrichment(&v, &kit.known, &item.translations) {
        Err(e) => {
            let v2 = ai.json_correction(system, &prompt, &v, &format!("{e:#}")).await?;
            crate::ai::validate_enrichment(&v2, &kit.known, &item.translations).map_err(|e2| e2.context(format!("after correction (first: {e:#})")))
        }
        ok => ok,
    }
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

    let kit = enrich_kit(ctx.pool).await?;
    let items: Vec<crate::ai::EnrichItem> = sqlx::query_as(ENRICH_ITEMS_SELECT).bind(batch).bind(boothdex_core::ENRICH_ATTEMPTS).fetch_all(ctx.pool).await?;
    let total = items.len() as i64;
    let (mut ok, mut failed) = (0i64, 0i64);
    let mut stream = futures_util::stream::iter(items.into_iter().map(|it| {
        let (ai, kit) = (&ai, &kit);
        async move {
            let linked = kit.avatars.matcher.find(&it.match_text());
            (enrich_reply(ai, kit, &it, &linked).await, it, linked)
        }
    }))
    .buffer_unordered(concurrency);
    use futures_util::StreamExt;
    while let Some((reply, it, linked)) = stream.next().await {
        match reply {
            Ok(e) => {
                if write_enrichment(ctx.pool, it.id, &it.title, &it.bio, &e, &linked).await? {
                    ok += 1;
                } else {
                    ctx.log(&format!("item {}: changed while enriching, left pending", it.id)).await?;
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
                    .bind(it.id)
                    .execute(ctx.pool)
                    .await?;
                ctx.log(&format!("item {}: {err:#}", it.id)).await?;
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
        // a batch with no success stops the chain, and items failing ENRICH_ATTEMPTS times drop out of the query
        ctx.again = sqlx::query_scalar("select exists (select 1 from items where needs_enrichment and status = 'ok' and enrich_failures < $1)")
            .bind(boothdex_core::ENRICH_ATTEMPTS)
            .fetch_one(ctx.pool)
            .await?;
    }
    // the chain ends here (backlog done, or a batch without success): embed what enrichment left without a
    // vector in one run, not after every batch, which would also embed the not-yet-enriched backlog
    if !ctx.again && cfg.embed_model.is_some() {
        queue_embed(ctx.pool).await?;
    }
    Ok(format!("{ok} enriched, {failed} failed of {total}"))
}

async fn queue_embed(pool: &PgPool) -> Result<()> {
    sqlx::query(
        "insert into jobs (kind, params, created_by) select 'embed', '{}', 'enrich'
         where exists (select 1 from items where status = 'ok' and not needs_enrichment and embedding is null)
         on conflict do nothing",
    )
    .execute(pool)
    .await?;
    Ok(())
}

async fn write_enrichment(pool: &PgPool, id: i64, title: &str, bio: &str, e: &crate::ai::Enrichment, linked: &[String]) -> Result<bool> {
    let mut tx = pool.begin().await?;
    let affected = sqlx::query(
        "update items set content_rating = coalesce($2, content_rating), quest = coalesce($3, quest), smart_price = $4,
                translations = $5, features = $6, original_language = $5->>'original_language',
                needs_enrichment = false, enrich_failures = 0, embedding = null
         where id = $1 and title = $7 and bio = $8",
    )
    .bind(id)
    .bind(&e.content_rating)
    .bind(&e.quest)
    .bind(e.smart_price)
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

/// Hash of the text an item's embedding is made from, over unqualified `items` columns: the embed job takes it when it
/// reads an item and writes the vector only while it still matches. A macro, so the queries stay static strings.
macro_rules! embedding_fingerprint {
    () => {
        "md5(concat_ws(E'\\x1f', title, bio, translations->'en'->>'title', array_to_string(booth_tags, E'\\x1e')))"
    };
}

/// (id, title, booth_tags, translations.en.title, bio, input_fingerprint)
type EmbedRow = (i64, String, Vec<String>, Option<String>, String, String);

const EMBED_ROWS_SELECT: &str = concat!(
    "select id, title, booth_tags, translations->'en'->>'title', bio,
                    ",
    embedding_fingerprint!(),
    "
             from items
             where status = 'ok' and embedding is null and id < $1 order by id desc limit $2"
);

const EMBEDDING_UPDATE: &str = concat!(
    "update items i set embedding = v.e
    from unnest($1::bigint[], $2::halfvec[], $3::text[]) as v(id, e, fp)
    where i.id = v.id and ",
    embedding_fingerprint!(),
    " = v.fp"
);

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

    clear_embeddings_on_model_change(ctx, &model).await?;

    let total: i64 = sqlx::query_scalar("select count(*) from items where status = 'ok' and embedding is null").fetch_one(ctx.pool).await?;
    let http = reqwest::Client::builder().timeout(Duration::from_secs(120)).build()?;
    // done: vectors written; sent: items sent to the provider, which bounds a trial run's `limit` even when
    // vectors are discarded because the item changed meanwhile
    let (mut done, mut sent, mut below) = (0i64, 0i64, i64::MAX);
    loop {
        // keyset paging: an item the provider can't embed is not fetched again in this run
        let rows: Vec<EmbedRow> = sqlx::query_as(EMBED_ROWS_SELECT)
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
    prune_query_embeddings(ctx, &model).await?;
    Ok(format!("{done} items embedded with {model}"))
}

/// Vectors from another model aren't comparable: start over when the model changes.
async fn clear_embeddings_on_model_change(ctx: &Ctx<'_>, model: &str) -> Result<()> {
    let previous: Option<String> = sqlx::query_scalar("select value->>'model' from app_settings where key = 'embed_state'").fetch_optional(ctx.pool).await?.flatten();
    if previous.as_deref() == Some(model) {
        return Ok(());
    }
    if previous.is_some() {
        let n = sqlx::query("update items set embedding = null where embedding is not null").execute(ctx.pool).await?.rows_affected();
        ctx.log(&format!("model changed from {} to {model}: cleared {n} embeddings", previous.as_deref().unwrap_or("-"))).await?;
    }
    sqlx::query("insert into app_settings (key, value) values ('embed_state', $1) on conflict (key) do update set value = excluded.value, updated_at = now()")
        .bind(serde_json::json!({ "model": model }))
        .execute(ctx.pool)
        .await?;
    Ok(())
}

/// Search-text cache: other models' rows are useless, stale ones and the long tail go.
async fn prune_query_embeddings(ctx: &Ctx<'_>, model: &str) -> Result<()> {
    let pruned = sqlx::query(
        "delete from query_embeddings where model <> $1 or last_used_at < now() - interval '90 days'
            or ctid in (select ctid from query_embeddings order by last_used_at desc offset 50000)",
    )
    .bind(model)
    .execute(ctx.pool)
    .await?
    .rows_affected();
    if pruned > 0 {
        ctx.log(&format!("pruned {pruned} cached search embeddings")).await?;
    }
    Ok(())
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
    match JobKind::parse(kind) {
        Some(JobKind::Embed) => embed(ctx, params).await,
        Some(JobKind::Localize) => localize(ctx, params).await,
        Some(JobKind::Sweep) => sweep(ctx, params).await,
        Some(JobKind::Incremental) => incremental(ctx, params).await,
        Some(JobKind::Details) => details(ctx, params).await,
        Some(JobKind::Refresh) => refresh(ctx, params).await,
        Some(JobKind::Enrich) => enrich(ctx, params).await,
        None => bail!("unknown job kind {kind}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use boothdex_core::embed::{DIMS, HalfVector};
    use serde_json::json;

    #[sqlx::test(migrations = "../core/migrations")]
    #[ignore = "requires PostgreSQL with pgvector"]
    async fn listing_images_preserve_complete_galleries(pool: PgPool) {
        // 1 a full gallery from the item page; 2 an older listing gallery; 3 and 4 not stored yet, and not in items
        sqlx::query("insert into booth_images (item_id, images, complete, fetched_at) values
                     (1, '{f1,f2,f3,f4,f5}', true, '2000-01-01'), (2, '{old}', false, '2000-01-01')")
            .execute(&pool).await.unwrap();
        let galleries: [(i64, &[&str]); 4] = [(1, &["l1"]), (2, &["n1", "n2"]), (3, &["c", "a", "b"]), (4, &[])];
        let ids: Vec<i64> = galleries.iter().map(|g| g.0).collect();
        let imgs: Vec<String> = galleries.iter().map(|g| serde_json::to_string(&g.1).unwrap()).collect();
        let expected = [
            (1, vec!["f1", "f2", "f3", "f4", "f5"], true, false),
            (2, vec!["n1", "n2"], false, true),
            (3, vec!["c", "a", "b"], false, true),
            (4, vec![], false, true),
        ];
        for _ in 0..2 {
            sqlx::query(LISTING_IMAGES_UPSERT).bind(&ids).bind(&imgs).execute(&pool).await.unwrap();
            let stored: Vec<(i64, Vec<String>, bool, bool)> =
                sqlx::query_as("select item_id, images, complete, fetched_at > '2000-01-01' from booth_images order by item_id")
                    .fetch_all(&pool).await.unwrap();
            assert_eq!(stored, expected.iter().map(|(id, images, complete, fresh)| (*id, images.iter().map(|s| s.to_string()).collect(), *complete, *fresh)).collect::<Vec<_>>());
        }
        let empty = sqlx::query(LISTING_IMAGES_UPSERT).bind(Vec::<i64>::new()).bind(Vec::<String>::new())
            .execute(&pool).await.unwrap().rows_affected();
        assert_eq!(empty, 0);
    }

    #[sqlx::test(migrations = "../core/migrations")]
    #[ignore = "requires PostgreSQL with pgvector"]
    async fn binary_embedding_batch_skips_changed_items(pool: PgPool) {
        sqlx::query("insert into items (id, title, bio, author_name, author_uri, is_adult, status, scraped_at, price_jpy, booth_tags, translations)
                     select id, 'ab', 'c', 'creator', 'https://example.test', false, 'ok', now(), 500, '{t1,t2}',
                            case when id = 8 then '{}' else '{\"en\": {\"title\": \"English\"}, \"ko\": {\"title\": \"Korean\"}}' end::jsonb
                     from generate_series(1, 8) as id")
            .execute(&pool).await.unwrap();
        let rows: Vec<EmbedRow> = sqlx::query_as(EMBED_ROWS_SELECT).bind(i64::MAX).bind(8i64).fetch_all(&pool).await.unwrap();
        let (ids, fingerprints): (Vec<_>, Vec<_>) = rows.into_iter().map(|r| (r.0, r.5)).unzip();
        // 1 unchanged; 2 to 5 one input each; 6 only fields outside the text; 7 the title/bio boundary moves; 8 a missing English title becomes ''
        for edit in [
            "update items set title = 'edited during embedding' where id = 2",
            "update items set bio = 'edited during embedding' where id = 3",
            "update items set translations = jsonb_set(translations, '{en,title}', '\"Edited\"') where id = 4",
            "update items set booth_tags = '{t1,t3}' where id = 5",
            "update items set price_jpy = 600, translations = jsonb_set(translations, '{ko,title}', '\"Edited\"') where id = 6",
            "update items set title = 'a', bio = 'bc' where id = 7",
            "update items set translations = '{\"en\": {\"title\": \"\"}}' where id = 8",
        ] {
            sqlx::query(edit).execute(&pool).await.unwrap();
        }
        let values = vec![0.125; DIMS];
        let vectors = vec![HalfVector::from_f32_slice(&values); ids.len()];
        let written = sqlx::query(EMBEDDING_UPDATE).bind(&ids).bind(&vectors).bind(&fingerprints)
            .execute(&pool).await.unwrap().rows_affected();
        assert_eq!(written, 2);
        let stored: Vec<(i64, bool)> = sqlx::query_as("select id, embedding is not null from items order by id").fetch_all(&pool).await.unwrap();
        assert_eq!(stored, [(1, true), (2, false), (3, false), (4, false), (5, false), (6, true), (7, false), (8, false)]);
        let same: bool = sqlx::query_scalar("select bool_and(halfvec_send(embedding) = halfvec_send($1::halfvec)) from items where embedding is not null")
            .bind(&vectors[0]).fetch_one(&pool).await.unwrap();
        assert!(same);
        let empty = sqlx::query(EMBEDDING_UPDATE).bind(Vec::<i64>::new()).bind(Vec::<HalfVector>::new()).bind(Vec::<String>::new())
            .execute(&pool).await.unwrap().rows_affected();
        assert_eq!(empty, 0);
    }

    #[sqlx::test(migrations = "../core/migrations")]
    #[ignore = "requires PostgreSQL with pgvector"]
    async fn enrich_rows_read_into_the_typed_item(pool: PgPool) {
        sqlx::query("insert into items (id, title, bio, author_name, author_uri, is_adult, status, scraped_at, needs_enrichment, enrich_failures, booth_category, price_jpy, booth_tags, variations, translations)
                     values (1, 'Ring', 'desc', 'c', 'https://example.test', false, 'ok', now(), true, 0, '3D', 500, '{VRChat,MA}',
                             '[{\"id\":\"1\",\"name\":\"Full set\",\"price_jpy\":500},{\"id\":\"2\",\"name\":\"\",\"price_jpy\":null}]', '{\"en\":{\"title\":\"Ring\"}}'),
                            (2, 'Bare', '', 'c', 'https://example.test', false, 'ok', now(), true, 2, null, null, '{}', '[]', '{}'),
                            (3, 'Failed thrice', '', 'c', 'https://example.test', false, 'ok', now(), true, 3, null, null, '{}', '[]', '{}'),
                            (4, 'Done', '', 'c', 'https://example.test', false, 'ok', now(), false, 0, null, null, '{}', '[]', '{}')")
            .execute(&pool).await.unwrap();
        let items: Vec<crate::ai::EnrichItem> = sqlx::query_as(ENRICH_ITEMS_SELECT).bind(10i64).bind(boothdex_core::ENRICH_ATTEMPTS).fetch_all(&pool).await.unwrap();
        assert_eq!(items.iter().map(|i| i.id).collect::<Vec<_>>(), [2, 1]);
        let ring = &items[1];
        assert_eq!(ring.match_text(), "Ring VRChat MA Full set ");
        let prompt = crate::ai::enrich_prompt(ring, "cats", &[]);
        assert!(prompt.contains("Listed price: ¥500\nTags: VRChat, MA\nVariations:\n- Full set (¥500)\n-  (¥null)"), "{prompt}");
        let bare = crate::ai::enrich_prompt(&items[0], "cats", &[]);
        assert!(bare.contains("Booth category: \nListed price: ¥null\nTags: \nVariations:\n(none)"), "{bare}");
    }

    #[sqlx::test(migrations = "../core/migrations")]
    #[ignore = "requires PostgreSQL with pgvector"]
    async fn enrich_names_the_listings_avatars_and_keeps_them_in_titles(pool: PgPool) {
        for key in ["AI_BASE_URL", "AI_API_KEY", "AI_MODEL"] {
            assert!(std::env::var(key).ok().is_none_or(|v| v.trim().is_empty()), "unset {key}: this test's provider is a local stand-in");
        }
        crate::install_tls();
        // the stand-in provider records each request and answers with titles that keep the avatar's name as written
        let titles = json!({
            "en": {"title": "[For Manuka] Frill Dress"},
            "ja": {"title": "【マヌカ対応】フリルドレス"},
            "ko": {"title": "【マヌカ対応】프릴 드레스"},
            "zh": {"title": "【マヌカ対応】荷叶边连衣裙"},
        });
        let reply = json!({
            "categories": ["clothing--outfit-dresses"], "content_rating": "SFW", "quest_compatible": "No", "smart_price": 1500,
            "features": [], "translations": titles, "original_language": "ja",
        });
        let requests = std::sync::Arc::new(std::sync::Mutex::new(Vec::<Value>::new()));
        let seen = requests.clone();
        let app = axum::Router::new().fallback(move |axum::Json(body): axum::Json<Value>| {
            seen.lock().unwrap().push(body);
            let content = reply.to_string();
            async move { axum::Json(json!({"choices": [{"message": {"content": content}}]})) }
        });
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base_url = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        sqlx::query("insert into app_settings (key, value) values ('ai', $1)")
            .bind(json!({"base_url": base_url, "model": "stand-in", "api_key": "test"}))
            .execute(&pool).await.unwrap();
        sqlx::query("insert into items (id, title, author_name, author_uri, is_adult, status, scraped_at, needs_enrichment, booth_tags)
                     values (1, '【マヌカ対応】フリルドレス', 'c', 'https://example.test', false, 'ok', now(), true, '{ドレス,VRChat}')")
            .execute(&pool).await.unwrap();
        let job: i64 = sqlx::query_scalar("insert into jobs (kind, status, started_at) values ('enrich', 'running', now()) returning id")
            .fetch_one(&pool).await.unwrap();
        let booth = Booth::new().unwrap();
        let mut ctx = Ctx::new(&pool, &booth, job);
        assert_eq!(enrich(&mut ctx, &json!({})).await.unwrap(), "1 enriched, 0 failed of 1");

        let requests = requests.lock().unwrap().clone();
        assert_eq!(requests.len(), 1);
        let system = requests[0]["messages"][0]["content"].as_str().unwrap();
        for rule in ["never translated or transliterated", "虚拟形象 for an avatar", "3D模型 for a 3D model file", "贴图 for a texture", "法线贴图 for a normal map", "材质 for a material", "着色器 for a shader", "网格 for a mesh"] {
            assert!(system.contains(rule), "{rule}: {system}");
        }
        let user = requests[0]["messages"][1]["content"].as_str().unwrap();
        assert!(user.contains("\nAvatar names (Japanese / romaji): マヌカ / Manuka\nDescription:"), "{user}");
        let (translations, avatars): (Value, Vec<String>) =
            sqlx::query_as("select translations, array(select avatar_slug from item_avatars where item_id = 1) from items where id = 1")
                .fetch_one(&pool).await.unwrap();
        let mut expected = titles.clone();
        expected["original_language"] = json!("ja");
        assert_eq!(translations, expected);
        assert_eq!(avatars, ["manuka"]);
    }

    /// What the stand-in provider answers a tag request (its body, the asked tags in id order): the reply's content, or
    /// a refusal's status and body.
    type TagAnswer = std::result::Result<String, (u16, String)>;

    /// A local stand-in for the AI provider: a taxonomy request gets "中 " before each English name, a tag request what
    /// `answer` gives. Returns every request body it got.
    async fn stand_in_provider(pool: &PgPool, answer: impl Fn(&Value, &[String]) -> TagAnswer + Send + Sync + 'static) -> std::sync::Arc<std::sync::Mutex<Vec<Value>>> {
        use axum::response::IntoResponse;
        for key in ["AI_BASE_URL", "AI_API_KEY", "AI_MODEL"] {
            assert!(std::env::var(key).ok().is_none_or(|v| v.trim().is_empty()), "unset {key}: this test's provider is a local stand-in");
        }
        crate::install_tls();
        let requests = std::sync::Arc::new(std::sync::Mutex::new(Vec::<Value>::new()));
        let seen = requests.clone();
        let answer = std::sync::Arc::new(answer);
        let app = axum::Router::new().fallback(move |axum::Json(body): axum::Json<Value>| {
            seen.lock().unwrap().push(body.clone());
            let reply = if body["messages"][0]["content"] == crate::ai::TAG_SYSTEM {
                answer(&body, &asked_in(&body))
            } else {
                let asked: serde_json::Map<String, Value> = serde_json::from_str(body["messages"][1]["content"].as_str().unwrap()).unwrap();
                Ok(Value::Object(asked.keys().map(|k| (k.clone(), json!(format!("中 {k}")))).collect()).to_string())
            };
            async move {
                match reply {
                    Ok(content) => axum::Json(json!({"choices": [{"message": {"content": content}}]})).into_response(),
                    Err((status, text)) => (axum::http::StatusCode::from_u16(status).unwrap(), text).into_response(),
                }
            }
        });
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base_url = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        sqlx::query("insert into app_settings (key, value) values ('ai', $1)")
            .bind(json!({"base_url": base_url, "model": "stand-in", "api_key": "test"}))
            .execute(pool).await.unwrap();
        requests
    }

    /// The tags a tag request asks for, in id order: the JSON array on the user message's last line.
    fn asked_in(request: &Value) -> Vec<String> {
        let items: Vec<Value> = serde_json::from_str(request["messages"][1]["content"].as_str().unwrap().lines().last().unwrap()).unwrap();
        items.iter().map(|item| item["tag"].as_str().unwrap().to_owned()).collect()
    }

    /// The tags each tag request asked for, request by request, sorted.
    fn asked_tags(requests: &[Value]) -> Vec<Vec<String>> {
        let tag_requests = requests.iter().filter(|r| r["messages"][0]["content"] == crate::ai::TAG_SYSTEM);
        tag_requests
            .map(|r| {
                let mut tags = asked_in(r);
                tags.sort();
                tags
            })
            .collect()
    }

    /// A reply keyed by id with each asked tag's raw JSON answer from `answers` (none for a tag it lacks).
    fn by_id(tags: &[String], answers: &[(&str, &str)]) -> String {
        let entries: Vec<String> =
            tags.iter().enumerate().filter_map(|(i, tag)| answers.iter().find(|(t, _)| t == tag).map(|(_, raw)| format!("\"{}\": {raw}", crate::ai::tag_id(i)))).collect();
        format!("{{{}}}", entries.join(", "))
    }

    async fn localize_job(pool: &PgPool, params: Value) -> String {
        crate::install_tls();
        let job: i64 = sqlx::query_scalar("insert into jobs (kind, status, started_at) values ('localize', 'running', now()) returning id")
            .fetch_one(pool).await.unwrap();
        let booth = Booth::new().unwrap();
        let mut ctx = Ctx::new(pool, &booth, job);
        let summary = localize(&mut ctx, &params).await.unwrap();
        sqlx::query("update jobs set status = 'succeeded' where id = $1").bind(job).execute(pool).await.unwrap();
        summary
    }

    type DictionaryRow = (String, Option<String>, Option<String>, Option<String>, Option<String>, Vec<String>, String);

    async fn dictionary(pool: &PgPool) -> Vec<DictionaryRow> {
        sqlx::query_as("select tag, en, ja, ko, zh, keywords, source from booth_tag_translations order by tag").fetch_all(pool).await.unwrap()
    }

    #[sqlx::test(migrations = "../core/migrations")]
    #[ignore = "requires PostgreSQL with pgvector"]
    async fn localize_gives_missing_tags_a_row_and_keeps_every_row_there(pool: PgPool) {
        // the taxonomy already has its Chinese names
        sqlx::raw_sql("update smart_categories set name_zh = 'x', group_zh = 'x'; update smart_tags set display_zh = 'x';").execute(&pool).await.unwrap();
        // 1-5 listed with six tags (6 delisted with them too); 7-10 listed and 11 delisted with レア; 12-16 with 壊れ
        for (ids, status, tags) in [
            (1..=5, "ok", "{テクスチャ,スカート,マヌカ,マヌカ用,ポジ,ミミ}"),
            (6..=6, "delisted", "{テクスチャ,スカート,マヌカ,マヌカ用,ポジ,ミミ}"),
            (7..=10, "ok", "{レア}"),
            (11..=11, "delisted", "{レア}"),
            (12..=16, "ok", "{壊れ}"),
        ] {
            for id in ids {
                sqlx::query("insert into items (id, title, author_name, author_uri, is_adult, status, scraped_at, likes, booth_tags) values ($1, $2, 'c', 'https://example.test', false, $3, now(), $1, $4::text[])")
                    .bind(id as i64).bind(format!("Item {id}")).bind(status).bind(tags).execute(&pool).await.unwrap();
            }
        }
        sqlx::query("insert into booth_tag_translations (tag, en, ja, ko, zh, source) values ('スカート', 'skirt', 'スカート', '스커트', '裙子', 'import')")
            .execute(&pool).await.unwrap();
        let answers = [
            ("テクスチャ", r#"{"en": "texture", "ja": "テクスチャ", "ko": "텍스처", "zh": "纹理"}"#),
            // the avatar name transliterated in Korean
            ("マヌカ用", r#"{"en": "for Manuka", "ja": "マヌカ用", "ko": "마누카용", "zh": "マヌカ用"}"#),
            ("壊れ", r#"{"en": "broken", "ja": "壊れ", "ko": "고장", "zh": "损坏\n坏了"}"#),
            // the four values as a positional array, and the input structure echoed back with repeated keys
            ("ポジ", r#"["positive", "ポジ", "포지", "正面"]"#),
            ("ミミ", r#"{"id": "t1", "tag": "ミミ", "id": "t1", "context": ["Item 5"]}"#),
        ];
        let requests = stand_in_provider(&pool, move |_, tags| Ok(by_id(tags, &answers))).await;

        // max_tags caps what one run asks for; a run that adds nothing leaves the catalogue version alone
        let version = || async { sqlx::query_scalar::<_, i64>("select version from catalog_meta").fetch_one(&pool).await.unwrap() };
        let before = version().await;
        assert_eq!(localize_job(&pool, json!({"max_tags": 0})).await, "0 names translated to Chinese (0 unique strings); no new tags");
        assert!(requests.lock().unwrap().is_empty());
        assert_eq!(version().await, before);
        let summary = localize_job(&pool, json!({})).await;
        assert_eq!(
            summary,
            "0 names translated to Chinese (0 unique strings); 4 tags added to the dictionary (1 avatar names, 2 with a language left empty), 2 without a valid translation"
        );
        // the added rows reach search with the next snapshot
        assert_eq!(version().await, before + 1);
        let row = |tag: &str, v: [Option<&str>; 4], keywords: &[&str], source: &str| {
            let [en, ja, ko, zh] = v.map(|v| v.map(str::to_owned));
            (tag.to_owned(), en, ja, ko, zh, keywords.iter().map(|k| k.to_string()).collect(), source.to_owned())
        };
        let expected = vec![
            // an avatar's name keeps its romaji in English and the tag elsewhere
            row("マヌカ", [Some("Manuka"), Some("マヌカ"), Some("マヌカ"), Some("マヌカ")], &[], "localize"),
            // a value that fails its checks is left empty, the others kept
            row("マヌカ用", [Some("for Manuka"), Some("マヌカ用"), None, Some("マヌカ用")], &[], "localize"),
            row("壊れ", [Some("broken"), Some("壊れ"), Some("고장"), None], &[], "localize"),
            row("スカート", [Some("skirt"), Some("スカート"), Some("스커트"), Some("裙子")], &[], "import"),
            // Unity's 纹理 is also found as 贴图
            row("テクスチャ", [Some("texture"), Some("テクスチャ"), Some("텍스처"), Some("纹理")], &["贴图"], "localize"),
        ];
        let mut sorted = expected.clone();
        sorted.sort();
        assert_eq!(dictionary(&pool).await, sorted);
        // one request for the batch, its reply held to the schema; no request for the avatar's name, the tag with a row
        // or the tag on four listed items
        let sent = requests.lock().unwrap().clone();
        assert_eq!(asked_tags(&sent), [vec!["テクスチャ", "ポジ", "マヌカ用", "ミミ", "壊れ"]]);
        let format = &sent[0]["response_format"];
        assert_eq!((format["type"].as_str(), format["json_schema"]["strict"].as_bool()), (Some("json_schema"), Some(true)));
        assert_eq!(format["json_schema"]["schema"], crate::ai::tag_schema(5));
        let (names, items) = sent[0]["messages"][1]["content"].as_str().unwrap().split_once('\n').unwrap();
        assert_eq!(names, "Avatar names (Japanese / romaji): マヌカ / Manuka");
        let items: Vec<Value> = serde_json::from_str(items).unwrap();
        let texture = items.iter().find(|i| i["tag"] == "テクスチャ").unwrap();
        assert_eq!(texture["context"], json!(["Item 5", "Item 4", "Item 3"]));
        let system = sent[0]["messages"][0]["content"].as_str().unwrap();
        for rule in ["never translate it or copy it", "never translated or transliterated", "Product, app, tool, brand and shop names stay as written", "白模", "虚拟形象 for an avatar", "纹理 for a texture"] {
            assert!(system.contains(rule), "{rule}: {system}");
        }

        // a tag without any valid value gets no row and is asked again by the next runs, three in all, then left alone
        let failures = || async {
            sqlx::query_as::<_, (String, i32)>("select tag, failures from booth_tag_failures order by tag").fetch_all(&pool).await.unwrap()
        };
        for run in 2..=3 {
            assert_eq!(failures().await, [("ポジ".to_owned(), run - 1), ("ミミ".to_owned(), run - 1)]);
            requests.lock().unwrap().clear();
            let again = localize_job(&pool, json!({})).await;
            assert_eq!(again, "0 names translated to Chinese (0 unique strings); 0 tags added to the dictionary (0 avatar names, 0 with a language left empty), 2 without a valid translation");
            assert_eq!(asked_tags(&requests.lock().unwrap()), [vec!["ポジ", "ミミ"]]);
            assert_eq!(version().await, before + 1);
        }
        requests.lock().unwrap().clear();
        assert_eq!(localize_job(&pool, json!({})).await, "0 names translated to Chinese (0 unique strings); no new tags");
        assert!(requests.lock().unwrap().is_empty());
        assert_eq!(dictionary(&pool).await, sorted);

        // a forced run translates every taxonomy name again and leaves the dictionary alone
        requests.lock().unwrap().clear();
        let forced = localize_job(&pool, json!({"force": true})).await;
        assert!(forced.ends_with(" unique strings)") && !forced.starts_with("0 "), "{forced}");
        assert!(asked_tags(&requests.lock().unwrap()).is_empty());
        assert_eq!(dictionary(&pool).await, sorted);
        let untranslated: i64 = sqlx::query_scalar("select count(*) from smart_categories where name_zh not like '中 %'").fetch_one(&pool).await.unwrap();
        assert_eq!(untranslated, 0);
    }

    #[sqlx::test(migrations = "../core/migrations")]
    #[ignore = "requires PostgreSQL with pgvector"]
    async fn localize_asks_for_plain_json_once_the_provider_refuses_json_schema(pool: PgPool) {
        sqlx::raw_sql("update smart_categories set name_zh = 'x', group_zh = 'x'; update smart_tags set display_zh = 'x';").execute(&pool).await.unwrap();
        // 41 tags on 5 listed items: two batches
        let tags: Vec<String> = (0..=40).map(|n| format!("タグ{n:02}")).collect();
        for id in 1..=5i64 {
            sqlx::query("insert into items (id, title, author_name, author_uri, is_adult, status, scraped_at, booth_tags) values ($1, 'Item', 'c', 'https://example.test', false, 'ok', now(), $2)")
                .bind(id).bind(&tags).execute(&pool).await.unwrap();
        }
        let requests = stand_in_provider(&pool, |body, tags| {
            if body["response_format"]["type"] == "json_schema" {
                return Err((400, r#"{"error": {"message": "response_format type json_schema is not supported"}}"#.into()));
            }
            let answers: Vec<String> = tags.iter().map(|t| format!(r#"{{"en": "tag {0}", "ja": "{t}", "ko": "태그 {0}", "zh": "标签 {0}"}}"#, &t[t.len() - 2..])).collect();
            let answers: Vec<(&str, &str)> = tags.iter().map(String::as_str).zip(answers.iter().map(String::as_str)).collect();
            Ok(by_id(tags, &answers))
        })
        .await;
        let summary = localize_job(&pool, json!({})).await;
        assert_eq!(summary, "0 names translated to Chinese (0 unique strings); 41 tags added to the dictionary (0 avatar names, 0 with a language left empty), 0 without a valid translation");
        // the first batch refused with json_schema and asked again as plain JSON; the second batch plain JSON at once
        let sent = requests.lock().unwrap().clone();
        let asked: Vec<(&str, usize)> = sent.iter().map(|r| (r["response_format"]["type"].as_str().unwrap(), asked_in(r).len())).collect();
        assert_eq!(asked, [("json_schema", 40), ("json_object", 40), ("json_object", 1)]);
        let zh: Option<String> = sqlx::query_scalar("select zh from booth_tag_translations where tag = 'タグ40'").fetch_one(&pool).await.unwrap();
        assert_eq!(zh.as_deref(), Some("标签 40"));
    }

    #[sqlx::test(migrations = "../core/migrations")]
    #[ignore = "requires PostgreSQL with pgvector"]
    async fn localize_stops_when_the_provider_refuses_a_tag_request(pool: PgPool) {
        sqlx::raw_sql("update smart_categories set name_zh = 'x', group_zh = 'x'; update smart_tags set display_zh = 'x';").execute(&pool).await.unwrap();
        for id in 1..=5i64 {
            sqlx::query("insert into items (id, title, author_name, author_uri, is_adult, status, scraped_at, booth_tags) values ($1, 'Item', 'c', 'https://example.test', false, 'ok', now(), '{スカート}')")
                .bind(id).execute(&pool).await.unwrap();
        }
        stand_in_provider(&pool, |_, _| Err((401, "invalid api key".into()))).await;
        let job: i64 = sqlx::query_scalar("insert into jobs (kind, status, started_at) values ('localize', 'running', now()) returning id").fetch_one(&pool).await.unwrap();
        let booth = Booth::new().unwrap();
        let err = localize(&mut Ctx::new(&pool, &booth, job), &json!({})).await.unwrap_err();
        assert!(err.downcast_ref::<crate::ai::ProviderError>().is_some_and(|e| e.0.contains("401")), "{err:#}");
        // the refusal is the provider's, not the tag's: no row and no failed run
        let (rows, failures): (i64, i64) =
            sqlx::query_as("select (select count(*) from booth_tag_translations), (select count(*) from booth_tag_failures)").fetch_one(&pool).await.unwrap();
        assert_eq!((rows, failures), (0, 0));
    }

    #[test]
    fn a_tag_holds_an_avatar_name_in_the_forms_it_writes_and_in_japanese_and_romaji() {
        let forms: std::collections::HashMap<String, Vec<String>> = [("manuka".to_owned(), vec!["Manuka".to_owned(), "マヌカ".to_owned(), "마누카".to_owned()])].into();
        let kit = AvatarKit {
            matcher: crate::avatars::Matcher::new(forms.clone()),
            names: [("manuka".to_owned(), crate::ai::AvatarName { ja: Some("マヌカ".into()), romaji: "Manuka".into() })].into(),
            forms,
            romaji: Default::default(),
        };
        let held = |tag: &str| kit.held_names(tag).into_iter().map(|n| n.0).collect::<Vec<_>>();
        // the Korean name counts only where the tag writes it, so a Korean value transliterating マヌカ is caught
        assert_eq!(held("マヌカ用"), [vec!["マヌカ", "manuka"]]);
        assert_eq!(held("마누카용"), [vec!["마누카", "マヌカ", "manuka"]]);
        assert!(held("スカート").is_empty());
    }

    #[sqlx::test(migrations = "../core/migrations")]
    #[ignore = "requires PostgreSQL with pgvector"]
    async fn localize_without_a_provider_is_skipped(pool: PgPool) {
        for key in ["AI_BASE_URL", "AI_API_KEY", "AI_MODEL"] {
            assert!(std::env::var(key).ok().is_none_or(|v| v.trim().is_empty()), "unset {key}");
        }
        assert_eq!(localize_job(&pool, json!({})).await, "skipped: AI provider not configured");
        // the daily schedule waits a day before its first run
        let (enabled, minutes, later): (bool, i32, bool) =
            sqlx::query_as("select enabled, interval_minutes, next_run_at > now() + interval '23 hours' from job_schedules where kind = 'localize'").fetch_one(&pool).await.unwrap();
        assert_eq!((enabled, minutes, later), (true, 1440, true));
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
