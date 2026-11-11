# Requirements

This project's checks and numbers for the rules in [AGENTS.md](AGENTS.md).

## Checks and time limits

Run the checks for the parts a change touches. A suite that runs past its limit is made faster before tests are added.

| Part | Commands | Time limit |
|---|---|---|
| Rust | `cargo test --workspace -- --include-ignored`, with `DATABASE_URL` set as in the README | 3 min, not counting the build |
| Web | `bun test`, `bunx tsc --noEmit` and `bunx biome check` in `web/` | 15 s for `bun test` |

On 2026-11-11 the Rust suite ran 107 tests in 89–116 s, and `bun test` ran 103 tests in 3 s.

## Performance baseline

`scripts/bench/run.sh` measures the database side of opening an item, trimming viewed items, reading and changing
favourites, and the refresh order, on a throwaway Postgres with synthetic data. Run it before and after a change to
these statements on the same machine. Compare the two runs, because the absolute numbers depend on the machine.

Baseline from 2026-11-11, 16 clients, two runs:

| Script | Latency | Throughput |
|---|---|---|
| `item_open` | 3.8 ms | 4,200/s |
| `trim` | 0.24 ms | 66,000/s |
| `favourites_read` | 0.21–0.25 ms | 64,000–77,000/s |
| `favourite_toggle` | 15–17 ms | 950–1,050/s |
| `refresh_priority` (top 1,500) | 146–164 ms | |

`favourite_toggle` is a worst case that was accepted on purpose. Every client clicks the same popular items, so the
`count_favourite` trigger's updates of their `item_stats` rows wait for each other. When the clicks are spread over
many items, adding and removing a favourite costs the same as it did without the trigger.

The baseline was measured after these paths were reworked on 2026-11-11. Before the rework, the same statements on the
same data (in the development database, not with this script) took 7.3 ms per item open, 0.53–0.61 ms per trim,
366–519 ms for `refresh_priority` and 6.5 ms per favourite toggle.
