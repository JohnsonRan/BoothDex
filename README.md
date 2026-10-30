# vrcat-successor

Successor to VRCatalogue (closed 2026-11-01). Research, archive and the locked plan live in `../vrcat-research/REPORT.md`.

## Layout
- `crates/core`: shared code; Postgres schema in `migrations/`
- `crates/importer`: seeds Postgres from the VRCatalogue archive
- `crates/api`: search API (Axum). Loads everything into memory at startup: roaring bitmaps per filter value,
  numeric columns for ranges/sorting, Tantivy (Lindera IPADIC words + bigram substring). Page payloads come from Postgres.

- `crates/crawler`: Booth crawler. `listing` walks search pages (60 items, up to 4 image URLs each);
  `details` fetches `items/<id>.json` for items still missing images (full gallery; 404 = delisted)
- `crates/imgproxy`: image proxy for behind Cloudflare. `/b/{size}/{uuid}/i/{item}/{file}[?format=png|jpeg]`
  re-encodes booth.pximg.net images to WebP (sizes `48x48 72x72_a2_g5 300x300_a2_g5 620x620 orig`),
  `/v/{sha1}.webp` serves archived images of delisted items. foyer memory+disk cache, immutable responses.

Never start a server in the same step as the `cargo build` that replaces its exe (Windows locks it).

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
