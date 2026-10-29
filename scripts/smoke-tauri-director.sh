#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
APP_BIN="${TAURI_APP_BIN:-$ROOT/target/aarch64-apple-darwin/release/bundle/macos/Biz Novel Studio.app/Contents/MacOS/novel-desktop}"
if [[ ! -x "$APP_BIN" ]]; then
  echo "Tauri release binary not found: $APP_BIN" >&2
  echo "Run: cargo tauri build --target aarch64-apple-darwin --bundles app" >&2
  exit 2
fi

DATA_DIR="$(mktemp -d /tmp/biz-novel-tauri-director.XXXXXX)"
LOG_FILE="$DATA_DIR/director.log"
cleanup() {
  if [[ -n "${APP_PID:-}" ]] && kill -0 "$APP_PID" 2>/dev/null; then
    kill -TERM "$APP_PID" 2>/dev/null || true
    sleep 1
    kill -KILL "$APP_PID" 2>/dev/null || true
    wait "$APP_PID" 2>/dev/null || true
  fi
}
trap cleanup EXIT

BIZ_NOVEL_DATA_DIR="$DATA_DIR" BIZ_NOVEL_SMOKE_DIRECTOR=1 RUST_LOG=info "$APP_BIN" >"$LOG_FILE" 2>&1 &
APP_PID=$!
sleep 10

if ! grep -q "director ui smoke ok page=true start=true" "$LOG_FILE"; then
  echo "Director UI smoke failed; log follows:" >&2
  cat "$LOG_FILE" >&2
  exit 1
fi

echo "Tauri Director UI smoke passed: route rendered, start form visible"
grep "director ui smoke" "$LOG_FILE"
