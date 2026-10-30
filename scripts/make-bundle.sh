#!/usr/bin/env bash
# Builds the data bundle for a fresh server, outside the repo:
#   catalogue.dump    pg_dump (custom format) of the dev database, without user/test data
#   images.tar        archived <sha1>.webp images of delisted items
#   SHA256SUMS
# Usage: scripts/make-bundle.sh [out-dir=../vrcat-bundle]   (takes a few minutes; run via scripts/detach.sh)
set -euo pipefail
out="${1:-../vrcat-bundle}"
images=../vrcat-research/data/images
podman="${PODMAN:-podman}"
mkdir -p "$out"
# paths after `podman exec` are inside the Linux container; stop git-bash rewriting /tmp to C:/...
export MSYS_NO_PATHCONV=1

echo "[$(date +%T)] dumping database"
# Rows of these tables are local test data (test account with a known password, its sessions/favourites,
# job history, test view counts, AI settings). Their schema is kept. worker_status and catalog_meta must
# keep their single row: the services update it rather than insert it.
"$podman" exec vrcat-pg pg_dump -U postgres -d catalogue -Fc -Z 6 \
  --exclude-table-data=users --exclude-table-data=sessions --exclude-table-data=favourites \
  --exclude-table-data=saved_searches --exclude-table-data=jobs --exclude-table-data=item_stats \
  --exclude-table-data=app_settings \
  -f /tmp/catalogue.dump
# host side is a Windows program: give it a Windows path
"$podman" cp vrcat-pg:/tmp/catalogue.dump "$(cygpath -w "$out")\catalogue.dump"
"$podman" exec vrcat-pg rm /tmp/catalogue.dump

echo "[$(date +%T)] packing $(ls "$images" | wc -l) images"
tar -cf "$out/images.tar" -C "$(dirname "$images")" "$(basename "$images")"

echo "[$(date +%T)] checksums"
(cd "$out" && sha256sum catalogue.dump images.tar > SHA256SUMS)
ls -la "$out"
echo "[$(date +%T)] done"
