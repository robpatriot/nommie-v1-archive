use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "game_rounds")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub game_id: Uuid,
    pub round_number: i32,
    pub dealer_player_id: Option<Uuid>,
    pub trump_suit: Option<String>,
    pub created_at: DateTimeWithTimeZone,
}



#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::games::Entity",
        from = "Column::GameId",
        to = "super::games::Column::Id"
    )]
    Game,
    #[sea_orm(
        belongs_to = "super::game_players::Entity",
        from = "Column::DealerPlayerId",
        to = "super::game_players::Column::Id"
    )]
    DealerPlayer,
    #[sea_orm(has_many = "super::round_bids::Entity")]
    RoundBids,
}

impl Related<super::games::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Game.def()
    }
}

impl Related<super::game_players::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::DealerPlayer.def()
    }
}

impl Related<super::round_bids::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::RoundBids.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

 