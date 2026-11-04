#!/usr/bin/env bash
# Builds the data bundle for a fresh server, outside the repo:
#   catalogue.dump    pg_dump (custom format) of the dev database, without user/test data
#   images.tar        archived <sha1>.webp images of delisted items
#   SHA256SUMS
# Usage: ARCHIVE_IMAGES=<archived images dir> [PG_CONTAINER=boothdex-pg] scripts/make-bundle.sh <out-dir>   (takes a few minutes; run via scripts/detach.sh)
set -euo pipefail
out="${1:?usage: scripts/make-bundle.sh <out-dir>}"
images="${ARCHIVE_IMAGES:?set ARCHIVE_IMAGES to the archived images dir}"
podman="${PODMAN:-podman}"
pg="${PG_CONTAINER:-boothdex-pg}"   # the dev database container
mkdir -p "$out"
# paths after `podman exec` are inside the Linux container; stop git-bash rewriting /tmp to C:/...
export MSYS_NO_PATHCONV=1

echo "[$(date +%T)] dumping database"
# Rows of these tables are local test data (test account with a known password, its sessions/favourites/views,
# job history, test view counts, AI settings, search texts). Their schema is kept. worker_status and catalog_meta must
# keep their single row: the services update it rather than insert it.
"$podman" exec "$pg" pg_dump -U postgres -d catalogue -Fc -Z 6 \
  --exclude-table-data=users --exclude-table-data=sessions --exclude-table-data=favourites \
  --exclude-table-data=item_views --exclude-table-data=saved_searches --exclude-table-data=jobs \
  --exclude-table-data=item_stats --exclude-table-data=app_settings --exclude-table-data=query_embeddings \
  -f /tmp/catalogue.dump
# host side is a Windows program: give it a Windows path
"$podman" cp "$pg":/tmp/catalogue.dump "$(cygpath -w "$out")\catalogue.dump"
"$podman" exec "$pg" rm /tmp/catalogue.dump

echo "[$(date +%T)] packing $(ls "$images" | wc -l) images"
# b/ holds copies from the removed delisted-image archive job; nothing reads them any more
tar -cf "$out/images.tar" --exclude="$(basename "$images")/b" -C "$(dirname "$images")" "$(basename "$images")"

echo "[$(date +%T)] checksums"
(cd "$out" && sha256sum catalogue.dump images.tar > SHA256SUMS)
ls -la "$out"
echo "[$(date +%T)] done"
