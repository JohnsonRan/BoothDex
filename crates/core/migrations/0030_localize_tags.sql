-- The localize job also gives new BOOTH tags a dictionary row (crates/worker/src/jobs.rs), so it runs daily; without
-- an AI provider it does nothing. The first run waits a day, so that a dictionary imported right after the deploy
-- comes first and the job asks only for the tags the import lacks.
insert into job_schedules (kind, enabled, interval_minutes, params, next_run_at)
values ('localize', true, 1440, '{}', now() + interval '1 day')
on conflict (kind) do nothing;

-- Tags the job got no valid translation for, with the number of runs that tried. The job stops asking for a tag after
-- a few failed runs (TAG_MAX_FAILURES in jobs.rs); deleting its row lets the next run ask again.
create table if not exists booth_tag_failures (
  tag      text primary key,
  failures integer not null
);
