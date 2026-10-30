-- daily full listing sweep on by default (prices + delisting check for every listed item); run at a quiet hour
update job_schedules
set enabled = true,
    interval_minutes = 1440,
    next_run_at = date_trunc('day', now() at time zone 'UTC') + interval '1 day 18 hours' -- 03:00 JST
where kind = 'sweep';
