//! Background worker: runs jobs from the `jobs` queue and `job_schedules`, one at a time per lane (see `lane`):
//! crawls stay serial (polite to Booth) while AI and embedding jobs run alongside them.
//!   worker            daemon: enqueue due schedules, claim queued jobs, heartbeat into worker_status
//!   worker once <kind> [params-json]   run a single job now (still recorded in `jobs`)
//! Job kinds: incremental (new items + image links), details (items missing images), refresh (re-check stalest),
//! enrich (AI; not configured yet). Needs DATABASE_URL.

mod ai;
mod avatars;
mod booth;
mod jobs;

use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

use anyhow::{Context, Result};
use serde_json::Value;
use sqlx::PgPool;

use booth::Booth;

async fn heartbeat(pool: PgPool, booth: Arc<Booth>) {
    let pid = sysinfo::get_current_pid().ok();
    let mut sys = sysinfo::System::new().ok();
    loop {
        let rss = match (&mut sys, pid) {
            (Some(s), Some(pid)) => {
                s.refresh_processes_specifics(sysinfo::ProcessesToUpdate::Some(&[pid]), true, sysinfo::ProcessRefreshKind::nothing().with_memory());
                s.process(pid).map(|p| p.memory() as i64)
            }
            _ => None,
        };
        let c = &booth.counters;
        let r = sqlx::query(
            "update worker_status set heartbeat_at = now(), rss_bytes = $1, booth_requests = $2, booth_errors = $3, booth_rate_limited = $4,
               current_job = (select id from jobs where status in ('running', 'cancelling') order by started_at desc limit 1)",
        )
        .bind(rss)
        .bind(c.requests.load(Ordering::Relaxed) as i64)
        .bind(c.errors.load(Ordering::Relaxed) as i64)
        .bind(c.rate_limited.load(Ordering::Relaxed) as i64)
        .execute(&pool)
        .await;
        if let Err(e) = r {
            eprintln!("heartbeat: {e}");
        }
        tokio::time::sleep(Duration::from_secs(5)).await;
    }
}

async fn execute(pool: &PgPool, booth: &Booth, id: i64, kind: &str, params: &Value, chain: bool) -> Result<()> {
    println!("job {id} {kind} started");
    let mut ctx = jobs::Ctx::new(pool, booth, id);
    let result = jobs::run(&mut ctx, kind, params).await;
    let (ok, summary, error) = match &result {
        Ok(s) => (true, Some(s.clone()), None),
        Err(e) => (false, None, Some(format!("{e:#}"))),
    };
    // one transaction: a continuation shows up the moment its parent ends, so the embed lane never sees a gap
    // without a live enrich job (and the unique index checks per statement, so the insert doesn't collide)
    let mut tx = pool.begin().await?;
    let status: String = sqlx::query_scalar(
        "update jobs set
            status = case when not $2 then 'failed' when status = 'cancelling' then 'cancelled' else 'succeeded' end,
            finished_at = now(),
            message = coalesce($3, message),
            error = $4
         where id = $1
         returning status",
    )
    .bind(id)
    .bind(ok)
    .bind(&summary)
    .bind(&error)
    .fetch_one(&mut *tx)
    .await?;
    if chain && ctx.again && status == "succeeded" {
        sqlx::query("insert into jobs (kind, params, created_by) values ($1, $2, $3) on conflict do nothing")
            .bind(kind)
            .bind(params)
            .bind(format!("job {id}"))
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    ctx.log(&format!("{status}: {}", summary.or(error).unwrap_or_default())).await?;
    println!("job {id} {kind} {status}");
    Ok(())
}

/// Jobs in one lane run one at a time; lanes run side by side. Grouped by the outside service they load.
fn lane(kind: &str) -> &str {
    match kind {
        "incremental" | "details" | "refresh" | "sweep" => "booth",
        "enrich" | "localize" => "ai",
        other => other,
    }
}

async fn daemon(pool: PgPool, booth: Arc<Booth>) -> Result<()> {
    // jobs left running by a previous process can't be resumed
    sqlx::query("update jobs set status = 'failed', finished_at = now(), error = 'worker restarted' where status in ('running', 'cancelling')").execute(&pool).await?;
    sqlx::query("update worker_status set started_at = now(), version = $1").bind(env!("CARGO_PKG_VERSION")).execute(&pool).await?;
    tokio::spawn(heartbeat(pool.clone(), booth.clone()));
    println!("worker running");
    let mut running = tokio::task::JoinSet::new();
    let mut busy: Vec<String> = Vec::new(); // lanes with a job running
    loop {
        // the partial unique index keeps one live job per kind, so a busy kind just skips its tick
        sqlx::query(
            "with due as (update job_schedules set next_run_at = now() + make_interval(mins => interval_minutes)
                          where enabled and next_run_at <= now() returning kind, params)
             insert into jobs (kind, params, created_by) select kind, params, 'schedule' from due on conflict do nothing",
        )
        .execute(&pool)
        .await?;
        // oldest queued job whose lane is free; embed waits until no enrich job is live, because enrichment
        // clears the vectors of the items it touches and they would be embedded twice
        let busy_kinds: Vec<&str> = ["incremental", "details", "refresh", "sweep", "enrich", "localize", "embed"]
            .into_iter()
            .filter(|k| busy.iter().any(|l| l == lane(k)))
            .collect();
        let claimed: Option<(i64, String, Value)> = sqlx::query_as(
            "update jobs set status = 'running', started_at = now()
             where id = (select id from jobs where status = 'queued' and kind <> all($1)
                           and not (kind = 'embed' and exists (select 1 from jobs e where e.kind = 'enrich' and e.status in ('queued', 'running', 'cancelling')))
                         order by created_at limit 1 for update skip locked)
             returning id, kind, params",
        )
        .bind(&busy_kinds)
        .fetch_optional(&pool)
        .await?;
        if let Some((id, kind, params)) = claimed {
            busy.push(lane(&kind).to_owned());
            let (pool, booth) = (pool.clone(), booth.clone());
            running.spawn(async move {
                if let Err(e) = execute(&pool, &booth, id, &kind, &params, true).await {
                    eprintln!("job {id}: {e:#}");
                }
                kind
            });
            continue; // another lane may have work too
        }
        tokio::select! {
            done = running.join_next(), if !running.is_empty() => {
                match done {
                    Some(Ok(kind)) => busy.retain(|l| l != lane(&kind)),
                    // a panicked job leaves its lane marked busy, so stop and let the container restart us
                    Some(Err(e)) => anyhow::bail!("job task failed: {e}"),
                    None => {}
                }
            }
            _ = tokio::time::sleep(Duration::from_secs(5)) => {}
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let pool = PgPool::connect(&std::env::var("DATABASE_URL").context("DATABASE_URL not set")?).await?;
    catalogue_core::MIGRATOR.run(&pool).await?;
    let booth = Arc::new(Booth::new()?);
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        None | Some("run") => daemon(pool, booth).await,
        Some("once") => {
            let kind = args.get(2).context("usage: worker once <kind> [params-json]")?;
            let params: Value = args.get(3).map(|s| serde_json::from_str(s)).transpose()?.unwrap_or_else(|| serde_json::json!({}));
            let id: i64 = sqlx::query_scalar("insert into jobs (kind, params, created_by, status, started_at) values ($1, $2, 'cli', 'running', now()) returning id")
                .bind(kind)
                .bind(&params)
                .fetch_one(&pool)
                .await
                .context("could not create job (is one of this kind already running?)")?;
            execute(&pool, &booth, id, kind, &params, false).await
        }
        Some(other) => anyhow::bail!("unknown command {other}; use `run` or `once <kind>`"),
    }
}
