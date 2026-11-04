#!/usr/bin/env bash
# Restores the data bundle into a throwaway Postgres container the way DEPLOY.md says, runs the migrations of the
# published worker image on it (as a deployment would at startup), and checks row counts.
#   [WORKER_IMAGE=ghcr.io/johnsonran/boothdex-worker:latest] scripts/test-restore.sh <bundle-dir>
set -euo pipefail
bundle="${1:?usage: scripts/test-restore.sh <bundle-dir>}"
podman="${PODMAN:-podman}"
export MSYS_NO_PATHCONV=1
name=catalogue-restore-test
"$podman" rm -f "$name" >/dev/null 2>&1 || true
"$podman" run -d --name "$name" -e POSTGRES_DB=catalogue -e POSTGRES_USER=catalogue -e POSTGRES_PASSWORD=x \
  docker.io/pgvector/pgvector:pg17 >/dev/null
for _ in $(seq 1 60); do "$podman" exec "$name" pg_isready -U catalogue -d catalogue >/dev/null 2>&1 && break; sleep 1; done
sleep 2
echo "[$(date +%T)] restoring"
"$podman" exec -i "$name" pg_restore -U catalogue -d catalogue --no-owner --role=catalogue < "$bundle/catalogue.dump"
echo "[$(date +%T)] restored, migrating"
"$podman" run --rm --pull=always --network "container:$name" -e DATABASE_URL=postgres://catalogue:x@localhost:5432/catalogue \
  "${WORKER_IMAGE:-ghcr.io/johnsonran/boothdex-worker:latest}" worker migrate
"$podman" exec "$name" psql -U catalogue -d catalogue -tAc "
  select 'items', count(*) from items union all
  select 'booth_images', count(*) from booth_images union all
  select 'avatars', count(*) from avatars union all
  select 'users', count(*) from users union all
  select 'job_schedules', count(*) from job_schedules union all
  select 'worker_status', count(*) from worker_status union all
  select 'catalog_meta', count(*) from catalog_meta"
"$podman" exec "$name" psql -U catalogue -d catalogue -tAc "select 'migration', version, description, success from _sqlx_migrations order by 1"
"$podman" rm -f "$name" >/dev/null
echo "[$(date +%T)] ok"
