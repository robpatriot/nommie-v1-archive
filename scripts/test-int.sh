#!/usr/bin/env bash
set -euo pipefail

# Load .env from repo root (export all vars)
if [ -f .env ]; then
  set -a
  . ./.env
  set +a
fi

# ---- Config (override via env) ----
POSTGRES_SERVICE="${POSTGRES_SERVICE:-postgres}"
POSTGRES_CONTAINER="${POSTGRES_CONTAINER:-nommie-postgres}"
POSTGRES_HOST="${POSTGRES_HOST:-127.0.0.1}"
POSTGRES_PORT="${POSTGRES_PORT:-5432}"
POSTGRES_DB="${POSTGRES_DB:-nommie}"          # base name (dev DB)
POSTGRES_USER="${POSTGRES_USER:-nommie}"
POSTGRES_PASSWORD="${POSTGRES_PASSWORD:-nommie}"

: "${APP_DB_USER:?APP_DB_USER must be set}"
: "${APP_DB_PASSWORD:?APP_DB_PASSWORD must be set}"

TEST_DB_SUFFIX="${TEST_DB_SUFFIX:-_test}"     # required suffix for test DB
RECREATE_TEST_DB="${RECREATE_TEST_DB:-0}"     # set to 1 to drop/recreate test DB

TEST_DB_NAME="${POSTGRES_DB}${TEST_DB_SUFFIX}"
TEST_DATABASE_URL="postgres://${APP_DB_USER}:${APP_DB_PASSWORD}@${POSTGRES_HOST}:${POSTGRES_PORT}/${TEST_DB_NAME}"

echo "🔑 Connecting as APP role: ${APP_DB_USER}"

echo "▶ Starting Postgres service ($POSTGRES_SERVICE)…"
pnpm db:start >/dev/null

echo "⏳ Waiting for Postgres ($POSTGRES_CONTAINER) to become ready…"
for i in {1..60}; do
  if docker exec "$POSTGRES_CONTAINER" pg_isready -h localhost -p "$POSTGRES_PORT" -U "$POSTGRES_USER" -d "$POSTGRES_DB" >/dev/null 2>&1; then
    echo "✅ Postgres is ready."
    break
  fi
  sleep 1
  if [[ $i -eq 60 ]]; then
    echo "❌ Postgres did not become ready in time" >&2
    echo "ℹ︎ Tip: run 'pnpm db:logs' in another terminal to watch logs."
    exit 1
  fi
done

# Reset only the test DB if requested, then reapply infra SQL using the same templates.
if [[ "$RECREATE_TEST_DB" == "1" ]]; then
  echo "♻️  Recreating test database ${TEST_DB_NAME}…"
  export PGPASSWORD="${POSTGRES_PASSWORD}"

  docker exec -e PGPASSWORD="$PGPASSWORD" "$POSTGRES_CONTAINER" \
    psql -U "$POSTGRES_USER" -d postgres -v ON_ERROR_STOP=1 \
      -c "DROP DATABASE IF EXISTS ${TEST_DB_NAME} WITH (FORCE);"

  docker exec -e PGPASSWORD="$PGPASSWORD" "$POSTGRES_CONTAINER" \
    psql -U "$POSTGRES_USER" -d postgres -v ON_ERROR_STOP=1 \
      -c "CREATE DATABASE ${TEST_DB_NAME}
             WITH OWNER ${POSTGRES_USER}
             TEMPLATE template0
             ENCODING 'UTF8'
             LC_COLLATE 'C'
             LC_CTYPE 'C';"

  echo "🔧 Re-applying infra SQL to ${TEST_DB_NAME}…"
  # Prepend \set vars then pipe the .sql.in into psql (-f -)
  docker exec -e PGPASSWORD="$PGPASSWORD" "$POSTGRES_CONTAINER" bash -lc "
    {
      printf '\\set APP_DB_USER %q\n' '$APP_DB_USER'
      printf '\\set APP_DB_PASSWORD %q\n' '$APP_DB_PASSWORD'
      cat /docker-entrypoint-initdb.d/ensure_role.sql.in
    } | psql -U '$POSTGRES_USER' -d postgres -v ON_ERROR_STOP=1 -f -
  "

  docker exec -e PGPASSWORD="$PGPASSWORD" "$POSTGRES_CONTAINER" bash -lc "
    {
      printf '\\set APP_DB_USER %q\n' '$APP_DB_USER'
      printf '\\set APP_DB_PASSWORD %q\n' '$APP_DB_PASSWORD'
      cat /docker-entrypoint-initdb.d/apply_schema_and_grants.sql.in
    } | psql -U '$POSTGRES_USER' -d '$TEST_DB_NAME' -v ON_ERROR_STOP=1 -f -
  "

  echo "✅ Test database ${TEST_DB_NAME} infra ready."
fi

# Compose a *_test DATABASE_URL if none provided
export DATABASE_URL="${DATABASE_URL:-$TEST_DATABASE_URL}"
echo "🔒 Using DATABASE_URL=${DATABASE_URL}"

# Unit tests (pure)
echo "🧪 Running backend unit tests…"
cargo test --manifest-path apps/backend/Cargo.toml --lib -- --nocapture

# Integration tests (bootstrap enforces *_test and runs migrations)
echo "🧪 Running backend integration tests…"
cargo test --manifest-path apps/backend/Cargo.toml --tests -- --nocapture

