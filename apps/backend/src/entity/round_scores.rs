use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "round_scores")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub round_id: Uuid,
    pub player_id: Uuid,
    pub tricks_won: i32,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::game_rounds::Entity",
        from = "Column::RoundId",
        to = "super::game_rounds::Column::Id"
    )]
    GameRound,
    #[sea_orm(
        belongs_to = "super::game_players::Entity",
        from = "Column::PlayerId",
        to = "super::game_players::Column::Id"
    )]
    GamePlayer,
}

impl Related<super::game_rounds::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::GameRound.def()
    }
}

impl Related<super::game_players::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::GamePlayer.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
