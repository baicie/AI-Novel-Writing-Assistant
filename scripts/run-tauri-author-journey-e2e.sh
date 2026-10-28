#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
BIN="$ROOT/target/aarch64-apple-darwin/release/novel-desktop"
PKG="pnpm --filter @ai-novel/desktop-e2e"
DIR="$(mktemp -d /tmp/biz-novel-journey.XXXXXX)"
KEEP="${BIZ_NOVEL_JOURNEY_KEEP:-0}"
PHASES="${BIZ_NOVEL_JOURNEY_PHASES:-prepare batch}"

cleanup() {
  pkill -f "$BIN" 2>/dev/null || true
  if [[ "$KEEP" != "1" ]]; then rm -rf "$DIR"; fi
}
trap cleanup EXIT

if [[ "${SKIP_E2E_BUILD:-0}" != "1" ]]; then
  echo "[journey] building dedicated E2E Tauri app (feature e2e, isolated config)"
  cargo tauri build --features e2e --config src-tauri/tauri.e2e.conf.json \
    --target aarch64-apple-darwin --bundles app
fi

echo "[journey] data=$DIR"

run_phase() {
  local name="$1"
  echo "[journey] phase: $name"
  env BIZ_NOVEL_E2E_DATA_DIR="$DIR" BIZ_NOVEL_E2E_KEEP=1 \
      $PKG "test:journey-$name" >"$DIR/$name.log" 2>&1 || {
    tail -50 "$DIR/$name.log" >&2
    exit 1
  }
}

for phase in $PHASES; do
  run_phase "$phase"
done

if [[ -s "$DIR/journey-batch.json" ]]; then
  node -e "const fs=require('fs');const b=JSON.parse(fs.readFileSync('$DIR/journey-batch.json','utf8'));console.log('[journey] batch',b.batchId,'runs',b.runIds.length);"
fi

echo "[journey] PASS"
