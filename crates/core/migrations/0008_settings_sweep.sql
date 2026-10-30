-- admin-editable settings (e.g. the AI endpoint); secrets are never sent back to the browser in full
create table app_settings (
  key        text primary key,
  value      jsonb not null,
  updated_at timestamptz not null default now()
);

-- last time an item appeared on a Booth listing page (sweep job); lets a full sweep spot delisting candidates
alter table items add column last_listed_at timestamptz;

alter table jobs drop constraint jobs_kind_check;
alter table jobs add constraint jobs_kind_check check (kind in ('incremental', 'details', 'refresh', 'enrich', 'sweep'));
alter table job_schedules drop constraint job_schedules_kind_check;
alter table job_schedules add constraint job_schedules_kind_check check (kind in ('incremental', 'details', 'refresh', 'enrich', 'sweep'));
-- off until the admin turns it on
insert into job_schedules (kind, enabled, interval_minutes, params) values ('sweep', false, 1440, '{}');
