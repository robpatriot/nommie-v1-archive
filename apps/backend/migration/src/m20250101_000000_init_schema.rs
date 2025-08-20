use sea_orm::Statement;
use sea_orm_migration::prelude::*;
use uuid::Uuid;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Create users table
        manager
            .create_table(
                Table::create()
                    .table(Users::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Users::Id).uuid().not_null().primary_key())
                    .col(
                        ColumnDef::new(Users::ExternalId)
                            .string()
                            .not_null()
                            .unique_key(),
                    )
                    .col(
                        ColumnDef::new(Users::Email)
                            .string()
                            .not_null()
                            .unique_key(),
                    )
                    .col(ColumnDef::new(Users::Name).string().null())
                    .col(
                        ColumnDef::new(Users::IsAi)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .col(
                        ColumnDef::new(Users::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Users::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        // Insert 3 AI users for development
        let now = chrono::Utc::now();
        let ai_users = vec![
            (
                "ai_user_1",
                "__ai+1@nommie.dev",
                "ChessMaster Bot",
                now,
                now,
            ),
            ("ai_user_2", "__ai+2@nommie.dev", "Strategy Sage", now, now),
            (
                "ai_user_3",
                "__ai+3@nommie.dev",
                "Tactical Turtle",
                now,
                now,
            ),
        ];

        for (external_id, email, name, created_at, updated_at) in ai_users {
            manager
                .get_connection()
                .execute(
                    Statement::from_sql_and_values(
                        manager.get_database_backend(),
                        r#"INSERT INTO users (id, external_id, email, name, is_ai, created_at, updated_at) 
                           VALUES ($1, $2, $3, $4, $5, $6, $7)"#,
                        vec![
                            Uuid::new_v4().into(),
                            external_id.into(),
                            email.into(),
                            name.into(),
                            true.into(),
                            created_at.into(),
                            updated_at.into(),
                        ],
                    ),
                )
                .await?;
        }

        // Create games table
        manager
            .create_table(
                Table::create()
                    .table(Games::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Games::Id).uuid().not_null().primary_key())
                    .col(ColumnDef::new(Games::State).string_len(20).not_null())
                    .col(
                        ColumnDef::new(Games::Phase)
                            .string_len(20)
                            .not_null()
                            .default("bidding"),
                    )
                    .col(ColumnDef::new(Games::CurrentTurn).integer().null())
                    .col(
                        ColumnDef::new(Games::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Games::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Games::StartedAt)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(Games::CompletedAt)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .to_owned(),
            )
            .await?;

        // Create game_players table
        manager
            .create_table(
                Table::create()
                    .table(GamePlayers::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(GamePlayers::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(GamePlayers::GameId).uuid().not_null())
                    .col(ColumnDef::new(GamePlayers::UserId).uuid().not_null())
                    .col(ColumnDef::new(GamePlayers::TurnOrder).integer().null())
                    .col(
                        ColumnDef::new(GamePlayers::IsReady)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_game_players_game_id")
                            .from(GamePlayers::Table, GamePlayers::GameId)
                            .to(Games::Table, Games::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_game_players_user_id")
                            .from(GamePlayers::Table, GamePlayers::UserId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // Create game_rounds table
        manager
            .create_table(
                Table::create()
                    .table(GameRounds::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(GameRounds::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(GameRounds::GameId).uuid().not_null())
                    .col(ColumnDef::new(GameRounds::RoundNumber).integer().not_null())
                    .col(ColumnDef::new(GameRounds::DealerPlayerId).uuid().null())
                    .col(ColumnDef::new(GameRounds::TrumpSuit).string_len(10).null())
                    .col(
                        ColumnDef::new(GameRounds::CardsDealt)
                            .integer()
                            .not_null()
                            .default(13),
                    )
                    .col(
                        ColumnDef::new(GameRounds::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_game_rounds_game_id")
                            .from(GameRounds::Table, GameRounds::GameId)
                            .to(Games::Table, Games::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_game_rounds_dealer_player_id")
                            .from(GameRounds::Table, GameRounds::DealerPlayerId)
                            .to(GamePlayers::Table, GamePlayers::Id)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .to_owned(),
            )
            .await?;

        // Create round_bids table
        manager
            .create_table(
                Table::create()
                    .table(RoundBids::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(RoundBids::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(RoundBids::RoundId).uuid().not_null())
                    .col(ColumnDef::new(RoundBids::PlayerId).uuid().not_null())
                    .col(ColumnDef::new(RoundBids::Bid).integer().not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_round_bids_round_id")
                            .from(RoundBids::Table, RoundBids::RoundId)
                            .to(GameRounds::Table, GameRounds::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_round_bids_player_id")
                            .from(RoundBids::Table, RoundBids::PlayerId)
                            .to(GamePlayers::Table, GamePlayers::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // Create round_tricks table
        manager
            .create_table(
                Table::create()
                    .table(RoundTricks::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(RoundTricks::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(RoundTricks::RoundId).uuid().not_null())
                    .col(
                        ColumnDef::new(RoundTricks::TrickNumber)
                            .integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(RoundTricks::WinnerPlayerId).uuid().null())
                    .col(
                        ColumnDef::new(RoundTricks::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_round_tricks_round_id")
                            .from(RoundTricks::Table, RoundTricks::RoundId)
                            .to(GameRounds::Table, GameRounds::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_round_tricks_winner_player_id")
                            .from(RoundTricks::Table, RoundTricks::WinnerPlayerId)
                            .to(GamePlayers::Table, GamePlayers::Id)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .to_owned(),
            )
            .await?;

        // Create round_scores table
        manager
            .create_table(
                Table::create()
                    .table(RoundScores::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(RoundScores::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(RoundScores::RoundId).uuid().not_null())
                    .col(ColumnDef::new(RoundScores::PlayerId).uuid().not_null())
                    .col(
                        ColumnDef::new(RoundScores::TricksWon)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_round_scores_round_id")
                            .from(RoundScores::Table, RoundScores::RoundId)
                            .to(GameRounds::Table, GameRounds::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_round_scores_player_id")
                            .from(RoundScores::Table, RoundScores::PlayerId)
                            .to(GamePlayers::Table, GamePlayers::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // Create trick_plays table
        manager
            .create_table(
                Table::create()
                    .table(TrickPlays::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(TrickPlays::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(TrickPlays::TrickId).uuid().not_null())
                    .col(ColumnDef::new(TrickPlays::PlayerId).uuid().not_null())
                    .col(ColumnDef::new(TrickPlays::Card).string_len(10).not_null())
                    .col(ColumnDef::new(TrickPlays::PlayOrder).integer().not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_trick_plays_trick_id")
                            .from(TrickPlays::Table, TrickPlays::TrickId)
                            .to(RoundTricks::Table, RoundTricks::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_trick_plays_player_id")
                            .from(TrickPlays::Table, TrickPlays::PlayerId)
                            .to(GamePlayers::Table, GamePlayers::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // Create round_hands table
        manager
            .create_table(
                Table::create()
                    .table(RoundHands::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(RoundHands::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(RoundHands::RoundId).uuid().not_null())
                    .col(ColumnDef::new(RoundHands::PlayerId).uuid().not_null())
                    .col(ColumnDef::new(RoundHands::Card).string_len(10).not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_round_hands_round_id")
                            .from(RoundHands::Table, RoundHands::RoundId)
                            .to(GameRounds::Table, GameRounds::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_round_hands_player_id")
                            .from(RoundHands::Table, RoundHands::PlayerId)
                            .to(GamePlayers::Table, GamePlayers::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // Create performance indexes
        manager
            .create_index(
                Index::create()
                    .name("idx_games_status")
                    .table(Games::Table)
                    .col(Games::State)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_games_started_at")
                    .table(Games::Table)
                    .col(Games::StartedAt)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_game_players_game_id")
                    .table(GamePlayers::Table)
                    .col(GamePlayers::GameId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_rounds_game_round_unique")
                    .table(GameRounds::Table)
                    .col(GameRounds::GameId)
                    .col(GameRounds::RoundNumber)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_rounds_game_id")
                    .table(GameRounds::Table)
                    .col(GameRounds::GameId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_bids_round_player_unique")
                    .table(RoundBids::Table)
                    .col(RoundBids::RoundId)
                    .col(RoundBids::PlayerId)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_bids_round_id")
                    .table(RoundBids::Table)
                    .col(RoundBids::RoundId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_hands_round_id")
                    .table(RoundHands::Table)
                    .col(RoundHands::RoundId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_hands_player_id")
                    .table(RoundHands::Table)
                    .col(RoundHands::PlayerId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_tricks_round_trick_unique")
                    .table(RoundTricks::Table)
                    .col(RoundTricks::RoundId)
                    .col(RoundTricks::TrickNumber)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_tricks_round_id")
                    .table(RoundTricks::Table)
                    .col(RoundTricks::RoundId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_trick_plays_trick_player_unique")
                    .table(TrickPlays::Table)
                    .col(TrickPlays::TrickId)
                    .col(TrickPlays::PlayerId)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_trick_plays_trick_id")
                    .table(TrickPlays::Table)
                    .col(TrickPlays::TrickId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_round_scores_round_id")
                    .table(RoundScores::Table)
                    .col(RoundScores::RoundId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_round_scores_player_id")
                    .table(RoundScores::Table)
                    .col(RoundScores::PlayerId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Drop indexes first
        manager
            .drop_index(
                Index::drop()
                    .name("idx_round_scores_player_id")
                    .table(RoundScores::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_index(
                Index::drop()
                    .name("idx_round_scores_round_id")
                    .table(RoundScores::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_index(
                Index::drop()
                    .name("idx_trick_plays_trick_id")
                    .table(TrickPlays::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_index(
                Index::drop()
                    .name("idx_trick_plays_trick_player_unique")
                    .table(TrickPlays::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_index(
                Index::drop()
                    .name("idx_tricks_round_id")
                    .table(RoundTricks::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_index(
                Index::drop()
                    .name("idx_tricks_round_trick_unique")
                    .table(RoundTricks::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_index(
                Index::drop()
                    .name("idx_hands_player_id")
                    .table(RoundHands::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_index(
                Index::drop()
                    .name("idx_hands_round_id")
                    .table(RoundHands::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_index(
                Index::drop()
                    .name("idx_bids_round_id")
                    .table(RoundBids::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_index(
                Index::drop()
                    .name("idx_bids_round_player_unique")
                    .table(RoundBids::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_index(
                Index::drop()
                    .name("idx_rounds_game_id")
                    .table(GameRounds::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_index(
                Index::drop()
                    .name("idx_rounds_game_round_unique")
                    .table(GameRounds::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_index(
                Index::drop()
                    .name("idx_game_players_game_id")
                    .table(GamePlayers::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_index(
                Index::drop()
                    .name("idx_games_started_at")
                    .table(Games::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_index(
                Index::drop()
                    .name("idx_games_status")
                    .table(Games::Table)
                    .to_owned(),
            )
            .await?;

        // Drop tables in reverse order
        manager
            .drop_table(Table::drop().table(RoundHands::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(TrickPlays::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(RoundScores::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(RoundTricks::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(RoundBids::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(GameRounds::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(GamePlayers::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Games::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Users::Table).to_owned())
            .await?;
        Ok(())
    }
}

#[derive(DeriveIden)]
enum Users {
    Table,
    Id,
    ExternalId,
    Email,
    Name,
    IsAi,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum Games {
    Table,
    Id,
    State,
    Phase,
    CurrentTurn,
    CreatedAt,
    UpdatedAt,
    StartedAt,
    CompletedAt,
}

#[derive(DeriveIden)]
enum GamePlayers {
    Table,
    Id,
    GameId,
    UserId,
    TurnOrder,
    IsReady,
}

#[derive(DeriveIden)]
enum GameRounds {
    Table,
    Id,
    GameId,
    RoundNumber,
    DealerPlayerId,
    TrumpSuit,
    CardsDealt,
    CreatedAt,
}

#[derive(DeriveIden)]
enum RoundBids {
    Table,
    Id,
    RoundId,
    PlayerId,
    Bid,
}

#[derive(DeriveIden)]
enum RoundTricks {
    Table,
    Id,
    RoundId,
    TrickNumber,
    WinnerPlayerId,
    CreatedAt,
}

#[derive(DeriveIden)]
enum RoundScores {
    Table,
    Id,
    RoundId,
    PlayerId,
    TricksWon,
}

#[derive(DeriveIden)]
enum TrickPlays {
    Table,
    Id,
    TrickId,
    PlayerId,
    Card,
    PlayOrder,
}

#[derive(DeriveIden)]
enum RoundHands {
    Table,
    Id,
    RoundId,
    PlayerId,
    Card,
}
