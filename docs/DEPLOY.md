# Deploying to a Linux server

Everything runs in containers: Postgres (pgvector), the three Rust services, the Next.js site and Caddy.
Caddy terminates TLS and serves one origin; Cloudflare can sit in front.

```
browser ─► Cloudflare ─► Caddy :443 ─┬─ /api/*  ─► api :3000 ──► postgres
                                     ├─ /img/*  ─► imgproxy :3001 ──► booth.pximg.net, archived images
                                     └─ /*      ─► web :3100 (Next.js)
                        worker (no port) ──► booth.pm, postgres
```

## 1. Server

| | Minimum | Why |
|---|---|---|
| RAM | **4 GB** (8 GB comfortable) | api ≈0.5 GB steady, ≈1.6 GB peak while it rebuilds the catalogue after crawls (the search index is memory-mapped from disk); Postgres ≈1 GB; the rest is small |
| CPU | 2 vCPU (4 better) | image transcoding and index rebuilds are CPU-bound |
| Disk | 40 GB SSD | database ≈3 GB, archived images ≈1 GB, image cache 10 GB (configurable), container images, backups |
| OS | Any Linux with Docker or Podman (+ compose) | |

## 2. Get the code and data onto the server

```sh
git clone <repo> /srv/boothdex/app
```

Without a data bundle, skip to step 3 and follow "Starting without a data bundle" in step 4.

A data bundle is a snapshot of an existing instance, made with `scripts/make-bundle.sh <out-dir>`:

| File | What | Size |
|---|---|---|
| `catalogue.dump` | Postgres dump: all items, crawled Booth images, schema. **No user accounts, sessions, favourites, job history or AI settings** — those were test data | ~0.5–1 GB |
| `images.tar` | Archived images of delisted items (`images/<sha1>.webp`). Booth no longer serves these and the original CDN is gone | ~1 GB |
| `SHA256SUMS` | Checksums | |

```sh
# on the server
git clone <repo> /srv/catalogue/app
mkdir -p /srv/catalogue/bundle && cd /srv/catalogue/bundle
# copy the three bundle files here (scp / rsync), then:
sha256sum -c SHA256SUMS
tar -xf images.tar -C /srv/catalogue          # -> /srv/catalogue/images/*.webp
mkdir -p /srv/catalogue/app/deploy/restore && mv catalogue.dump /srv/catalogue/app/deploy/restore/
```

The importer (`crates/importer`) is only for rebuilding from the raw VRCatalogue archive; you don't need it here.

## 3. Configure

```sh
cd /srv/catalogue/app/deploy
cp .env.example .env
$EDITOR .env    # SITE_DOMAIN, SITE_URL, POSTGRES_PASSWORD, ADMIN_USERNAMES, ARCHIVE_IMAGES_DIR
```

| Variable | Used by | Notes |
|---|---|---|
| `SITE_DOMAIN` | caddy | DNS A/AAAA record must point at the server (or at Cloudflare, proxied) |
| `SITE_URL` | api | `https://<domain>`; appears in RSS links |
| `POSTGRES_PASSWORD` | postgres, api, worker | long random string |
| `ADMIN_USERNAMES` | api | comma-separated; applied at every api start (anyone not listed loses admin) |
| `ARCHIVE_IMAGES_DIR` | imgproxy | host path of the extracted images; without a bundle, any empty directory |
| `IMG_CACHE_DISK_MB` / `IMG_CACHE_MEM_MB` | imgproxy | transcoded-image cache |
| `AI_API_KEY` / `AI_BASE_URL` / `AI_MODEL` / `AI_REASONING_EFFORT` | api, worker | optional; override the admin console AI settings and lock them there, so the key stays out of the database |

Set in compose and not needed in `.env`: `DATABASE_URL`, `IMGPROXY_URL`, `COOKIE_SECURE=1` (session cookie only over HTTPS), `BIND`.

## 4. First start

```sh
cd /srv/catalogue/app/deploy
docker compose build                      # first build takes a while; rebuild if lindera-ipadic fails to download
# with a data bundle only:
docker compose up -d postgres
docker compose exec postgres pg_restore -U catalogue -d catalogue --no-owner --role=catalogue /restore/catalogue.dump
docker compose up -d                       # api runs pending migrations itself, then loads the catalogue (~20 s)
docker compose logs -f api                 # wait for "listening on"
```

Then:
1. Open `https://<domain>`, register your account.
2. Put that username in `ADMIN_USERNAMES`, `docker compose up -d api` to apply.
3. Open `https://<domain>/admin`: the worker should show as running; the hourly incremental crawl is on by default.
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
links or translations until the `enrich` job runs, so configure the AI provider and enable the `enrich`
schedule in the admin console; at ~200k items, check your provider's price per call first.


## 5. Cloudflare

- DNS record proxied (orange cloud), SSL mode **Full (strict)** (Caddy has a real certificate).
- Caching: `/img/*` responses are `Cache-Control: public, max-age=31536000, immutable`, so Cloudflare keeps them. Nothing else needs a cache rule.
- Don't enable "Rocket Loader" or HTML minification (they can break the Next.js hydration).
- The admin live view uses server-sent events on `/api/admin/stream`; Cloudflare passes them through.

## 6. Operations

| Task | Command |
|---|---|
| Update | `git pull && docker compose up -d --build` |
| Logs | `docker compose logs -f api worker` |
| Restart one service | `docker compose restart worker` |
| Backup database | `docker compose exec postgres pg_dump -U catalogue -Fc catalogue > backup-$(date +%F).dump` (daily, keep off-server) |
| Backup images | the `images` directory never changes except when new delistings are archived; copy it once |
| Restore | stop api/worker, `pg_restore --clean -d catalogue backup.dump`, start them |

Not backed up and fine to lose: the `imgcache` volume (re-fills from Booth), `textindex` (rebuilt at start, ~10 s) and `caddydata` (certificates are re-issued).

## 7. Before going live — checklist

- [ ] `.env` filled; `POSTGRES_PASSWORD` is not `dev`
- [ ] `ADMIN_USERNAMES` contains only your account (the dev `admin` account is not in the bundle)
- [ ] `https://<domain>/api/categories` returns JSON; `/img/v/<any sha1>.webp` returns an image
- [ ] admin console shows the worker as running and a successful incremental job within an hour
- [ ] a database backup job is scheduled
- [ ] the AI provider is configured (otherwise newly crawled items stay out of the avatar-fit view and category filters)

## What was verified before handover (2026-10-31, on the dev machine)

- Re-checked after the review fixes: api, worker and imgproxy images rebuilt from the current Dockerfile;
  inside the image `/index` and `/cache` exist, are owned by the non-root `app` user and are writable.

- All four container images build on Linux (Podman): api 156 MB, worker 100 MB, imgproxy 99 MB, web 272 MB;
  the web container serves pages.
  The lindera dictionary download succeeded inside the build.
- The Linux api binary starts (and stops cleanly when the database is unreachable).
- `catalogue.dump` restores into a fresh `pgvector:pg17` with exactly the command in step 4
  (`scripts/test-restore.sh`): 215,911 items, 207,962 image rows, 379 avatars, 0 users, 8 migrations.
- `podman compose config` accepts `compose.yaml`; the Next.js production build passes.

Not verified: the full stack running together (the dev VM has 2 GB RAM, less than the api needs), Caddy
obtaining a certificate, Cloudflare in front.

## Local development

See `README.md` (`scripts/dev-up.sh` etc.). The dev scripts hard-code the dev database password and a test admin;
they are not used by the deployment.
