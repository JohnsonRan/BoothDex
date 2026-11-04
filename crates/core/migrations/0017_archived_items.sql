-- Delisted items whose Booth images the "archive" job copied into the imgproxy archive (b/<item_id>/...).
-- saved + missing + failed = the item's Booth images; failed ones (Booth errors) are retried by later runs.
-- Rows go away again (with the copies) when the item is relisted.
create table archived_items (
  item_id     bigint primary key references items (id) on delete cascade,
  saved       integer not null,
  missing     integer not null, -- gone from Booth and from our cache: lost
  failed      integer not null,
  attempts    integer not null default 1,
  archived_at timestamptz not null default now()
);

alter table jobs drop constraint jobs_kind_check;
alter table jobs add constraint jobs_kind_check check (kind in ('incremental', 'details', 'refresh', 'enrich', 'sweep', 'localize', 'embed', 'archive'));
alter table job_schedules drop constraint job_schedules_kind_check;
alter table job_schedules add constraint job_schedules_kind_check check (kind in ('incremental', 'details', 'refresh', 'enrich', 'sweep', 'localize', 'embed', 'archive'));
-- hourly; delistings also queue a run right away
insert into job_schedules (kind, enabled, interval_minutes, params) values ('archive', true, 60, '{}');
