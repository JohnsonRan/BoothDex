//! Taxonomy and BOOTH tag localization job.

use std::collections::HashMap;

use anyhow::Result;
use serde_json::Value;
use sqlx::PgPool;

use super::avatar_kit::{AvatarKit, load_avatars};
use super::{Ctx, bump_catalog, param};

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
    let (written, unique, asked, finished) = localize_names(ctx, &ai, force).await?;
    if written > 0 {
        bump_catalog(ctx.pool).await?;
    }
    if force && finished && unique == asked {
        // every name now follows this prompt, so the API stops queuing forced runs for it; a name the reply left out
        // keeps its old wording, so the next start asks again
        sqlx::query("insert into app_settings (key, value) values ('localize_state', $1) on conflict (key) do update set value = excluded.value, updated_at = now()")
            .bind(serde_json::json!({ "prompt_version": boothdex_core::LOCALIZE_PROMPT_VERSION }))
            .execute(ctx.pool)
            .await?;
    }
    let names = format!("{written} names translated to Chinese ({unique} unique strings)");
    if force || !finished {
        return Ok(names);
    }
    let tags = localize_tags(ctx, &ai, param(params, "max_tags", 400i64)).await?;
    Ok(format!("{names}; {tags}"))
}

/// The taxonomy part of `localize`: (rows written, unique strings translated, unique strings asked for, whether it ran
/// to the end).
async fn localize_names(ctx: &mut Ctx<'_>, ai: &crate::ai::Ai, force: bool) -> Result<(u64, usize, usize, bool)> {
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
    let mut zh: HashMap<String, String> = HashMap::new();
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
    Ok((written, zh.len(), texts.len(), finished))
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

type TagRow = (String, [Option<String>; 4]);
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

#[derive(Default)]
struct TagTally {
    added: u64,
    partial: usize,
    failed: usize,
}

async fn localize_tags(ctx: &mut Ctx<'_>, ai: &crate::ai::Ai, max: i64) -> Result<String> {
    let missing: Vec<TagAsk> = sqlx::query_as(MISSING_TAGS_SELECT).bind(max).bind(TAG_MIN_ITEMS).bind(TAG_MAX_FAILURES).fetch_all(ctx.pool).await?;
    if missing.is_empty() {
        return Ok("no new tags".into());
    }
    let (avatars, _) = load_avatars(ctx.pool).await?;
    let romaji = |tag: &str| avatars.romaji.get(&crate::avatars::norm(tag.trim())).cloned();
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
        tally.added, named.len(), tally.partial, tally.failed
    ))
}

async fn ask_tags(ctx: &mut Ctx<'_>, ai: &crate::ai::Ai, avatars: &AvatarKit, asked: Vec<TagAsk>, tally: &mut TagTally) -> Result<()> {
    let total = asked.len();
    let mut schema = true;
    for (n, batch) in asked.chunks(TAG_BATCH).enumerate() {
        let reply = ask_batch(ctx, ai, avatars, batch, &mut schema).await?;
        let (mut rows, mut failed) = (Vec::new(), Vec::new());
        for (i, (tag, titles)) in batch.iter().enumerate() {
            let answer = &reply[crate::ai::tag_id(i)];
            if !answer.is_null() && !crate::ai::answers_tag(answer, tag) {
                ctx.log(&format!("tag {tag}: the answer under its id copies back {} instead; a later run asks again", answer["tag"])).await?;
                failed.push(tag.clone());
                continue;
            }
            let values = crate::ai::tag_values(answer, tag, titles, &avatars.held_names(tag), &avatars.japanese);
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
