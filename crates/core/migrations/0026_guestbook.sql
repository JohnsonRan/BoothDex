-- The guestbook: anonymous messages behind a Turnstile check (crates/api/src/guestbook.rs).

create table guestbook (
  id bigint generated always as identity primary key,
  nickname text not null check (char_length(nickname) between 1 and 24),
  body text not null check (char_length(body) between 1 and 500),
  created_at timestamptz not null default now(),
  hidden_at timestamptz
);

create index guestbook_shown on guestbook (id desc) where hidden_at is null;
