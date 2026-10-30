-- avatar bodies VRCatalogue listed as "supported"; a superset of avatar_slug is not null
-- (a few supported items have no known avatar slug)
alter table items add column supported boolean not null default false;
-- avatar body gender, per item
alter table items add column gender text check (gender in ('female', 'male'));
