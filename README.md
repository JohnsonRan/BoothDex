<div align="center">

<img src="web/src/app/icon.svg" width="104" alt="BoothDex">

# BoothDex

**A self-hosted search engine for VRChat assets on BOOTH, written in Rust and Next.js.**

**[Deployment guide](docs/DEPLOY.md)** / **[API](#api)** / **[License](#license)**

English / [简体中文](README.zh-CN.md) / [日本語](README.ja.md)

[Capabilities](#capabilities) / [Getting started](#getting-started) / [Architecture](#architecture) / [Development](#development)

</div>

> **Early-stage software.** The services and Linux containers work end-to-end, but the stack has not yet been run
> long-term as a public multi-user site. Expect rough edges.

BoothDex indexes the VRChat items sold on [BOOTH](https://booth.pm) and lets you filter them by the avatar body
they fit, category, tags, features, price and more. It started as a successor to VRCatalogue (which closed on
2026-11-01) and keeps its search syntax. It is an independent project, not affiliated with pixiv, BOOTH or
VRCatalogue. Item data and images belong to their creators; every item links back to its BOOTH page.

![The home page filtered to the hair category: the filter row above a carousel of avatar bodies and the item grid](docs/screenshots/en/home.webp)

## Capabilities

### Search

- Filter ~200k items in a few milliseconds by avatar fit, smart category, BOOTH category, tags (include and
  exclude), features (MA, lilToon, Poiyomi, VRCFury), price, release date, free items and content rating.
- The filter row holds the search field and one selector per filter; the filters that do not fit fold behind a
  "+N" button.
- The search field suggests recent searches, avatars, items, shops and tags as you type. `Ctrl+K` (`Cmd+K` on
  macOS) focuses it.
- Each search word is its own token, removable on its own; the words read as an avatar or a category join one
  token, led by the avatar's picture or a category icon.
- Japanese text search uses Lindera (IPADIC). Optional semantic search finds items by meaning across languages
  ("哥特裙" finds ゴスロリ dresses) and merges both rankings.

<details>
<summary>Suggestions for "manuka": the avatar body, then matching items</summary>

![Search suggestions grouped into avatars and items under the search field](docs/screenshots/en/search.webp)

</details>

### Browsing and accounts

- Pick an avatar body from the carousel above the results to list the items that fit it; the avatar-body view
  lists the bodies themselves, filterable by gender.
- An item opens in a dialog with the full BOOTH gallery, variations, title translations and image download.
- Favourites, recently viewed items (marked "Viewed" in results), a remembered avatar, and saved searches as
  saved views that you can rename and follow by RSS.

<details>
<summary>An item's details open over the results</summary>

![The item dialog with the gallery, translated title and Buy on Booth button](docs/screenshots/en/item.webp)

</details>

### Appearance and languages

- The interface is built on Meta's [Astryx](https://github.com/meta/astryx) design system.
- Choose light, dark or system mode and one of 7 Astryx styles: Neutral, Stone, Matcha, Y2K, Gothic, Chocolate
  and Butter. A style loads only when chosen, and the choice follows your account when you are signed in.
- Chinese, English, Japanese and Korean; item titles in the original language or translated; currency
  conversion; a layout that works on phones.

<details>
<summary>Choose the colour mode and the style in Settings</summary>

![Settings, Appearance: the theme modes and the 7 styles](docs/screenshots/en/settings.webp)

</details>

<details>
<summary>The Matcha style in dark mode</summary>

![The home page in the Matcha style, dark mode](docs/screenshots/matcha.webp)

</details>

<details>
<summary>The home page on a phone</summary>

<p align="center"><img src="docs/screenshots/phone.webp" width="390" alt="The home page on a phone, with the filters folded behind +8"></p>

</details>

### Crawling, AI and images

- A worker checks BOOTH hourly for new items, sweeps the full listing daily, re-checks popular items and marks
  delisted ones.
- AI enrichment *(optional, any OpenAI-compatible API, configurable reasoning effort)* adds smart categories,
  content rating, features and title translations (Chinese, English, Japanese, Korean) to new items, links
  avatars by name, and localizes category and tag names. Embeddings for semantic search come from the same
  provider.
- A proxy re-encodes BOOTH images to AVIF and caches them; archived copies cover delisted items.

### Administration

- The admin console at `/admin` shows an overview with live status (SSE) and 30-day trend charts.
- Run and stop jobs and follow each job's live log; turn schedules on and off; browse the job history.
- Configure the AI provider and reset user passwords.
- The console is available in Chinese, English and Japanese.

<details>
<summary>The admin overview</summary>

![The admin overview: worker status, index and activity cards above the trend charts](docs/screenshots/en/admin.webp)

</details>

Standalone item pages, price history, social logins and password-reset emails are intentionally omitted
(administrators reset passwords). Item descriptions and variation names are kept in the original language; only
titles are translated for cross-language search.

## Getting started

See the [deployment guide](docs/DEPLOY.md) for full setup, reverse-proxy examples and backup commands. The stack
runs PostgreSQL 17 (`pgvector`), three Rust services and the Next.js site with Docker or Podman Compose, exposes
a single port (`WEB_LISTEN`, default `3100`) for your TLS reverse proxy, and needs a Linux server with **6 GB+
RAM** (8 GB recommended, or 4 GB with swap).

```mermaid
flowchart LR
  env["1. compose.yaml + .env<br/>(POSTGRES_PASSWORD, SITE_URL)"] --> data{"2. Initial data"}

  data -- "Data bundle (~5 min)" --> bundle["Extract images.tar<br/>pg_restore < catalogue.dump"]
  data -- "Fresh DB (~2–3 days)" --> crawl["Auto-seeded taxonomy & ~380 avatars<br/>worker once incremental (full)"]

  bundle --> up["3. docker compose up -d"]
  crawl --> up
  up --> admin["4. Register account → ADMIN_USERNAMES<br/>Configure optional AI in /admin"]
```

A fresh database already includes the category tree, smart tags and ~380 avatar bodies. To skip the initial
multi-day BOOTH crawl ([Starting without a data bundle](docs/DEPLOY.md#starting-without-a-data-bundle)), restore
the [published data bundle](https://github.com/JohnsonRan/BoothDex/releases/tag/data-2026-11-04) (a 2026-11-04
snapshot with ~217k items and archived delisted images; ~2.1 GB):

```sh
mkdir -p /srv/boothdex && cd /srv/boothdex
curl -fLO https://raw.githubusercontent.com/JohnsonRan/BoothDex/master/deploy/compose.yaml
curl -fLo .env https://raw.githubusercontent.com/JohnsonRan/BoothDex/master/deploy/.env.example
$EDITOR .env    # set POSTGRES_PASSWORD, SITE_URL, WEB_LISTEN, and (with a bundle) ARCHIVE_IMAGES_DIR
docker compose pull && docker compose up -d
```

## Architecture

### Service topology

```mermaid
flowchart LR
  browser["Browser / RSS"] --> web["web (Next.js :3100)<br/>UI & /admin"]

  subgraph stack["BoothDex Services"]
    web -- "/api/*" --> api["api (Rust, Axum :3000)<br/>Roaring bitmaps · Tantivy"]
    web -- "/img/*" --> imgproxy["imgproxy (Rust :3001)<br/>foyer AVIF cache"]
    worker["worker (Rust)<br/>booth · ai · embed lanes"]
    pg[("PostgreSQL 17<br/>+ pgvector")]

    api --> pg
    worker --> pg
    api --- idx[("INDEX_DIR<br/>(Tantivy mmap)")]
    imgproxy --- cache[("imgcache +<br/>archived WebP")]
  end

  imgproxy --> pximg["booth.pximg.net"]
  worker --> booth["booth.pm"]
  worker -. "enrich / embed" .-> ai["OpenAI-compatible API"]
  api -. "query embed" .-> ai
```

### Search & ingestion flow

```mermaid
flowchart TD
  subgraph query["Hybrid Search (/api/search)"]
    q["Query + facet filters"] --> parse["Extract avatar names<br/>& category phrases"]
    parse --> kw["Lindera (IPADIC) + Tantivy<br/>& Roaring bitmap facets"]
    parse -. "if AI embed enabled" .-> embq["Query vector<br/>(RAM → DB cache → AI API, 2.5 s cap)"]
    embq -.-> ann["pgvector HNSW cosine match"]
    kw --> merge["Merge rankings & facet counts"]
    ann -.-> merge
    merge --> hydrate["Load page items from PostgreSQL"]
  end

  subgraph ingest["Background Worker & Hot Reload"]
    crawl_job["booth lane:<br/>incremental · sweep · refresh"] --> dbw[("PostgreSQL")]
    ai_job["ai lane:<br/>enrich (200/batch chain) · localize"] --> dbw
    ai_job -. "chain done" .-> emb_job["embed lane:<br/>1024-d halfvec"]
    emb_job --> dbw
    dbw -- "catalog_meta.version++" --> snap["api builds new Snapshot<br/>& swaps via ArcSwap"]
  end
```

### Repository layout

| Path | Contents |
|---|---|
| `crates/core` | Shared types, BOOTH client and PostgreSQL `migrations/`, applied automatically by services on startup |
| `crates/api` | Search API. Facets are Roaring bitmaps in memory; the Tantivy text index lives on disk (`INDEX_DIR`) and is reused across restarts; page payloads come from PostgreSQL |
| `crates/worker` | Job runner across 3 lanes: `incremental` (new items), `sweep` (full listing), `details`, `refresh`, `enrich`, `localize` and `embed` (AI). `worker run` is the daemon; `worker once <kind> [params-json]` runs a single job |
| `crates/imgproxy` | `/b/{size}/...` transcodes and caches BOOTH images as AVIF; `/v/{sha1}.webp` serves archived images |
| `web` | Next.js website and `/admin` console |
| `deploy` | Dockerfiles, `docker-bake.hcl`, `compose.yaml` and `.env.example` |
| `scripts` | Data bundle scripts and mock AI server |

On a 32-thread machine with 216k items, searches take 1–6 ms inside the API and 5–40 ms per request (~550 req/s
across 32 concurrent clients). On Linux containers, `api` uses ~0.4–1 GB RSS steady (plus the
memory-mapped Tantivy index in page cache) and peaks at ~1.8 GB while atomically reloading a new catalogue (`mem_limit: 4g`).

## Development

Requirements: Rust (stable), [Bun](https://bun.sh), and PostgreSQL 17 with `pgvector` (via Podman or Docker).

```sh
podman run -d --name boothdex-pg -e POSTGRES_PASSWORD=dev -e POSTGRES_DB=catalogue \
  -p 5432:5432 -v boothdex-pgdata:/var/lib/postgresql/data docker.io/pgvector/pgvector:pg17

cargo build --release            # first build downloads the IPADIC dictionary; rerun if download fails
# with nasm installed, add --features imgproxy/asm: AVIF encoding is about three times faster (the container images are built with it)
(cd web && bun install)

export DATABASE_URL=postgres://postgres:dev@localhost:5432/catalogue COOKIE_SECURE=0 ADMIN_USERNAMES=admin
target/release/api &             # :3000
target/release/imgproxy &        # :3001
target/release/worker run &
(cd web && bun run dev)          # :3100, proxies /api and /img to the services above
```

Register the account `admin`, open `http://localhost:3100/admin`, and crawl a few pages to get items:

```sh
target/release/worker once incremental '{"max_pages": 3, "full": true}'
```

- **Mock AI provider**: run `bun scripts/mock-ai.ts` and set the AI base URL to `http://127.0.0.1:3555/v1` in
  `/admin` to test enrichment and embeddings without an API key.

Tests and checks:

```sh
cargo test
cd web && bun test && bunx next typegen && bunx tsc --noEmit && bunx biome check src
```

## API

`/api/search` accepts a VRCatalogue-compatible query string: `v q sort dir page pageSize adult avatars authors
tags extags btags exbtags sc scp bc if free pmin pmax range from to asup gender fields`. Other endpoints:
`/api/avatars`, `/api/authors`, `/api/tags` and `/api/booth-tags` (`?q=&offset=&limit=`), `/api/categories`,
`/api/items?ids=`, `/api/palette?q=` and `/api/rss?<search query>`.

Differences from VRCatalogue: feature facet counts respect the selected category; Trending is likes decayed by
age; `range` also accepts `7d`, `30d` and `90d`; `fields=card` opts into compact card records (bio, variations,
booth_tags, linked_avatars and categories emptied, translations keep only titles,
booth_images/media at most 2; default response unchanged; fetch full records via `/api/items?ids=`).

## Acknowledgments

- VRCatalogue: the original site, its search model and the taxonomy this project starts from
- [BOOTH](https://booth.pm): item data and images
- [Tantivy](https://github.com/quickwit-oss/tantivy) and [Lindera](https://github.com/lindera/lindera): text search
- [roaring-rs](https://github.com/RoaringBitmap/roaring-rs): filter bitmaps
- [foyer](https://github.com/foyer-rs/foyer): hybrid image cache
- [Astryx](https://github.com/meta/astryx): design system, components and styles

## License

[AGPL-3.0-or-later](LICENSE). If you run a modified version as a public service, you must publish your changes.

```text
SPDX-License-Identifier: AGPL-3.0-or-later
Copyright (c) 2026, BoothDex contributors
```
