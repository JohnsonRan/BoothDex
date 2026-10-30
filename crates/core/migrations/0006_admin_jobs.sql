alter table users add column is_admin boolean not null default false;

-- items ingested by our own crawler still lack VRCatalogue's AI fields (categories, avatars, translations…)
alter table items add column needs_enrichment boolean not null default false;
create index on items (needs_enrichment) where needs_enrichment;

-- bumped by the worker after writes; the API reloads its in-memory catalogue when it changes
create table catalog_meta (
  id         integer primary key default 1 check (id = 1),
  version    bigint not null default 0,
  changed_at timestamptz not null default now()
);
insert into catalog_meta default values;

create table jobs (
  id          bigserial primary key,
  kind        text not null check (kind in ('incremental', 'details', 'refresh', 'enrich')),
  status      text not null default 'queued' check (status in ('queued', 'running', 'cancelling', 'cancelled', 'succeeded', 'failed')),
  params      jsonb not null default '{}',
  done        bigint not null default 0,
  total       bigint,
  message     text,             -- latest human-readable progress line
  error       text,
  log         text not null default '', -- bounded tail, newest last
  created_by  text not null default 'schedule',
  created_at  timestamptz not null default now(),
  started_at  timestamptz,
  finished_at timestamptz
);
create index on jobs (status, created_at);
-- at most one live job per kind
create unique index jobs_one_live_per_kind on jobs (kind) where status in ('queued', 'running', 'cancelling');

create table job_schedules (
  kind             text primary key check (kind in ('incremental', 'details', 'refresh', 'enrich')),
  enabled          boolean not null default false,
  interval_minutes integer not null check (interval_minutes between 5 and 10080),
  params           jsonb not null default '{}',
  next_run_at      timestamptz not null default now()
);
insert into job_schedules (kind, enabled, interval_minutes, params) values
  ('incremental', true, 60, '{"max_pages": 40}'),
  ('refresh', false, 60, '{"batch": 1500}'),
  ('details', false, 1440, '{}'),
  ('enrich', false, 60, '{"batch": 200}');

-- single-row heartbeat the worker updates every few seconds
create table worker_status (
  id                integer primary key default 1 check (id = 1),
  heartbeat_at      timestamptz,
  started_at        timestamptz,
  current_job       bigint,
  booth_requests    bigint not null default 0,
  booth_errors      bigint not null default 0,
  booth_rate_limited bigint not null default 0,
  rss_bytes         bigint,
  version           text
);
insert into worker_status default values;
