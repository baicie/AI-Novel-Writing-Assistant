#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
BIN="$ROOT/target/aarch64-apple-darwin/release/novel-desktop"
PKG="pnpm --filter @ai-novel/desktop-e2e"
DIR="$(mktemp -d /tmp/biz-novel-journey.XXXXXX)"
KEEP="${BIZ_NOVEL_JOURNEY_KEEP:-0}"
PHASES="${BIZ_NOVEL_JOURNEY_PHASES:-prepare batch memory knowledge editor backup backup-verify}"

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
  local tolerate="${2:-0}"
  echo "[journey] phase: $name"
  set +e
  env BIZ_NOVEL_E2E_DATA_DIR="$DIR" BIZ_NOVEL_E2E_KEEP=1 \
      $PKG "test:journey-$name" >"$DIR/$name.log" 2>&1
  local code=$?
  set -e
  if [[ "$tolerate" != "1" && "$code" -ne 0 ]]; then
    echo "[journey] phase $name failed (exit=$code)" >&2
    tail -50 "$DIR/$name.log" >&2
    exit 1
  fi
  if [[ "$tolerate" == "1" ]]; then
    echo "[journey] phase $name exited with $code (expected when the app is killed)"
  fi
  # 下一个阶段要重新拉起应用：先确认上一个进程真的退出了，避免驱动端口被占用。
  pkill -f "$BIN" 2>/dev/null || true
  for _ in $(seq 1 60); do
    if ! pgrep -f "$BIN" >/dev/null; then break; fi
    sleep 0.25
  done
  if pgrep -f "$BIN" >/dev/null; then
    echo "[journey] previous app instance did not exit before phase $name" >&2
    exit 1
  fi
}

for phase in $PHASES; do
  case "$phase" in
    backup)
      run_phase backup 1
      if [[ ! -s "$DIR/journey-backup.json" ]]; then
        echo "[journey] backup phase did not persist its state" >&2
        tail -50 "$DIR/backup.log" >&2
        exit 1
      fi
      sleep 1
      if pgrep -f "$BIN" >/dev/null; then
        echo "[journey] app still running after the backup phase" >&2
        exit 1
      fi
      ;;
    *)
      run_phase "$phase"
      ;;
  esac
done

if [[ -s "$DIR/journey-batch.json" ]]; then
  node -e "const fs=require('fs');const b=JSON.parse(fs.readFileSync('$DIR/journey-batch.json','utf8'));console.log('[journey] batch',b.batchId,'runs',b.runIds.length);"
fi
if [[ -s "$DIR/journey-backup.json" ]]; then
  node -e "const fs=require('fs');const b=JSON.parse(fs.readFileSync('$DIR/journey-backup.json','utf8'));console.log('[journey] restore rolled back chapter to version', b.versionBeforeRestore - 1, 'backup', b.backupFile);"
fi

echo "[journey] PASS"
