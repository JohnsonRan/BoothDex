-- The BOOTH tag dictionary: each tag in English, Japanese, Korean and Simplified Chinese. Search matches these words
-- in an item's tags (crates/api/src/tag_words.rs); filters and facets keep the raw tags. A row comes from an import,
-- from the localize job (tags it finds without a row) or from a manual fix; the job never changes a row.
-- keywords: more words search matches for the tag, never shown (a zh 纹理 is also found as 贴图).

create table if not exists booth_tag_translations (
  tag        text primary key,
  en         text,
  ja         text,
  ko         text,
  zh         text,
  keywords   text[] not null default '{}',
  source     text not null check (source in ('import', 'localize', 'manual')),
  updated_at timestamptz not null default now()
);
