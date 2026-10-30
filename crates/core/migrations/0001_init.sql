create extension if not exists pg_trgm;
create extension if not exists vector;

create table smart_categories (
  slug        text primary key,
  parent_slug text references smart_categories (slug),
  name_en     text not null,
  name_ja     text,
  name_ko     text,
  group_en    text,
  group_ja    text,
  group_ko    text,
  mode        text check (mode in ('items', 'avatars')) -- top level only
);

-- master tags; an item's smart tags are derived from booth_tags via aliases at index time
create table smart_tags (
  id      integer primary key,
  name    text not null unique,
  display text not null,
  aliases text[] not null default '{}'
);

create table avatars (
  slug           text primary key,
  name_en        text not null,
  name_ja        text,
  name_ko        text,
  image_url      text,
  avatar_item_id bigint, -- Booth item that sells the avatar itself
  gender         text check (gender in ('female', 'male'))
);

create table items (
  id                 bigint primary key, -- Booth item id
  title              text not null,
  bio                text not null default '',
  author_name        text not null,
  author_uri         text not null,
  author_icon        text,
  booth_category     text,
  price_jpy          integer, -- null for a couple of malformed listings
  smart_price        integer,
  smart_price_reason text,
  likes              integer not null default 0,
  is_adult           boolean not null,
  content_rating     text check (content_rating in ('SFW', 'Suggestive', 'NSFW')),
  quest              text check (quest in ('Yes', 'No', 'Non-Applicable')),
  features           text[] not null default '{}', -- ma, liltoon, poiyomi, vrcfury
  status             text not null check (status in ('ok', 'delisted')),
  published_at       timestamptz,
  scraped_at         timestamptz not null,
  original_language  text,
  media              text[] not null default '{}',
  booth_tags         text[] not null default '{}',
  variations         jsonb not null default '[]',
  translations       jsonb not null default '{}'
);

create table item_avatars (
  item_id     bigint not null references items (id) on delete cascade,
  avatar_slug text not null references avatars (slug),
  primary key (item_id, avatar_slug)
);
create index on item_avatars (avatar_slug);

create table item_categories (
  item_id       bigint not null references items (id) on delete cascade,
  category_slug text not null references smart_categories (slug),
  is_primary    boolean not null,
  primary key (item_id, category_slug)
);
create index on item_categories (category_slug);
