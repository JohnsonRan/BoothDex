-- "details" now also fetches the full gallery of listed items known only from listing pages (at most 4 images),
-- so their delisting can archive every image. Hourly in small batches: ~200k items take about 8 days.
update job_schedules set enabled = true, interval_minutes = 60, params = '{"limit": 1000}' where kind = 'details';
