-- Embeddings of search texts, so a text searched before (also before a restart) needs no provider request.
-- Holds search texts only, no user or session; the embed job prunes rows unused for 90 days or beyond the newest 50,000.
create table query_embeddings (
  model        text not null,
  query        text not null,  -- normalized search text (see api main.rs)
  embedding    halfvec(1024) not null,
  hits         integer not null default 1,
  created_at   timestamptz not null default now(),
  last_used_at timestamptz not null default now(),
  primary key (model, query)
);
create index on query_embeddings (last_used_at);
