use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Deserialize, Serialize)]
#[sea_orm(table_name = "round_hands")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: Uuid,
    pub round_id: Uuid,
    pub player_id: Uuid,
    pub card: String,
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
        from = "Column::PlayerId",
        to = "super::game_players::Column::Id"
    )]
    GamePlayers,
}

impl Related<super::game_rounds::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::GameRounds.def()
    }
}

impl Related<super::game_players::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::GamePlayers.def()
    }
}

impl ActiveModelBehavior for ActiveModel {} 