#!/usr/bin/env bash
# Runs any long command fully detached and returns at once; output goes to the given log file.
#   scripts/detach.sh <log-file> <command...>
# Same WMI trick as dev-up.sh: the process is not a child of this shell, so it survives the caller exiting.
set -euo pipefail
log="$(cygpath -w "$1")"
shift
cwd="$(pwd -W)"
cmd="$*"
line="cmd.exe /d /c \"$cmd > \"$log\" 2>&1\""
pid=$(powershell.exe -NoProfile -Command "\$r = Invoke-CimMethod -ClassName Win32_Process -MethodName Create -Arguments @{ CommandLine = '${line//\'/\'\'}'; CurrentDirectory = '$cwd' }; \$r.ProcessId")
echo "started (pid ${pid//$'\r'/}), log: $log"
