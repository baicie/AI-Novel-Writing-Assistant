#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
APP_BIN="${TAURI_APP_BIN:-$ROOT/target/aarch64-apple-darwin/release/bundle/macos/Biz Novel Studio.app/Contents/MacOS/novel-desktop}"
if [[ ! -x "$APP_BIN" ]]; then
  echo "Tauri release binary not found: $APP_BIN" >&2
  echo "Run: pnpm build:tauri" >&2
  exit 2
fi

DATA_DIR="$(mktemp -d /tmp/biz-novel-tauri-save.XXXXXX)"
LOG_FILE="$DATA_DIR/save.log"
cleanup() {
  if [[ -n "${APP_PID:-}" ]] && kill -0 "$APP_PID" 2>/dev/null; then
    kill -TERM "$APP_PID" 2>/dev/null || true
    sleep 1
    kill -KILL "$APP_PID" 2>/dev/null || true
    wait "$APP_PID" 2>/dev/null || true
  fi
}
trap cleanup EXIT

# This exercises the real renderer -> Tauri commands -> SQLite path: create book,
# create chapter, save正文 with expectedVersion=0, reopen it, then report the result.
BIZ_NOVEL_DATA_DIR="$DATA_DIR" BIZ_NOVEL_SMOKE_SAVE=1 RUST_LOG=info "$APP_BIN" >"$LOG_FILE" 2>&1 &
APP_PID=$!
sleep 10

if ! grep -q "save smoke ok version=1 chars=" "$LOG_FILE"; then
  echo "Save smoke failed; log follows:" >&2
  cat "$LOG_FILE" >&2
  exit 1
fi

echo "Tauri save smoke passed:"
grep -E "save smoke|chapter served" "$LOG_FILE"
echo "SQLite rows:"
sqlite3 "$DATA_DIR/novel.sqlite" "SELECT 'novel=' || title FROM novel; SELECT 'chapter=' || title FROM chapter; SELECT 'content=' || content FROM chapter_revision LIMIT 1;"
