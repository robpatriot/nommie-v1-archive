#!/usr/bin/env bash
set -euo pipefail

# Required env:
#   POSTGRES_USER, POSTGRES_PASSWORD, POSTGRES_DB
#   APP_DB_USER, APP_DB_PASSWORD

TEST_DB="${POSTGRES_DB}_test"

echo "[init] starting …"

: "${POSTGRES_USER:?must be set}"
: "${POSTGRES_DB:?must be set}"
: "${APP_DB_USER:?must be set}"
: "${APP_DB_PASSWORD:?must be set}"

# --- 0) Ensure app role exists (idempotent) ---
ensure_role() {
  echo "[init] ensuring role ${APP_DB_USER}"
  psql -U "$POSTGRES_USER" -d "postgres" <<-EOSQL
DO \$\$
BEGIN
  IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = '${APP_DB_USER}') THEN
    EXECUTE format('CREATE ROLE %I WITH LOGIN PASSWORD %L', '${APP_DB_USER}', '${APP_DB_PASSWORD}');
  END IF;
END
\$\$;
EOSQL
}

# --- 1) Ensure a database exists (idempotent) ---
ensure_db() {
  local dbname="$1"
  echo "[init] ensuring database ${dbname}"

  if psql -U "$POSTGRES_USER" -d "postgres" -tAc "SELECT 1 FROM pg_database WHERE datname='${dbname}'" | grep -q 1; then
    echo "[init] database ${dbname} already exists"
  else
    psql -U "$POSTGRES_USER" -d "postgres" -c \
      "CREATE DATABASE ${dbname}
         WITH OWNER ${POSTGRES_USER}
         TEMPLATE template0
         ENCODING 'UTF8'
         LC_COLLATE 'C'
         LC_CTYPE 'C';"
  fi
}

# --- 2) Apply schema + grants + default privileges (idempotent) ---
apply_schema_and_grants() {
  local dbname="$1"
  echo "[init] schema/grants for ${dbname}"
  psql -U "$POSTGRES_USER" -d "$dbname" <<-EOSQL
-- Schema
CREATE SCHEMA IF NOT EXISTS nommie;

-- Grants to app role
GRANT USAGE, CREATE ON SCHEMA nommie TO ${APP_DB_USER};

GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA nommie TO ${APP_DB_USER};
GRANT USAGE, SELECT, UPDATE ON ALL SEQUENCES IN SCHEMA nommie TO ${APP_DB_USER};

ALTER DEFAULT PRIVILEGES IN SCHEMA nommie
  GRANT SELECT, INSERT, UPDATE, DELETE ON TABLES TO ${APP_DB_USER};

ALTER DEFAULT PRIVILEGES IN SCHEMA nommie
  GRANT USAGE, SELECT, UPDATE ON SEQUENCES TO ${APP_DB_USER};

-- Set role search_path
ALTER ROLE ${APP_DB_USER} SET search_path TO nommie;
EOSQL
}

# --- 3) Apply performance indexes (idempotent) ---
apply_indexes() {
  local dbname="$1"
  echo "[init] indexes for ${dbname}"
  psql -U "$POSTGRES_USER" -d "$dbname" <<-EOSQL
-- Lobby
CREATE INDEX IF NOT EXISTS idx_games_status     ON nommie.games(status);
CREATE INDEX IF NOT EXISTS idx_games_started_at ON nommie.games(started_at DESC);

-- Players
CREATE INDEX IF NOT EXISTS idx_game_players_game_id ON nommie.game_players(game_id);

-- Rounds
CREATE UNIQUE INDEX IF NOT EXISTS idx_rounds_game_round_unique ON nommie.game_rounds(game_id, round_number);
CREATE INDEX IF NOT EXISTS idx_rounds_game_id ON nommie.game_rounds(game_id);

-- Bids
CREATE UNIQUE INDEX IF NOT EXISTS idx_bids_round_player_unique ON nommie.round_bids(round_id, player_id);
CREATE INDEX IF NOT EXISTS idx_bids_round_id ON nommie.round_bids(round_id);

-- Hands
CREATE INDEX IF NOT EXISTS idx_hands_round_id   ON nommie.round_hands(round_id);
CREATE INDEX IF NOT EXISTS idx_hands_player_id  ON nommie.round_hands(player_id);

-- Tricks
CREATE UNIQUE INDEX IF NOT EXISTS idx_tricks_round_trick_unique ON nommie.round_tricks(round_id, trick_number);
CREATE INDEX IF NOT EXISTS idx_tricks_round_id ON nommie.round_tricks(round_id);

-- Trick plays
CREATE UNIQUE INDEX IF NOT EXISTS idx_trick_plays_trick_player_unique ON nommie.trick_plays(trick_id, player_id);
CREATE INDEX IF NOT EXISTS idx_trick_plays_trick_id ON nommie.trick_plays(trick_id);

-- Round scores
CREATE INDEX IF NOT EXISTS idx_round_scores_round_id  ON nommie.round_scores(round_id);
CREATE INDEX IF NOT EXISTS idx_round_scores_player_id ON nommie.round_scores(player_id);
EOSQL
}

# ---- run all steps for both DBs ----
ensure_role

ensure_db "${POSTGRES_DB}"
ensure_db "${TEST_DB}"

apply_schema_and_grants "${POSTGRES_DB}"
apply_schema_and_grants "${TEST_DB}"

apply_indexes "${POSTGRES_DB}"
apply_indexes "${TEST_DB}"

echo "[init] done."
