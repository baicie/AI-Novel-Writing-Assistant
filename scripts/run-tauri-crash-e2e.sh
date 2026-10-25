#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
BIN="$ROOT/target/aarch64-apple-darwin/release/novel-desktop"
PKG="pnpm --filter @ai-novel/desktop-e2e"
EVIDENCE="${BIZ_NOVEL_E2E_EVIDENCE_DIR:-$ROOT/tests/desktop-e2e/artifacts/$(date +%Y%m%d-%H%M%S)}"
mkdir -p "$EVIDENCE"

if [[ "${SKIP_E2E_BUILD:-0}" != "1" ]]; then
  echo "[e2e-crash] building dedicated E2E app"
  cargo tauri build --features e2e --config src-tauri/tauri.e2e.conf.json \
    --target aarch64-apple-darwin --bundles app
fi

run_crash() {
  local name="$1" spec="$2" state="$3" dir="$4"
  echo "[e2e-crash] $name crash: data=$dir"
  set +e
  BIZ_NOVEL_E2E_DATA_DIR="$dir" BIZ_NOVEL_E2E_KEEP=1 $PKG "test:$spec" \
    >"$EVIDENCE/$name-crash.log" 2>&1
  local code=$?
  set -e
  if [[ ! -s "$dir/$state" ]]; then
    echo "[e2e-crash] $name failed before durable state was written (exit=$code)" >&2
    tail -40 "$EVIDENCE/$name-crash.log" >&2 || true
    return 1
  fi
  # The crash spec intentionally kills the app; nonzero WDIO exit alone is not a failure.
  sleep 1
  if pgrep -f "$BIN" >/dev/null; then
    echo "[e2e-crash] $name app still alive after SIGKILL" >&2
    return 1
  fi
  echo "[e2e-crash] $name durable state=$dir/$state crash_exit=$code (expected SIGKILL path)"
}

run_restart() {
  local name="$1" spec="$2" dir="$3"
  echo "[e2e-crash] $name restart: data=$dir"
  BIZ_NOVEL_E2E_DATA_DIR="$dir" BIZ_NOVEL_E2E_KEEP=1 $PKG "test:$spec" \
    >"$EVIDENCE/$name-restart.log" 2>&1
  echo "[e2e-crash] $name restart PASS"
}

EDITOR_DIR="$(mktemp -d /tmp/biz-novel-crash-editor.XXXXXX)"
CANDIDATE_DIR="$(mktemp -d /tmp/biz-novel-crash-candidate.XXXXXX)"
RESTART_DIR="$(mktemp -d /tmp/biz-novel-restart.XXXXXX)"
cleanup() {
  rm -rf "$EDITOR_DIR" "$CANDIDATE_DIR" "$RESTART_DIR"
}
trap cleanup EXIT

run_crash editor editor-crash editor-crash-state.json "$EDITOR_DIR"
run_restart editor editor-crash-restart "$EDITOR_DIR"

run_crash candidate candidate-journal-crash candidate-state.json "$CANDIDATE_DIR"
run_restart candidate candidate-journal-restart "$CANDIDATE_DIR"

echo "[e2e-crash] normal restart + confirm-after-edit restart"
BIZ_NOVEL_E2E_DATA_DIR="$RESTART_DIR" BIZ_NOVEL_E2E_KEEP=1 $PKG test:director \
  >"$EVIDENCE/director-keep.log" 2>&1
test -s "$RESTART_DIR/e2e-state.json"
test -s "$RESTART_DIR/confirm-edit-state.json"
BIZ_NOVEL_E2E_DATA_DIR="$RESTART_DIR" BIZ_NOVEL_E2E_KEEP=1 $PKG test:restart \
  >"$EVIDENCE/restart.log" 2>&1
BIZ_NOVEL_E2E_DATA_DIR="$RESTART_DIR" BIZ_NOVEL_E2E_KEEP=1 $PKG test:confirm-edit-restart \
  >"$EVIDENCE/confirm-edit-restart.log" 2>&1

echo "[e2e-crash] PASS; evidence=$EVIDENCE"
