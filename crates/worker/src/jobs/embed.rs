//! Semantic-search embedding job, model-change invalidation and query-embedding cache pruning.

use std::time::Duration;

use anyhow::Result;
use serde_json::Value;
use sqlx::PgPool;

use super::{Ctx, param};

/// Hash of the text an item's embedding is made from, over unqualified `items` columns: the embed job takes it when it
/// reads an item and writes the vector only while it still matches. A macro, so the queries stay static strings.
macro_rules! embedding_fingerprint {
    () => {
        "md5(concat_ws(E'\\x1f', title, bio, translations->'en'->>'title', array_to_string(booth_tags, E'\\x1e')))"
    };
}

/// (id, title, booth_tags, translations.en.title, bio, input_fingerprint)
pub(super) type EmbedRow = (i64, String, Vec<String>, Option<String>, String, String);

pub(super) const EMBED_ROWS_SELECT: &str = concat!(
    "select id, title, booth_tags, translations->'en'->>'title', bio,
                    ",
    embedding_fingerprint!(),
    "
             from items
             where status = 'ok' and embedding is null and id < $1 order by id desc limit $2"
);

pub(super) const EMBEDDING_UPDATE: &str = concat!(
    "update items i set embedding = v.e
    from unnest($1::bigint[], $2::halfvec[], $3::text[]) as v(id, e, fp)
    where i.id = v.id and ",
    embedding_fingerprint!(),
    " = v.fp"
);

pub(super) async fn queue_embed(pool: &PgPool) -> Result<()> {
    sqlx::query(
        "insert into jobs (kind, params, created_by) select 'embed', '{}', 'enrich'
         where exists (select 1 from items where status = 'ok' and not needs_enrichment and embedding is null)
         on conflict do nothing",
    )
    .execute(pool)
    .await?;
    Ok(())
}

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
        let written = sqlx::query(EMBEDDING_UPDATE).bind(&ids).bind(&vecs).bind(&fps).execute(ctx.pool).await?.rows_affected();
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
