#!/usr/bin/env bash
# Database cost of the account data paths (viewed items, favourites) and the refresh order, measured with pgbench on a
# throwaway Postgres with synthetic data (seed.sql). The *.pgbench scripts copy the API's statements; keep them in
# step with account/favourites.rs and account/views.rs. Compare with the baseline in REQUIREMENTS.md, or run it on the commit before a change.
#   [CLIENTS=16] [BENCH_SECONDS=15] scripts/bench/run.sh
set -euo pipefail
cd "$(dirname "$0")/../.."
podman="${PODMAN:-podman}"
export MSYS_NO_PATHCONV=1
name=boothdex-bench
clients="${CLIENTS:-16}"
"$podman" rm -f "$name" >/dev/null 2>&1 || true
trap '"$podman" rm -f "$name" >/dev/null' EXIT
"$podman" run -d --name "$name" -e POSTGRES_PASSWORD=x docker.io/pgvector/pgvector:pg17 >/dev/null
for _ in $(seq 1 60); do "$podman" exec "$name" pg_isready -U postgres >/dev/null 2>&1 && break; sleep 1; done
sleep 2
psql() { "$podman" exec -i "$name" psql -U postgres -q -v ON_ERROR_STOP=1 "$@"; }
for f in crates/core/migrations/*.sql; do psql -1 < "$f" >/dev/null; done
psql < scripts/bench/seed.sql
"$podman" exec "$name" mkdir /bench
for f in scripts/bench/*.pgbench; do "$podman" exec -i "$name" sh -c "cat > /bench/${f##*/}" < "$f"; done

echo "$clients clients, ${BENCH_SECONDS:-15} s each: average latency, throughput"
for f in scripts/bench/*.pgbench; do
  b="${f##*/}"
  printf '%-18s' "${b%.pgbench}"
  "$podman" exec "$name" pgbench -U postgres -n -M prepared -c "$clients" -j "$clients" -T "${BENCH_SECONDS:-15}" -f "/bench/$b" postgres 2>&1 \
    | awk '/^latency average/ {l = $4} /^tps/ {t = $3} END {printf "%8.2f ms %9.0f/s\n", l, t}'
done
printf '%-18s' refresh_priority
psql -tA -c "explain (analyze, format json) select id from refresh_priority order by priority desc limit 1500" \
  | grep -o '"Execution Time": [0-9.]*' | awk '{printf "%8.0f ms (top 1500)\n", $3}'
