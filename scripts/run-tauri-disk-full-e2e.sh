#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

echo "[disk-full] 挂载一个 16MB 磁盘映像，把卷写满后尝试备份"
BIZ_NOVEL_ENOSPC_TEST=1 cargo test -p novel-desktop --lib \
  backup_fails_when_the_target_volume_runs_out_of_space -- --ignored --nocapture 2>&1 | tail -12
echo "[disk-full] 残留挂载点检查"
mount | grep -c TinyDisk || true
echo "[disk-full] PASS"
