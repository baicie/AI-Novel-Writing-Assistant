#!/usr/bin/env bash
# P17.4.7：手动创建自然导航 + 重新打开工作区的双进程验收。
#
# 两个 spec 必须共用同一份数据目录：第一个 spec 从首页创建小说，
# 第二个 spec 启动新进程读取同一份数据，确认重新打开后仍进入专业工作台。
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

APP="${P16_E2E_APP:-$ROOT/target/aarch64-apple-darwin/release/bundle/macos/Biz Novel Studio E2E.app/Contents/MacOS/novel-desktop}"
if [[ ! -x "$APP" ]]; then
  echo "[e2e] 找不到 E2E 应用产物：$APP" >&2
  echo "[e2e] 先运行 pnpm build:tauri:e2e" >&2
  exit 1
fi

DATA_DIR="$(mktemp -d "/tmp/p1747-create.XXXXXX")"
trap 'rm -rf "$DATA_DIR"' EXIT

run_spec() {
  local spec="$1"
  pkill -f "Biz Novel Studio E2E" >/dev/null 2>&1 || true
  sleep 1
  echo "[e2e] $spec（数据目录 $DATA_DIR）"
  BIZ_NOVEL_DATA_DIR="$DATA_DIR" P16_E2E_APP="$APP" pnpm e2e --spec "./$spec"
}

run_spec e2e/p17-4-7-create-navigation.e2e.js
run_spec e2e/p17-4-7-create-restart.e2e.js

echo "[e2e] 创建自然导航与重启验收通过"
