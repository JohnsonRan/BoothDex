-- Semantic search: one embedding per listed item (half precision; 1024 dimensions, see core/src/embed.rs).
-- Cleared when the text it was made from changes; the "embed" job fills it.
alter table items add column embedding halfvec(1024);
create index items_embedding_idx on items using hnsw (embedding halfvec_cosine_ops);

alter table jobs drop constraint jobs_kind_check;
alter table jobs add constraint jobs_kind_check check (kind in ('incremental', 'details', 'refresh', 'enrich', 'sweep', 'localize', 'embed'));
alter table job_schedules drop constraint job_schedules_kind_check;
alter table job_schedules add constraint job_schedules_kind_check check (kind in ('incremental', 'details', 'refresh', 'enrich', 'sweep', 'localize', 'embed'));
-- hourly; does nothing until an embeddings model is configured
insert into job_schedules (kind, enabled, interval_minutes, params) values ('embed', true, 60, '{}');
