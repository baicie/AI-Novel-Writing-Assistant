#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

if [[ "${SKIP_E2E_BUILD:-0}" != "1" ]]; then
  echo "[batch-repair] building dedicated E2E Tauri app (feature e2e, isolated config)"
  cargo tauri build \
    --features e2e \
    --config src-tauri/tauri.e2e.conf.json \
    --target aarch64-apple-darwin \
    --bundles app
fi

echo "[batch-repair] Native bounded repair: review issues -> one repair -> author decisions -> approval -> commit"
pnpm --filter @ai-novel/desktop-e2e test:batch-repair

echo "[batch-repair] PASS"
