-- our own popularity signal: item detail opens (palette picks open the detail too)
create table item_stats (
  item_id        bigint primary key references items (id) on delete cascade,
  views          bigint not null default 0,
  last_viewed_at timestamptz not null default now()
);

-- Refresh priority = heat x staleness. Heat starts from Booth signals (likes, recency) and is dominated by our
-- own views/favourites once there is traffic. Cold items still rise as they get older.
create view refresh_priority as
select i.id,
       (1 + ln(1 + i.likes)
          + 3 * ln(1 + coalesce(s.views, 0))
          + 5 * ln(1 + coalesce(f.n, 0))
          + case when i.published_at > now() - interval '30 days' then 3 else 0 end)
       * (extract(epoch from now() - i.scraped_at) / 86400.0) as priority
from items i
left join item_stats s on s.item_id = i.id
left join (select item_id, count(*) as n from favourites group by item_id) f on f.item_id = i.id
where i.status = 'ok';
