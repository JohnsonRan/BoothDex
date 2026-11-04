delete from jobs where kind = 'archive';
delete from job_schedules where kind = 'archive';

drop table if exists archived_items;

alter table jobs drop constraint jobs_kind_check;
alter table jobs add constraint jobs_kind_check check (kind in ('incremental', 'details', 'refresh', 'enrich', 'sweep', 'localize', 'embed'));
alter table job_schedules drop constraint job_schedules_kind_check;
alter table job_schedules add constraint job_schedules_kind_check check (kind in ('incremental', 'details', 'refresh', 'enrich', 'sweep', 'localize', 'embed'));

-- Restore details schedule to what it was before 0018 only if it matches what 0018 set,
-- leaving an admin-customised schedule alone.
update job_schedules
set enabled = false, interval_minutes = 1440, params = '{}'
where kind = 'details' and enabled and interval_minutes = 60 and params = '{"limit": 1000}';
