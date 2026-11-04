#!/usr/bin/env bash
# P17.2：选区改写的真实进程重启验证。
#
# 一条 spec 内的 browser.refresh() 只能证明 WebView 重新加载后数据一致，
# 不能证明「关掉应用再打开」。这里用同一个数据目录跑两次真实 app 进程：
# 第一次写入并保存，第二次只读取并断言正文与正式版本没有变化。
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

APP="${P16_E2E_APP:-$ROOT/target/aarch64-apple-darwin/release/bundle/macos/Biz Novel Studio E2E.app/Contents/MacOS/novel-desktop}"
if [[ ! -x "$APP" ]]; then
  echo "[p17.2] 找不到 E2E 应用产物：$APP" >&2
  echo "[p17.2] 先运行 pnpm build:tauri:e2e" >&2
  exit 1
fi

DATA_DIR="$(mktemp -d /tmp/p172-rewrite-restart.XXXXXX)"
echo "[p17.2] 数据目录：$DATA_DIR"

echo "[p17.2] 第一次运行：发起改写、采纳并保存"
BIZ_NOVEL_DATA_DIR="$DATA_DIR" P16_E2E_APP="$APP" \
  pnpm e2e --spec ./e2e/p17-selection-rewrite.e2e.js

echo "[p17.2] 第二次运行：新进程只读校验"
BIZ_NOVEL_DATA_DIR="$DATA_DIR" P16_E2E_APP="$APP" P172_RESTART_VERIFY=1 \
  pnpm e2e --spec ./e2e/p17-selection-rewrite-restart.e2e.js

echo "[p17.2] 重启验证通过"
