#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
BIN="$ROOT/target/aarch64-apple-darwin/release/novel-desktop"
PKG="pnpm --filter @ai-novel/desktop-e2e"
DIR="$(mktemp -d /tmp/biz-novel-drafts.XXXXXX)"
cleanup() { rm -rf "$DIR"; }
trap cleanup EXIT
set +e
BIZ_NOVEL_E2E_DATA_DIR="$DIR" BIZ_NOVEL_E2E_KEEP=1 $PKG test:multiwindow-draft-write >"$DIR/write.log" 2>&1
set -e
if [[ ! -s "$DIR/draft-restart-state.json" ]]; then echo "draft state missing" >&2; tail -40 "$DIR/write.log" >&2; exit 1; fi
sleep 1
BIZ_NOVEL_E2E_DATA_DIR="$DIR" BIZ_NOVEL_E2E_KEEP=1 $PKG test:multiwindow-draft-verify >"$DIR/verify.log" 2>&1
echo "[drafts] PASS"
