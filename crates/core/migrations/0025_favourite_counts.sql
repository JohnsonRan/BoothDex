-- Each item's favourites counted in item_stats by a trigger, which also sees the deletes of an account or item
-- cascading to favourites, so refresh_priority no longer counts the whole favourites table on every refresh.
alter table item_stats add column favourites bigint not null default 0;

-- no favourite may be added or removed between the count and the trigger
lock table favourites in share mode;

insert into item_stats (item_id, views, favourites)
select item_id, 0, count(*) from favourites group by item_id
on conflict (item_id) do update set favourites = excluded.favourites;

create function count_favourite() returns trigger language plpgsql as $$
begin
  if tg_op = 'INSERT' then
    insert into item_stats (item_id, views, favourites) values (new.item_id, 0, 1)
    on conflict (item_id) do update set favourites = item_stats.favourites + 1;
  else
    update item_stats set favourites = favourites - 1 where item_id = old.item_id;
  end if;
  return null;
end $$;

create trigger count_favourite after insert or delete on favourites
for each row execute function count_favourite();

create or replace view refresh_priority as
select i.id,
       (1 + ln(1 + i.likes)
          + 3 * ln(1 + coalesce(s.views, 0))
          + 5 * ln(1 + coalesce(s.favourites, 0))
          + case when i.published_at > now() - interval '30 days' then 3 else 0 end)
       * (extract(epoch from now() - i.scraped_at) / 86400.0) as priority
from items i
left join item_stats s on s.item_id = i.id
where i.status = 'ok';
