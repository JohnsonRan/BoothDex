<div align="center">

<img src="web/src/app/icon.svg" width="104" alt="BoothDex">

# BoothDex

**A self-hosted search engine for VRChat assets on BOOTH, written in Rust and Next.js.**

**[Deployment guide](docs/DEPLOY.md)** / **[API](#api)** / **[License](#license)**

English / [简体中文](README.zh-CN.md) / [日本語](README.ja.md)

[Getting started](#getting-started) / [Capabilities](#capabilities) / [Architecture](#architecture) / [Development](#development)

</div>

BoothDex indexes the VRChat items sold on [BOOTH](https://booth.pm) and lets you filter them by the avatar body
they fit, category, tags, features, price and more. It started as a successor to VRCatalogue (which closed on
2026-11-01) and keeps its search syntax. It is an independent project, not affiliated with pixiv, BOOTH or
VRCatalogue. Item data and images belong to their creators; every item links back to its BOOTH page.

![The home page filtered to the hair category: the filter sidebar beside a carousel of avatar bodies and the item grid](docs/screenshots/en/home.webp)

## Getting started

See the [deployment guide](docs/DEPLOY.md) for full setup, reverse-proxy examples and backup commands. The stack
runs PostgreSQL 17 (`pgvector`), three Rust services and the Next.js site with Docker or Podman Compose, exposes
a single port (`WEB_LISTEN`, default `127.0.0.1:3100`) for your TLS reverse proxy, and needs a Linux server with **4 GB+
RAM** and 2 GB of swap (6 GB recommended).

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
$EDITOR .env    # set POSTGRES_PASSWORD, SITE_URL, WEB_LISTEN, CONTACT_EMAIL, and (with a bundle) ARCHIVE_IMAGES_DIR
docker compose pull && docker compose up -d
```

## Capabilities

### Search

- Filter ~200k items in a few milliseconds by avatar fit, smart category, BOOTH category, tags (include and
  exclude), features (MA, lilToon, Poiyomi, VRCFury), price, release date, free items and content rating.
- The filter row holds the search field and one selector per filter; the filters that do not fit fold behind a
  "+N" button.
- The search field suggests recent searches, avatars, items, shops and tags as you type. `Ctrl+K` (`Cmd+K` on
  macOS) focuses it.
- Each search word is its own token, removable on its own; the words read as an avatar or a category join one
  token, led by the avatar's picture or a category icon. An avatar's token shows the avatar's name.
- A BOOTH item link (`booth.pm/<lang>/items/<id>` or `<shop>.booth.pm/items/<id>`), or an item number of five or
  more digits, puts that item first in whichever view is open: an avatar shows among the items, and an item relisted
  as a new one shows although other searches leave it out. The filters and the adult setting still apply, and a sort
  other than relevance (a search's default) places it like any other result. Such a search is not sent to the AI
  provider and is not searched as translated tags. A number that is no item's id is searched as text.
- Japanese text search uses Lindera (IPADIC); Chinese titles are segmented with its Chinese dictionary (CC-CEDICT), so
  水着 (swimsuit) does not find 水着色器 (water shader). Full-width and half-width forms match each other, and so do hiragana
  and katakana ("しなの" finds シナノ). A Latin query of up to three words that matches no text is tried again over
  titles, tags and author names, allowing one typo in each all-letter word of five or more letters ("shinnano" finds
  Shinano), and the results page says so. A word that no title, tag or author name has, one typo from the Latin name
  of exactly one avatar, is read as that avatar ("shinnano short hair" searches like "shinano short hair"); the
  results page then offers to search the words as typed.
  BOOTH tags also match by their English, Japanese, Korean and Chinese words in a tag dictionary, as whole words
  only ("裙子" finds the items tagged スカート; "hair" does not find a tag translated as "chair"). Descriptions are
  mostly Japanese and only titles are translated, so a query of up to three words is also searched as up to three
  BOOTH tags that translate it: tags carried by the items whose titles match the query, whose
  own title matches largely overlap the query's among the items titled in the query's language ("skirt" also finds
  the items only tagged スカート). Those items rank
  among the word matches at half weight; the results page names the tags and offers to search without them.
  Optional semantic search finds items by meaning across languages ("哥特裙" finds ゴスロリ dresses) and merges both
  rankings. When the AI provider is slow, the results page shows the word matches with a note and reads the search
  once more a moment later; if the matches by meaning are still not ready, it says so and stops. A search it answered also offers up to four related searches above the results: BOOTH tags whose items
  are closest to the query in meaning (ミニスカート and long skirt for "skirt"), each in the site's language when the
  tag dictionary has it, and never one of the query's words or a translation of the query. One replaces the query's
  words and keeps the avatar names and the filters. A search that found nothing offers them under "No items found" once the
  query has been embedded.

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
- A "Back to top" button appears at the bottom end after one screen of scrolling and returns to the top of the
  page; it keeps the filters and results.
- Change your display name and password in Settings; a new password signs your other devices out.
- Leave a message in the guestbook from the top bar without an account: a nickname, up to 500 characters and a
  Cloudflare Turnstile check. An admin opens it by setting the Turnstile keys, hides or deletes messages, and
  closes one as completed or as not planned, which every visitor sees on the message.
- An admin can make sign-in and registration ask for the same Cloudflare Turnstile check.
- An admin can give visitors who haven't passed that check stricter limits on search, the palette, item pages and
  the lookups. A visitor who reaches one is offered the check; passing it restores the usual limits for 10 minutes.
  While sign-in and registration ask for the check, a visitor whose sign-in or registration passed it keeps the usual
  limits.

<details>
<summary>An item's details open over the results</summary>

![The item dialog with the gallery, translated title and Buy on Booth button](docs/screenshots/en/item.webp)

</details>

### Appearance and languages

- The interface is built on Meta's [Astryx](https://github.com/meta/astryx) design system.
- Choose light, dark or system mode, and pure black for dark mode. The choice follows your account when you are
  signed in.
- Reduce motion reduces the site's animations whatever the system says; off, the site follows the system's setting.
  It follows your account too.
- Chinese, English, Japanese and Korean; item titles in the original language or translated; currency
  conversion; a layout that works on phones.

<details>
<summary>Choose the colour mode in Settings</summary>

![Settings, Appearance: the theme modes, pure black and filters at the top](docs/screenshots/en/settings.webp)

</details>

<details>
<summary>The home page on a phone</summary>

<p align="center"><img src="docs/screenshots/phone.webp" width="390" alt="The home page on a phone, with the filters folded behind +8"></p>

</details>

### Crawling, AI and images

- A worker checks BOOTH hourly for new items, sweeps the full listing daily, re-checks popular items and marks
  delisted ones.
- AI enrichment *(optional, any OpenAI-compatible API, configurable reasoning effort)* adds smart categories, content
  rating, features and title translations (Chinese, English, Japanese, Korean; avatar names stay as written; a title
  that drops an avatar name, adds a number or uses another language's script gets one correction and is otherwise not
  written) to new items, links avatars by name, and localizes category and tag names. Each day it also translates the
  BOOTH tags on 5 or more items that the tag dictionary lacks, up to 400 a run, into the four languages; avatar names
  stay as written, and it never changes a dictionary row. A value that fails its checks is left empty, and a tag
  without any valid value, or whose answer names another tag, is asked again on later runs, three runs at most. When
  an update changes the prompt that words the Chinese category and smart tag names, it translates all of them again
  once. Embeddings for semantic search come from the same provider.
- A proxy re-encodes BOOTH images and shop icons to AVIF and caches them; archived copies cover delisted items.

### Administration

- The admin console at `/admin` shows an overview with live status (SSE) and 30-day trend charts.
- Run and stop jobs and follow each job's live log; turn schedules on and off; browse the job history, or from the overview's failures tile the failed runs of the last 30 days that no later run of the same kind made good.
- List the items AI enrichment gave up on after 3 failures, and retry one or all of them.
- Configure the AI provider and open or close registration.
- Find users, see where they are signed in, sign out a device, disable an account or reset its password; a reset or re-enable also lifts a sign-in lockout.
- The console is available in Chinese, English and Japanese.

<details>
<summary>The admin overview</summary>

![The admin overview: worker status, index and activity cards above the trend charts](docs/screenshots/en/admin.webp)

</details>

Standalone item pages, price history, social logins and password-reset emails are intentionally omitted
(administrators reset passwords). Item descriptions and variation names are kept in the original language; only
titles and BOOTH tags are translated for cross-language search.

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
| `crates/api` | Search API. Facets are Roaring bitmaps in memory; the Tantivy text index lives on disk (`INDEX_DIR`) and is reused across restarts; page payloads come from PostgreSQL. Related searches compare the stored query embedding with one centroid per BOOTH tag (the mean embedding of its items), built at startup and carried across reloads for up to 6 hours |
| `crates/worker` | Job runner across 3 lanes: `incremental` (new items), `sweep` (full listing), `details`, `refresh`, `enrich`, `localize` and `embed` (AI). `worker run` is the daemon; `worker once <kind> [params-json]` runs a single job |
| `crates/imgproxy` | `/b/{size}/...` (item images), `/u/{size}/...` (shop icons), `/a/{file}` (BOOTH static images) and `/f/{size}/{uuid}/{file}` (pixiv Factory drawings, scaled down to the size by imgproxy) transcode and cache images as AVIF; `/v/{sha1}.webp` serves archived images |
| `web` | Next.js website and `/admin` console |
| `deploy` | Dockerfiles, `docker-bake.hcl`, `compose.yaml` and `.env.example` |
| `scripts` | Data bundle scripts and mock AI server |

On a 32-thread machine with 216k items, searches take 2–10 ms inside the API and 5–40 ms per request (~550 req/s
across 32 concurrent clients). On Linux containers, `api` uses ~0.4–1 GB RSS steady (plus the
memory-mapped Tantivy index in page cache) and peaks at ~1.8 GB while atomically reloading a new catalogue (`mem_limit: 3g`). The tag centroids take 30 MB for
the 7,396 tags of the development catalogue (an estimated 50 MB for production's ~12,000), one copy across reloads.
Each catalogue keeps up to about 20 MB of the matches of the tags that translate queries.

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

Differences from VRCatalogue: feature facet counts respect the selected category; Trending is likes decayed by
age; `range` also accepts `7d`, `30d` and `90d`; `fields=card` opts into compact card records (bio, variations,
booth_tags, linked_avatars and categories emptied, translations keep only titles,
booth_images/media at most 2; default response unchanged; fetch full records via `/api/items?ids=`).
`without=` (groups of filter names, `+` between groups and `,` within one, at most 8, such as
`without=sc+pmin,pmax`) makes a search that finds nothing also return `widerCounts`: what it finds without each
group, in order. `typoTolerant` is true when the query's words matched nothing as typed and the typo retry
found the results, or when a misspelt word was read as an avatar. `nameTypo` is true when a misspelt word
was read as an avatar; `typo=0` reads no word as a misspelt avatar, while a name spelt right still counts.
`expandedWith` lists the BOOTH tags that translate the query and were searched too, when items only they found are
among the results; `xl=0` searches the words alone. `/api/palette` searches those tags too; `/api/rss` searches the
words alone, so a feed's items do not change. A `q` that is a BOOTH item link, or an item's number of five or
more digits, ranks that item first by relevance in any view, relisted or not, and is not embedded (`semantic` is
`skipped`). `semantic` is `pending` when the AI provider is still embedding the query after the search's 2.5 s
wait: the request runs on and caches the vector, so the same search a moment later uses it. It is `unavailable`
when the provider failed, an embedding rate limit refused the request, or every concurrent provider request
slot stayed busy.

`/api/related` takes the same query string and returns up to four related searches in `searches`, each a `label`
and the `q` that searches it, avatar names of the query kept. They are BOOTH tags whose items' mean embedding is
nearest to the query's, minus tags that respell or translate the query (as the tag dictionary or the expansion
reads it), repeat one of its words, name an avatar, come from a few shops or from more than 5% of the items, seldom
go with the query's own matches, or find nothing under the same filters, adult setting and view. With `hl` (`en`,
`ja`, `ko` or `zh`), `label` is the tag in that language when the tag dictionary has it and no other search or
query word reads the same, and `q` uses that label when its words find items, else the tag. It reads only the
embedding a search already stored for the query and never asks the AI provider, so it returns none for a query
without one, for an item link or number, and while the configured embeddings model is not the one the items were
embedded with.

## Acknowledgments

- VRCatalogue: the original site, its search model and the taxonomy this project starts from
- [BOOTH](https://booth.pm): item data and images
- [Tantivy](https://github.com/quickwit-oss/tantivy) and [Lindera](https://github.com/lindera/lindera): text search
- [CC-CEDICT](https://www.mdbg.net/chinese/dictionary?page=cedict) ([CC BY-SA 4.0](https://creativecommons.org/licenses/by-sa/4.0/)): the Chinese dictionary built into `api`, through Lindera's CC-CEDICT-MeCab, that segments Chinese titles
- [roaring-rs](https://github.com/RoaringBitmap/roaring-rs): filter bitmaps
- [foyer](https://github.com/foyer-rs/foyer): hybrid image cache
- [Astryx](https://github.com/meta/astryx): design system, components and styles

## License

[AGPL-3.0-or-later](LICENSE). If you run a modified version as a public service, you must publish your changes.

```text
SPDX-License-Identifier: AGPL-3.0-or-later
Copyright (c) 2026, BoothDex contributors
```
