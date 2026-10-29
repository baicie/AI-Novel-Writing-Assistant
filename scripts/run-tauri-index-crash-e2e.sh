#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
BIN="$ROOT/target/aarch64-apple-darwin/release/novel-desktop"
PKG="pnpm --filter @ai-novel/desktop-e2e"
DIR="$(mktemp -d /tmp/biz-novel-index-crash.XXXXXX)"

cleanup() {
  pkill -f "$BIN" 2>/dev/null || true
  rm -rf "$DIR"
}
trap cleanup EXIT

if [[ "${SKIP_E2E_BUILD:-0}" != "1" ]]; then
  echo "[index-crash] building dedicated E2E Tauri app (feature e2e, isolated config)"
  cargo tauri build \
    --features e2e \
    --config src-tauri/tauri.e2e.conf.json \
    --target aarch64-apple-darwin \
    --bundles app
fi

BASE_ENV=(
  "BIZ_NOVEL_E2E_DATA_DIR=$DIR"
  "BIZ_NOVEL_E2E_KEEP=1"
  "BIZ_NOVEL_E2E_INDEX_FAIL_ONCE=0"
  "BIZ_NOVEL_E2E_INDEX_LEASE_MS=4000"
  "BIZ_NOVEL_E2E_INDEX_RECOVER_SECS=2"
)

echo "[index-crash] phase 1: index source revision v1"
env "${BASE_ENV[@]}" $PKG test:index-crash-prepare >"$DIR/phase1.log" 2>&1 || {
  tail -40 "$DIR/phase1.log" >&2
  exit 1
}
if [[ ! -s "$DIR/index-crash-state.json" ]]; then
  echo "[index-crash] phase 1 did not persist its state" >&2
  tail -40 "$DIR/phase1.log" >&2
  exit 1
fi
echo "[index-crash] data=$DIR"

echo "[index-crash] phase 2: claim v2 and SIGKILL the app"
set +e
env "${BASE_ENV[@]}" BIZ_NOVEL_E2E_INDEX_HOLD=1 $PKG test:index-crash-claim >"$DIR/phase2.log" 2>&1
PHASE2_EXIT=$?
set -e
if [[ ! -s "$DIR/index-crash-claim.json" ]]; then
  echo "[index-crash] phase 2 never persisted the claim" >&2
  tail -40 "$DIR/phase2.log" >&2
  exit 1
fi
sleep 1
if pgrep -f "$BIN" >/dev/null; then
  echo "[index-crash] app still alive after SIGKILL" >&2
  exit 1
fi
echo "[index-crash] claim persisted, app killed (phase2 exit=$PHASE2_EXIT expected)"

echo "[index-crash] phase 3: new process recovers the expired claim"
env "${BASE_ENV[@]}" $PKG test:index-crash-restart >"$DIR/phase3.log" 2>&1 || {
  tail -60 "$DIR/phase3.log" >&2
  exit 1
}

echo "[index-crash] PASS"
