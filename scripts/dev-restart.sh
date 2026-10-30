#!/usr/bin/env bash
# Stops the given Rust services, waits until they have really exited (taskkill returns before the process is
# gone, which made "already running" checks race), optionally rebuilds, then starts them detached.
#   scripts/dev-restart.sh [--build] api worker imgproxy
set -euo pipefail
cd "$(dirname "$0")/.."
build=false
[[ "${1:-}" == --build ]] && { build=true; shift; }
for s in "$@"; do taskkill //F //IM "$s.exe" >/dev/null 2>&1 || true; done
for s in "$@"; do
  for _ in $(seq 1 50); do tasklist //FI "IMAGENAME eq $s.exe" | grep -qi "$s.exe" || break; sleep 0.2; done
done
if $build; then
  args=(); for s in "$@"; do args+=(-p "$s"); done
  cargo build -q --release "${args[@]}"
fi
bash scripts/dev-up.sh "$@"
