#!/usr/bin/env bash
set -euo pipefail
conn_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
conn_sysroot="$conn_root/.local-setup/linux-build/sysroot"
if [[ -d "$conn_sysroot" ]]; then
  export PKG_CONFIG_PATH="$conn_sysroot/usr/lib/x86_64-linux-gnu/pkgconfig:$conn_sysroot/usr/share/pkgconfig${PKG_CONFIG_PATH:+:$PKG_CONFIG_PATH}"
  export LIBRARY_PATH="$conn_sysroot/usr/lib/x86_64-linux-gnu${LIBRARY_PATH:+:$LIBRARY_PATH}"
fi
export CARGO_INCREMENTAL=0
export CARGO_PROFILE_DEV_DEBUG=0
cd "$conn_root/frontends/tauri"
exec npm run tauri dev -- "$@"
