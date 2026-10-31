<div align="center">

# BoothDex

**A self-hosted search engine for VRChat assets on BOOTH, written in Rust and Next.js.**

**[Deployment guide](docs/DEPLOY.md)** / **[API](#api)** / **[License](#license)**

English / [简体中文](README.zh-CN.md)

[Capabilities](#capabilities) / [Getting started](#getting-started) / [Architecture](#architecture) / [Development](#development)

</div>

> **Early, not yet run as a public site.** Every part works on a development machine, but the full container
> stack has not been run on a real server yet. Expect rough edges.

BoothDex indexes the VRChat items sold on [BOOTH](https://booth.pm) and lets you filter them by the model they
fit, category, tags, price and more. It started as a successor to VRCatalogue, which closed on 2026-11-01, and
keeps its search syntax. It is an independent project, not affiliated with pixiv, BOOTH or VRCatalogue. Item
data and images belong to their creators; every item links back to its BOOTH page.

## Capabilities

- Search: faceted filtering over ~200k items in a few milliseconds by model fit, smart category, BOOTH
  category, tags (include and exclude), features (MA, lilToon, Poiyomi, VRCFury), price, release date, free
  items and adult-content mode. Japanese text search uses Lindera (IPADIC); optional semantic search finds items
  by meaning across languages ("哥特裙" finds ゴスロリ dresses) and merges both rankings.
- Browsing: a model view grouped by gender, an item panel with the full BOOTH gallery, variations,
  translations and image download, and a Ctrl+K palette across items, models, authors and tags.
- Accounts: favourites, saved searches, a remembered model, and RSS feeds for any search.
- Interface: Chinese, English, Japanese and Korean; item titles in the original language or translated;
  currency display; light and dark themes; mobile-first layout.
- Crawling: a worker checks BOOTH hourly for new items, sweeps the full listing daily, re-checks popular
  items and marks delisted ones.
- AI enrichment (optional, any OpenAI-compatible API, configurable reasoning effort): categories, content
  rating, features and title translations (Chinese, English, Japanese, Korean) for new items, model links by name matching, and Chinese names for
  categories and tags. Embeddings for semantic search come from the same provider.
- Images: a proxy re-encodes BOOTH images to WebP and caches them; archived copies cover delisted items.
- Administration: live job progress, schedules, resource usage, AI settings and password resets.

There are no standalone item pages, price history, social logins or password-reset emails (administrators
reset passwords). Item descriptions and variation names are not translated: titles are, for cross-language
search.

## Getting started

Follow the [deployment guide](docs/DEPLOY.md). It runs PostgreSQL, the three Rust services and the website
with Docker or Podman compose, exposes one port for your own reverse proxy, and needs a Linux server with 4 GB
of RAM.

A fresh database already contains the category tree, smart tags and about 380 model bodies. Items come from
BOOTH: run one full crawl ([Starting without a data bundle](docs/DEPLOY.md#starting-without-a-data-bundle)),
then the hourly crawl keeps up. To skip the crawl, restore the [published data bundle](https://github.com/JohnsonRan/BoothDex/releases/tag/data-2026-10-31)
(a 2026-10-31 snapshot with ~216k items and the archived images; about 1.4 GB).

## Architecture

```
browser ─► web (Next.js) ─┬─ /api/*  ─► api (Rust, Axum) ──► PostgreSQL
                          └─ /img/*  ─► imgproxy (Rust) ──► booth.pximg.net, archived images
          worker (Rust) ──► booth.pm, AI provider, PostgreSQL
```

| Path | Contents |
|---|---|
| `crates/core` | Shared code; the PostgreSQL schema in `migrations/`, applied by the services at start |
| `crates/api` | Search API. Filters are roaring bitmaps in memory; the Tantivy text index lives on disk (`INDEX_DIR`) and is reused while the catalogue is unchanged; page payloads come from PostgreSQL |
| `crates/worker` | Job runner: `incremental` (new items), `sweep` (full listing), `details`, `refresh`, `enrich`, `localize` and `embed` (AI). `worker run` is the daemon; `worker once <kind> [params-json]` runs one job |
| `crates/imgproxy` | `/b/{size}/...` serves BOOTH images as WebP; `/v/{sha1}.webp` serves archived images |
| `crates/importer` | One-off import of a VRCatalogue archive directory |
| `web` | Next.js website and the `/admin` console |
| `deploy` | Container images and compose stack |
| `scripts` | Dev stack launcher (Windows and Git Bash), data bundle tools and a mock AI provider |

On a 32-thread development machine with 216k items, searches take 1–6 ms inside the API and 5–40 ms per
request, and 32 concurrent clients get about 550 requests per second. The API uses about 400 MB at rest and
peaks at about 1.2 GB while loading.

## Development

Requirements: Rust (stable), [Bun](https://bun.sh), and PostgreSQL 17 with pgvector (for example in Podman
or Docker).

```sh
podman run -d --name boothdex-pg -e POSTGRES_PASSWORD=dev -e POSTGRES_DB=catalogue \
  -p 5432:5432 -v boothdex-pgdata:/var/lib/postgresql/data docker.io/pgvector/pgvector:pg17

cargo build --release            # the first build downloads the IPADIC dictionary; rerun if that fails
(cd web && bun install)

export DATABASE_URL=postgres://postgres:dev@localhost:5432/catalogue COOKIE_SECURE=0 ADMIN_USERNAMES=admin
target/release/api &             # :3000
target/release/imgproxy &        # :3001
target/release/worker run &
(cd web && bun run dev)          # :3100, forwards /api and /img to the services above
```

Register the account `admin`, open http://localhost:3100/admin, and crawl a few pages to get items:
`target/release/worker once incremental '{"max_pages": 3, "full": true}'`.

On Windows with Git Bash, `scripts/dev-up.sh`, `scripts/dev-down.sh` and `scripts/dev-restart.sh` start and
stop all four services in the background, with logs in `<service>.log`.

To try the AI jobs without a key, run `bun scripts/mock-ai.ts` and set the provider to
`http://127.0.0.1:3555/v1` in the admin console.

With an archive in VRCatalogue's API format (`items/`, `avatars/`, `dict/`, `labels/`), import it with
`DATABASE_URL=... cargo run --release -p importer -- <archive dir>`. Rerunning it is safe; it takes about
90 seconds for 215k items.

Tests:

```sh
cargo test
cd web && bun test src/lib && bunx tsc --noEmit && bunx biome check src
```

## API

`/api/search` takes a VRCatalogue-compatible query string: `v q sort dir page pageSize adult avatars authors
tags extags btags exbtags sc scp bc if free pmin pmax range from to asup gender`. Other endpoints:
`/api/avatars`, `/api/authors`, `/api/tags` and `/api/booth-tags` (`?q=&offset=&limit=`), `/api/categories`,
`/api/items?ids=`, `/api/palette?q=` and `/api/rss?<search query>`.

Differences from VRCatalogue: feature facet counts respect the selected category; Trending is likes decayed
by age; `range` also accepts `7d`, `30d` and `90d`.

## Acknowledgments

- VRCatalogue: the original site, its search model and the taxonomy this project starts from
- [BOOTH](https://booth.pm): item data and images
- [Tantivy](https://github.com/quickwit-oss/tantivy) and [Lindera](https://github.com/lindera/lindera): text search
- [roaring-rs](https://github.com/RoaringBitmap/roaring-rs): filter bitmaps
- [foyer](https://github.com/foyer-rs/foyer): image cache
- [shadcn/ui](https://ui.shadcn.com): interface components

## License

[AGPL-3.0-or-later](LICENSE). If you run a modified version as a public service, you must publish your changes.

```text
SPDX-License-Identifier: AGPL-3.0-or-later
Copyright (c) 2026, BoothDex contributors
```
