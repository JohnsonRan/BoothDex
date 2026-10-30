-- Chinese names for the taxonomy (VRCatalogue only shipped en/ja/ko); filled by the AI "localize" job
alter table smart_categories add column name_zh text, add column group_zh text;
alter table smart_tags add column display_zh text;

alter table jobs drop constraint jobs_kind_check;
alter table jobs add constraint jobs_kind_check check (kind in ('incremental', 'details', 'refresh', 'enrich', 'sweep', 'localize'));
alter table job_schedules drop constraint job_schedules_kind_check;
alter table job_schedules add constraint job_schedules_kind_check check (kind in ('incremental', 'details', 'refresh', 'enrich', 'sweep', 'localize'));
