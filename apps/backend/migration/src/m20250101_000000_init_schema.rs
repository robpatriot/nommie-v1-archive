use sea_orm_migration::prelude::*;

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
                    .col(ColumnDef::new(Users::ExternalId).string().not_null().unique_key())
                    .col(ColumnDef::new(Users::Email).string().not_null().unique_key())
                    .col(ColumnDef::new(Users::Name).string().null())
                    .col(ColumnDef::new(Users::CreatedAt).timestamp_with_time_zone().not_null())
                    .col(ColumnDef::new(Users::UpdatedAt).timestamp_with_time_zone().not_null())
                    .to_owned(),
            )
            .await?;

        // Create games table
        manager
            .create_table(
                Table::create()
                    .table(Games::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Games::Id).uuid().not_null().primary_key())
                    .col(ColumnDef::new(Games::State).string_len(20).not_null())
                    .col(ColumnDef::new(Games::CreatedAt).timestamp_with_time_zone().not_null())
                    .col(ColumnDef::new(Games::UpdatedAt).timestamp_with_time_zone().not_null())
                    .to_owned(),
            )
            .await?;

        // Create game_players table
        manager
            .create_table(
                Table::create()
                    .table(GamePlayers::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(GamePlayers::Id).uuid().not_null().primary_key())
                    .col(ColumnDef::new(GamePlayers::GameId).uuid().not_null())
                    .col(ColumnDef::new(GamePlayers::UserId).uuid().not_null())
                    .col(ColumnDef::new(GamePlayers::TurnOrder).integer().null())
                    .col(ColumnDef::new(GamePlayers::IsReady).boolean().not_null().default(false))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_game_players_game_id")
                            .from(GamePlayers::Table, GamePlayers::GameId)
                            .to(Games::Table, Games::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_game_players_user_id")
                            .from(GamePlayers::Table, GamePlayers::UserId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                    )
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Drop tables in reverse order due to foreign key constraints
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
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum Games {
    Table,
    Id,
    State,
    CreatedAt,
    UpdatedAt,
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