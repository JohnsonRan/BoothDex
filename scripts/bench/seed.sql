-- Synthetic data for scripts/bench/run.sh: 200k listed items, 20k accounts, 2,000 of them with 500 viewed items
-- each, every account with 50 favourites.
insert into items (id, title, author_name, author_uri, is_adult, status, scraped_at, published_at, likes)
select g, 't', 'c', 'https://example.test', false, 'ok', now() - (g % 30) * interval '1 day',
       now() - (g % 60) * interval '1 day', g % 500
from generate_series(1, 200000) g;

insert into users (username, email, display_name, password_hash)
select 'u' || g, 'u' || g || '@example.test', 'U', 'x' from generate_series(1, 20000) g;

insert into item_views (user_id, item_id, viewed_at)
select u, (u * 500 + k) % 200000 + 1, now() - k * interval '1 minute'
from generate_series(1, 2000) u, generate_series(1, 500) k;

insert into item_stats (item_id, views) select g, g % 1000 from generate_series(1, 200000, 2) g;

-- counted once afterwards, as migration 0025 does, instead of a million trigger calls
alter table favourites disable trigger count_favourite;
insert into favourites (user_id, item_id, created_at)
select u, (u * 50 + k) * 7 % 200000 + 1, now() - k * interval '1 hour'
from generate_series(1, 20000) u, generate_series(1, 50) k;
alter table favourites enable trigger count_favourite;
insert into item_stats (item_id, views, favourites)
select item_id, 0, count(*) from favourites group by item_id
on conflict (item_id) do update set favourites = excluded.favourites;

vacuum analyze;
