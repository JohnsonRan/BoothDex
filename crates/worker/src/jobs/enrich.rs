//! AI enrichment job: classification, title translation with one correction round, and avatar linking.

use anyhow::Result;
use serde_json::Value;
use sqlx::PgPool;

use super::avatar_kit::{AvatarKit, load_avatars};
use super::embed::queue_embed;
use super::{Ctx, bump_catalog, param};

pub(super) const ENRICH_ITEMS_SELECT: &str = "select id, title, bio, booth_category, price_jpy, booth_tags, variations, translations
         from items where needs_enrichment and status = 'ok' and enrich_failures < $2 order by id desc limit $1";

/// What one enrich batch shares: the category list for the prompt, the slugs a reply may use and the avatars.
struct EnrichKit {
    category_list: String,
    known: Vec<String>,
    avatars: AvatarKit,
    /// finds the avatars an item fits by their names and aliases; translated titles keep to the names themselves
    links: crate::avatars::Matcher,
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
    let (avatars, links) = load_avatars(pool).await?;
    Ok(EnrichKit { category_list, known, avatars, links })
}

/// One model call for one item. A reply that parsed but failed validation (e.g. a misspelt category slug), or whose
/// translated titles fail their checks (`ai::TitleCheck`), gets one correction round. A title that still fails is not
/// written: the item keeps its earlier title in that language when that one passes. A correction that fails
/// validation leaves the first reply, when that one passed, minus its failing titles.
/// `linked`: the slugs of the avatars the matcher found in the listing.
async fn enrich_reply(ai: &crate::ai::Ai, kit: &EnrichKit, item: &crate::ai::EnrichItem, linked: &[String]) -> Result<crate::ai::Enrichment> {
    let system = crate::ai::ENRICH_SYSTEM;
    let names: Vec<&crate::ai::AvatarName> = linked.iter().filter_map(|slug| kit.avatars.names.get(slug)).collect();
    let prompt = crate::ai::enrich_prompt(item, &kit.category_list, &names);
    let titles = crate::ai::TitleCheck { original: &item.title, names: kit.avatars.held_names(&item.title), japanese: &kit.avatars.japanese };
    let v = ai.json(system, &prompt).await?;
    let first = crate::ai::validate_enrichment(&v, &kit.known, &item.translations);
    let error = match &first {
        Ok(e) => match titles.problems(&e.translations) {
            problems if problems.is_empty() => return first,
            problems => crate::ai::title_problems_message(&problems),
        },
        Err(e) => format!("{e:#}"),
    };
    let v2 = match ai.json_correction(system, &prompt, &v, &error).await {
        Ok(v2) => v2,
        // a correction with no JSON object in it is no valid reply either: the first one still counts when it passed
        Err(e) if first.is_ok() && !e.is::<crate::ai::ProviderError>() => Value::Null,
        Err(e) => return Err(e),
    };
    match (crate::ai::validate_enrichment(&v2, &kit.known, &item.translations), first) {
        (Ok(e), _) | (Err(_), Ok(e)) => Ok(titles.keep_passing(e, &item.translations)),
        (Err(e2), Err(e)) => Err(e2.context(format!("after correction (first: {e:#})"))),
    }
}

/// AI enrichment for items our crawler ingested (needs_enrichment): categories, rating, Quest, smart price,
/// translations and features via the model; avatar links by name matching (see avatars.rs).
pub async fn enrich(ctx: &mut Ctx<'_>, params: &Value) -> Result<String> {
    // avatar names were added (a migration added avatars or aliases): every item is linked to them first, provider or not
    let relinked = match crate::relink::relink(ctx).await? {
        Some(r) if !r.finished => return Ok(format!("cancelled while linking avatars, {} links added", r.added)),
        Some(r) => Some(format!("{} avatar links added", r.added)),
        None => None,
    };
    // queued at startup while the enrich schedule is off (relink::queue): no AI request
    if param(params, "relink_only", false) {
        return Ok(relinked.unwrap_or_else(|| "no avatar names added".into()));
    }
    let relinked = relinked.map(|r| format!("{r}; ")).unwrap_or_default();
    // scheduled hourly by default; without a provider there is nothing to do, which isn't a failure
    let cfg = boothdex_core::settings::ai(ctx.pool).await?;
    let Some(ai) = crate::ai::Ai::from_config(&cfg)? else {
        return Ok(format!("{relinked}skipped: AI provider not configured"));
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
            let linked = kit.links.find(&it.match_text());
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
                sqlx::query("update items set enrich_failures = enrich_failures + 1 where id = $1").bind(it.id).execute(ctx.pool).await?;
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
    Ok(format!("{relinked}{ok} enriched, {failed} failed of {total}"))
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
