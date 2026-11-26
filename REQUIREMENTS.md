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
| Merge gate | `cargo test --workspace -- --include-ignored` with `DATABASE_URL` set as in the README, `cargo clippy --workspace --all-targets -- -D warnings` with `CARGO_BUILD_WARNINGS=deny`, `cargo test -p imgproxy --features libavif`; in `web/`: `bun test`, `bunx next typegen`, `bunx tsc --noEmit`, `bunx biome check`; `python3 scripts/check-requirements.py` | CI: `.github/workflows/requirements.yml` and `.github/workflows/test.yml` on every pull request, both required to merge into master; on a push to master, requirements.yml always and test.yml through `.github/workflows/images.yml` | 3.5 min | 174 s, 188 s, 154 s, slowest job `rust` (2026-11-11, GitHub-hosted ubuntu-latest, load not recorded) |
| Merge gate, when a migration changes | `cargo build -p worker`; with `PODMAN=docker WORKER_BIN=target/debug/worker` set: `scripts/test-restore-selftest.sh`, then the public data bundle's `catalogue.dump` checked against its `SHA256SUMS` and `scripts/test-restore.sh bundle` | CI: `.github/workflows/migrations.yml` on a pull request that changes `crates/core/migrations`, `scripts/test-restore.sh` or the workflow; not required to merge, because a path-filtered required check stays pending on every other pull request | 9 min | 435 s, 340 s, 368 s, of which the self-test 13 s and restoring the bundle about 5 min (2026-11-12, GitHub-hosted ubuntu-latest, load not recorded) |
| Release | `docker buildx bake -f deploy/docker-bake.hcl images`, after the merge gate | CI: `.github/workflows/images.yml` on every `v*` tag and on a push to master that changes a path in its `paths` filter; builds the Rust group and `web` on native AMD64 and ARM64 runners, then publishes the four dual-platform indexes to ghcr.io | 17 min | Dual-platform workflow: 852 s (PR #96), 685 s (PR #97), 378 s (PR #98), merge gate included (2026-11-24 to 2026-11-25, GitHub-hosted ubuntu-26.04 and ubuntu-26.04-arm, load not recorded); earlier AMD64-only workflow: 280 s, 394 s, 344 s (2026-11-11, ubuntu-24.04) |

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
| Home page first load, gzip -9: HTML | 53,500 B | 52,744 B over 3 reads with TopNav primary view links, mobile drawer, signed-out favourites dialog and local-only recent sync notice, against 52,142 B on HEAD with the same dump (2026-11-26, local `next start` with the API, load not recorded). Before that: 52,250 B over 3 reads with the Ko-fi support link at the end of the filter rail, against 51,890 B on HEAD with the same dump (2026-11-25, local `next start` with the API, load not recorded). Before that: 51,858 B over 3 reads with the stricter limits merged with master's single page read (2026-11-17, local `next start` with the API, load not recorded). Before that: 51,818 B over 3 reads with the stricter limits' check host, against 51,858 B before it, inside the build ID's noise (2026-11-17, local `next start` with the API, load not recorded). Before that: 51,796 B over 3 reads with the guestbook button and its name in the page's catalogue, against 51,609 B before it (2026-11-17, local `next start` with the API, load not recorded). Before that: 51,628 B over 3 reads with the "search instead" link and copy (one script tag fewer), against 51,648 B with the no-image state's listener before it, with live titles; the build ID Next writes into the HTML moves the same code by about 5 B (2026-11-15, local `next start` with the API, load not recorded) | `bun run check:size` in `web/` (lab) |
| Home page first load, gzip -9: JS | 530,000 B | 528,810 B over 3 reads with TopNav primary view links (`TopNavItem`, `Badge` counts, `MobileNav` below `lg`), `AppLink` (`prefetch={false}`), shared `SignInDialog` host and local-only recent sync prompt, against 524,343 B on HEAD with the same dump (2026-11-26, local `next start` with the API, load not recorded). Before that: 523,856 B over 3 reads with the Ko-fi support link and its coffee glyph, against 523,529 B on HEAD (2026-11-25, local `next start` with the API, load not recorded). Before that: 523,999 B over 3 reads with the stricter limits merged with master's single page read (2026-11-17, local `next start` with the API, load not recorded). Before that: 523,969 B over 3 reads with the stricter limits' check host and its retry in `api()`, against 523,710 B before it; with the dialog behind a lazy import of its own it read 525,307 B over 40 chunks, so it loads through `frame/sign-in` (2026-11-17, local `next start` with the API, load not recorded). Before that: 524,030 B over 3 reads with the guestbook button, whose drawer loads on first open, against 523,712 B before it (2026-11-17, local `next start` with the API, load not recorded). Before that: 523,712 B over 3 reads with the "search instead" link as a filter change: Turbopack groups the same first-load modules into 39 chunks instead of 40, about 20 KB less code before gzip; the plain link read 524,971 B. Against 524,871 B with the refresh bar's code back in the first load and 522,819 B with it loaded when the page is idle (2026-11-15, local `next start` with the API, load not recorded) | `bun run check:size` in `web/` (lab) |
| Home page first load, gzip -9: CSS | 52,000 B | 51,481 B over 3 reads with TopNav view links (`TopNavItem`, `MobileNav` below `lg`) and recent local-sync banner, against 51,463 B on HEAD with the same dump (2026-11-26, local `next start` with the API, load not recorded). Before that: 51,231 B over 3 reads with the no-image state, against 50,880 B with the reduce-motion switch before it (2026-11-15, local `next start` with the API, load not recorded) | `bun run check:size` in `web/` (lab) |
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
| crates/AGENTS.md: Performance (no unnecessary dependencies) | `cargo clippy --workspace --all-targets -- -D warnings` with `CARGO_BUILD_WARNINGS=deny` (Cargo's `unused_dependencies` lint, in `.github/workflows/test.yml`) | |
| crates/AGENTS.md: Verification | `cargo test --workspace -- --include-ignored` (against PostgreSQL in CI) | |

Lab checks, run by hand and not part of a gate: `web/tools/motion.cjs`, `web/tools/loading-layout.cjs`,
`web/tools/nav.cjs` and `bun run check:size` in `web/`; `scripts/bench/run.sh`. The two loading checks run once on
`/`, which arrives in one paint, and once on a URL with `q`, which streams its results behind silhouettes; the
second, and every run of `nav.cjs`, needs `LAB_AI_OFF=1`, an API on this machine started with `AI_BASE_URL` at a
closed port (`http://127.0.0.1:9`), so the search embeds nothing, and `slow-api.mjs` pointed at that API; the proxy
refuses a search with `q` to any other upstream. Build and start the site with `API_ORIGIN` set to the proxy:
`next build` fixes the browser's `/api` rewrite; `nav.cjs` takes the proxy's URL as its third argument. When
`check:size` reads over budget, `web/tools/size-diff.mjs` compares two `next analyze` snapshots by package and
source directory, to find what grew before a raise is proposed.

## Exceptions

Backend endpoints with no frontend caller:

| Endpoint | Caller | Reason |
| --- | --- | --- |
| `/api/authors`, `/api/booth-tags` | other clients of the VRCatalogue-compatible API | public API, listed in the README, "API" |
| imgproxy `/stats` | the API's admin overview (`crates/api/src/admin.rs`) | operator counters for the admin console; `web/next.config.ts` does not forward it, so only the API reaches it (owner, 2026-11-12) |

Intentional designs that look like defects:

| What | Why | Owner ruling (date, quote) |
| --- | --- | --- |
| With the stricter limits on, a signed-in visitor without a pass gets today's numbers only while "Sign-in and registration need verification" is on too, and only if the sign-in or registration that made the session passed the Turnstile check (`sessions.turnstile_passed`); a session let through unchecked, made while that switch was off, or any session while it is off counts as no pass (the `Tier` extractor in `crates/api/src/pass.rs`, `Tiered::check` in `crates/api/src/limit.rs`) | Only a sign-in that passed the check cost a script more than a request. How the two buckets count a session's requests is in `Tiered` (`crates/api/src/limit.rs`) | 2026-11-17, choosing among "any signed-in visitor", "only while sign-in is checked" and "no change": "1. B" |
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
| The guestbook's Turnstile widget loads its script and frame from `challenges.cloudflare.com` in the visitor's browser, an exception to "The browser talks only to the project's own origin" (root `AGENTS.md`, "Both sides"); so do sign-in and registration while they are checked, the check offered when a stricter rate limit refuses, and the console's check of new keys for sign-in | Turnstile can only judge a visitor from the visitor's own browser; the token it hands out is still checked by the API (siteverify), and the widget loads only when the guestbook drawer, the sign-in dialog or the offered check opens, or when an admin's save in the console puts new keys in front of sign-in | 2026-11-17, asked to record it as an exception: "同意"; the console's check, 2026-11-21: recorded here only, the privacy policy unchanged |
| Sign-in and registration are let through when Turnstile cannot judge the token (siteverify unreachable or failing, or the secret key refused), where the guestbook refuses; a request without a token is let through then too | A Cloudflare outage or a mistyped secret key must not keep an admin out of the console that would fix it. The password, the per-account lockout and the per-address limit still stand, and a token Cloudflare judges bad is refused | 2026-11-17, the owner chose "放行并记错误日志" over refusing with an environment variable as the way out |
| `api` applies no per-address rate limit to a request whose client IP is internal (`own_server` in `crates/api/src/limit.rs`): `web`'s own cached lookups, a LAN visitor, local development. Reads `web` makes for a visitor carry the visitor's address (`visitorHeaders`) and are limited as that visitor; the embedding limits, which bound a paid upstream, still apply | The site's own server is never to be rate-limited; no outside request is attributed to it (a public peer's headers are not read; behind the front proxy the appended public hop wins, each forwarded entry is parsed on its own, and a header holding an entry that does not parse puts the request in one shared limited bucket, never the server's). A front proxy that sets no forwarding header leaves every visitor unlimited, which `api` reports in its log (`docs/DEPLOY.md`, "Host firewall & rate limiting") | 2026-11-17: "查询缓存可以不应用吗，加验证的目的是为了减少对服务端的不必要请求"; "我们永远不应该对服务端自身请求进行任何限速吧 这本身就是不应该的" |
| A text search (`q`) with smart tags in its URL looks up the tags' names (up to 20) without waiting for the visitor's search, so a visitor whose search a rate limit refused still costs those lookups; a search without `q` skips them (`tagLabels` in `web/src/lib/server-data.ts`) | Waiting would hold the page frame until the query was embedded, where it used to draw first and stream the results; the lookups are in-memory lists, cached for hours by the web server, and the server's own requests are not rate-limited (owner, same day) | 2026-11-17: "同意。体验优先" |
| Visitors who reach the site over a LAN, and local development, are not rate-limited at all, sign-in and registration included; the per-account sign-in lockout still applies | Their client IP is internal, the address the site's own server has (`own_server` in `crates/api/src/limit.rs`), and the server is never rate-limited | 2026-11-17: "接受" |
| Saved-search feeds (`/api/rss`) search the words of their query alone, while `/api/search` and the palette (`/api/palette`) also search the BOOTH tags that translate them (`crates/api/src/expansion.rs`) | The translating tags are chosen anew from each snapshot's titles, so a feed would take in other items whenever titles change; leaving them out keeps a feed's items from moving with that choice. A feed's words still match the tag dictionary's words, as every search's do (`crates/api/src/tag_words.rs`) | 2026-11-21, relayed by the coordinator: "The palette (Ctrl+K) also uses the cross-language expansion; RSS stays as it is." |
| The guestbook stores a nickname and a message without a card-number check, an exception to "Sensitive values" (`crates/AGENTS.md`) for these two fields | The site takes no payments, so a visitor has no reason to type a card number; a check kept out only the plain spellings, and it refused about one message in ten that held two 7-digit numbers such as item ids. An admin hides or deletes a message. The message field carries `autocomplete="off"` and the nickname field `autocomplete="nickname"`, so a form filler does not take either for a card field | 2026-11-17: "整个去掉吧，反正PR还没合也没上线" |
| `web/tools/nav.cjs` (search, scroll and place restoration in a browser) stays a lab check and is not run in CI | A job would need PostgreSQL with data, the API, `next build`, the slow-API proxy and Chromium: about 5 min and a new source of flaky failures | 2026-11-15: "维持 lab"; closed as a decision on 2026-11-21 |
| The filter rail's ja and ko section names sit 1.0 to 1.34 px above their chevrons at 1440 and 2560 (1.5x), while zh and en are within 0.5 px | The kana and Hangul fallback fonts place their glyphs differently from Figtree's capitals and Chrome has no ideographic edge for `text-box`, so only a guessed nudge per locale is left, and 1 px is not visible | 2026-11-15: "1px肉眼看不出来，暂时不处理吧"; closed as a decision on 2026-11-21 |
| Below 375 px the top bar wraps to two rows with the guestbook button: at 320 and 340 px in English and at 340 and 360 px in Japanese (with `MobileNavToggle` added below `lg`, 340–390 px stay on one 48 px row with zero overflow across all four locales and 320 px stays on one 48 px row with 8 px horizontal overflow in signed-out Japanese) | The supported phone width is 390 px ("Numbers"), where the bar stays on one row, and the site is used mainly on desktop; moving the button into the account menu on narrow screens would hide it from visitors who never open that menu | 2026-11-21, reviewing the older deferred options: closed as a decision |
| The admin guestbook row is not checked at phone width, where a hidden, closed message carries two `Token`s, a time and three buttons at its end | The console is used on a desktop | 2026-11-21, reviewing the older deferred options: closed as a decision |
| Opening a modal (the guestbook drawer, settings) on a page with many tiles recalculates the whole page's style: `showModal()` took 37 ms at 2560x1440 with about 9,700 elements against 10 ms on a phone, and two fewer elements per tile (closed PR #81) changed nothing measurable. `content-visibility: auto` on the tiles was tried and not kept: with 12,528 elements at 2560x1440 the median of 25 calls went from 44.5-44.8 ms to 36.4-38.0 ms over three rounds each (2026-11-21, lab, host CPU 0-7%), still far above a phone's, and it needs a guessed intrinsic height per tile, a literal size the tile's width changes | The cost is the browser's, and the tried change takes off about a sixth of it for a guessed size that scroll restoration would have to live with | 2026-11-17: accepted; 2026-11-21: the experiment's keep-or-drop rule (a clear drop, `web/tools/nav.cjs` green and the size budget held); closed as an accepted cost |
| The stricter limits' pass is bound to the address range: one pass, copied, serves every address of its IPv4 /24 or IPv6 /64 for its 10 minutes | A visitor whose address changes inside the range keeps the pass; even copied, a pass gives each address only today's numbers | 2026-11-17, the approved plan; closed as a decision on 2026-11-21 |
| With the stricter limits on, a visitor without a pass reads at most 15 records a second, burst 1,200, from `/api/search` (its `pageSize`), `/api/items` with several ids (its ids) and `/api/rss` (its 50 items; refused with a plain 429, as a feed reader cannot pass the check, and shared with any visitor behind the same address) together, beside the request buckets (`Limits::records` in `crates/api/src/limit.rs`); a pass or the switch off pays no records, and a session whose sign-in passed the check pays them until the strict buckets refuse, when it is read and admitted at today's numbers. The same scripted browsing session in the lab, replayed through that bucket: 1,440x900 left 922 of the burst at its lowest, 2,560x1,440 left 625, and 3,840x2,160 (4K at 100%) was refused 4 times, the first after 57 s, so a long scroll at that size is offered the check after about a minute | The records a request asks for are what a scraper wants; requests alone let one address read 100 records a request at the search's rate. The owner's screen is 4K at 150% (2,560 CSS px), where the session is never refused; a refused visitor passes the check and gets today's numbers | 2026-11-21: 15 records a second with a burst of 1,200 approved, and kept after seeing the three window sizes |
| Related-search filtering compares labels in the visitor's language (`crates/api/src/related/mod.rs`), leaving out homographs (e.g. `hl=ko` `머리`), splits query words on spaces only (so `short-hair` still offers `hair`), and kanji-only Japanese queries are measured under the Chinese expansion population (`crates/api/src/expansion.rs`) | Homographs read identically to the visitor, `hair` is a valid wider chip, and script alone cannot distinguish kanji-only Japanese from Chinese while Japanese queries already match mostly Japanese tags | 2026-11-25, reviewing Deferred options: "按你说的来" (closed as a decision) |
| Once strict buckets refuse, a request carrying a session cookie draws from today's bucket before reading the session row; each unjudged `/api/verify` writes one warn line bounded per client address only | Drawing first bounds database session reads regardless of forged cookies; a global log cap would hide the scope of a real Cloudflare outage | 2026-11-25, reviewing Deferred options: "按你说的来" (closed as a decision) |
| Typo-tolerant avatar recognition (`split_names` in `crates/api/src/snapshot.rs`) accepts a word one edit from an avatar name when the catalogue does not know the word as typed, with `typo=0` as the UI escape | Measured on 163 distinct production embedded search texts with 0 false readings; the UI offers the exact typed words | 2026-11-25, reviewing Deferred options: "按你说的来" (closed as a decision) |
| Title script and held-name checks (`crates/worker/src/ai/check.rs`, `crates/worker/src/jobs/avatar_kit.rs`) mask only an avatar's own names (not aliases) and match written forms via substring on folded text | Aliases feed only linking and search; restricting title checks to avatar names matches the owner's ruling | 2026-11-25, reviewing Deferred options: "按你说的来" (closed as a decision) |

Outside requests allowed in local runs. Each relaxes the root `AGENTS.md` hard limit "no test traffic to a real
upstream unless the owner allows it" for that request only:

| Request | Why | Owner ruling (date, quote) |
| --- | --- | --- |
| `api.frankfurter.dev`, from `exchangeRates` (`web/src/lib/server-data.ts`), in local development and test runs of the web site | It renders prices in a currency other than yen; the URL is a literal that no setting points elsewhere or turns off, and the request carries no visitor data | 2026-11-13: "允许本地请求" |
| `challenges.cloudflare.com`: the Turnstile widget and siteverify, with Cloudflare's test keys or the site's own, in local development and manual test runs of the guestbook (and, on the same terms, of the check the stricter rate limits offer: brief of 2026-11-17); with Cloudflare's published testing keys only (the always-pass and always-block pairs), from the local browser and API, one manual run of the console's check of new keys for sign-in | The widget and its check exist only at Cloudflare; the automated tests check tokens against a local stand-in (`crates/api/src/guestbook_tests.rs`, `crates/api/src/admin/turnstile.rs`) and send nothing | 2026-11-17: "可向cloudflare发请求，真的总是好的"; the console's check, 2026-11-21: allowed for one manual local run with the testing keys |

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
| Expand more Chinese queries: production has a Chinese title on 8.7% of items (19,451 of 222,883), and a Chinese query is measured among those (`crates/api/src/expansion.rs`). With the 12,728 production Chinese titles whose items are in the development catalogue, 长发, 夹克, 尾巴, 和服 and 猫 expand (长发 is also searched as ロングヘア and ロング: 43 to 3,234 items), while 短发, 双马尾 and 发型 miss the coverage of 0.25 (0.24, 0.19, 0.23) and 耳环 the containment of 0.7 (0.69). Chinese titles are segmented with Lindera's Chinese dictionary (CC-CEDICT) since 2026-11-21; the Japanese one (IPADIC) cut 水着 out of 水着色器 (water shader) | the expansion run on the development catalogue with production's Chinese titles (read-only export, owner's go), 2026-11-20; the evaluation of the translated titles, 2026-11-21 | The near misses rest on 7 to 68 items each, so a lower threshold would fit noise. Chinese titles translated locally for the items without one are imported into production once, with the owner's go, after the tag dictionary's pull request. With 119,244 of them on the development catalogue, 短发, 双马尾, 耳环 and 眼镜 expand at the current thresholds, and 长发 no longer does (2,178 items fewer; 187 of them are long boots, skirts, coats or dresses, 28 have long hair in the title or tags) |
| Measure the text index build with the tag dictionary in production: on the development host (32 threads) the dictionary's words raise the build from 6.6-7.4 s to 11.2-15.2 s, with no measurable change in peak memory or index size | the tag dictionary's measurements, 2026-11-21 | Production runs on 4 vCPU, where the build is slower, and a reload rebuilds the index whenever new items or dictionary rows changed its text; each production read needs the owner's go |
| A bound on imgproxy's upstream requests in flight from all clients together (a global miss limit) | security scan, 2026-11-15; left after the per-client miss limit (100/s, burst 2,000) | Each client's misses are bounded and the body budget caps memory, but many clients together can still send any number of misses at once; a global bound would refuse every visitor's images during a spread-out flood, so it needs the owner's choice of a number. A client holding an IPv6 /48 has the same reach: each /64 is its own bucket, and touching 8,192 other /64s evicts its own drained bucket within the refill time (the API's limits behave the same; review of the miss limit, 2026-11-15). A miss opens its upstream request before it waits for the body budget, so each waiting miss holds an upstream connection for up to `FETCH_WAIT`; the number is picked after measuring cold-cache browsing and a slow upstream, and a bound would come with a short queue rather than an outright refusal (outside review, 2026-11-15) |
| Extend the `Content-Security-Policy` in `web/next.config.ts` with `script-src` and `style-src` | the change that sent `img-src 'self' data:`; `object-src 'none'`, `base-uri 'self'` and `frame-ancestors 'none'` were added on 2026-11-15 | Without `'unsafe-inline'` either directive needs a nonce per request, and Next applies nonces only to dynamically rendered pages, so the statically prerendered parts (`/privacy`, `/terms`, the page shells) would turn dynamic; each directive needs its own sweep of every page |
| Make the reduce-motion switch reach the Astryx motion that follows only the system's setting: the entrances of dialogs, menus, popovers, tooltips and selectors (StyleX CSS under `prefers-reduced-motion` media queries, `Layer/layerAnimations`, `Dialog`), the press feedback, the Carousel's smooth scroll on its prev/next buttons, and the AppShell, MobileNav, Resizable, BottomSheet and swipe timings that read `matchMedia` | the reduce-motion switch, 2026-11-15 | Astryx 0.7.16 has no public way in: its private `data-astryx-reduced-motion` mark reaches only its animator, and its public theme knob `reducedMotion: 'always'` is not shipped yet. Each would take an override of hashed StyleX classes or a patch of the library; the entrances are at most 200 ms and the switch already stops the site's own motion and the Skeleton pulse (`tools/motion.cjs` lists what still runs) |
| Turnstile on forms open to abuse other than the guestbook, sign-in and registration | owner, 2026-11-17, while planning the guestbook | No such form is named yet |
| A pass that stops counting when a visitor's requests alternate between IPv4 and IPv6, or between /24s of a carrier NAT pool: a pass names one range, so each request from the other range is strict again, and the dialog may reopen after `router.refresh()` | independent review of PR #83, 2026-11-17 (hypothesis, not run) | It needs a dual-stack or carrier NAT client to reproduce; binding a pass to more than one range is a design change the owner has not asked for |
| Add shared bases as avatar rows: まめふれんず, PlusHead, MARUBODY, Bodyset2, LSBody, EFBody, 珍飯亭ボディ | avatar research, 2026-11-21 | The owner chose not now (2026-11-21); each base would get its own page and search key |
| Keep the matcher from reading クマリ inside ブラックマリア, ブラックマリー, クラシックマリィ and ミクマリ: a Japanese name matches anywhere in the text | relink sample on the development catalogue, 2026-11-21 (2 of 30 sampled alias links) | The owner named クマリ as an alias; a boundary rule for Japanese names changes the matcher for every avatar |
| Leave an avatar's own body listing out of its links: the matcher links a body listing to the avatar it sells (3 of 100 sampled relink links), where VRCatalogue's links never do | relink sample on the development catalogue, 2026-11-21 | `enrich` links the same way today; changing both is outside the change that added the avatars |
| Mark a body listing that is crawled after its avatar row: migration 0032 sets `avatar_slug`, `supported` and `gender` on the body listings already in the catalogue, and one crawled later keeps the defaults, as the starter rows' body listings do on a fresh install | review of the migration 0032 plan, 2026-11-21 | The crawler and `enrich` do not read the avatars table for this; a body listing missing from the catalogue today is rare |
| Add the original Masscat (ますきゃっと, 量産型のらきゃっと) as an avatar of its own; its listings are not linked to ますきゃっとぷらす | alias measurement for migration 0032, 2026-11-21 | It was not among the measured candidates |
| Check the gender of the 42 added avatars without one and the Hepburn romaji of the 68 whose body listing shows no Latin name | avatar research, 2026-11-21 | It needs reading the listing images and the shops' pages |
| Consolidate Saved searches' sign-in dialog in `web/src/components/catalog/toolbar.tsx` onto `AccountProvider.openSignIn` instead of its pre-existing local `dynamic` `SignInDialog` | fresh-context diff review, 2026-11-26 | Pre-existing modal outside TopNav navigation; can migrate if `check:size` stays within the 530,000 B JS budget |

## Open questions

Questions no rule settles, for the owner. They are asked together at a stopping point; a ruling moves the item
to the file it decides.

| Question | Source | Raised |
| --- | --- | --- |
