-- A guestbook message's state, as an issue has one: open until an admin closes it as completed or as not planned
-- (crates/api/src/admin/guestbook.rs). Messages posted before this column are open.

alter table guestbook
  add column status text not null default 'open' check (status in ('open', 'completed', 'not_planned'));
