-- AI enrichment runs hourly by default; the job does nothing until an AI provider is configured.
update job_schedules set enabled = true where kind = 'enrich';
