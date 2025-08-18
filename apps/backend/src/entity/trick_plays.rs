use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "trick_plays")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub trick_id: Uuid,
    pub player_id: Uuid,
    pub card: String,
    pub play_order: i32,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::round_tricks::Entity",
        from = "Column::TrickId",
        to = "super::round_tricks::Column::Id"
    )]
    RoundTricks,
    #[sea_orm(
        belongs_to = "super::game_players::Entity",
        from = "Column::PlayerId",
        to = "super::game_players::Column::Id"
    )]
    GamePlayers,
}

impl Related<super::round_tricks::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::RoundTricks.def()
    }
}

impl Related<super::game_players::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::GamePlayers.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
