#!/usr/bin/env bash
# Stops the local dev stack started by dev-up.sh.
for n in api imgproxy worker; do taskkill //F //IM "$n.exe" >/dev/null 2>&1 && echo "$n stopped"; done
# next dev runs under node; stop only the one serving this repo
wmic process where "name='node.exe' and commandline like '%vrcat-successor%next%'" call terminate >/dev/null 2>&1 \
  || powershell.exe -NoProfile -Command "Get-CimInstance Win32_Process -Filter \"Name='node.exe'\" | Where-Object { \$_.CommandLine -match 'vrcat-successor' -and \$_.CommandLine -match 'next' } | ForEach-Object { Stop-Process -Id \$_.ProcessId -Force }"
echo "web stopped"
