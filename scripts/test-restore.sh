#!/usr/bin/env bash
# Restores a data bundle's catalogue.dump into a throwaway Postgres container the way DEPLOY.md says, applies a
# worker's migrations to it (as a deployment does at startup), and prints row counts. CI runs it on the public bundle
# (.github/workflows/migrations.yml) with a worker built from the branch.
#   [PODMAN=docker] [WORKER_IMAGE=ghcr.io/johnsonran/boothdex-worker:latest] scripts/test-restore.sh <bundle-dir>
#   [PODMAN=docker] WORKER_BIN=target/debug/worker scripts/test-restore.sh <bundle-dir>
# Exit 1: a migration failed. Exit 2: the check could not run, or found no migration to apply.
set -euo pipefail
bundle="${1:?usage: scripts/test-restore.sh <bundle-dir>}"
podman="${PODMAN:-podman}"
export MSYS_NO_PATHCONV=1
name=catalogue-restore-test
port="${RESTORE_PORT:-55432}"
cannot() { echo "test-restore: $*" >&2; exit 2; }

[ -s "$bundle/catalogue.dump" ] || cannot "no catalogue.dump in $bundle"
"$podman" rm -f "$name" >/dev/null 2>&1 || true
# the container holds the bundle's data, which can be a production backup: remove it however the script ends
trap '"$podman" rm -f "$name" >/dev/null 2>&1 || true' EXIT
publish=()
[ -n "${WORKER_BIN:-}" ] && publish=(-p "127.0.0.1:$port:5432")
"$podman" run -d --name "$name" "${publish[@]}" -e POSTGRES_DB=catalogue -e POSTGRES_USER=catalogue \
  -e POSTGRES_PASSWORD=x docker.io/pgvector/pgvector:pg17 >/dev/null || cannot "could not start Postgres"
psql() { "$podman" exec -i "$name" psql -U catalogue -d catalogue -tA "$@"; }
# TCP, not the socket: the image's first-start setup runs a server on the socket only, then restarts it
ready=
for _ in $(seq 1 60); do
  "$podman" exec "$name" pg_isready -h 127.0.0.1 -U catalogue -d catalogue >/dev/null 2>&1 && { ready=1; break; }
  sleep 1
done
[ -n "$ready" ] || cannot "Postgres did not start"

echo "[$(date +%T)] restoring"
"$podman" exec -i "$name" pg_restore -U catalogue -d catalogue --no-owner --role=catalogue < "$bundle/catalogue.dump" ||
  cannot "pg_restore failed"
items=$(psql -c 'select count(*) from items') || cannot "the dump has no items table"
[ "$items" -gt 0 ] || cannot "the dump restored no items"
before=$(psql -c 'select coalesce(max(version), 0) from _sqlx_migrations')

echo "[$(date +%T)] restored $items items at migration $before, migrating"
if [ -n "${WORKER_BIN:-}" ]; then
  DATABASE_URL="postgres://catalogue:x@127.0.0.1:$port/catalogue" "$WORKER_BIN" migrate || exit 1
else
  image="${WORKER_IMAGE:-ghcr.io/johnsonran/boothdex-worker:latest}"
  "$podman" pull -q "$image" >/dev/null || cannot "could not pull $image"
  "$podman" run --rm --network "container:$name" -e DATABASE_URL=postgres://catalogue:x@localhost:5432/catalogue \
    "$image" worker migrate || exit 1
fi

applied=$(psql -c "select string_agg(version::text, ' ' order by version) from _sqlx_migrations where version > $before")
[ -n "$applied" ] || cannot "no migration newer than $before to apply"
echo "[$(date +%T)] applied migrations: $applied"
psql -c "
  select 'items', count(*) from items union all
  select 'booth_images', count(*) from booth_images union all
  select 'avatars', count(*) from avatars union all
  select 'users', count(*) from users union all
  select 'job_schedules', count(*) from job_schedules union all
  select 'worker_status', count(*) from worker_status union all
  select 'catalog_meta', count(*) from catalog_meta"
echo "[$(date +%T)] ok"
