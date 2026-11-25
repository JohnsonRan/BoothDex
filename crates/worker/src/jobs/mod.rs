//! Job implementations. Each returns a one-line summary; progress/cancel go through `Ctx`.

mod avatar_kit;
mod crawl;
mod embed;
mod enrich;
mod localize;
#[cfg(test)]
mod enrich_tests;
#[cfg(test)]
mod localize_extra_tests;
#[cfg(test)]
mod localize_tests;
#[cfg(test)]
mod tests;

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

pub(crate) fn param<T: serde::de::DeserializeOwned>(p: &Value, key: &str, default: T) -> T {
    p.get(key).and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or(default)
}

pub async fn bump_catalog(pool: &PgPool) -> Result<()> {
    link_relisted(pool).await?;
    boothdex_core::bump_catalog_version(pool).await?;
    Ok(())
}

/// Points delisted items at the shop's re-upload (same shop, same title ignoring case and whitespace, newer id,
/// still listed) and clears links that no longer hold. Returns how many items changed.
pub(crate) async fn link_relisted(pool: &PgPool) -> Result<u64> {
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
        Some(JobKind::Embed) => embed::embed(ctx, params).await,
        Some(JobKind::Localize) => localize::localize(ctx, params).await,
        Some(JobKind::Sweep) => crawl::sweep(ctx, params).await,
        Some(JobKind::Incremental) => crawl::incremental(ctx, params).await,
        Some(JobKind::Details) => crawl::details(ctx, params).await,
        Some(JobKind::Refresh) => crawl::refresh(ctx, params).await,
        Some(JobKind::Enrich) => enrich::enrich(ctx, params).await,
        None => bail!("unknown job kind {kind}"),
    }
}
