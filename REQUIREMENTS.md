# REQUIREMENTS.md: numbers, gates and exceptions

Commands, numbers, gates and exceptions for this repository, and its deferred options and open questions. The rules themselves are in `AGENTS.md`,
`.github/AGENTS.md`, `web/AGENTS.md` and `crates/AGENTS.md` and are not repeated here. When another
file in this repository disagrees with this one on a command, a number, a gate or an exception, this
file wins.

`scripts/check-requirements.py` checks this file in the merge gate. It reads files and runs
nothing. Write a pipe inside a cell as `\|`. Write a command in backticks; the check confirms that
a script named in it exists.

## Rule files

Every `AGENTS.md` in this repository, by path. The check reads rule groups only from the files
marked `yes`, and fails on an `AGENTS.md` it finds that is not listed. It searches every directory
except `.git`, `node_modules`, `vendor`, `third_party` and hidden directories other than
`.github`. Mark a file `no`, with the reason in Note, when its rules are not this project's (a
vendored package, a generated tree).

| File | Checked | Note |
| --- | --- | --- |
| AGENTS.md | yes | |
| .github/AGENTS.md | yes | |
| web/AGENTS.md | yes | |
| crates/AGENTS.md | yes | |

## Commands

One row per tier in `.github/AGENTS.md`, "Tiers, jobs and gates". Write the exact command that a
person and CI both run. Fill in this order: the command; then three runs on the same host, recorded
as the baseline; then the limit, as the baseline plus a margin (start at 20%).

| Tier | Command | Runs (where, when) | Wall-time limit | Baseline (date, host, load) |
| --- | --- | --- | --- | --- |
| Merge gate | `cargo test --workspace -- --include-ignored` with `DATABASE_URL` set as in the README, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test -p imgproxy --features libavif`; in `web/`: `bun test`, `bunx next typegen`, `bunx tsc --noEmit`, `bunx biome check`; `python3 scripts/check-requirements.py` | CI: `.github/workflows/requirements.yml` and `.github/workflows/test.yml` on every pull request, both required to merge into master; on a push to master, requirements.yml always and test.yml through `.github/workflows/images.yml` | 3.5 min | 174 s, 188 s, 154 s, slowest job `rust` (2026-11-11, GitHub-hosted ubuntu-latest, load not recorded) |
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
| Page frame width; readable column width | | | review |
| Supported widths (phone, laptop, wide) | 390 px phone, 1440 px desktop | a setting, not measured; the widths `web/tools/loading-layout.cjs` checks | screenshots, `web/tools/loading-layout.cjs` (lab) |
| Rust tests, not counting the build | 3 min | 109 tests in 4 s (2026-11-11, development host, load not recorded) | review |
| `bun test` in `web/` | 15 s | 103 tests in 3 s (2026-11-11, development host, load not recorded) | review |
| Slowest test package | 16 s | `api` unit tests: 12.1 s, 15.4 s, 12.4 s (2026-11-11, GitHub-hosted ubuntu-latest, load not recorded) | review |
| Home page first load, gzip -9: HTML | 49,500 B | 47,500 to 49,000 B over 7 reads (2026-11-12, local `next start`, load not recorded) | `bun run check:size` in `web/` (lab) |
| Home page first load, gzip -9: JS | 410,000 B | 396,067 B (2026-11-12, local `next start`, load not recorded) | `bun run check:size` in `web/` (lab) |
| Home page first load, gzip -9: CSS | 45,750 B | 45,503 B (2026-11-12, local `next start`, load not recorded) | `bun run check:size` in `web/` (lab) |
| Source file size, p90 of this repository (pass bar) | 274 lines | 274 lines: nearest-rank p90 of the 272 tracked `.rs`, `.ts`, `.tsx`, `.mjs`, `.cjs`, `.css`, `.sql`, `.sh` and `.py` files outside `web/templates` (2026-11-12, `wc -l`) | review |
| Wait timeout before a stuck step is recorded and left | 30 min | a setting, not measured | agent |
| Context handoff limit (share of the model's window) | 60% | a setting, not measured | orchestrator |
| Status check interval while several jobs run | 10 min | a setting, not measured | orchestrator |
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

One row per rule group of every file checked under "Rule files": each `###` heading, named
`path: heading`, and each rule tagged `[rule: <id>]`, named `path#id`. The check reports a group
with no row, and a row whose group no longer exists, so renaming a heading means renaming its row.

Fill one or both of:

- Gate command: the command, in backticks, that turns red on a violation, run by a tier in
  "Commands";
- Review criterion: the `REVIEW.md` section that checks it, with the file name in backticks.

Or fill only "Does not apply because" with what the project lacks (no admin panel, no i18n). A
tagged rule marked as not applying needs the owner's approval, quoted in the commit body.

| Rule group | Gate command | Review criterion | Does not apply because |
| --- | --- | --- | --- |
| AGENTS.md: Change risk | | `REVIEW.md`, "Scope" | |
| AGENTS.md: Workflow | | `REVIEW.md`, "Pass bar" | |
| AGENTS.md: When unsure | | `REVIEW.md`, "Scope" | |
| AGENTS.md: Waiting | | | |
| AGENTS.md: Reporting | | | |
| AGENTS.md: Code quality | `cargo clippy --workspace --all-targets -- -D warnings`; in `web/`: `bunx biome check` | `REVIEW.md`, "Pass bar" | |
| AGENTS.md: Impact and edge cases | | `REVIEW.md`, "Checking against the rules", Newly reachable defects | |
| AGENTS.md: Secrets | | `REVIEW.md`, "Checking against the rules", Sensitive data | |
| AGENTS.md: Performance baseline | | | |
| AGENTS.md: Tests (if present) | | `REVIEW.md`, "CI, docs and commits", CI | |
| AGENTS.md: Merging | | `REVIEW.md`, "Report", verdict rule | |
| AGENTS.md: Model usage | | `REVIEW.md`, "Before you start" | |
| AGENTS.md: Documentation | | `REVIEW.md`, "CI, docs and commits", Docs | |
| AGENTS.md: Writing (docs, comments, reports, PR and issue text) | | `REVIEW.md`, "CI, docs and commits" | |
| AGENTS.md: Commits and pull requests | | `REVIEW.md`, "CI, docs and commits", Commits | |
| AGENTS.md: Frontend–backend mapping (when both exist) | | `REVIEW.md`, "Checking against the rules"; "Exceptions" below | |
| .github/AGENTS.md: Tiers, jobs and gates | `python3 scripts/check-requirements.py` | `REVIEW.md`, "CI, docs and commits", CI | |
| web/AGENTS.md: Components and modularity | | `REVIEW.md`, "Checking against the rules" | |
| web/AGENTS.md: Consistency | | `REVIEW.md`, "Checking against the rules"; lab check `web/tools/motion.cjs` | |
| web/AGENTS.md: Layout, type and loading | | `REVIEW.md`, "Checking against the rules", Layout, type and loading; lab check `web/tools/loading-layout.cjs` | |
| web/AGENTS.md: i18n (if present) | in `web/`: `bunx tsc --noEmit` (every locale holds exactly the keys of `web/src/lib/messages-zh.ts`) | `REVIEW.md`, "Checking against the rules", Copy and i18n | |
| web/AGENTS.md: Usability | | `REVIEW.md`, "Checking against the rules" | |
| web/AGENTS.md: Accessibility | | `REVIEW.md`, "Checking against the rules", Accessibility | |
| web/AGENTS.md: Performance | | `REVIEW.md`, "Checking against the rules"; lab check `bun run check:size` in `web/` | |
| crates/AGENTS.md: APIs and modularity | | `REVIEW.md`, "Checking against the rules" | |
| crates/AGENTS.md: Compatibility | `cargo test --workspace -- --include-ignored` (applies every migration to an empty database) | `REVIEW.md`, "Checking against the rules", Migrations | |
| crates/AGENTS.md: Logging | | `REVIEW.md`, "Checking against the rules" | |
| crates/AGENTS.md: Security (if present) | | `REVIEW.md`, "Checking against the rules", Authorization and Sensitive data | |
| crates/AGENTS.md#authz | `cargo test --workspace -- --include-ignored` (`crates/api/src/admin/authz.rs`: every admin route answers 401 signed out and 403 to a member) | `REVIEW.md`, "Checking against the rules", Authorization | |
| crates/AGENTS.md#sensitive-values | | `REVIEW.md`, "Checking against the rules", Sensitive data | |
| crates/AGENTS.md: Money and state (if present) | | `REVIEW.md`, "Checking against the rules", Money and state (retried worker jobs and requests) | |
| crates/AGENTS.md#money-idempotent | | | BoothDex moves no money; item prices are catalogue data |
| crates/AGENTS.md#ledger | | | BoothDex keeps no ledger |
| crates/AGENTS.md: Background work (if present) | | `REVIEW.md`, "Checking against the rules" | |
| crates/AGENTS.md: Operations (if present) | | `REVIEW.md`, "Checking against the rules"; restore rehearsal `scripts/test-restore.sh` | |
| crates/AGENTS.md: Admin panel (if present) | | `REVIEW.md`, "Checking against the rules" | |
| crates/AGENTS.md: Performance | | `REVIEW.md`, "Checking against the rules"; benchmark `scripts/bench/run.sh` | |
| crates/AGENTS.md: Verification | `cargo test --workspace -- --include-ignored` (against PostgreSQL in CI) | `REVIEW.md`, "Checking against the rules" | |

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
| Numbers: Page frame width; readable column width | widths are set per component; no shared value | measure the frames and set one width per kind of page | 2026-12-31 |
| Enforcement: AGENTS.md: Waiting | the owner reads the agent's report; no `REVIEW.md` section covers it | the owner decides how it is checked | 2026-12-31 |
| Enforcement: AGENTS.md: Reporting | the owner reads the agent's report; no `REVIEW.md` section covers it | the owner decides how it is checked | 2026-12-31 |
| Enforcement: AGENTS.md: Performance baseline | `scripts/bench/run.sh` measures; nothing compares a change with the baseline | the owner decides between a review item and a gate | 2026-12-31 |

## Deferred options

Options and findings set aside, with their source and the reason they wait. The owner decides from this list
after a release.

| Option | Source | Why it waits |
| --- | --- | --- |
| Move install and the shortest use in the three READMEs above "Capabilities", which runs for about 95 lines first | `AGENTS.md`, "Writing", README; review of 3045384 (2026-11-12) | rewriting the project page in three languages is a copy decision |
| Re-sign the 15 unsigned commits from ed15fcf to a007c85 (2026-10-31 to 2026-11-05) | `AGENTS.md`, "Commits and pull requests"; review of 3045384 (2026-11-12) | it rewrites published history |
| Release-only checks (a reproducible build, migrations on a copy of production data, a deploy rehearsal) and a planted violation for each gate command in the release tier | `.github/AGENTS.md`, "Tiers, jobs and gates" | the owner plans no release soon (2026-11-12) |
| Split `scripts/check-requirements.py` (314 lines) and `scripts/test_check_requirements.py` (346 lines), both above the p90 file size (274 lines) | `REVIEW.md`, "Pass bar"; review of 3045384 | the owner ruled on 2026-11-12 that they may diverge from the kit's copies; each works as one file, so the split waits for the next change to either |

## Open questions

Questions no rule settles, for the owner. They are asked together at a stopping point; a ruling moves the item
to the file it decides.

| Question | Source | Raised |
| --- | --- | --- |

## Example (illustrative values; measure your own)

| Tier | Command | Runs (where, when) | Wall-time limit | Baseline (date, host, load) |
| --- | --- | --- | --- | --- |
| Pre-commit | `scripts/check-all.sh --fast` | locally, every commit | 5 s | 3.8 s (2026-11-12, build host, load 0.4) |
| Merge gate | `scripts/check-all.sh --strict` | CI, every merge | 15 min | 11 min (2026-11-12, build host, load 1.2) |

| Rule group | Gate command | Review criterion | Does not apply because |
| --- | --- | --- | --- |
| web/AGENTS.md: i18n (if present) | `scripts/check-i18n.sh` | | |
| server/AGENTS.md#authz | `scripts/check-authz.sh` | `REVIEW.md`, "Checking against the rules", Authorization | |
| server/AGENTS.md: Admin panel (if present) | | | the project has no admin panel |
| AGENTS.md: Writing (docs, comments, reports, PR and issue text) | | `REVIEW.md`, "CI, docs and commits" | |

| Row | Current state | Plan | Due |
| --- | --- | --- | --- |
| Numbers: Slowest test package | not measured | measure after the race suite lands | 2027-01-31 |
