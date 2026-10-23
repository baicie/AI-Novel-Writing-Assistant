#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

echo "[e2e] building dedicated E2E Tauri app (feature e2e, isolated config)"
cargo tauri build \
  --features e2e \
  --config src-tauri/tauri.e2e.conf.json \
  --target aarch64-apple-darwin \
  --bundles app

echo "[e2e] director full flow (real UI clicks)"
pnpm --filter @ai-novel/desktop-e2e test:director

echo "[e2e] cancellation"
pnpm --filter @ai-novel/desktop-e2e test:cancel

echo "[e2e] safety review pause"
pnpm --filter @ai-novel/desktop-e2e test:safety

echo "[e2e] restart persistence: first run keeps its temp directory"
BIZ_NOVEL_E2E_KEEP=1 pnpm --filter @ai-novel/desktop-e2e test:director \
  | tee /tmp/biz-novel-e2e-keep.log
DATA_DIR="$(grep -o 'data=[^ ]*' /tmp/biz-novel-e2e-keep.log | head -1 | cut -d= -f2-)"
if [[ -z "$DATA_DIR" || ! -d "$DATA_DIR" ]]; then
  echo "[e2e] could not locate kept E2E data directory" >&2
  exit 1
fi
echo "[e2e] second app process on $DATA_DIR"
BIZ_NOVEL_E2E_DATA_DIR="$DATA_DIR" pnpm --filter @ai-novel/desktop-e2e test:restart

echo "[e2e] all Native Director UI tests passed"
