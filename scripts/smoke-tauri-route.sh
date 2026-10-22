#!/usr/bin/env bash
set -euo pipefail

# Reproducible, isolated smoke test for the packaged Tauri shell.
# It never touches the user's application data: BIZ_NOVEL_DATA_DIR points at a temp dir.
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
APP_BIN="${TAURI_APP_BIN:-$ROOT/target/aarch64-apple-darwin/release/bundle/macos/Biz Novel Studio.app/Contents/MacOS/novel-desktop}"
if [[ ! -x "$APP_BIN" ]]; then
  echo "Tauri release binary not found: $APP_BIN" >&2
  echo "Run: pnpm build:tauri" >&2
  exit 2
fi

DATA_DIR="$(mktemp -d /tmp/biz-novel-tauri-smoke.XXXXXX)"
LOG_FILE="$DATA_DIR/route.log"
ROUTE="${TAURI_ROUTE:-/novels}"
EXPECTED_LOG="${TAURI_EXPECT_LOG:-novel list served to webview count=1}"
cleanup() {
  if [[ -n "${APP_PID:-}" ]] && kill -0 "$APP_PID" 2>/dev/null; then
    kill -TERM "$APP_PID" 2>/dev/null || true
    sleep 1
    kill -KILL "$APP_PID" 2>/dev/null || true
    wait "$APP_PID" 2>/dev/null || true
  fi
}
trap cleanup EXIT

# First launch creates the schema. The second launch opens a known seeded book through
# the real Vite route and verifies that the Tauri command layer served the list.
BIZ_NOVEL_DATA_DIR="$DATA_DIR" "$APP_BIN" >/dev/null 2>&1 &
APP_PID=$!
sleep 3
cleanup
APP_PID=""

sqlite3 "$DATA_DIR/novel.sqlite" \
  "INSERT INTO novel(id,title,description,status,created_at,updated_at) VALUES ('smoke-n1','路由烟测','Tauri command smoke','draft',1,1);"

BIZ_NOVEL_DATA_DIR="$DATA_DIR" BIZ_NOVEL_START_ROUTE="$ROUTE" RUST_LOG=info "$APP_BIN" >"$LOG_FILE" 2>&1 &
APP_PID=$!
sleep 10

if ! grep -q "$EXPECTED_LOG" "$LOG_FILE"; then
  echo "Route smoke failed for $ROUTE; expected log: $EXPECTED_LOG" >&2
  cat "$LOG_FILE" >&2
  exit 1
fi

echo "Tauri route smoke passed for $ROUTE:"
grep -E "runtime diagnostics|novel list served|chapter editor workspace" "$LOG_FILE"
