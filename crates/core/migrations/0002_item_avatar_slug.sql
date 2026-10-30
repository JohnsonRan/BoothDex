-- set when the item sells an avatar body itself (VRCatalogue "supported avatar")
alter table items add column avatar_slug text references avatars (slug);
create index on items (avatar_slug) where avatar_slug is not null;
