create table users (
  id            bigserial primary key,
  username      text not null,
  email         text not null,
  display_name  text not null,
  password_hash text not null,
  preferences   jsonb not null default '{}',
  created_at    timestamptz not null default now()
);
create unique index on users (lower(username));
create unique index on users (lower(email));

create table sessions (
  token_hash bytea primary key, -- sha256 of the cookie token; the token itself is never stored
  user_id    bigint not null references users (id) on delete cascade,
  created_at timestamptz not null default now(),
  expires_at timestamptz not null,
  user_agent text,
  ip         text
);
create index on sessions (user_id);

create table favourites (
  user_id    bigint not null references users (id) on delete cascade,
  item_id    bigint not null references items (id) on delete cascade,
  created_at timestamptz not null default now(),
  primary key (user_id, item_id)
);

-- named searches shown in the account; each also has a stateless RSS URL (/api/rss?<query>)
create table saved_searches (
  id         bigserial primary key,
  user_id    bigint not null references users (id) on delete cascade,
  name       text not null,
  query      text not null,
  created_at timestamptz not null default now()
);
create index on saved_searches (user_id);
