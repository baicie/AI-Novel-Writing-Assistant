#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
ROUNDS="${1:-12}"
for i in $(seq 1 "$ROUNDS"); do
  echo "[p11-lease-stress] round $i/$ROUNDS"
  cargo test -p novel-application --test p11_lease_fence
done
echo "[p11-lease-stress] PASS $ROUNDS rounds"
