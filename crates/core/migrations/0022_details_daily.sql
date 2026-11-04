-- "details" (fetch missing galleries) on by default, once a day. Only touches the untouched default schedule,
-- leaving an admin-customised one alone.
update job_schedules set enabled = true
where kind = 'details' and not enabled and interval_minutes = 1440 and params = '{}';
