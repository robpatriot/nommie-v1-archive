#!/bin/bash
set -e

psql -U "$POSTGRES_USER" -d "$POSTGRES_DB" <<-EOSQL
  CREATE ROLE ${APP_DB_USER} WITH LOGIN PASSWORD '${APP_DB_PASSWORD}';

  CREATE SCHEMA IF NOT EXISTS nommie;

  GRANT USAGE ON SCHEMA nommie TO ${APP_DB_USER};
  GRANT CREATE ON SCHEMA nommie TO ${APP_DB_USER};

  GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA nommie TO ${APP_DB_USER};
  GRANT USAGE, SELECT, UPDATE ON ALL SEQUENCES IN SCHEMA nommie TO ${APP_DB_USER};

  ALTER DEFAULT PRIVILEGES IN SCHEMA nommie
    GRANT SELECT, INSERT, UPDATE, DELETE ON TABLES TO ${APP_DB_USER};

  ALTER DEFAULT PRIVILEGES IN SCHEMA nommie
    GRANT USAGE, SELECT, UPDATE ON SEQUENCES TO ${APP_DB_USER};

  ALTER ROLE ${APP_DB_USER} SET search_path TO nommie;
EOSQL

# Performance indexes for optimal query performance
psql -U "$POSTGRES_USER" -d "$POSTGRES_DB" <<-EOSQL
  -- Lobby
  CREATE INDEX IF NOT EXISTS idx_games_status ON nommie.games(status);
  CREATE INDEX IF NOT EXISTS idx_games_started_at ON nommie.games(started_at DESC);
  
  -- Players
  CREATE INDEX IF NOT EXISTS idx_game_players_game_id ON nommie.game_players(game_id);
  
  -- Rounds
  CREATE UNIQUE INDEX IF NOT EXISTS idx_rounds_game_round_unique ON nommie.game_rounds(game_id, round_number);
  CREATE INDEX IF NOT EXISTS idx_rounds_game_id ON nommie.game_rounds(game_id);
  
  -- Bids
  CREATE UNIQUE INDEX IF NOT EXISTS idx_bids_round_player_unique ON nommie.round_bids(round_id, player_id);
  CREATE INDEX IF NOT EXISTS idx_bids_round_id ON nommie.round_bids(round_id);
  
  -- Hands (individual cards per player per round)
  CREATE INDEX IF NOT EXISTS idx_hands_round_id ON nommie.round_hands(round_id);
  CREATE INDEX IF NOT EXISTS idx_hands_player_id ON nommie.round_hands(player_id);
  
  -- Tricks
  CREATE UNIQUE INDEX IF NOT EXISTS idx_tricks_round_trick_unique ON nommie.round_tricks(round_id, trick_number);
  CREATE INDEX IF NOT EXISTS idx_tricks_round_id ON nommie.round_tricks(round_id);
  
  -- Trick plays
  CREATE UNIQUE INDEX IF NOT EXISTS idx_trick_plays_trick_player_unique ON nommie.trick_plays(trick_id, player_id);
  CREATE INDEX IF NOT EXISTS idx_trick_plays_trick_id ON nommie.trick_plays(trick_id);
  
  -- Round scores
  CREATE INDEX IF NOT EXISTS idx_round_scores_round_id ON nommie.round_scores(round_id);
  CREATE INDEX IF NOT EXISTS idx_round_scores_player_id ON nommie.round_scores(player_id);
EOSQL
