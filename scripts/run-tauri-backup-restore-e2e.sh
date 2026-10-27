#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
BIN="$ROOT/target/aarch64-apple-darwin/release/novel-desktop"
PKG="pnpm --filter @ai-novel/desktop-e2e"
DIR="$(mktemp -d /tmp/biz-novel-restore.XXXXXX)"
cleanup() { rm -rf "$DIR"; }
trap cleanup EXIT
set +e
BIZ_NOVEL_E2E_DATA_DIR="$DIR" BIZ_NOVEL_E2E_KEEP=1 $PKG test:backup-restore-request >"$DIR/request.log" 2>&1
set -e
if [[ ! -s "$DIR/restore-state.json" ]]; then echo "restore state missing" >&2; tail -40 "$DIR/request.log" >&2; exit 1; fi
sleep 1
if pgrep -f "$BIN" >/dev/null; then echo "app still alive after restore request" >&2; exit 1; fi
BIZ_NOVEL_E2E_DATA_DIR="$DIR" BIZ_NOVEL_E2E_KEEP=1 $PKG test:backup-restore-verify >"$DIR/verify.log" 2>&1
echo "[restore] PASS"
