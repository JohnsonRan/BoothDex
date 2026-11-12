//! `GET /api/admin/trends`: the overview's charts over the last 30 UTC days, from existing tables only. Per day:
//! the items published on Booth (`items.published_at`) and the items the crawler fetched (`jobs.done` of the
//! details and refresh runs, which count fetched item pages; the listing walks count pages), plus the finished
//! runs by kind and outcome. `items` keeps only its latest scrape time, so refreshes per day come from the jobs.
//! The attention count excludes failures followed by a successful run of the same kind; history stays intact.

use std::sync::Arc;

use axum::Json;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Response};
use serde_json::Value;
use sqlx::PgPool;

use super::require_admin;
use crate::{ApiError, AppState};

const DAYS: i32 = 30;

pub(super) async fn trends(State(s): State<Arc<AppState>>, headers: HeaderMap) -> Result<Response, ApiError> {
    require_admin(&s, &headers).await?;
    Ok(Json(trends_json(&s.pool, DAYS).await?).into_response())
}

/// Both scans filter their column on a range from a constant start (midnight UTC, `days` - 1 days ago), so
/// the planner can estimate it, scan in parallel, and use an index on the column if one is ever added.
async fn trends_json(pool: &PgPool, days: i32) -> Result<Value, sqlx::Error> {
    sqlx::query_scalar(
        "with days as (select (now() at time zone 'utc')::date - g as day from generate_series(0, $1 - 1) g),
         published as (
           select (published_at at time zone 'utc')::date as day, count(*) as n
           from items where published_at >= ((now() at time zone 'utc')::date - ($1 - 1))::timestamp at time zone 'utc' group by 1),
         finished as (
           select kind, status, done, finished_at,
                  max(finished_at) filter (where status = 'succeeded') over (partition by kind) as last_success_at
           from jobs where finished_at >= ((now() at time zone 'utc')::date - ($1 - 1))::timestamp at time zone 'utc'),
         runs as (
           select (finished_at at time zone 'utc')::date as day, kind, status, count(*) as n,
                  coalesce(sum(done) filter (where kind in ('details', 'refresh')), 0) as fetched
           from finished group by 1, 2, 3)
         select jsonb_build_object(
           'unresolved_failures', (select count(*) from finished where status = 'failed'
                                   and (last_success_at is null or finished_at > last_success_at)),
           'days', (select jsonb_agg(jsonb_build_object(
                      'day', d.day,
                      'published', coalesce(p.n, 0),
                      'fetched', coalesce((select sum(r.fetched) from runs r where r.day = d.day), 0)) order by d.day)
                    from days d left join published p using (day)),
           'runs', (select coalesce(jsonb_agg(jsonb_build_object('day', day, 'kind', kind, 'status', status, 'n', n)
                      order by day, kind, status), '[]') from runs))",
    )
    .bind(days)
    .fetch_one(pool)
    .await
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use sqlx::PgPool;

    #[sqlx::test(migrations = "../core/migrations")]
    #[ignore = "requires PostgreSQL with pgvector"]
    async fn trends_count_published_items_fetched_items_and_finished_runs_per_day(pool: PgPool) {
        sqlx::raw_sql(
            "insert into items (id, title, author_name, author_uri, is_adult, status, scraped_at, published_at) values
               (1, 'a', 'c', 'https://example.test', false, 'ok', now(), now()),
               (2, 'b', 'c', 'https://example.test', false, 'ok', now(), now()),
               (3, 'c', 'c', 'https://example.test', false, 'ok', now(), now() - interval '1 day'),
               (4, 'd', 'c', 'https://example.test', false, 'ok', now(), now() - interval '40 days'),
               (5, 'e', 'c', 'https://example.test', false, 'ok', now(), null);
             insert into jobs (kind, status, done, finished_at) values
               ('refresh', 'succeeded', 100, now()),
               ('details', 'failed', 5, now()),
               ('incremental', 'succeeded', 7, now()),
               ('refresh', 'succeeded', 50, now() - interval '40 days');
             insert into jobs (kind, status, done) values ('sweep', 'running', 3);",
        )
        .execute(&pool)
        .await
        .unwrap();

        let v = super::trends_json(&pool, 30).await.unwrap();
        assert_eq!(v["unresolved_failures"], 1);
        let days = v["days"].as_array().unwrap();
        assert_eq!(days.len(), 30);
        let today = &days[29];
        assert_eq!((today["published"].as_i64(), today["fetched"].as_i64()), (Some(2), Some(105)));
        assert_eq!(days[28]["published"], 1);
        // the 40-day-old item and run fall outside the window; the running job has not finished
        assert_eq!(days.iter().map(|d| d["published"].as_i64().unwrap()).sum::<i64>(), 3);
        let day = &today["day"];
        assert_eq!(
            v["runs"],
            json!([
                { "day": day, "kind": "details", "status": "failed", "n": 1 },
                { "day": day, "kind": "incremental", "status": "succeeded", "n": 1 },
                { "day": day, "kind": "refresh", "status": "succeeded", "n": 1 },
            ])
        );

        // A successful retry clears the warning, not its history; a later failure counts again.
        sqlx::query("insert into jobs (kind, status, finished_at) values ('details', 'succeeded', now())").execute(&pool).await.unwrap();
        let recovered = super::trends_json(&pool, 30).await.unwrap();
        assert_eq!(recovered["unresolved_failures"], 0);
        assert!(recovered["runs"].as_array().unwrap().contains(&json!({ "day": day, "kind": "details", "status": "failed", "n": 1 })));
        sqlx::query("insert into jobs (kind, status, finished_at) values ('details', 'failed', now())").execute(&pool).await.unwrap();
        assert_eq!(super::trends_json(&pool, 30).await.unwrap()["unresolved_failures"], 1);
    }
}
