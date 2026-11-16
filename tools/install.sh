#!/usr/bin/env bash
# Copies the built Torchmaster files into Horizon's mod folders. Only run while Horizon and any
# native-build are closed. horizon-mods.txt is not touched.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
H=/mnt/c/Users/mxpbx/AppData/Roaming/horizon
if /mnt/c/Windows/System32/WindowsPowerShell/v1.0/powershell.exe -NoProfile -Command \
   "if (Get-CimInstance Win32_Process | Where-Object { \$_.Name -eq 'horizon-client.exe' -or \$_.CommandLine -match 'native-build' }) { exit 0 } else { exit 1 }"; then
  echo "Horizon or a native-build is running: close it first" >&2; exit 1
fi
cp "$ROOT/target/torchmaster.hmod" "$H/horizon-mods/torchmaster.hmod"
cp "$ROOT/target/torchmaster_native.hnmod" "$H/horizon-native-mods/torchmaster_native.hnmod"
sha256sum "$H/horizon-mods/torchmaster.hmod" "$H/horizon-native-mods/torchmaster_native.hnmod"
grep -n "torchmaster" "$H/horizon-mods.txt" || echo "torchmaster not listed in horizon-mods.txt yet"
