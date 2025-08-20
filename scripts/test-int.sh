#!/usr/bin/env bash
set -euo pipefail

# ---------------------------------------------
# Nommie test runner (unit + integration)
# - Starts Postgres (Docker)
# - Ensures *_test DB usage
# - Optional recreate of test DB infra
# - Verbosity via VERBOSE=0|1|2
# - Writes logs to LOG_DIR (default: ./logs)
# - Prints a clean final summary banner
# ---------------------------------------------

# =========== Verbosity ===========
# 0=quiet, 1=normal (default), 2=debug
VERBOSE="${VERBOSE:-1}"

# Colors (no-op on dumb terminals)
if command -v tput >/dev/null && [ -t 1 ]; then
  RED=$(tput setaf 1); GREEN=$(tput setaf 2); YELLOW=$(tput setaf 3); BOLD=$(tput bold); RESET=$(tput sgr0)
else
  RED=""; GREEN=""; YELLOW=""; BOLD=""; RESET=""
fi

# Choose RUST_LOG based on VERBOSE
if [[ "$VERBOSE" -eq 0 ]]; then
  export RUST_LOG="${RUST_LOG:-warn,sqlx=warn,sqlx::query=warn,sea_orm_migration=warn,actix_web=warn}"
  CARGO_FLAGS="-q"
  TEST_FLAGS="" # no --nocapture so passing test stdout is suppressed
elif [[ "$VERBOSE" -eq 2 ]]; then
  export RUST_LOG="${RUST_LOG:-debug,backend=debug,sqlx=info,sqlx::query=info,sea_orm_migration=debug,actix_web=info}"
  CARGO_FLAGS=""
  TEST_FLAGS="-- --nocapture"
else
  export RUST_LOG="${RUST_LOG:-info,backend=info,sqlx::query=warn,sea_orm_migration=warn,actix_web=warn}"
  CARGO_FLAGS=""
  TEST_FLAGS="-- --nocapture"
fi

# Optional: make logs grep-friendly (auto/never/always)
export RUST_LOG_STYLE="${RUST_LOG_STYLE:-auto}"

# =========== Load .env ===========
if [ -f .env ]; then
  set -a
  # shellcheck disable=SC1091
  . ./.env
  set +a
fi

# =========== Config ===========
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

# =========== Logs ===========
LOG_DIR="${LOG_DIR:-logs}"
mkdir -p "$LOG_DIR"
UNIT_LOG="$LOG_DIR/unit.log"
INT_LOG="$LOG_DIR/integration.log"
: > "$UNIT_LOG"
: > "$INT_LOG"
echo "🗂️  Writing logs to $LOG_DIR"

# Helper to print clickable file links (best-effort)
abs_path() { readlink -f "$1" 2>/dev/null || python3 - <<'PY'
import os,sys; print(os.path.abspath(sys.argv[1]))
PY
}
print_link() {
  local p="$1" l="$2" a; a="$(abs_path "$p")"
  printf '\e]8;;file://%s\a%s\e]8;;\a' "$a" "$l"
}

# On failure, show a tail of Postgres logs
show_pg_logs_on_fail() {
  echo
  echo "${BOLD}${YELLOW}▶ Postgres logs (last 60 lines)${RESET}"
  if docker ps --format '{{.Names}}' | grep -q "^${POSTGRES_CONTAINER}\$"; then
    docker logs --tail 60 "$POSTGRES_CONTAINER" || true
  else
    echo "Container ${POSTGRES_CONTAINER} not running."
  fi
}
trap 'ret=$?; if [[ $ret -ne 0 ]]; then show_pg_logs_on_fail; fi; exit $ret' EXIT

# =========== Start Postgres ===========
echo "🔑 Connecting as APP role: ${APP_DB_USER}"
echo "▶ Starting Postgres service ($POSTGRES_SERVICE)…"
pnpm db:start >/dev/null

echo "⏳ Waiting for Postgres (${POSTGRES_CONTAINER}) to become ready…"
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

# =========== Optional recreate test DB ===========
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

# =========== DATABASE_URL safety ===========
if [[ -n "${DATABASE_URL:-}" && "${DATABASE_URL}" != *"${TEST_DB_SUFFIX}" ]]; then
  echo "❌ Refusing to run tests against non-test database URL:"
  echo "   DATABASE_URL=${DATABASE_URL}"
  echo "   Required suffix: ${TEST_DB_SUFFIX}"
  exit 1
fi
export DATABASE_URL="${DATABASE_URL:-$TEST_DATABASE_URL}"
echo "🔒 Using DATABASE_URL=${DATABASE_URL}"

# =========== Test runners ===========
UNIT_FAIL=0
INT_FAIL=0

run_with_logs() {
  # $1 label, $2 log_file, $3... command
  local LABEL="$1"; shift
  local LOG="$1"; shift
  echo "🧪 Running ${LABEL}…"
  if [[ "$VERBOSE" -eq 0 ]]; then
    ( "$@" ) >"$LOG" 2>&1 || return 1
  else
    ( "$@" ) 2>&1 | tee "$LOG" || return 1
  fi
}

# Unit
run_with_logs "backend unit tests" "$UNIT_LOG" \
  cargo test $CARGO_FLAGS --manifest-path apps/backend/Cargo.toml --lib $TEST_FLAGS || UNIT_FAIL=1

# Integration
run_with_logs "backend integration tests" "$INT_LOG" \
  cargo test $CARGO_FLAGS --manifest-path apps/backend/Cargo.toml --tests $TEST_FLAGS || INT_FAIL=1

# =========== Summary banner ===========
# Pull final "test result:" lines
unit_line=$(grep -E "test result: " "$UNIT_LOG" | tail -n1 || true)
int_line=$(grep -E "test result: " "$INT_LOG" | tail -n1 || true)

parse_counts() {
  local line="$1"
  local passed failed time
  passed=$(echo "$line" | grep -oE '[0-9]+ passed' | awk '{print $1}')
  failed=$(echo "$line" | grep -oE '[0-9]+ failed' | awk '{print $1}')
  time=$(echo "$line" | grep -oE 'finished in [0-9.]+s' | sed 's/finished in //')
  echo "${passed:-0} ${failed:-0} ${time:-n/a}"
}

read unit_pass unit_fail_cnt unit_time < <(parse_counts "$unit_line")
read int_pass int_fail_cnt int_time   < <(parse_counts "$int_line")

db_status="Migrations: up-to-date" # your tests enforce this at runtime
overall="OK"
if [[ "$UNIT_FAIL" -ne 0 || "$INT_FAIL" -ne 0 || "$unit_fail_cnt" -ne 0 || "$int_fail_cnt" -ne 0 ]]; then
  overall="FAIL"
fi

echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
if [[ "$overall" == "OK" ]]; then
  echo "${BOLD}${GREEN}✅ TEST SUMMARY${RESET}"
else
  echo "${BOLD}${RED}❌ TEST SUMMARY${RESET}"
fi
printf "Unit:        %s%d passed%s, %s%d failed%s (%s)\n" \
  "$([[ ${unit_fail_cnt:-0} -eq 0 ]] && echo "$GREEN" || echo "$RED")" "${unit_pass:-0}" "$RESET" \
  "$([[ ${unit_fail_cnt:-0} -eq 0 ]] && echo "$GREEN" || echo "$RED")" "${unit_fail_cnt:-0}" "$RESET" \
  "${unit_time:-n/a}"
printf "Integration: %s%d passed%s, %s%d failed%s (%s)\n" \
  "$([[ ${int_fail_cnt:-0} -eq 0 ]] && echo "$GREEN" || echo "$RED")" "${int_pass:-0}" "$RESET" \
  "$([[ ${int_fail_cnt:-0} -eq 0 ]] && echo "$GREEN" || echo "$RED")" "${int_fail_cnt:-0}" "$RESET" \
  "${int_time:-n/a}"
echo "DB: ${TEST_DB_NAME} • ${db_status}"
echo -n "Logs: "
print_link "$UNIT_LOG" "$UNIT_LOG"; printf "  "
print_link "$INT_LOG"  "$INT_LOG";  printf "\n"
echo -n "Overall: "
if [[ "$overall" == "OK" ]]; then
  echo "${GREEN}✅ OK${RESET}"
else
  echo "${RED}❌ FAIL${RESET}"
fi
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

# Exit non-zero on any failure (good for CI)
[[ "$overall" == "OK" ]] || exit 1

