#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
APP="${TAURI_APP_BUNDLE:-$ROOT/target/aarch64-apple-darwin/release/bundle/macos/Biz Novel Studio.app}"
BIN="$APP/Contents/MacOS/novel-desktop"
if [[ ! -x "$BIN" ]]; then
  echo "Tauri app binary not found: $BIN" >&2
  exit 2
fi

suspicious="$(find "$APP" -type f | grep -Ei 'node|nodejs|sidecar|electron' || true)"
if [[ -n "$suspicious" ]]; then
  echo "Found suspicious Node/Electron/sidecar files:" >&2
  echo "$suspicious" >&2
  exit 1
fi

if otool -L "$BIN" | grep -Ei 'node|electron' >/dev/null; then
  echo "Binary links against Node/Electron libraries" >&2
  exit 1
fi

for marker in tauri_plugin_wdio tauri-plugin-wdio wdio-webdriver TAURI_WEBDRIVER_PORT; do
  if strings "$BIN" | grep -Fq "$marker"; then
    echo "Production binary contains E2E WebDriver marker: $marker" >&2
    exit 1
  fi
done

echo "Tauri release audit passed: no Node/Electron/sidecar, no E2E WebDriver markers."
