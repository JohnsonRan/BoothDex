# vrcat-successor

Successor to VRCatalogue (closed 2026-11-01). Research, archive and the locked plan live in `../vrcat-research/REPORT.md`.

## Layout
- `crates/core`: shared code; Postgres schema in `migrations/`
- `crates/importer`: seeds Postgres from the VRCatalogue archive
- `crates/api`: search API (Axum). Loads everything into memory at startup: roaring bitmaps per filter value,
  numeric columns for ranges/sorting; Tantivy (Lindera IPADIC words + bigram substring) lives on disk in
  `INDEX_DIR` (default `./textindex`, memory-mapped, reused when the catalogue is unchanged). Page payloads come from Postgres.

- `crates/worker`: background job runner (replaces the old one-off crawler). `worker` runs as a daemon: enqueues
  due `job_schedules`, claims `jobs` one at a time, heartbeats into `worker_status`. `worker once <kind> [params]`
  runs one job now. Kinds: `incremental` (newest listing pages -> image links + ingest unknown items from
  `/en/items/<id>.json`, stops when caught up; hourly by default), `details` (items missing images), `refresh`
  (re-check stalest items), `enrich` (AI, not configured yet). Writes bump `catalog_meta.version`.
  Items ingested by the worker have `needs_enrichment = true` (no AI categories/avatars/translations yet).
- `crates/imgproxy`: image proxy for behind Cloudflare. `/b/{size}/{uuid}/i/{item}/{file}[?format=png|jpeg]`
  re-encodes booth.pximg.net images to WebP (sizes `48x48 72x72_a2_g5 300x300_a2_g5 620x620 orig`),
  `/v/{sha1}.webp` serves archived images of delisted items. foyer memory+disk cache, immutable responses.

Never start a server in the same step as the `cargo build` that replaces its exe (Windows locks it).

## Deployment
See [docs/DEPLOY.md](docs/DEPLOY.md): containers (`deploy/`), data bundle (`scripts/make-bundle.sh`), env checklist.

## Local dev stack
```sh
scripts/dev-up.sh               # api :3000, imgproxy :3001, worker, web :3100 — detached, returns at once
scripts/dev-up.sh api           # just one
scripts/dev-down.sh
```
Logs go to `<service>.log`. Startup: api ~20 s (loads and indexes the catalogue), others ~1 s.
Admin console: http://localhost:3100/admin (admins come from `ADMIN_USERNAMES`).

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

## Run the API
```sh
DATABASE_URL=postgres://postgres:dev@localhost:5432/catalogue cargo run --release -p api   # BIND defaults to 127.0.0.1:3000
```
Endpoints: `/api/search` (VRCatalogue-compatible query string: `v q sort dir page pageSize adult avatars authors tags extags
btags exbtags sc scp bc if free pmin pmax range from to asup gender`), `/api/avatars|authors|tags|booth-tags?q=&offset=&limit=`,
`/api/categories`.

The first build downloads the IPADIC dictionary in `lindera-ipadic`'s build script; if it fails, rerun the build.

Parity with the live site is checked by `bun parity.ts` in `vrcat-research` (fixture captured before closure).
Deliberate deviations: feature facet counts respect the selected category (live kept them global); Trending is
likes decayed by age (live formula unknown); `range` also accepts `7d`, `30d`, `90d`.
