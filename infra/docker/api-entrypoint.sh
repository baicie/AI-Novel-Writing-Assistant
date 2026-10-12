#!/bin/sh
set -eu

cd /app/server

i=0
until /app/node_modules/.bin/prisma migrate deploy --config /app/server/prisma.config.ts; do
  i=$((i + 1))
  if [ "$i" -ge 30 ]; then
    echo "[api] database is not ready after waiting for prisma migrate deploy." >&2
    exit 1
  fi
  echo "[api] waiting for database before applying migrations (${i}/30)..." >&2
  sleep 2
done

cd /app
exec "$@"
