#!/usr/bin/env bash
# P17.2：每个 E2E spec 用独立的数据目录与独立的应用进程跑一遍。
#
# 这些 spec 会写入模型来源、章节草稿与本地偏好；共用同一份数据时，
# 前一个 spec 的残留会让后一个 spec 失败在与代码无关的状态上。
# 这里显式隔离，失败就一定是该 spec 自己的问题。
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

APP="${P16_E2E_APP:-$ROOT/target/aarch64-apple-darwin/release/bundle/macos/Biz Novel Studio E2E.app/Contents/MacOS/novel-desktop}"
if [[ ! -x "$APP" ]]; then
  echo "[e2e] 找不到 E2E 应用产物：$APP" >&2
  echo "[e2e] 先运行 pnpm build:tauri:e2e" >&2
  exit 1
fi

SPECS=("$@")
if [[ ${#SPECS[@]} -eq 0 ]]; then
  SPECS=(
    e2e/provider-smoke.e2e.js
    e2e/provider-flow.e2e.js
    e2e/provider-no-missing-routes.e2e.js
    e2e/quick-setup-models.e2e.js
    e2e/provider-selector.e2e.js
    e2e/provider-protocol-guard.e2e.js
    e2e/author-journey.e2e.js
    e2e/p17-workbench.e2e.js
    e2e/p17-cancel-navigation.e2e.js
    e2e/p17-selection-rewrite.e2e.js
    e2e/p17-edit-position.e2e.js
    e2e/p17-3-continuity.e2e.js
    e2e/p17-3-context.e2e.js
    e2e/p17-4-plot-timeline.e2e.js
  )
fi

FAILED=()
for spec in "${SPECS[@]}"; do
  DATA_DIR="$(mktemp -d "/tmp/p172-spec.XXXXXX")"
  echo "[e2e] $spec（数据目录 $DATA_DIR）"
  pkill -f "Biz Novel Studio E2E" >/dev/null 2>&1 || true
  sleep 1
  if BIZ_NOVEL_DATA_DIR="$DATA_DIR" P16_E2E_APP="$APP" pnpm e2e --spec "./$spec"; then
    echo "[e2e] PASS $spec"
  else
    echo "[e2e] FAIL $spec"
    FAILED+=("$spec")
  fi
  rm -rf "$DATA_DIR"
done

if [[ ${#FAILED[@]} -gt 0 ]]; then
  echo "[e2e] 失败 ${#FAILED[@]} 个：${FAILED[*]}" >&2
  exit 1
fi
echo "[e2e] 全部通过（${#SPECS[@]} 个 spec）"
