#!/usr/bin/env bash
set -euo pipefail
conn_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
conn_app="$conn_root/.local-setup/bin/conn-desktop"
if [[ ! -x "$conn_app" ]]; then
  conn_app="$conn_root/frontends/tauri/src-tauri/target/debug/conn-desktop"
fi
if [[ ! -x "$conn_app" ]]; then
  echo "No prepared Linux app. Build with: cd frontends/tauri && npm run tauri build -- --debug --no-bundle" >&2
  exit 1
fi
exec "$conn_app" "$@"
