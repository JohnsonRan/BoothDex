#!/usr/bin/env bash
# Starts the local dev stack fully detached and returns at once. Logs: <repo>/<service>.log
#   scripts/dev-up.sh                 # api :3000, imgproxy :3001, worker, web :3100
#   scripts/dev-up.sh api imgproxy    # only these
# Processes are created via WMI (Win32_Process.Create) so they aren't children of this shell; tools and
# terminals that kill their process tree on exit would otherwise take the servers down. That one call is
# the only PowerShell use: git-bash has no way to escape the caller's job object.
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd -W)"
services=("${@:-api imgproxy worker web}")
read -r -a services <<< "${services[*]}"

envs="set DATABASE_URL=postgres://postgres:dev@localhost:5432/catalogue&& set COOKIE_SECURE=0&& set ADMIN_USERNAMES=${ADMIN_USERNAMES:-admin}&& set ARCHIVE_IMAGES=../vrcat-research/data/images&& set CACHE_DISK_MB=1024&& "

for name in "${services[@]}"; do
  cwd="$root"
  case "$name" in
    api) cmd="\"$root\\target\\release\\api.exe\"" ;;
    imgproxy) cmd="\"$root\\target\\release\\imgproxy.exe\"" ;;
    worker) cmd="\"$root\\target\\release\\worker.exe\" run" ;;
    web) cmd="bun run dev"; cwd="$root\\web" ;;
    *) echo "unknown service $name" >&2; continue ;;
  esac
  if [[ "$name" != web ]] && tasklist //FI "IMAGENAME eq $name.exe" | grep -qi "$name.exe"; then
    echo "$name already running"; continue
  fi
  line="cmd.exe /d /c \"$envs$cmd > \"$root\\$name.log\" 2>&1\""
  pid=$(powershell.exe -NoProfile -Command "\$r = Invoke-CimMethod -ClassName Win32_Process -MethodName Create -Arguments @{ CommandLine = '${line//\'/\'\'}'; CurrentDirectory = '$cwd' }; \$r.ProcessId")
  echo "$name started (pid ${pid//$'\r'/})"
done
