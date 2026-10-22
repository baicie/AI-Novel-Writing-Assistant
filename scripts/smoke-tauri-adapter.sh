#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
APP_BIN="${TAURI_APP_BIN:-$ROOT/target/aarch64-apple-darwin/release/bundle/macos/Biz Novel Studio.app/Contents/MacOS/novel-desktop}"
if [[ ! -x "$APP_BIN" ]]; then
  echo "Tauri release binary not found: $APP_BIN" >&2
  echo "Run: pnpm build:tauri" >&2
  exit 2
fi

DATA_DIR="$(mktemp -d /tmp/biz-novel-tauri-adapter.XXXXXX)"
LOG_FILE="$DATA_DIR/adapter.log"
cleanup() {
  if [[ -n "${APP_PID:-}" ]] && kill -0 "$APP_PID" 2>/dev/null; then
    kill -TERM "$APP_PID" 2>/dev/null || true
    sleep 1
    kill -KILL "$APP_PID" 2>/dev/null || true
    wait "$APP_PID" 2>/dev/null || true
  fi
}
trap cleanup EXIT

# This exercises the centralized axios-compatible Adapter, not direct transport:
# POST /novels -> POST chapters -> PUT chapter content -> GET chapter.
BIZ_NOVEL_DATA_DIR="$DATA_DIR" BIZ_NOVEL_SMOKE_ADAPTER=1 RUST_LOG=info "$APP_BIN" >"$LOG_FILE" 2>&1 &
APP_PID=$!
sleep 10

if ! grep -q "adapter smoke ok version=1 chars=" "$LOG_FILE"; then
  echo "Adapter smoke failed; log follows:" >&2
  cat "$LOG_FILE" >&2
  exit 1
fi

echo "Tauri adapter smoke passed:"
grep -E "adapter smoke|chapter served" "$LOG_FILE"
echo "SQLite rows:"
sqlite3 "$DATA_DIR/novel.sqlite" "SELECT 'novel=' || title FROM novel; SELECT 'chapter=' || title FROM chapter; SELECT 'content=' || content FROM chapter_revision LIMIT 1;"
