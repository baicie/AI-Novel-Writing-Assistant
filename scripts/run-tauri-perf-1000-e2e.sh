#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
BIN="$ROOT/target/aarch64-apple-darwin/release/novel-desktop"
PKG="pnpm --filter @ai-novel/desktop-e2e"
SIZES="${BIZ_NOVEL_PERF_SIZES:-100 500 1000}"
DRIFT_SECS="${BIZ_NOVEL_PERF_DRIFT_SECS:-0}"
SUMMARY="${BIZ_NOVEL_PERF_SUMMARY:-/tmp/biz-novel-perf-summary.json}"
KEEP="${BIZ_NOVEL_PERF_KEEP:-0}"
DIRS=()

cleanup() {
  pkill -f "$BIN" 2>/dev/null || true
  if [[ "$KEEP" != "1" ]]; then
    for dir in "${DIRS[@]:-}"; do [[ -n "$dir" ]] && rm -rf "$dir"; done
  fi
}
trap cleanup EXIT

if [[ "${SKIP_E2E_BUILD:-0}" != "1" ]]; then
  echo "[perf] building dedicated E2E Tauri app (feature e2e, isolated config)"
  cargo tauri build --features e2e --config src-tauri/tauri.e2e.conf.json \
    --target aarch64-apple-darwin --bundles app
fi

MACHINE="$(sysctl -n machdep.cpu.brand_string 2>/dev/null || uname -m) / $(sysctl -n hw.memsize 2>/dev/null || echo unknown) bytes / $(sw_vers -productVersion 2>/dev/null || uname -r)"
echo "[]" > "$SUMMARY"

sample_rss() { ps -o rss= -p "$1" 2>/dev/null | tr -d ' ' || echo 0; }
sample_cpu() { ps -o %cpu= -p "$1" 2>/dev/null | tr -d ' ' || echo 0; }

for SIZE in $SIZES; do
  DIR="$(mktemp -d "/tmp/biz-novel-perf-$SIZE.XXXXXX")"
  DIRS+=("$DIR")
  REPORT="$DIR/perf-report.json"
  echo "[perf] size=$SIZE data=$DIR"

  env BIZ_NOVEL_E2E_DATA_DIR="$DIR" BIZ_NOVEL_E2E_KEEP=1 \
      BIZ_NOVEL_E2E_PERF_CHAPTERS="$SIZE" \
      BIZ_NOVEL_E2E_PERF_REPORT="$REPORT" BIZ_NOVEL_E2E_MACHINE="$MACHINE" \
      $PKG test:perf-1000 >"$DIR/measure.log" 2>&1 || {
    tail -60 "$DIR/measure.log" >&2
    exit 1
  }
  if [[ ! -s "$REPORT" ]]; then
    echo "[perf] size=$SIZE produced no report" >&2
    tail -60 "$DIR/measure.log" >&2
    exit 1
  fi

  COLD="$DIR/cold-start.jsonl"
  : >"$COLD"
  for run in 1 2 3; do
    MARKER="$DIR/startup-$run.json"
    rm -f "$MARKER"
    START_MS="$(node -e 'process.stdout.write(String(Date.now()))')"
    env BIZ_NOVEL_DATA_DIR="$DIR" BIZ_NOVEL_E2E_STARTUP_MARKER="$MARKER" "$BIN" >/dev/null 2>&1 &
    APP_PID=$!
    for _ in $(seq 1 200); do
      [[ -s "$MARKER" ]] && break
      sleep 0.05
    done
    if [[ ! -s "$MARKER" ]]; then
      echo "[perf] size=$SIZE cold start $run never reported ready" >&2
      kill -9 "$APP_PID" 2>/dev/null || true
      exit 1
    fi
    END_MS="$(node -e 'process.stdout.write(String(Date.now()))')"
    CPU_READY="$(sample_cpu "$APP_PID")"
    RSS_READY="$(sample_rss "$APP_PID")"
    sleep 3
    CPU_IDLE="$(sample_cpu "$APP_PID")"
    RSS_IDLE="$(sample_rss "$APP_PID")"
    node -e "const fs=require('fs');const marker=JSON.parse(fs.readFileSync('$MARKER','utf8'));const row={run:$run,totalMs:$END_MS-$START_MS,initMs:marker.initMs,cpuReadyPercent:$CPU_READY,cpuIdlePercent:$CPU_IDLE,rssReadyKb:$RSS_READY,rssIdleKb:$RSS_IDLE};fs.appendFileSync('$COLD',JSON.stringify(row)+'\n');console.log('[perf] size=$SIZE cold',JSON.stringify(row));"
    kill -9 "$APP_PID" 2>/dev/null || true
    sleep 1
  done

  DRIFT_FILE=""
  if [[ "$DRIFT_SECS" -gt 0 ]]; then
    DRIFT_FILE="$DIR/memory-drift.jsonl"
    : >"$DRIFT_FILE"
    MARKER="$DIR/startup-drift.json"
    rm -f "$MARKER"
    env BIZ_NOVEL_DATA_DIR="$DIR" BIZ_NOVEL_E2E_STARTUP_MARKER="$MARKER" "$BIN" >/dev/null 2>&1 &
    APP_PID=$!
    for _ in $(seq 1 200); do
      [[ -s "$MARKER" ]] && break
      sleep 0.05
    done
    echo "[perf] size=$SIZE memory drift for ${DRIFT_SECS}s"
    ELAPSED=0
    while [[ "$ELAPSED" -lt "$DRIFT_SECS" ]]; do
      node -e "const fs=require('fs');fs.appendFileSync('$DRIFT_FILE',JSON.stringify({atSec:$ELAPSED,rssKb:$(sample_rss "$APP_PID")})+'\n');"
      sleep 10
      ELAPSED=$((ELAPSED + 10))
    done
    kill -9 "$APP_PID" 2>/dev/null || true
  fi

  node -e "
    const fs = require('fs');
    const report = JSON.parse(fs.readFileSync('$REPORT', 'utf8'));
    report.coldStart = fs.readFileSync('$COLD', 'utf8').trim().split(/\n/).filter(Boolean).map(JSON.parse);
    report.dbBytes = fs.statSync('$DIR/novel.sqlite').size;
    try { report.dbWalBytes = fs.statSync('$DIR/novel.sqlite-wal').size; } catch { report.dbWalBytes = 0; }
    const driftPath = '$DRIFT_FILE';
    if (driftPath) {
      try {
        report.memoryDrift = fs.readFileSync(driftPath, 'utf8').trim().split(/\n/).filter(Boolean).map(JSON.parse);
      } catch {}
    }
    fs.writeFileSync('$REPORT', JSON.stringify(report, null, 2));
    const summary = JSON.parse(fs.readFileSync('$SUMMARY', 'utf8'));
    summary.push({
      size: report.chapters,
      chars: report.characters,
      contentBytes: report.totalBytes,
      dbBytes: report.dbBytes,
      metrics: report.metrics,
      coldStart: report.coldStart,
      memoryDrift: report.memoryDrift ?? [],
      machine: report.machine,
      reportPath: '$REPORT',
    });
    fs.writeFileSync('$SUMMARY', JSON.stringify(summary, null, 2));
    console.log('[perf] size=$SIZE db', report.dbBytes, 'chars', report.characters);
  "
done

node -e "
  const summary = require('$SUMMARY');
  for (const row of summary) {
    const m = row.metrics;
    const cold = row.coldStart.map((item) => item.totalMs).join('/');
    console.log('[perf] ' + row.size + ' 章: 列表 ' + m.chapterList.p50 + '/' + m.chapterList.p95 + 'ms, 打开 ' + m.chapterOpen.p95 + 'ms, FTS ' + m.ftsSearch.p50 + '/' + m.ftsSearch.p95 + 'ms, 导出 ' + m.fullExport.p50 + 'ms, 备份 ' + m.backup.p50 + 'ms, 冷启动 ' + cold + 'ms, DB ' + (row.dbBytes/1048576).toFixed(1) + 'MiB');
  }
  const drift = summary.flatMap((row) => row.memoryDrift ?? []);
  if (drift.length) {
    console.log('[perf] 内存漂移: ' + drift[0].rssKb + 'KB -> ' + drift[drift.length - 1].rssKb + 'KB (' + drift.length + ' 个采样)');
  }
"
echo "[perf] summary: $SUMMARY"
echo "[perf] PASS"
