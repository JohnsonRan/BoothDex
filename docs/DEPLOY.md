# Deploying to a Linux server

Everything runs in containers: Postgres (pgvector), the three Rust services and the Next.js site. The site
(`web`) is the only service meant to be reached from outside; it serves one origin and forwards `/api` and `/img`
to the Rust services. TLS is up to you: put your own reverse proxy, Cloudflare Tunnel or similar in front of it.

```
browser ─► your proxy (TLS) ─► web :3100 (Next.js) ─┬─ /api/*  ─► api :3000 ──► postgres
                                                    └─ /img/*  ─► imgproxy :3001 ──► booth.pximg.net, archived images
                                worker (no port) ──► booth.pm, postgres
```

Only `web` publishes a port, on `WEB_LISTEN` (default `3100`, all interfaces). `postgres`, `api` and `imgproxy` are
reachable only on the compose network; for maintenance use `docker compose exec`, e.g.
`docker compose exec postgres psql -U catalogue catalogue`.

## 1. Server

| | Minimum | Why |
|---|---|---|
| RAM | **4 GB** (8 GB comfortable) | api ≈0.5 GB steady, ≈1.6 GB peak while it rebuilds the catalogue after crawls (the search index is memory-mapped from disk); Postgres ≈1 GB; the rest is small |
| CPU | 2 vCPU (4 better) | image transcoding and index rebuilds are CPU-bound |
| Disk | 40 GB SSD | database ≈3 GB (≈4 GB with semantic search), archived images ≈1 GB, image cache 10 GB (configurable), container images, backups |
| OS | Any Linux with Docker or Podman (+ compose) | |

## 2. Get the code and data onto the server

```sh
git clone <repo> /srv/boothdex/app
```

Without a data bundle, skip to step 3 and follow "Starting without a data bundle" in step 4.

A data bundle is a snapshot of an existing instance, made with `scripts/make-bundle.sh <out-dir>` (any instance can make one).

A ready-made bundle is published as the GitHub release [data-2026-10-31](https://github.com/JohnsonRan/BoothDex/releases/tag/data-2026-10-31): a snapshot from 2026-10-31
with 215,911 items, taken from the VRCatalogue archive and the BOOTH crawl that day (mirror: [Filen](https://app.filen.io/#/f/f910e157-1651-42f4-8516-116b7af17c83%236d4a646241516c595a43473546664533504c5a664d4e4c4f7a4d4b48636a745f),
browser download only). The services apply newer migrations at start, and the hourly crawl adds what is newer.

Bundle contents:

| File | What | Size |
|---|---|---|
| `catalogue.dump` | Postgres dump: all items, crawled Booth images, schema. **No user accounts, sessions, favourites, job history or AI settings** — those were test data | ~0.5–1 GB |
| `images.tar` | Archived images of delisted items (`images/<sha1>.webp`). Booth no longer serves these and the original CDN is gone | ~1 GB |
| `SHA256SUMS` | Checksums | |

```sh
# on the server
mkdir -p /srv/boothdex/bundle && cd /srv/boothdex/bundle
# download the published bundle (or copy your own three files here):
for f in catalogue.dump images.tar SHA256SUMS; do
  curl -fLO https://github.com/JohnsonRan/BoothDex/releases/download/data-2026-10-31/$f
done
sha256sum -c SHA256SUMS
tar -xf images.tar -C /srv/boothdex          # -> /srv/boothdex/images/*.webp
mkdir -p /srv/boothdex/app/deploy/restore && mv catalogue.dump /srv/boothdex/app/deploy/restore/
```

The importer (`crates/importer`) is only for a raw VRCatalogue archive; you don't need it here.

## 3. Configure

```sh
cd /srv/boothdex/app/deploy
cp .env.example .env
$EDITOR .env    # WEB_LISTEN, SITE_URL, POSTGRES_PASSWORD, ADMIN_USERNAMES, ARCHIVE_IMAGES_DIR
```

| Variable | Used by | Notes |
|---|---|---|
| `WEB_LISTEN` | web | host `[address:]port` for the site; `127.0.0.1:3100` when the reverse proxy runs on the same host |
| `SITE_URL` | api | the public URL, e.g. `https://<domain>`; appears in RSS links |
| `COOKIE_SECURE` | api | `1` (default): session cookies over HTTPS only. `0` only for testing over plain `http://` |
| `POSTGRES_PASSWORD` | postgres, api, worker | long random string |
| `ADMIN_USERNAMES` | api | comma-separated; applied at every api start (anyone not listed loses admin) |
| `ARCHIVE_IMAGES_DIR` | imgproxy | host path of the extracted images; without a bundle, any empty directory |
| `IMG_CACHE_DISK_MB` / `IMG_CACHE_MEM_MB` | imgproxy | transcoded-image cache |
| `BOOTHDEX_TAG` | api, worker, imgproxy, web | image version on ghcr.io: `latest` (default, follows master), a release such as `0.1.0`, or `sha-<commit>` |
| `AI_API_KEY` / `AI_BASE_URL` / `AI_MODEL` / `AI_REASONING_EFFORT` / `AI_EMBED_MODEL` | api, worker | optional; override the admin console AI settings and lock them there, so the key stays out of the database |

Set in compose and not needed in `.env`: `DATABASE_URL` and `PGPASSWORD` (from `POSTGRES_PASSWORD`; any characters work), `IMGPROXY_URL`, `BIND`.

## 4. First start

```sh
cd /srv/boothdex/app/deploy
docker compose pull                       # prebuilt images from ghcr.io (or `docker compose build` to build from source)
docker compose up -d postgres
# with a data bundle only:
docker compose exec postgres pg_restore -U catalogue -d catalogue --no-owner --role=catalogue /restore/catalogue.dump
docker compose up -d                       # api runs pending migrations itself, then loads the catalogue (~20 s)
docker compose logs -f api                 # wait for "listening on"
```

Then:
1. Open the site (through your proxy, or `http://<server>:3100` with `COOKIE_SECURE=0` for a quick test) and register your account.
2. Put that username in `ADMIN_USERNAMES`, `docker compose up -d api` to apply.
3. Open `/admin`: the worker should show as running; the hourly incremental crawl is on by default.
4. In the admin console: set the AI provider (and test it). The daily full sweep is enabled by default (runs daily at 03:00 JST); you can change its schedule or disable it in the admin console.

### Starting without a data bundle

The migrations create the category tree, smart tags and avatar bodies. Items come from one full crawl of the
Booth `VRChat` tag listing (about 3,400 pages, one item page per new item at ~0.7 s each: expect two to three
days for ~200k items; it is polite to Booth on purpose):

```sh
docker compose exec worker worker once incremental '{"max_pages": 5000, "full": true}'
# interrupted? continue from the page shown in the admin console (a few pages back is fine):
docker compose exec worker worker once incremental '{"max_pages": 5000, "full": true, "start_page": 1200}'
```

The site is usable while it runs (the api reloads as items arrive). Crawled items have no AI categories, avatar
links or translations until the `enrich` job has handled them. It runs hourly (200 items per run) once the AI
provider is configured; at ~200k items, check your provider's price per call first, and change or pause the
schedule in the admin console.

### Semantic search (optional)

Set an embeddings model in the admin console (or `AI_EMBED_MODEL`), served by the same provider. It must be able
to return 1024-dimension vectors through the `dimensions` parameter, as Qwen3-Embedding does; the console's
connection test checks this. The hourly `embed` job then embeds every item without a vector: the first run
covers the whole catalogue (about 70 items per second during development, so roughly an hour for ~200k items;
at the $0.02 per million tokens Qwen3-Embedding-4B cost there, a few dollars in total). After that it only
handles new and edited items.

While items are missing vectors, search finds fewer of them by meaning; nothing else changes.

Each new search text waits for one embeddings request before results appear (on the development provider: median
0.7 s, 95th percentile 1.2 s). The result is kept in the `query_embeddings` table, so a text searched before, by
anyone and across restarts, takes milliseconds. The table stores search texts only (no user or session); the
`embed` job drops entries unused for 90 days and keeps at most 50,000. The chip next to the result count shows
how each search was answered and where the time went. If the provider takes longer than
2.5 s or fails, that search uses words alone and the site says so above the results. Queries that only match by
meaning (Chinese queries for Japanese items, for example) then show nothing until the user searches again. Vectors take about 0.5 GB in the database plus a similar
amount for the index.

## 5. Putting a proxy in front

Forward everything for your domain to the `web` port; it needs no special paths. For example with Caddy on the
host (`WEB_LISTEN=127.0.0.1:3100`):

```
boothdex.example.com {
	encode zstd gzip
	reverse_proxy 127.0.0.1:3100 {
		flush_interval -1
	}
}
```

- Turn off response buffering for `/api/admin/stream` (server-sent events for the admin console's live view);
  `flush_interval -1` does that in Caddy, `proxy_buffering off` in nginx. Without it the live view only updates
  on reload.
- The site itself doesn't compress responses; let the proxy do it (`encode` above, `gzip on` in nginx).
- Docker publishes ports past host firewalls such as ufw. If `WEB_LISTEN` is a bare port, the site is reachable
  from anywhere on it; use `127.0.0.1:3100` when only a local proxy or tunnel should reach it.
- Cloudflare: `/img/*` responses are `Cache-Control: public, max-age=31536000, immutable`, so Cloudflare keeps them;
  nothing else needs a cache rule. Don't enable Rocket Loader or HTML minification (they can break Next.js
  hydration). Server-sent events pass through.

## 6. Operations

| Task | Command |
|---|---|
| Update | `git pull && docker compose pull && docker compose up -d` (from source: `docker compose up -d --build`) |
| Logs | `docker compose logs -f api worker` |
| Restart one service | `docker compose restart worker` |
| Backup database | `docker compose exec postgres pg_dump -U catalogue -Fc catalogue > backup-$(date +%F).dump` (daily, keep off-server) |
| Backup images | the `images` directory never changes except when new delistings are archived; copy it once |
| Restore | stop api/worker, `pg_restore --clean -d catalogue backup.dump`, start them |

Not backed up and fine to lose: the `imgcache` volume (re-fills from Booth), `textindex` (rebuilt at start, ~10 s).

## 7. Before going live — checklist

- [ ] `.env` filled; `POSTGRES_PASSWORD` is not `dev`
- [ ] `ADMIN_USERNAMES` contains only your account
- [ ] `https://<domain>/api/categories` returns JSON; `/img/v/<any sha1>.webp` returns an image
- [ ] admin console shows the worker as running and a successful incremental job within an hour
- [ ] a database backup job is scheduled
- [ ] the AI provider is configured (otherwise newly crawled items stay out of the avatar-fit view and category filters)

## Tested so far

- api, worker and imgproxy images: `/index` and `/cache` exist inside the image, owned by the non-root `app`
  user and writable.
- All four container images build on Linux (Podman): api 156 MB, worker 100 MB, imgproxy 99 MB, web 272 MB;
  the web container serves pages.
  The lindera dictionary download succeeded inside the build.
- The Linux api binary starts (and stops cleanly when the database is unreachable).
- `catalogue.dump` restores into a fresh `pgvector:pg17` with exactly the command in step 4
  (`scripts/test-restore.sh`): 215,911 items, 207,962 image rows, 379 avatars, 0 users, 8 migrations.
- `podman compose config` accepts `compose.yaml`; the Next.js production build passes.

- The Next.js server forwards `/api` and `/img`: JSON with cookies, binary images with their cache headers, and
  server-sent events delivered as they are sent.

Not yet tested: the full stack running together on one server, and a full crawl from an empty database.

## Local development

See `README.md` (`scripts/dev-up.sh` etc.). The dev scripts hard-code the dev database password;
they are not used by the deployment.
