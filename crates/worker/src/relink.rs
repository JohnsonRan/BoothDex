//! Links every item to the avatar names added since the ones its links were made against (`catalog_meta.avatar_names`),
//! so a migration that adds avatars or aliases reaches the whole catalogue once, not only the items enriched after it.

use std::collections::HashMap;

use anyhow::Result;
use sqlx::PgPool;

use crate::avatars::{Matcher, match_text, names};
use crate::jobs::Ctx;

/// Items read per round of `relink`.
const BATCH: i64 = 2000;

const RECORD: &str = "update catalog_meta set avatar_names = $1";
/// One statement, so the API reloads once the work is recorded, also when an earlier run inserted the links.
const RECORD_AND_PUBLISH: &str = "update catalog_meta set avatar_names = $1, version = version + 1, changed_at = now()";

type NamesBySlug = HashMap<String, Vec<String>>;

/// The avatar names now and the recorded ones. Migration 0031 recorded the names it found: enrichment matched items
/// against them, and VRCatalogue's links in the imported archive stand for them. An empty record means no name.
struct Names {
    current: NamesBySlug,
    recorded: NamesBySlug,
}

impl Names {
    async fn read(pool: &PgPool) -> Result<Self> {
        let current = names(pool).await?.into_iter().collect();
        let recorded = serde_json::from_value(sqlx::query_scalar("select coalesce(avatar_names, '{}') from catalog_meta").fetch_one(pool).await?)?;
        Ok(Self { current, recorded })
    }

    /// Per avatar, its names the record lacks.
    fn added(&self) -> Vec<(String, Vec<String>)> {
        self.current
            .iter()
            .map(|(slug, names)| (slug.clone(), names.iter().filter(|n| !self.recorded.get(slug).is_some_and(|r| r.contains(n))).cloned().collect::<Vec<_>>()))
            .filter(|(_, names)| !names.is_empty())
            .collect()
    }
}

pub struct Relinked {
    pub added: u64,
    /// false when the job was cancelled first: nothing is recorded, so the next run matches every item again
    pub finished: bool,
}

/// Links every item, listed or not, to the avatar names the record lacks, then records the names. It only adds links,
/// and only by the added names: links by the earlier names, VRCatalogue's among them, stay as they are. `None` when no
/// name was added.
pub async fn relink(ctx: &mut Ctx<'_>) -> Result<Option<Relinked>> {
    let pool = ctx.pool;
    let names = Names::read(pool).await?;
    let added = names.added();
    if added.is_empty() {
        // names removed, if any: links by them stay until their items are enriched again
        if names.recorded != names.current {
            sqlx::query(RECORD).bind(serde_json::json!(names.current)).execute(pool).await?;
        }
        return Ok(None);
    }
    let matcher = Matcher::new(added);
    let total: i64 = sqlx::query_scalar("select count(*) from items").fetch_one(pool).await?;
    let (mut after, mut done, mut linked) = (0i64, 0i64, 0u64);
    loop {
        let rows: Vec<(i64, String, Vec<String>, Vec<String>)> = sqlx::query_as(
            "select id, title, booth_tags, array(select coalesce(v->>'name', '') from jsonb_array_elements(variations) v)
             from items where id > $1 order by id limit $2",
        )
        .bind(after)
        .bind(BATCH)
        .fetch_all(pool)
        .await?;
        let Some(last) = rows.last() else { break };
        after = last.0;
        done += rows.len() as i64;
        let (ids, slugs): (Vec<i64>, Vec<String>) = rows
            .iter()
            .flat_map(|(id, title, tags, variations)| matcher.find(&match_text(title, tags, variations.iter().map(String::as_str))).into_iter().map(move |slug| (*id, slug)))
            .unzip();
        linked += sqlx::query("insert into item_avatars (item_id, avatar_slug) select * from unnest($1::bigint[], $2::text[]) on conflict do nothing")
            .bind(&ids)
            .bind(&slugs)
            .execute(pool)
            .await?
            .rows_affected();
        if !ctx.progress(done, Some(total), &format!("linking avatars: {done} of {total} items"), false).await? {
            if linked > 0 {
                boothdex_core::bump_catalog_version(pool).await?;
            }
            return Ok(Some(Relinked { added: linked, finished: false }));
        }
    }
    sqlx::query(RECORD_AND_PUBLISH).bind(serde_json::json!(names.current)).execute(pool).await?;
    Ok(Some(Relinked { added: linked, finished: true }))
}

/// Queues `enrich`, which relinks first, when the record lacks a name, so the links do not wait for the schedule: an
/// ordinary run while the enrich schedule is on, since its own tick finds the slot taken and waits a full interval;
/// one with `relink_only`, which makes no AI request, while the schedule is off.
pub async fn queue(pool: &PgPool) -> Result<()> {
    if !Names::read(pool).await?.added().is_empty() {
        sqlx::query(
            "insert into jobs (kind, params, created_by)
             select 'enrich', coalesce((select params from job_schedules where kind = 'enrich' and enabled), '{\"relink_only\": true}'::jsonb), 'avatars'
             on conflict do nothing",
        )
        .execute(pool)
        .await?;
    }
    Ok(())
}

/// Records the avatars table's names as matched, as a finished relink does, so a test of `enrich` starts with no
/// relink pending (migration 0032 adds names to those 0031 recorded).
#[cfg(test)]
pub(crate) async fn record_current(pool: &PgPool) {
    let current: NamesBySlug = names(pool).await.unwrap().into_iter().collect();
    sqlx::query(RECORD).bind(serde_json::json!(current)).execute(pool).await.unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A running enrich job, the previous one finished (one live job per kind).
    async fn job(pool: &PgPool) -> i64 {
        sqlx::query("update jobs set status = 'succeeded', finished_at = now() where status <> 'succeeded'").execute(pool).await.unwrap();
        sqlx::query_scalar("insert into jobs (kind, status, started_at) values ('enrich', 'running', now()) returning id").fetch_one(pool).await.unwrap()
    }

    async fn links(pool: &PgPool) -> Vec<(i64, String)> {
        sqlx::query_as("select item_id, avatar_slug from item_avatars order by 1, 2").fetch_all(pool).await.unwrap()
    }

    #[sqlx::test(migrations = "../core/migrations")]
    #[ignore = "requires PostgreSQL with pgvector"]
    async fn relink_links_every_item_to_the_added_names_once(pool: PgPool) {
        crate::install_tls();
        record_current(&pool).await;
        // names added after the record: an alias of manuka and a new avatar. By them: 1 the alias, 2 the new avatar in a
        // tag of a delisted item, 5 in a variation name. 3 a link VRCatalogue made that the matcher would not; 4 a link
        // by an earlier name; 6 no avatar; 7 an earlier name (ライム) not linked so far
        sqlx::raw_sql(
            "update avatars set aliases = aliases || '{ぬぺぽ}' where slug = 'manuka';
             insert into avatars (slug, name_en, name_ja) values ('test-body', 'Testbody', '試験素体');
             insert into items (id, title, author_name, author_uri, is_adult, status, scraped_at, booth_tags, variations) values
               (1, '【ぬぺぽ対応】服', 's', 'u', false, 'ok', now(), '{}', '[]'),
               (2, 'ドレス', 's', 'u', false, 'delisted', now(), '{試験素体}', '[]'),
               (3, 'ドレス', 's', 'u', false, 'ok', now(), '{}', '[]'),
               (4, 'マヌカ用 髪', 's', 'u', false, 'ok', now(), '{}', '[]'),
               (5, 'セット', 's', 'u', false, 'ok', now(), '{}', '[{\"name\": \"Testbody\"}, {}]'),
               (6, 'nothing', 's', 'u', false, 'ok', now(), '{}', '[]'),
               (7, 'ライム用 髪', 's', 'u', false, 'ok', now(), '{}', '[]');
             insert into item_avatars (item_id, avatar_slug) values (3, 'shinano'), (4, 'manuka');",
        )
        .execute(&pool)
        .await
        .unwrap();
        let booth = crate::booth::Booth::new().unwrap();
        let version = || async { sqlx::query_scalar::<_, i64>("select version from catalog_meta").fetch_one(&pool).await.unwrap() };
        let recorded = || async { Names::read(&pool).await.unwrap().added() };
        let run = || async { relink(&mut Ctx::new(&pool, &booth, job(&pool).await)).await.unwrap().map(|r| (r.added, r.finished)) };
        let before = version().await;
        assert_eq!(run().await, Some((3, true)));
        let expected = [(1, "manuka"), (2, "test-body"), (3, "shinano"), (4, "manuka"), (5, "test-body")];
        assert_eq!(links(&pool).await, expected.map(|(i, s)| (i, s.to_owned())));
        assert_eq!(version().await, before + 1);
        assert!(recorded().await.is_empty());
        // linked to these names already
        assert_eq!(run().await, None);

        // a new alias: every item again, by that alias only (7 stays unlinked to lime)
        sqlx::query("update avatars set aliases = aliases || '{nothing}' where slug = 'manuka'").execute(&pool).await.unwrap();
        assert_eq!(run().await, Some((1, true)));
        assert_eq!(links(&pool).await.len(), 6);
        assert!(links(&pool).await.contains(&(6, "manuka".to_owned())));

        // cancelled after a batch: its links count and show, nothing is recorded; the next run finds them in place and
        // still records and publishes the work
        sqlx::query("update avatars set aliases = aliases || '{ドレス}' where slug = 'shinano'").execute(&pool).await.unwrap();
        let cancelled = job(&pool).await;
        sqlx::query("update jobs set status = 'cancelling' where id = $1").bind(cancelled).execute(&pool).await.unwrap();
        let before = version().await;
        let r = relink(&mut Ctx::new(&pool, &booth, cancelled)).await.unwrap().unwrap();
        assert_eq!((r.added, r.finished), (1, false));
        assert_eq!(version().await, before + 1);
        assert_eq!(recorded().await.len(), 1);
        assert_eq!(run().await, Some((0, true)));
        assert_eq!(version().await, before + 2);

        // an alias removed: nothing to link, its links stay, and the record follows the names
        sqlx::query("update avatars set aliases = array_remove(aliases, 'nothing') where slug = 'manuka'").execute(&pool).await.unwrap();
        assert_eq!(run().await, None);
        assert!(links(&pool).await.contains(&(6, "manuka".to_owned())));
        let names = Names::read(&pool).await.unwrap();
        assert_eq!(names.recorded, names.current);
        // an empty record (DEPLOY.md's way to match again): every name, so 7 links to lime too
        sqlx::query("update catalog_meta set avatar_names = '{}'").execute(&pool).await.unwrap();
        assert!(run().await.is_some_and(|(_, finished)| finished));
        assert!(links(&pool).await.contains(&(7, "lime".to_owned())));
    }

    #[sqlx::test(migrations = "../core/migrations")]
    #[ignore = "requires PostgreSQL with pgvector"]
    async fn migration_0031_records_the_starter_names_as_matched(pool: PgPool) {
        let recorded = Names::read(&pool).await.unwrap().recorded;
        assert_eq!(recorded.len(), 379);
        assert_eq!(recorded["kumaly"], ["Kumaly", "ひきくまりのクマリ", "ひきくまりのクマリ"]);
    }

    #[sqlx::test(migrations = "../core/migrations")]
    #[ignore = "requires PostgreSQL with pgvector"]
    async fn a_relink_is_queued_when_names_were_added(pool: PgPool) {
        let queued = || async {
            let params: Vec<serde_json::Value> = sqlx::query_scalar("select params from jobs where kind = 'enrich' and status = 'queued'").fetch_all(&pool).await.unwrap();
            sqlx::query("delete from jobs").execute(&pool).await.unwrap();
            params
        };
        let params = serde_json::json!({ "batch": 17, "concurrency": 2 });
        let schedule = |on: bool| sqlx::query("update job_schedules set enabled = $1, params = $2 where kind = 'enrich'").bind(on).bind(&params).execute(&pool);
        record_current(&pool).await;
        queue(&pool).await.unwrap();
        assert!(queued().await.is_empty());
        // a name added: relink only while the enrich schedule is off, an ordinary enrich while it is on
        sqlx::query("update avatars set aliases = aliases || '{nothing}' where slug = 'manuka'").execute(&pool).await.unwrap();
        schedule(false).await.unwrap();
        queue(&pool).await.unwrap();
        assert_eq!(queued().await, [serde_json::json!({ "relink_only": true })]);
        schedule(true).await.unwrap();
        queue(&pool).await.unwrap();
        assert_eq!(queued().await, [params]);
    }
}
