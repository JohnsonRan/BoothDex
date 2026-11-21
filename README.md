<div align="center">

<img src="web/src/app/icon.svg" width="104" alt="BoothDex">

# BoothDex

**A self-hosted search engine for VRChat assets on BOOTH, written in Rust and Next.js.**

**[Deployment guide](docs/DEPLOY.md)** / **[API](#api)** / **[License](#license)**

English / [简体中文](README.zh-CN.md) / [日本語](README.ja.md)

[Getting started](#getting-started) / [Capabilities](#capabilities) / [Documentation](#documentation) / [Architecture](#architecture) / [Development](#development)

</div>

BoothDex indexes the VRChat items sold on [BOOTH](https://booth.pm) and lets you filter them by the avatar body
they fit, category, tags, features, price and more. It started as a successor to VRCatalogue (which closed on
2026-11-01) and keeps its search syntax. It is an independent project, not affiliated with pixiv, BOOTH or
VRCatalogue. Item data and images belong to their creators; every item links back to its BOOTH page.

![The home page filtered to the hair category: the filter sidebar beside a carousel of avatar bodies and the item grid](docs/screenshots/en/home.webp)

## Getting started

BoothDex runs as PostgreSQL 17 (`pgvector`), three Rust services and the Next.js site, under Docker or Podman
Compose. It needs a Linux server with 4 GB or more of RAM and 2 GB of swap (6 GB recommended). The stack exposes one
port, `WEB_LISTEN` (default `127.0.0.1:3100`), for your TLS reverse proxy.

Install with the published data bundle: a 2026-11-04 snapshot of ~217k items with archived images of delisted items
(~2.1 GB). Download it as the [deployment guide](docs/DEPLOY.md#optional-download-the-data-bundle) says, then:

```sh
mkdir -p /srv/boothdex && cd /srv/boothdex
curl -fLO https://raw.githubusercontent.com/JohnsonRan/BoothDex/master/deploy/compose.yaml
curl -fLo .env https://raw.githubusercontent.com/JohnsonRan/BoothDex/master/deploy/.env.example
$EDITOR .env    # set POSTGRES_PASSWORD, SITE_URL, WEB_LISTEN, CONTACT_EMAIL, and (with a bundle) ARCHIVE_IMAGES_DIR
docker compose pull && docker compose up -d postgres
until docker compose exec -T postgres pg_isready -h 127.0.0.1 -U catalogue -d catalogue; do sleep 2; done
docker compose exec -T postgres pg_restore -U catalogue -d catalogue --no-owner --role=catalogue \
  < /srv/boothdex/bundle/catalogue.dump
docker compose up -d
```

Start the stack, the last line, only after `pg_restore` finishes without an error: `api` migrates whatever database it
finds. Without the bundle, leave out the `pg_restore` command and crawl BOOTH, which takes 2–3 days
([Starting without a data bundle](docs/DEPLOY.md#starting-without-a-data-bundle)); a fresh database already holds the
category tree, smart tags and ~380 avatar bodies.

Then open the site through your proxy, register an account, add its name to `ADMIN_USERNAMES` in `.env`, and run
`docker compose up -d api`. The admin console is at `/admin`.

## Capabilities

Search ([details](docs/SEARCH.md)):

- Filters by avatar fit, smart category, BOOTH category, tags (include and exclude), features (MA, lilToon, Poiyomi,
  VRCFury), price, release date, free items and content rating, over ~200k items in a few milliseconds.
- A search field with suggestions for recent searches, avatars, items, shops and tags, and one removable token per word.
- A BOOTH item link or an item number puts that item first.
- Japanese and Chinese text search, matching across full-width and half-width forms and kana, and one typo allowed in a
  Latin word.
- BOOTH tags match by their English, Japanese, Korean and Chinese words, so a query finds items tagged in another
  language.
- Optional semantic search by meaning across languages, and up to four related searches.

Browsing and accounts:

- Pick an avatar body from the carousel to list the items that fit it; the avatar-body view lists the bodies,
  filterable by gender.
- A "Back to top" button after one screen of scrolling, which keeps the filters and results.
- An item opens in a dialog with the full BOOTH gallery, variations, title translations and image download.
- Favourites, recently viewed items, a remembered avatar, and saved searches you can rename and follow by RSS.
- Display name and password in Settings; a new password signs your other devices out.
- A guestbook in the top bar that needs no account (a nickname, up to 500 characters in 10 lines and a
  Cloudflare Turnstile check). An admin hides or deletes messages and closes one as completed or as not planned.
- An admin can ask for the Turnstile check at sign-in and registration, and can give visitors who haven't passed it
  stricter limits ([Turnstile](docs/SERVICES.md#cloudflare-turnstile)).

<details>
<summary>Suggestions for "manuka": the avatar body, then matching items</summary>

![Search suggestions grouped into avatars and items under the search field](docs/screenshots/en/search.webp)

</details>

<details>
<summary>An item's details open over the results</summary>

![The item dialog with the gallery, translated title and Buy on Booth button](docs/screenshots/en/item.webp)

</details>

Appearance and languages:

- Built on Meta's [Astryx](https://github.com/meta/astryx) design system.
- Light, dark or system mode, pure black for dark mode, and reduced motion (off, the site follows the system's
  setting). The choices follow your account.
- Chinese, English, Japanese and Korean; item titles in the original language or translated; currency conversion; a
  layout that works on phones.

<details>
<summary>Choose the colour mode in Settings</summary>

![Settings, Appearance: the theme modes, pure black and filters at the top](docs/screenshots/en/settings.webp)

</details>

<details>
<summary>The home page on a phone</summary>

<p align="center"><img src="docs/screenshots/phone.webp" width="390" alt="The home page on a phone, with the filters folded behind +8"></p>

</details>

Crawling, AI and images ([details](docs/WORKER.md)):

- A worker checks BOOTH hourly for new items, sweeps the full listing daily, re-checks popular items and marks
  delisted ones.
- Optional AI enrichment (any OpenAI-compatible API) adds smart categories, content rating, features, avatar links and
  translated titles, and translates category and tag names. The same provider makes the embeddings for semantic search.
- A proxy re-encodes BOOTH images and shop icons to AVIF and caches them; archived copies cover delisted items.

Administration:

- The `/admin` console shows live status and 30-day trend charts, and runs, stops and schedules jobs with live logs
  and history, including the failed runs of the last 30 days that no later run made good.
- It lists the items AI enrichment gave up on, and retries one or all of them.
- It configures the AI provider and opens or closes registration.
- It finds users, shows where they are signed in, signs out a device, disables an account or resets its password.
- It is available in Chinese, English and Japanese.

<details>
<summary>The admin overview</summary>

![The admin overview: worker status, index and activity cards above the trend charts](docs/screenshots/en/admin.webp)

</details>

Standalone item pages, price history, social logins and password-reset emails are intentionally omitted
(administrators reset passwords). Item descriptions and variation names are kept in the original language; only
titles and BOOTH tags are translated.

## Documentation

| Page | Reader goal |
|---|---|
| [Deployment guide](docs/DEPLOY.md) | Install, configure, update and back up a server |
| [Search behaviour](docs/SEARCH.md) | Know what a search does with its words |
| [API reference](docs/API.md) | Call the HTTP API |
| [Background jobs and AI enrichment](docs/WORKER.md) | Know what the worker runs and what the AI provider does |
| [Service behaviour](docs/SERVICES.md) | Know how the image proxy, rate limits, caching and Turnstile behave |
| [Measurements](docs/MEASUREMENTS.md) | Look up the numbers the documentation quotes |

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
  imgproxy --> asset["asset.booth.pm"]
  imgproxy --> factory["factory.pixiv.net"]
  worker --> booth["booth.pm"]
  worker -. "enrich / embed" .-> ai["OpenAI-compatible API"]
  api -. "query embed" .-> ai
```

### Search & ingestion flow

```mermaid
flowchart TD
  subgraph query["Hybrid Search (/api/search)"]
    q["Query + facet filters"] --> parse["Read item links or numbers,<br/>avatar names & category phrases"]
    parse --> kw["Lindera (IPADIC, CC-CEDICT) + Tantivy<br/>+ tags that translate the query<br/>& Roaring bitmap facets"]
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
| `crates/limit` | The client IP behind the reverse proxy and the per-IP token bucket, shared by `api` and `imgproxy` |
| `crates/api` | Search API. Facets are Roaring bitmaps in memory; the Tantivy text index lives on disk (`INDEX_DIR`) and is reused across restarts; page payloads come from PostgreSQL |
| `crates/worker` | Job runner across 3 lanes: `incremental` (new items), `sweep` (full listing), `details`, `refresh`, `enrich`, `localize` and `embed` (AI). `worker run` is the daemon; `worker once <kind> [params-json]` runs a single job |
| `crates/imgproxy` | `/b/{size}/...` (item images), `/u/{size}/...` (shop icons), `/a/{file}` (BOOTH static images) and `/f/{size}/{uuid}/{file}` (pixiv Factory drawings, scaled down to the size by imgproxy) transcode and cache images as AVIF; `/v/{sha1}.webp` serves archived images |
| `web` | Next.js website and `/admin` console |
| `deploy` | Dockerfiles, `docker-bake.hcl`, `compose.yaml` and `.env.example` |
| `scripts` | Data bundle scripts and mock AI server |

Measured latency and memory are in [Measurements](docs/MEASUREMENTS.md).

## Development

Requirements: Rust (stable), [Bun](https://bun.sh), and PostgreSQL 17 with `pgvector` (via Podman or Docker).

```sh
podman run -d --name boothdex-pg -e POSTGRES_PASSWORD=dev -e POSTGRES_DB=catalogue \
  -p 5432:5432 -v boothdex-pgdata:/var/lib/postgresql/data docker.io/pgvector/pgvector:pg17

cargo build --release            # first build downloads the IPADIC and CC-CEDICT dictionaries; rerun if download fails
# the container images encode AVIF with libaom: --features imgproxy/libavif (needs libavif-dev and pkg-config).
# Without it the encoder is rav1e; with nasm installed, --features imgproxy/asm makes that about three times faster.
(cd web && bun install)

export DATABASE_URL=postgres://postgres:dev@127.0.0.1:5432/catalogue COOKIE_SECURE=0
target/release/api &             # :3000
target/release/imgproxy &        # :3001
target/release/worker run &
(cd web && bun run dev)          # :3100, proxies /api and /img to the services above
```

Register the account `admin` at `http://localhost:3100`, stop `api`, and start it again with the name listed. The API
refuses to register a name that `ADMIN_USERNAMES` lists, so list it only after the account exists:

```sh
ADMIN_USERNAMES=admin target/release/api &   # :3000, after the first api has stopped
```

Open `http://localhost:3100/admin` and crawl a few pages to get items:

```sh
target/release/worker once incremental '{"max_pages": 3, "full": true}'
```

- **Mock AI provider**: run `bun scripts/mock-ai.ts` and set the AI base URL to `http://127.0.0.1:3555/v1` in
  `/admin`, with any API key in the same save, to test enrichment and embeddings without a real key.

Tests and checks:

```sh
cargo test --workspace -- --include-ignored && CARGO_BUILD_WARNINGS=deny cargo clippy --workspace --all-targets -- -D warnings
python3 scripts/check-requirements.py && python3 scripts/test_check_requirements.py
cd web && bun test && bunx next typegen && bunx tsc --noEmit && bunx biome check
```

## API

`/api/search` accepts a VRCatalogue-compatible query string: `v q sort dir page pageSize adult avatars authors
tags extags btags exbtags sc scp bc if free pmin pmax range from to asup gender fields`. Other endpoints:
`/api/avatars`, `/api/authors`, `/api/tags` and `/api/booth-tags` (`?q=&offset=&limit=`), `/api/categories`,
`/api/items?ids=`, `/api/palette?q=`, `/api/related?<search query>` and `/api/rss?<search query>`.
The [API reference](docs/API.md) lists the differences from VRCatalogue, the added parameters and the response fields.

## Acknowledgments

- VRCatalogue: the original site, its search model and the taxonomy this project starts from
- [BOOTH](https://booth.pm): item data and images
- [Tantivy](https://github.com/quickwit-oss/tantivy) and [Lindera](https://github.com/lindera/lindera): text search
- [CC-CEDICT](https://www.mdbg.net/chinese/dictionary?page=cedict) ([CC BY-SA 4.0](https://creativecommons.org/licenses/by-sa/4.0/)): the Chinese dictionary built into `api`, through Lindera's CC-CEDICT-MeCab, that segments Chinese titles
- IPADIC (Nara Institute of Science and Technology): the Japanese dictionary built into `api`. Both dictionaries' licence texts are in [deploy/licenses](deploy/licenses), and the `api` image carries them in `/usr/share/doc/boothdex-api/`
- [roaring-rs](https://github.com/RoaringBitmap/roaring-rs): filter bitmaps
- [foyer](https://github.com/foyer-rs/foyer): hybrid image cache
- [Astryx](https://github.com/meta/astryx): design system, components and styles

## License

[AGPL-3.0-or-later](LICENSE). If you run a modified version as a public service, you must publish your changes.

```text
SPDX-License-Identifier: AGPL-3.0-or-later
Copyright (c) 2026, BoothDex contributors
```
