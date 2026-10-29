#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
BIN="$ROOT/target/aarch64-apple-darwin/release/novel-desktop"
PKG="pnpm --filter @ai-novel/desktop-e2e"
DIR="$(mktemp -d /tmp/biz-novel-backup-crash.XXXXXX)"
KEEP="${BIZ_NOVEL_BACKUP_CRASH_KEEP:-0}"

cleanup() {
  pkill -f "$BIN" 2>/dev/null || true
  if [[ "$KEEP" != "1" ]]; then rm -rf "$DIR"; fi
}
trap cleanup EXIT

if [[ "${SKIP_E2E_BUILD:-0}" != "1" ]]; then
  echo "[backup-crash] building dedicated E2E Tauri app"
  cargo tauri build --features e2e --config src-tauri/tauri.e2e.conf.json \
    --target aarch64-apple-darwin --bundles app
fi

echo "[backup-crash] data=$DIR"

echo "[backup-crash] phase: kill during backup"
set +e
env BIZ_NOVEL_E2E_DATA_DIR="$DIR" BIZ_NOVEL_E2E_KEEP=1 BIZ_NOVEL_E2E_BACKUP_HOLD=1 \
    $PKG test:backup-crash >"$DIR/phase1.log" 2>&1
code=$?
set -e
echo "[backup-crash] phase 1 exit=$code (expected when the app is killed)"
if [[ ! -s "$DIR/backup-crash.json" ]]; then
  echo "[backup-crash] phase 1 did not reach the crash window" >&2
  tail -40 "$DIR/phase1.log" >&2
  exit 1
fi
pkill -f "$BIN" 2>/dev/null || true
for _ in $(seq 1 60); do
  if ! pgrep -f "$BIN" >/dev/null; then break; fi
  sleep 0.25
done

echo "[backup-crash] phase: restart and verify"
env BIZ_NOVEL_E2E_DATA_DIR="$DIR" BIZ_NOVEL_E2E_KEEP=1 \
    $PKG test:backup-crash-verify >"$DIR/phase2.log" 2>&1 || {
  tail -40 "$DIR/phase2.log" >&2
  exit 1
}
grep -E 'passing|failing' "$DIR/phase2.log" | tail -1
echo "[backup-crash] PASS"
