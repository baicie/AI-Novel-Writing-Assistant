#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
BIN="$ROOT/target/aarch64-apple-darwin/release/novel-desktop"
PKG="pnpm --filter @ai-novel/desktop-e2e"
DIR="$(mktemp -d /tmp/biz-novel-batch-restart.XXXXXX)"
ENDPOINT_FILE="$DIR/endpoint.txt"
MOCK_PID=""
cleanup() {
  if [[ -n "$MOCK_PID" ]]; then kill "$MOCK_PID" 2>/dev/null || true; fi
  rm -rf "$DIR"
}
trap cleanup EXIT

node tests/desktop-e2e/support/mock-server.mjs >"$ENDPOINT_FILE" 2>"$DIR/mock.log" &
MOCK_PID=$!
for _ in $(seq 1 50); do
  if [[ -s "$ENDPOINT_FILE" ]]; then break; fi
  sleep 0.1
done
ENDPOINT="$(head -1 "$ENDPOINT_FILE")"
if [[ -z "$ENDPOINT" ]]; then echo "mock endpoint missing" >&2; exit 1; fi
echo "[batch-restart] data=$DIR endpoint=$ENDPOINT"

set +e
BIZ_NOVEL_E2E_DATA_DIR="$DIR" BIZ_NOVEL_E2E_KEEP=1 BIZ_NOVEL_E2E_EXTERNAL_ENDPOINT="$ENDPOINT" \
  $PKG test:batch-crash >"$DIR/crash.log" 2>&1
CRASH_EXIT=$?
set -e
if [[ ! -s "$DIR/batch-restart-state.json" ]]; then
  echo "[batch-restart] crash state missing" >&2
  tail -40 "$DIR/crash.log" >&2
  exit 1
fi
sleep 1
if pgrep -f "$BIN" >/dev/null; then echo "[batch-restart] app still alive" >&2; exit 1; fi
echo "[batch-restart] crash persisted state (exit=$CRASH_EXIT expected)"

BIZ_NOVEL_E2E_DATA_DIR="$DIR" BIZ_NOVEL_E2E_KEEP=1 BIZ_NOVEL_E2E_EXTERNAL_ENDPOINT="$ENDPOINT" \
  $PKG test:batch-restart >"$DIR/restart.log" 2>&1
echo "[batch-restart] PASS"
