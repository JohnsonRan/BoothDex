-- A delisted item the shop re-uploaded under a new id: the newest listed item of the same shop with the same title
-- (ignoring case and whitespace). Kept up to date by the "archive" job; shown on the old item's page.
alter table items add column relisted_as bigint;
