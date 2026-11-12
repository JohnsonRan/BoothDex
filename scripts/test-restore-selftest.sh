#!/usr/bin/env bash
# Self-test of scripts/test-restore.sh (.github/AGENTS.md, "Tiers, jobs and gates"). Builds a small dump with the
# worker's migrations and one item, then plants a violation: an applied migration whose checksum no longer matches
# its file, which the worker refuses as a deployment would. The check must exit 1 on the planted dump, and 2 on the
# unplanted one, which holds no migration to apply.
#   [PODMAN=docker] WORKER_BIN=target/debug/worker scripts/test-restore-selftest.sh
# Exit 1: the check gave a wrong answer. Exit 2: the self-test could not run.
set -euo pipefail
cannot() { echo "test-restore-selftest: $*" >&2; exit 2; }
cd "$(dirname "$0")/.." || cannot "could not enter the repository"
[ -x "${WORKER_BIN:-}" ] || cannot "set WORKER_BIN to a built worker"
podman="${PODMAN:-podman}"
export MSYS_NO_PATHCONV=1
name=catalogue-selftest
port=55433
dir=$(mktemp -d) || cannot "could not create a temporary directory"
trap '"$podman" rm -f "$name" >/dev/null 2>&1 || true; rm -rf "$dir"' EXIT

"$podman" rm -f "$name" >/dev/null 2>&1 || true
"$podman" run -d --name "$name" -p "127.0.0.1:$port:5432" -e POSTGRES_DB=catalogue -e POSTGRES_USER=catalogue \
  -e POSTGRES_PASSWORD=x docker.io/pgvector/pgvector:pg17 >/dev/null || cannot "could not start Postgres"
psql() { "$podman" exec -i "$name" psql -U catalogue -d catalogue -q -v ON_ERROR_STOP=1 "$@"; }
dump() { mkdir "$dir/$1" && "$podman" exec "$name" pg_dump -U catalogue -d catalogue -Fc > "$dir/$1/catalogue.dump"; }
for _ in $(seq 1 60); do
  "$podman" exec "$name" pg_isready -h 127.0.0.1 -U catalogue -d catalogue >/dev/null 2>&1 && break
  sleep 1
done
DATABASE_URL="postgres://catalogue:x@127.0.0.1:$port/catalogue" "$WORKER_BIN" migrate >/dev/null ||
  cannot "the worker could not migrate an empty database"
psql -c "insert into items (id, title, author_name, author_uri, is_adult, status, scraped_at)
         values (1, 't', 'c', 'https://example.test', false, 'ok', now())" || cannot "could not insert an item"
dump clean || cannot "could not dump"
psql -c "update _sqlx_migrations set checksum = '\x00' where version = (select max(version) from _sqlx_migrations)" ||
  cannot "could not plant the violation"
dump planted || cannot "could not dump"

expect() {
  local want=$1 got=0
  scripts/test-restore.sh "$dir/$2" >"$dir/log" 2>&1 || got=$?
  [ "$got" -eq "$want" ] || { cat "$dir/log"; echo "test-restore-selftest: $2 dump: exit $got, expected $want" >&2; exit 1; }
  echo "test-restore-selftest: $2 dump: exit $got, as expected"
}
expect 1 planted
expect 2 clean
