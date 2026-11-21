-- aliases: more names of an avatar that the worker's matcher (crates/worker/src/avatars.rs) and the search's model
-- names (crates/api/src/snapshot.rs) read, never shown: a short form listings write, each character of a listing that
-- sells several. Avatar names are never translated.

alter table avatars add column if not exists aliases text[] not null default '{}';

-- The avatar names every item has been matched against, by slug: enrichment matched items against these, and
-- VRCatalogue's links in the imported archive stand for them. The worker links every item to names added later and
-- records them here (crates/worker/src/relink.rs). Kept with the catalogue, so a data bundle carries it.
alter table catalog_meta add column if not exists avatar_names jsonb;
update catalog_meta
set avatar_names = (select coalesce(jsonb_object_agg(slug, array_remove(array[name_en, name_ja, name_ko], null) || aliases), '{}') from avatars)
where avatar_names is null;
