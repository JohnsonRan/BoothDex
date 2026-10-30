#!/usr/bin/env bash
# Restores the data bundle into a throwaway Postgres container the way DEPLOY.md says, and checks row counts.
#   scripts/test-restore.sh [bundle-dir=../vrcat-bundle]
set -euo pipefail
bundle="${1:-../vrcat-bundle}"
podman="${PODMAN:-podman}"
export MSYS_NO_PATHCONV=1
name=catalogue-restore-test
"$podman" rm -f "$name" >/dev/null 2>&1 || true
"$podman" run -d --name "$name" -e POSTGRES_DB=catalogue -e POSTGRES_USER=catalogue -e POSTGRES_PASSWORD=x \
  -v "$(cygpath -w "$bundle"):/restore:ro" docker.io/pgvector/pgvector:pg17 >/dev/null
for _ in $(seq 1 60); do "$podman" exec "$name" pg_isready -U catalogue -d catalogue >/dev/null 2>&1 && break; sleep 1; done
sleep 2
echo "[$(date +%T)] restoring"
"$podman" exec "$name" pg_restore -U catalogue -d catalogue --no-owner --role=catalogue /restore/catalogue.dump
echo "[$(date +%T)] restored"
"$podman" exec "$name" psql -U catalogue -d catalogue -tAc "
  select 'items', count(*) from items union all
  select 'booth_images', count(*) from booth_images union all
  select 'avatars', count(*) from avatars union all
  select 'users', count(*) from users union all
  select 'job_schedules', count(*) from job_schedules union all
  select 'worker_status', count(*) from worker_status union all
  select 'catalog_meta', count(*) from catalog_meta union all
  select 'migrations', count(*) from _sqlx_migrations"
"$podman" rm -f "$name" >/dev/null
echo "[$(date +%T)] ok"
