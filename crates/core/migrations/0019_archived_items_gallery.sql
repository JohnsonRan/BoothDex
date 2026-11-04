-- The gallery an archive row was made for: a changed gallery (relisted, edited, delisted again) is archived again.
-- Rows are now written before imgproxy is called, so copies of an interrupted run are still found and cleaned up.
alter table archived_items add column images text[];
alter table archived_items alter column attempts set default 0;
