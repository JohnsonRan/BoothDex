# REQUIREMENTS.md: numbers, gates and exceptions

Commands, numbers, gates and exceptions for this repository, and its deferred options and open
questions. The rules are in the `AGENTS.md` files and are not repeated here. When another file in
this repository disagrees with this one on a command, a number, a gate or an exception, this file
wins.

`scripts/check-requirements.py` checks this file in the merge gate; its docstring lists what it
checks. Write a command in backticks, and a pipe inside a cell as `\|`.

## Commands

One row per tier in `.github/AGENTS.md`, "Tiers, jobs and gates". Write the exact command that a
person and CI both run. Fill in this order: the command; then three runs on the same host, recorded
as the baseline; then the limit, as the baseline plus a margin (start at 20%).

| Tier | Command | Runs (where, when) | Wall-time limit | Baseline (date, host, load) |
| --- | --- | --- | --- | --- |
| Merge gate | `cargo test --workspace -- --include-ignored` with `DATABASE_URL` set as in the README, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test -p imgproxy --features libavif`; in `web/`: `bun test`, `bunx next typegen`, `bunx tsc --noEmit`, `bunx biome check`; `python3 scripts/check-requirements.py` | CI: `.github/workflows/requirements.yml` and `.github/workflows/test.yml` on every pull request, both required to merge into master; on a push to master, requirements.yml always and test.yml through `.github/workflows/images.yml` | 3.5 min | 174 s, 188 s, 154 s, slowest job `rust` (2026-11-11, GitHub-hosted ubuntu-latest, load not recorded) |
| Merge gate, when a migration changes | `cargo build -p worker`; with `PODMAN=docker WORKER_BIN=target/debug/worker` set: `scripts/test-restore-selftest.sh`, then the public data bundle's `catalogue.dump` checked against its `SHA256SUMS` and `scripts/test-restore.sh bundle` | CI: `.github/workflows/migrations.yml` on a pull request that changes `crates/core/migrations`, `scripts/test-restore.sh` or the workflow; not required to merge, because a path-filtered required check stays pending on every other pull request | 9 min | 435 s, 340 s, 368 s, of which the self-test 13 s and restoring the bundle about 5 min (2026-11-12, GitHub-hosted ubuntu-latest, load not recorded) |
| Release | `docker buildx bake -f deploy/docker-bake.hcl` and `docker build -f deploy/web.Dockerfile .`, after the merge gate | CI: `.github/workflows/images.yml` on every `v*` tag and on a push to master that changes a path in its `paths` filter; pushes the images to ghcr.io | 7 min | 280 s, 394 s, 344 s, merge gate included (2026-11-11, GitHub-hosted ubuntu-24.04, load not recorded) |

`python3 scripts/test_check_requirements.py` runs with the requirements check. Point `DATABASE_URL`
at `127.0.0.1`, not `localhost`: on Windows `localhost` tries IPv6 first, and every new connection to
the Postgres container then waits about 2 s, which made a local run of the Rust suite take 104 s
instead of 4 s.

## Numbers

Every limit has a measured baseline next to it. Record the date, the host and its load for each
baseline. Leave the limit empty until it has been measured, and list it under "Not yet enforced"
meanwhile.

| Item | Limit | Baseline (date, host, load) | Enforced by |
| --- | --- | --- | --- |
| Type scale (role: size / line height) | the Astryx type tokens; no other font size or line height | a setting of the design system, not measured | design system, review |
| Spacing scale (section, card, inside card, label to field) | the Astryx spacing tokens; no other gap or padding | a setting of the design system, not measured | design system, review |
| Page frame width; readable column width | one per kind of page: the catalogue spans the viewport, inset 24 px on a phone and 28 px on a desktop; the admin console, a 1440 px frame beside its side navigation (`web/src/components/admin/page-frame.tsx`); privacy and terms, a 960 px reading column (`web/src/components/legal/legal-screen.tsx`); a form, the settings template's two columns of at least 320 px (`web/src/components/admin/settings-page.tsx`) | at 390, 1440 and 1920 px every admin page puts its h1 at x = 16, 278 and 388 in the same 1440 px frame; privacy and terms at 24, 268 and 508 in the 960 px column; the catalogue at 24, 28 and 28; no page scrolls sideways (2026-11-12, `next dev` with a scratch database on the development host, measured with Playwright, load not recorded) | review |
| Supported widths (phone, laptop, wide) | 390 px phone, 1440 px desktop | a setting, not measured; the widths `web/tools/loading-layout.cjs` checks | screenshots, `web/tools/loading-layout.cjs` (lab) |
| Rust tests, not counting the build | 3 min | 109 tests in 4 s (2026-11-11, development host, load not recorded) | review |
| `bun test` in `web/` | 15 s | 103 tests in 3 s (2026-11-11, development host, load not recorded) | review |
| Slowest test package | 16 s | `api` unit tests: 12.1 s, 15.4 s, 12.4 s (2026-11-11, GitHub-hosted ubuntu-latest, load not recorded) | review |
| Home page first load, gzip -9: HTML | 51,500 B | 50,696 B over 3 reads, against 48,674 to 48,677 B for 0.7.8 on the same data (2026-11-13, local `next start` with the API, load not recorded) | `bun run check:size` in `web/` (lab) |
| Home page first load, gzip -9: JS | 522,500 B | 521,999 B over 3 reads with Astryx 0.7.16, the preloaded chunks included, against 520,914 B for 0.7.15 on the same host (2026-11-14, local `next start` with the API, load not recorded) | `bun run check:size` in `web/` (lab) |
| Home page first load, gzip -9: CSS | 50,750 B | 50,561 B over 3 reads with Astryx 0.7.16, against 49,901 B for 0.7.15 on the same host (2026-11-14, local `next start` with the API, load not recorded) | `bun run check:size` in `web/` (lab) |
| Source file size, p90 of this repository (pass bar) | 274 lines | 274 lines: nearest-rank p90 of the 272 tracked `.rs`, `.ts`, `.tsx`, `.mjs`, `.cjs`, `.css`, `.sql`, `.sh` and `.py` files (2026-11-12, `wc -l`) | review |
| Commit subject length | 72 characters | a setting, not measured | review (no commit hook) |

A test suite that runs past its limit is made faster before tests are added. The budgets for the
home page are set in `web/tools/size.mjs`, which records each measurement and each owner ruling
that raised one.

## Database benchmark

`scripts/bench/run.sh` measures the database side of opening an item, trimming viewed items, reading and changing
favourites, and the refresh order, on a throwaway Postgres with synthetic data. Run it before and after a change to
these statements on the same machine. Compare the two runs, because the absolute numbers depend on the machine.

Baseline from 2026-11-11, 16 clients, two runs:

| Script | Latency | Throughput |
| --- | --- | --- |
| `item_open` | 3.8 ms | 4,200/s |
| `trim` | 0.24 ms | 66,000/s |
| `favourites_read` | 0.21–0.25 ms | 64,000–77,000/s |
| `favourite_toggle` | 15–17 ms | 950–1,050/s |
| `refresh_priority` (top 1,500) | 146–164 ms | |

The baseline was measured after these paths were reworked on 2026-11-11. Before the rework, the same statements on the
same data (in the development database, not with this script) took 7.3 ms per item open, 0.53–0.61 ms per trim,
366–519 ms for `refresh_priority` and 6.5 ms per favourite toggle.

## Enforcement

The checks that turn red on a violation, by the rule they guard. Every rule tagged `[rule: <id>]` in
an `AGENTS.md` has a row named `path#id`, with a gate command, a review criterion (the `REVIEW.md`
section that checks it) or both. A rule without a row is checked in review against `REVIEW.md`.

| Rule | Gate command | Review criterion |
| --- | --- | --- |
| AGENTS.md: Code quality | `cargo clippy --workspace --all-targets -- -D warnings`; in `web/`: `bunx biome check` | |
| .github/AGENTS.md: Tiers, jobs and gates | `python3 scripts/check-requirements.py` | |
| web/AGENTS.md: i18n (if present) | in `web/`: `bunx tsc --noEmit` (every locale holds exactly the keys of `web/src/lib/messages-zh.ts`) | |
| crates/AGENTS.md: Compatibility | `cargo test --workspace -- --include-ignored` (applies every migration to an empty database); `scripts/test-restore.sh` with `WORKER_BIN` (`.github/workflows/migrations.yml`: applies a pull request's migrations to the public data bundle) | |
| crates/AGENTS.md#authz | `cargo test --workspace -- --include-ignored` (`crates/api/src/admin/authz.rs`: every admin route answers 401 signed out and 403 to a member) | `REVIEW.md`, "Checking against the rules", Authorization |
| crates/AGENTS.md#sensitive-values | | `REVIEW.md`, "Checking against the rules", Sensitive data |
| crates/AGENTS.md: Verification | `cargo test --workspace -- --include-ignored` (against PostgreSQL in CI) | |

Lab checks, run by hand and not part of a gate: `web/tools/motion.cjs`, `web/tools/loading-layout.cjs`,
`web/tools/nav.cjs` and `bun run check:size` in `web/`; `scripts/bench/run.sh`. The two loading checks run once on
`/`, which arrives in one paint, and once on a URL with `q`, which streams its results behind silhouettes; the
second, and every run of `nav.cjs`, needs `LAB_AI_OFF=1`, an API on this machine started with `AI_BASE_URL` at a
closed port (`http://127.0.0.1:9`), so the search embeds nothing, and `slow-api.mjs` pointed at that API; the proxy
refuses a search with `q` to any other upstream. Build and start the site with `API_ORIGIN` set to the proxy:
`next build` fixes the browser's `/api` rewrite. When `check:size` reads over budget, `web/tools/size-diff.mjs`
compares two `next analyze` snapshots by package and source directory, to find what grew before a raise is
proposed.

## Exceptions

Backend endpoints with no frontend caller:

| Endpoint | Caller | Reason |
| --- | --- | --- |
| `/api/authors`, `/api/booth-tags` | other clients of the VRCatalogue-compatible API | public API, listed in the README, "API" |
| imgproxy `/stats` | the API's admin overview (`crates/api/src/admin.rs`) | operator counters for the admin console; `web/next.config.ts` does not forward it, so only the API reaches it (owner, 2026-11-12) |

Intentional designs that look like defects:

| What | Why | Owner ruling (date, quote) |
| --- | --- | --- |
| `favourite_toggle` in the database benchmark is slow | Every client clicks the same popular items, so the `count_favourite` trigger's updates of their `item_stats` rows wait for each other. Spread over many items, a favourite costs the same as without the trigger. | 2026-11-11, commit 2507250: "a worst case that was accepted on purpose" |
| "Commands" has no Pre-commit and no Scheduled run on main tier | The merge gate covers this project; a hook or a scheduled run would be CI nobody needs. | 2026-11-12: "Drop both rows" |
| `test.yml` and `images.yml` name actions by version tag, not by SHA (`.github/AGENTS.md`, "Tiers, jobs and gates") | The owner chose tags after weighing both sides: a pin keeps a moved tag from changing the code that pushes the images; a tag gets the action's fixes without a bot. | 2026-11-12, on a commit that pinned every action by SHA: withdraw it |
| No release-only checks (a reproducible build, migrations on a copy of production data, a deploy rehearsal) and no release-tier self-test of each gate command (`.github/AGENTS.md`, "Tiers, jobs and gates") | BoothDex publishes no versioned release: every push to master builds the `latest` images, and the owner deploys them by hand. The one check with a real risk behind it, migrations on existing rows, runs on pull requests against the public data bundle instead of a production copy (`.github/workflows/migrations.yml`), and `docs/DEPLOY.md`, "Operations", says how to go back when an update fails to start. | 2026-11-12: "none of them"; on checking migrations, "A+C": the CI job on the data bundle and the rollback note |
| The 15 commits from ed15fcf to a007c85 (2026-10-31 to 2026-11-05) are unsigned (`AGENTS.md`, "Commits and pull requests") | Signing them now would rewrite published history. | 2026-11-12: "don't rewrite published history; leave them" |
| Every image the site shows loads from `/img/` through imgproxy; an image URL that no imgproxy route serves (`imgUrl` and `imageSrc` in `web/src/lib/catalog.ts`) shows the no-image state instead of loading from its own host | The browser talks only to the site's own origin (root `AGENTS.md`, "Both sides") | 2026-11-13: "改成全部走我们的imgproxy" |
| A locked sign-in answers like a wrong password (401 `bad_credentials`), even with the right password; failures by the username and the email count against one budget per account; an admin password reset, or disable then re-enable, lifts it; every sign-in attempt the API checks is logged with the client IP and, for a known account, its username | One budget per account; a visible lockout would show which username and email belong together; the log lets the operator see password guessing and credential stuffing | 2026-11-13: "Count per account", "Hide the lockout", "Reset clears it + log"; "感觉至少用户名和IP是要被记录的吧，不然怎么防止滥用和撞库呢"; on the attempts refused before any check: "改措辞" |
| The home page's first load is about 100 KB of JS larger than before Astryx 0.7.15 | Astryx 0.7.9 to 0.7.15 add motion, gesture and layer code to the shared modules that the site frame and the filter rail use (`next analyze`: motion +23 KB, gestures +22 KB, utils +16 KB, Layer +11 KB, Resizable +10 KB gzip), so no lazy import on this side removes it; the budgets in "Numbers" follow the measured size | 2026-11-13: "Accept, do it together"; "you can adjust budget" |
| Registration refuses an email with any character outside printable ASCII: non-ASCII letters (an IDN domain included; it must be typed in punycode), control characters, space and DEL. ASCII symbols such as `!#$%&'*/=?^{\|}~` stay allowed. Sign-in by email is unchanged | Rust and PostgreSQL lowercase some letters differently (`ΑΣ` gives `ας` in Rust and `ασ` in PostgreSQL), so such an email might not find its account. "特殊字符" is read as every character outside printable ASCII, not as ASCII symbols | 2026-11-13: "改成不支持特殊字符" |

Outside requests allowed in local runs. Each relaxes the root `AGENTS.md` hard limit "no test traffic to a real
upstream unless the owner allows it" for that request only:

| Request | Why | Owner ruling (date, quote) |
| --- | --- | --- |
| `api.frankfurter.dev`, from `exchangeRates` (`web/src/lib/server-data.ts`), in local development and test runs of the web site | It renders prices in a currency other than yen; the URL is a literal that no setting points elsewhere or turns off, and the request carries no visitor data | 2026-11-13: "允许本地请求" |

## Not yet enforced

A row of "Commands", "Numbers" or "Enforcement" that is not filled yet goes here, named
`Section: row`, for example `Numbers: Slowest test package`. The merge gate fails on:

- an unfilled row that is not listed here;
- a listed row that is already filled, or that does not exist;
- a Due date that has passed.

Due holds one date (YYYY-MM-DD). Moving it later needs the owner's approval, quoted in the commit
body.

| Row | Current state | Plan | Due |
| --- | --- | --- | --- |

## Deferred options

Options and findings set aside, with their source and the reason they wait. The owner decides from this list
after a release.

| Option | Source | Why it waits |
| --- | --- | --- |
| A per-client rate limit inside imgproxy for cache misses | security scan, 2026-11-15 | imgproxy now holds at most 200 MiB of upstream bodies at once and answers 503 past a 10 s wait for room, which caps its memory; it does not bound the number of upstream requests in flight, and one client can still fill the budget or send any number of misses to the upstream hosts. The client-IP extraction lives in `crates/api/src/limit.rs`, so a limit here would copy it, and the front proxy or CDN can limit `/img/*` per client without code (`docs/DEPLOY.md`) |
| Keep an ordinary misspelt word from becoming a model filter: "chifon dress" reads as the model Chiffon when no title, tag, author name or description has "chifon" | review of PR #58, 2026-11-14 | Following the text typo retry, a word the catalogue does not know is taken as the one model it misspells; how often such a word meant something else is unknown until `split_names` is run over a sample of real queries |
| Make "Clear" on the empty results page also drop the shop and avatar clauses, and show it when only those are set: `FILTER_RESET` keeps them and `activeFilterCount` does not count them, while the new per-clause buttons do offer them | building the empty-result suggestions, 2026-11-13 | Existing behaviour, kept on purpose (owner, 2026-11-13); changing what "Clear" means touches the filter row and the saved views as well |
| Cache search results in the API, keyed by the query and the snapshot version, and cache or precompute the card payloads `db::load_items` builds | search performance review, 2026-11-13 | Measured on the development host (master build, 216,179 items, idle, two runs): the API spends 4 to 13 ms per search, of which the search 0.5 to 8.8 ms and the card load 1.3 to 2.0 ms. A cache saves those milliseconds per hit and adds invalidation and memory; the first search of an uncached query is dominated by the embedding provider (median 0.7 s), and the process and `query_embeddings` caches already keep its answers. Kept as deferred (owner, 2026-11-13) |
| Rebuild the catalogue snapshot incrementally (reload changed items, patch the bitmaps) instead of reloading every item | search performance review, 2026-11-13 | A full rebuild loads 216,179 items in about 20 s on the development host and holds two catalogues in memory meanwhile (`snapshot.rs`). It is worth the complexity only if production memory or the 5-minute rebuild gap becomes a problem; not measured on production. Kept as deferred (owner, 2026-11-13) |
| Prefetch an item's record when the pointer rests on its tile | search performance review, 2026-11-13 | The record is already fetched on `pointerdown` (`item-host.tsx`), the card draws the tile's title, cover and price at the click (`keepTiles`), and `/api/items` is cached 12 h; hovering across tiles would spend the 4/s single-item rate limit to save one round trip for the detail fields. Kept as deferred (owner, 2026-11-13) |
| Extend the `Content-Security-Policy` in `web/next.config.ts` past `img-src`: `script-src` and `style-src`, `object-src 'none'`, `base-uri` and `frame-ancestors` | the change that sent `img-src 'self' data:` | That change covered images only; a `script-src` without `'unsafe-inline'` needs a nonce per request, and Next applies nonces only to dynamically rendered pages, so the statically prerendered parts (`/privacy`, `/terms`, the page shells) would turn dynamic; each directive needs its own sweep of every page |
| Test that the upstream client's redirect closure counts hops through `follow_redirect` (`crates/imgproxy/src/upstream.rs`): only the off-host test covers the closure, and no test fails if it drops the hop limit | pre-review of #41 | Low, no secret or production risk: redirects stay on the same https host and the 20 s timeout bounds them; a test needs an https loopback server, so a certificate generator and a test-only trust store as new dependencies |
| Restrict the hosts an admin-set AI base URL may reach, and trim the provider reply that the connection test shows (`test_ai` in `crates/api/src/admin.rs`) | security scan, 2026-11-15 | An admin session can still make the API send `GET {base_url}/models` and an embeddings request to any http(s) host, LAN and loopback included, with a key that session entered, and read the model ids and the status or error text that come back. A stored key is not part of this: the console moves no key to a base URL it was not saved with (`key_follows_url` and `save_ai` in `crates/api/src/admin.rs`). A LAN or loopback provider is a supported setup (`scripts/mock-ai.ts` listens on loopback), so a host rule needs the owner's choice of what stays allowed. Operators see upstream detail (`crates/AGENTS.md`, "Errors"), so trimming the reply needs the owner's choice too |

## Open questions

Questions no rule settles, for the owner. They are asked together at a stopping point; a ruling moves the item
to the file it decides.

| Question | Source | Raised |
| --- | --- | --- |
| Should a return to the results keep each tile in its column? A page whose tiles are not loaded is a box of whole rows (`pageBlock` in `web/src/lib/result-pages.ts`), and 100 results a page do not fill whole rows at 6 columns (1440 px). So Back from another page, or a reload, at results page 3 brings back page 3's first tile at the same height but in column 1 instead of column 3, and the two tiles of page 2 that led its row are gone. A deep start moves page 3's tiles 2 columns along when page 2's tiles arrive. Keeping columns means either padding each page to whole rows (a gap at every page end) or offsetting a page's first tile by its index modulo the column count | `web/tools/nav.cjs` step 5, 2026-11-14 | 2026-11-14 |
| With `AI_API_KEY` set and `AI_BASE_URL` not set, the console's API URL field is read-only and reuses the description "Set by the server environment; can't be changed here", although its value comes from the database. Is that wording acceptable, or should this one state get its own description ("Locked because the server sets the API key; set AI_BASE_URL to change it"), at the cost of a new string in each admin locale and a form branch? | the change that sends the AI key only to its base URL, 2026-11-15 | 2026-11-15 |
