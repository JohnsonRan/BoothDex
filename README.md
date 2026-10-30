# vrcat-successor

Successor to VRCatalogue (closed 2026-11-01). Research, archive and the locked plan live in `../vrcat-research/REPORT.md`.

## Layout
- `crates/core`: shared code; Postgres schema in `migrations/`
- `crates/importer`: seeds Postgres from the VRCatalogue archive

## Dev database (Podman)
```sh
podman run -d --name vrcat-pg -e POSTGRES_PASSWORD=dev -e POSTGRES_DB=catalogue \
  -p 5432:5432 -v vrcat-pgdata:/var/lib/postgresql/data docker.io/pgvector/pgvector:pg17
```

## Seed from the archive
```sh
DATABASE_URL=postgres://postgres:dev@localhost:5432/catalogue \
  cargo run --release -p importer -- ../vrcat-research/data
```
Runs migrations, then upserts everything (safe to rerun). About 90 s for 215k items.
