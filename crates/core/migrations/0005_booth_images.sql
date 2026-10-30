-- Booth image URLs (canonical, without the /c/<size>/ prefix), in gallery order.
-- No FK to items: listing crawls also see items we don't have yet.
create table booth_images (
  item_id    bigint primary key,
  images     text[] not null,
  complete   boolean not null, -- true when taken from items/<id>.json (full gallery); listing pages show at most 4
  fetched_at timestamptz not null default now()
);
