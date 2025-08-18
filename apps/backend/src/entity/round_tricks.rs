use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "round_tricks")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub round_id: Uuid,
    pub trick_number: i32,
    pub winner_player_id: Option<Uuid>,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::game_rounds::Entity",
        from = "Column::RoundId",
        to = "super::game_rounds::Column::Id"
    )]
    GameRounds,
    #[sea_orm(
        belongs_to = "super::game_players::Entity",
        from = "Column::WinnerPlayerId",
        to = "super::game_players::Column::Id"
    )]
    WinnerPlayer,
    #[sea_orm(has_many = "super::trick_plays::Entity")]
    TrickPlays,
}

impl Related<super::game_rounds::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::GameRounds.def()
    }
}

impl Related<super::game_players::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::WinnerPlayer.def()
    }
}

impl Related<super::trick_plays::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::TrickPlays.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
