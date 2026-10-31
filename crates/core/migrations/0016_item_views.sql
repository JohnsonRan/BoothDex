-- items a signed-in user opened, newest first; the api keeps the newest 500 per user
create table item_views (
  user_id   bigint not null references users (id) on delete cascade,
  item_id   bigint not null references items (id) on delete cascade,
  viewed_at timestamptz not null default now(),
  primary key (user_id, item_id)
);
create index on item_views (user_id, viewed_at desc);
