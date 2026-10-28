#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
BIN="$ROOT/target/aarch64-apple-darwin/release/novel-desktop"
PKG="pnpm --filter @ai-novel/desktop-e2e"
DIR="$(mktemp -d /tmp/biz-novel-perf.XXXXXX)"
REPORT="$DIR/perf-report.json"
KEEP="${BIZ_NOVEL_PERF_KEEP:-0}"

cleanup() {
  pkill -f "$BIN" 2>/dev/null || true
  if [[ "$KEEP" != "1" ]]; then rm -rf "$DIR"; fi
}
trap cleanup EXIT

if [[ "${SKIP_E2E_BUILD:-0}" != "1" ]]; then
  echo "[perf] building dedicated E2E Tauri app (feature e2e, isolated config)"
  cargo tauri build --features e2e --config src-tauri/tauri.e2e.conf.json \
    --target aarch64-apple-darwin --bundles app
fi

MACHINE="$(sysctl -n machdep.cpu.brand_string 2>/dev/null || uname -m) / $(sysctl -n hw.memsize 2>/dev/null || echo unknown) bytes / $(sw_vers -productVersion 2>/dev/null || uname -r)"

echo "[perf] seeding and measuring through the native app"
env BIZ_NOVEL_E2E_DATA_DIR="$DIR" BIZ_NOVEL_E2E_KEEP=1 \
    BIZ_NOVEL_E2E_PERF_REPORT="$REPORT" BIZ_NOVEL_E2E_MACHINE="$MACHINE" \
    $PKG test:perf-1000 >"$DIR/measure.log" 2>&1 || {
  tail -60 "$DIR/measure.log" >&2
  exit 1
}

if [[ ! -s "$REPORT" ]]; then
  echo "[perf] native measurement did not produce a report" >&2
  tail -60 "$DIR/measure.log" >&2
  exit 1
fi

echo "[perf] cold start: three independent app processes against the seeded database"
COLD="$DIR/cold-start.jsonl"
: >"$COLD"
for run in 1 2 3; do
  MARKER="$DIR/startup-$run.json"
  rm -f "$MARKER"
  START_MS="$(node -e 'process.stdout.write(String(Date.now()))')"
  env BIZ_NOVEL_DATA_DIR="$DIR" BIZ_NOVEL_E2E_STARTUP_MARKER="$MARKER" "$BIN" >/dev/null 2>&1 &
  APP_PID=$!
  for _ in $(seq 1 200); do
    if [[ -s "$MARKER" ]]; then break; fi
    sleep 0.05
  done
  if [[ ! -s "$MARKER" ]]; then
    echo "[perf] cold start $run never reported ready" >&2
    kill -9 "$APP_PID" 2>/dev/null || true
    exit 1
  fi
  END_MS="$(node -e 'process.stdout.write(String(Date.now()))')"
  RSS_KB="$(ps -o rss= -p "$APP_PID" | tr -d ' ' || echo 0)"
  node -e "const fs=require('fs');const marker=JSON.parse(fs.readFileSync('$MARKER','utf8'));const row={run:$run,totalMs:$END_MS-$START_MS,initMs:marker.initMs,rssKb:$RSS_KB};fs.appendFileSync('$COLD',JSON.stringify(row)+'\n');console.log('[perf] cold start',JSON.stringify(row));"
  kill -9 "$APP_PID" 2>/dev/null || true
  sleep 1
done

node -e "
const fs=require('fs');
const report=JSON.parse(fs.readFileSync('$REPORT','utf8'));
const cold=fs.readFileSync('$COLD','utf8').trim().split(/\n/).filter(Boolean).map(JSON.parse);
report.coldStart=cold;
report.dbBytes=fs.statSync('$DIR/novel.sqlite').size;
report.dbWalBytes=(()=>{try{return fs.statSync('$DIR/novel.sqlite-wal').size}catch{return 0}})();
fs.writeFileSync('$REPORT',JSON.stringify(report,null,2));
console.log('[perf] report saved to $REPORT');
console.log('[perf] db bytes', report.dbBytes, 'chapters', report.chapters, 'chars', report.characters);
"

echo "[perf] PASS"
