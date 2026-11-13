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
| Home page first load, gzip -9: HTML | 49,500 B | 47,500 to 49,000 B over 7 reads (2026-11-12, local `next start`, load not recorded) | `bun run check:size` in `web/` (lab) |
| Home page first load, gzip -9: JS | 410,000 B | 396,067 B (2026-11-12, local `next start`, load not recorded) | `bun run check:size` in `web/` (lab) |
| Home page first load, gzip -9: CSS | 45,750 B | 45,503 B (2026-11-12, local `next start`, load not recorded) | `bun run check:size` in `web/` (lab) |
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

Lab checks, run by hand and not part of a gate: `web/tools/motion.cjs`, `web/tools/loading-layout.cjs`
and `bun run check:size` in `web/`; `scripts/bench/run.sh`.

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
| A locked sign-in answers like a wrong password (401 `bad_credentials`), even with the right password; failures by the username and the email count against one budget per account; an admin password reset, or disable then re-enable, lifts it; every sign-in attempt is logged with the client IP and, for a known account, its username | One budget per account; a visible lockout would show which username and email belong together; the log lets the operator see password guessing and credential stuffing | 2026-11-13: "Count per account", "Hide the lockout", "Reset clears it + log"; "感觉至少用户名和IP是要被记录的吧，不然怎么防止滥用和撞库呢" |
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
| Make the feed's `image_path` (`crates/api/src/rss.rs`) skip an empty first gallery entry as `imgUrl` does (`if (b)`), so `booth_images = [""]` with an archive `media` shows the archive image in the feed too | review of #36 | Low, not reachable today: the development catalogue has no empty gallery entry; a feed entry would show no image, nothing wrong |
| Test the factory UUID group lengths and the 1 to 120 file-name bound in the feed's `image_path` (`crates/api/src/rss.rs`): changing either leaves the rss tests green | review of #36 | Low, no secret or production risk: the output is a URL that imgproxy validates again with its own tested checks |
| Test imgproxy's redirect hop limit (`previous().len() <= 10` in `crates/imgproxy/src/upstream.rs`): no test can fail when the limit changes | pre-review of #35 (raising the limit to 1000 left every imgproxy test green) | Low, no secret or production risk: redirects stay on the same https host and the 20 s timeout bounds them; a test of a followed same-host redirect and its count needs a TLS loopback server |
| Keep the image mapping of the RSS feed (`image_path` in `crates/api/src/rss.rs`) and of the web (`imgUrl` in `web/src/lib/catalog.ts`) in one shared fixture that both `cargo test` and `bun test` read | the plan of the pull request that sends the feed's images through `/img/` | The two mappings are about 20 lines each and name each other in comments; both have tests on the same sample URLs. A fixture would tie `crates/` to `web/` for that size, and returning proxied paths in the item JSON would change the public API for every client |
| Reserve six digits in the filter rail's count slot (`web/src/components/catalog/filter/filter-rail.tsx`) so a count of 100,000 or more does not widen its slot when it arrives | review of the pull request that held the rail's count slots from the first paint | Six digits made two Chinese category names wrap that fit before; the largest count is about 55,000, so no row reaches six digits today |
| `bun run check:motion` stops in some runs at its retry step: `getByRole("button", { name: "Try again" })` finds two buttons (Playwright strict mode), so the run ends before "retry clears the Banner" | the runs for the pull request that fixed the sort button's entrance and the rail's count slots, 2026-11-13, en-US context against the development catalogue: 1 of 5 runs on master 8d399d4, 1 of 6 on that branch; the checks before the retry step passed in those runs | A defect of the check or a second retry control on the page, not traced; outside the change that found it. Which element holds the second button is the first thing to read |
| Narrow the privacy policy's "each sign-in attempt" (`web/src/lib/legal-*.ts`) or log the two refusals that return before any line is printed: the per-IP limiter's (`s.limits.auth.check` in `crates/api/src/account/sign_in.rs`) and a name longer than 254 bytes (`valid_login`) | review of the pull request that counts sign-in failures per account | Low finding without a secret or production risk: the text promises more logging than happens, not less, so nothing is hidden from members; the limiter's log line is already in this list |
| Test two search conversions no test covers: a text query with no `sort` sorts by relevance (`crates/api/src/search_request.rs`), and the palette passes the caller's `adult` to its item search (`palette` in `crates/api/src/palette.rs`) | review of the pull request that typed the search request: a planted violation in either leaves all api tests green | Low finding without a secret or production risk; the behaviour is unchanged by that pull request, and the gap was there before it |
| Hold the strip states of `FilterBand` (`web/src/components/catalog/band/band.tsx`: `open`, `closing`, `tucked`) as one value instead of three booleans | the refactor survey, read through for the series that split `account.rs` and `main.rs` | No defect behind it: at most one of the three is true in every reachable state, and the CSS keys on `data-strip-open` and `data-strip-tucked`, never both. One value would change what reruns the layout effect, which depends on `open` and `tucked` but not `closing`; no test covers the band, so the change needs a manual check of open, close, tuck and untuck at the top, part-way and past the shrink |
| Add the Figtree WOFF files and the `next/og` renderer version to the share image hash in `web/next.config.ts`; a dependency upgrade that redraws `/og.png` keeps its URL, and platforms that cache images by URL keep showing the old picture | review of PR #15 | Low finding without a secret or production risk; `/og.png` is revalidated on every use, so only platform caches are affected; both are pinned (`@fontsource/figtree` 5.3.0, `next` 16.4.5), so only a deliberate upgrade redraws the picture |
| Give `search_cpu_timing_excludes_the_permit_queue` (`crates/api/src/route_tests.rs`) an assertion that does not depend on 1 ms of slack between two timers | the runs for the pull request that moved refusals into `ApiError`: the test failed once in about 170 local runs of the api tests, in the first run after a rebuild in a second worktree; its output was not kept | Low finding without a secret or production risk; 12 runs in a row on master and 12 on the branch passed, so the failure is not reproduced; the test and the code it checks are outside that change |
| Decide whether a start past page 1 may put a second batch of boxes up at once: at 1440x900, `/?page=60` showed 59 boxes above page 60 in one of three recordings and 40 in the other two, with nothing scrolled by the reader | the same runs, the scroll recording at 1440x900 on master 11cd404 | Seen on master before any change; the document stays far below the engines' height limits with 80 boxes, and the cause (a scroll up seen while the boxes go in) is not confirmed |
| Pass the embedding provider (base URL, key, model) to `query_embedding` (`crates/api/src/search.rs`) as one value instead of three strings, to shorten its seven parameters | the plan that moved `AppState` out of `crates/api/src/main.rs`: grouping the five embedding fields of `AppState` into one struct would remove none of those parameters, since the three strings come from `AiConfig` | A signature change outside a change that only moves code; no defect behind it |
| Read the items of the `enrich` job (`crates/worker/src/jobs.rs`) into a typed row instead of JSON read with `unwrap_or_default`, and split `enrich`, `embed` and `sweep`, which run to between 67 and 113 lines | the plan of PR #21 (shared listing-image and fingerprint SQL in the worker), which set it aside | No defect behind it: `id`, `title` and `bio` are `not null` in the schema, so their defaults are never taken. A typed row reaches into `ai::enrich_prompt`, which takes JSON, and into the raw translation JSON that `crates/api/src/db.rs` keeps on purpose; a split by length alone moves code without fixing anything |
| Keep the job kinds and their defaults in one registry for the worker (`jobs::run`), the API (`KINDS` in `crates/api/src/admin.rs`), the newest `jobs_kind_check` migration and the admin console (`web/src/components/admin/types.ts`) | the plan of PR #21, which set it aside | The seven kinds agree in all four places. `job_kinds_match_the_database_constraint` already compares the API with the migration, and the worker refuses an unknown kind with an error, so a mismatch corrupts nothing. A registry across Rust, TypeScript and migrations that cannot change would need a generator for a list that has not drifted |
| Log a refusal of the per-IP `auth` limiter on sign-in (`warn: sign-in rate limited from <ip>`), gated by a second limiter to at most one line per address a minute | the plan that counts failed sign-ins per account; its review, round 3, set it aside | Not needed for the owner's ruling to log the username and the IP: every attempt the limiter lets through is already logged with its IP. It would add a `Limits` field, a route test change and a planted check |
| Send a `Content-Security-Policy` with `img-src 'self' data: blob:`, so the browser itself refuses an image from another host | the plan that sends every image through imgproxy | No security header exists yet, and no rule asks for one; it needs its own check of every page, the Lightbox, the download and the Astryx components |
| Refuse private and loopback addresses in the DNS answers for imgproxy's upstream hosts | the same plan | The hosts are constants in `crates/imgproxy/src/upstream.rs`, and the response is only decoded and re-encoded, never relayed; this was already true of `booth.pximg.net` before that change |
| Resize pixiv Factory drawings in imgproxy for a sized request: `/f/{uuid}/{file}` serves the drawing at its own size, so the item dialog's 300 px gallery thumbnails get the full drawing, re-encoded | the same plan | pixiv Factory has no resize variants; the drawings appear only inside the item dialog (84 entries in the development database), so the cost is bounded |
| Split the rest of `crates/imgproxy/src/main.rs` (encoding, cache, handlers), which stays over the 274-line pass bar | the same plan, after the upstream code moved to `crates/imgproxy/src/upstream.rs` | A split by length alone; that change does not touch the encoding or the cache |

## Open questions

Questions no rule settles, for the owner. They are asked together at a stopping point; a ruling moves the item
to the file it decides.

| Question | Source | Raised |
| --- | --- | --- |
