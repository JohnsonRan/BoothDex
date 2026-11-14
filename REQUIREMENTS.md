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
| Home page first load, gzip -9: HTML | 51,520 B | 51,498 to 51,503 B for the same code across builds, which differ only in the random build ID Next writes into the HTML; the 20 B above 51,500 is room for that noise, not for new content (2026-11-15, local `next start` with the API, load not recorded) | `bun run check:size` in `web/` (lab) |
| Home page first load, gzip -9: JS | 522,500 B | 521,999 B over 3 reads with Astryx 0.7.16, the preloaded chunks included, against 520,914 B for 0.7.15 on the same host (2026-11-14, local `next start` with the API, load not recorded) | `bun run check:size` in `web/` (lab) |
| Home page first load, gzip -9: CSS | 51,000 B | 50,807 B over 3 reads with the avatar strip's arrows, the filter rail's section names and the item dialog's favourite label centred on their icons, against 50,737 B before them on the same host (2026-11-15, local `next start` with the API, load not recorded) | `bun run check:size` in `web/` (lab) |
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
| "Clear" on the empty results page keeps the shop and avatar clauses, and does not show when only those are set (`FILTER_RESET` and `activeFilterCount` in `web/src/lib/catalog.ts`), while the per-clause buttons beside it offer them | "Clear" resets the filters the visitor set in the filter row; changing what it means would also change the filter row and the saved views | 2026-11-13: kept on purpose; 2026-11-15: "按建议来" (closed as deferred) |
| The home page's first load is about 100 KB of JS larger than before Astryx 0.7.15 | Astryx 0.7.9 to 0.7.15 add motion, gesture and layer code to the shared modules that the site frame and the filter rail use (`next analyze`: motion +23 KB, gestures +22 KB, utils +16 KB, Layer +11 KB, Resizable +10 KB gzip), so no lazy import on this side removes it; the budgets in "Numbers" follow the measured size | 2026-11-13: "Accept, do it together"; "you can adjust budget" |
| Registration refuses an email with any character outside printable ASCII: non-ASCII letters (an IDN domain included; it must be typed in punycode), control characters, space and DEL. ASCII symbols such as `!#$%&'*/=?^{\|}~` stay allowed. Sign-in by email is unchanged | Rust and PostgreSQL lowercase some letters differently (`ΑΣ` gives `ας` in Rust and `ασ` in PostgreSQL), so such an email might not find its account. "特殊字符" is read as every character outside printable ASCII, not as ASCII symbols | 2026-11-13: "改成不支持特殊字符" |
| With `AI_API_KEY` set and `AI_BASE_URL` not set, the console shows the API URL field read-only under the description "Set by the server environment; can't be changed here", although that URL is stored in the database | The key from the environment locks the URL (`key_follows_url` in `crates/api/src/admin.rs`), so the field cannot be changed in the console, which is what the description tells the admin to expect; a description of its own for this one state would add a string to each admin locale | 2026-11-15, asked whether to reword it: "不用改" |

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
| A bound on imgproxy's upstream requests in flight from all clients together (a global miss limit) | security scan, 2026-11-15; left after the per-client miss limit (10/s, burst 200) | Each client's misses are bounded and the body budget caps memory, but many clients together can still send any number of misses at once; a global bound would refuse every visitor's images during a spread-out flood, so it needs the owner's choice of a number |
| Keep an ordinary misspelt word from becoming a model filter: "chifon dress" reads as the model Chiffon when no title, tag, author name or description has "chifon" | review of PR #58, 2026-11-14 | Following the text typo retry, a word the catalogue does not know is taken as the one model it misspells; how often such a word meant something else is unknown until `split_names` is run over a sample of real queries |
| Extend the `Content-Security-Policy` in `web/next.config.ts` with `script-src` and `style-src` | the change that sent `img-src 'self' data:`; `object-src 'none'`, `base-uri 'self'` and `frame-ancestors 'none'` were added on 2026-11-15 | Without `'unsafe-inline'` either directive needs a nonce per request, and Next applies nonces only to dynamically rendered pages, so the statically prerendered parts (`/privacy`, `/terms`, the page shells) would turn dynamic; each directive needs its own sweep of every page |
| Find why a start past page 1 records a layout-shift entry as the results hydrate (0.49 on this branch, 0.57 on master, at 1440x900 for `?page=3` and `?q=ドレス&page=3`): its sources go from an empty rect to their place, yet the first result keeps its x and y in every frame from its first paint | deep-start column probe, 2026-11-15 (Playwright on the lab build) | Present on master, so not caused by this change; Chromium attributes it as the pages above go in with the window scrolled by their height in the same frame, and finding it needs a trace of that frame. It counts toward CLS on deep links |
| Find why `next dev` logs "Encountered two children with the same key" for keys 1 and 2 when it opens `?page=3`: the results seem to put the pages above the start in twice, most likely from React Strict Mode running the mount layout effect twice (`web/src/components/catalog/results/use-result-place.ts`) | review of grid-page-columns, 2026-11-15 (`next dev`, Playwright console) | Present on master the same way and absent from `next start`, where effects run once; no visitor sees it |
| Centre the filter rail's ja and ko section names on their chevrons: they sit 1.0 to 1.34 px above at 1440 and 2560 (1.5x), while zh and en are within 0.5 px | PR #70, 2026-11-15 | The kana and Hangul fallback fonts place their glyphs differently from Figtree's capitals and Chrome has no ideographic edge for `text-box`, so only a nudge per locale is left; 1 px is not visible (owner, 2026-11-15: "1px肉眼看不出来，暂时不处理吧") |
| Keep the reader's place through a window resize: `keep()` takes its anchor in the resize handler (`web/src/components/catalog/results/use-result-frame.ts` onResize), after the browser has already reflowed the grid to the new width, so the reflow itself is not compensated (1440 to 800 px at `?page=3`: the tile at the top moves 2,441 px down) | review of grid-page-columns, 2026-11-15 (Playwright on the lab build) | Present on master too (the window jumps on resize there as well); needs the anchor taken before the reflow, for example from the place last saved, which is a change of its own |

## Open questions

Questions no rule settles, for the owner. They are asked together at a stopping point; a ruling moves the item
to the file it decides.

| Question | Source | Raised |
| --- | --- | --- |
