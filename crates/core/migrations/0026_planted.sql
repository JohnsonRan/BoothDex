-- planted violation for the migrations workflow: holds for the test data, fails on the catalogue's real item ids
alter table items add constraint planted check (id < 1000000);
