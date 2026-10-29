#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
APP_BIN="${TAURI_APP_BIN:-$ROOT/target/aarch64-apple-darwin/release/bundle/macos/Biz Novel Studio.app/Contents/MacOS/novel-desktop}"
if [[ ! -x "$APP_BIN" ]]; then
  echo "Tauri release binary not found: $APP_BIN" >&2
  echo "Run: pnpm build:tauri" >&2
  exit 2
fi

DATA_DIR="$(mktemp -d /tmp/biz-novel-tauri-channel.XXXXXX)"
LOG_FILE="$DATA_DIR/channel.log"
cleanup() {
  if [[ -n "${APP_PID:-}" ]] && kill -0 "$APP_PID" 2>/dev/null; then
    kill -TERM "$APP_PID" 2>/dev/null || true
    sleep 1
    kill -KILL "$APP_PID" 2>/dev/null || true
    wait "$APP_PID" 2>/dev/null || true
  fi
}
trap cleanup EXIT

BIZ_NOVEL_DATA_DIR="$DATA_DIR" BIZ_NOVEL_SMOKE_CHANNEL=1 RUST_LOG=info "$APP_BIN" >"$LOG_FILE" 2>&1 &
APP_PID=$!
sleep 8

if ! grep -q "channel smoke ok kind=run_started seq=1" "$LOG_FILE"; then
  echo "Channel smoke failed; log follows:" >&2
  cat "$LOG_FILE" >&2
  exit 1
fi

echo "Tauri Channel smoke passed: Rust -> Tauri Channel -> React"
grep "channel smoke" "$LOG_FILE"
