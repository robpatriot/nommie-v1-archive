#!/usr/bin/env bash
set -euo pipefail

# Required env (provided by docker-compose):
#   POSTGRES_USER, POSTGRES_PASSWORD, POSTGRES_DB
#   APP_DB_USER, APP_DB_PASSWORD

TEST_DB="${POSTGRES_DB}_test"

echo "[init] starting …"

: "${POSTGRES_USER:?must be set}"
: "${POSTGRES_DB:?must be set}"
: "${APP_DB_USER:?must be set}"
: "${APP_DB_PASSWORD:?must be set}"

ensure_db() {
  local dbname="$1"
  echo "[init] ensuring database ${dbname}"
  if psql -U "$POSTGRES_USER" -d "postgres" -tAc "SELECT 1 FROM pg_database WHERE datname='${dbname}'" | grep -q 1; then
    echo "[init] database ${dbname} already exists"
  else
    psql -U "$POSTGRES_USER" -d "postgres" -v ON_ERROR_STOP=1 -c \
      "CREATE DATABASE ${dbname}
         WITH OWNER ${POSTGRES_USER}
         TEMPLATE template0
         ENCODING 'UTF8'
         LC_COLLATE 'C'
         LC_CTYPE 'C';"
  fi
}

ROLE_SQL="/docker-entrypoint-initdb.d/ensure_role.sql.in"
GRANTS_SQL="/docker-entrypoint-initdb.d/apply_schema_and_grants.sql.in"

# Helper: run a .sql.in with psql variables pre-set
run_sql_with_vars() {
  local dbname="$1"
  local sql_file="$2"
  # Prepend \set lines so :'APP_DB_USER' etc. are defined for psql
  # We use -f - to read the combined stream from stdin.
  {
    printf "\\set APP_DB_USER '%s'\n" "$APP_DB_USER"
    printf "\\set APP_DB_PASSWORD '%s'\n" "$APP_DB_PASSWORD"
    cat "$sql_file"
  } | psql -U "$POSTGRES_USER" -d "$dbname" -v ON_ERROR_STOP=1 -f -
}

# 0) Ensure app role (idempotent)
echo "[init] ensuring app role ${APP_DB_USER}"
run_sql_with_vars "postgres" "$ROLE_SQL"

# 1) Ensure both databases exist
ensure_db "${POSTGRES_DB}"
ensure_db "${TEST_DB}"

# 2) Apply schema/grants/default privileges to both DBs
for DB in "${POSTGRES_DB}" "${TEST_DB}"; do
  echo "[init] schema/grants for ${DB}"
  run_sql_with_vars "$DB" "$GRANTS_SQL"
done

echo "[init] done."

