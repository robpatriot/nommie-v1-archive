use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "games")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub state: GameState,
    pub phase: GamePhase,
    pub current_turn: Option<i32>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
    pub started_at: Option<DateTimeWithTimeZone>,
    pub completed_at: Option<DateTimeWithTimeZone>,
}

#[derive(Clone, Debug, PartialEq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(20))")]
pub enum GameState {
    #[sea_orm(string_value = "waiting")]
    Waiting,
    #[sea_orm(string_value = "started")]
    Started,
    #[sea_orm(string_value = "completed")]
    Completed,
}

#[derive(Clone, Debug, PartialEq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(20))")]
pub enum GamePhase {
    #[sea_orm(string_value = "bidding")]
    Bidding,
    #[sea_orm(string_value = "trump_selection")]
    TrumpSelection,
    #[sea_orm(string_value = "playing")]
    Playing,
    #[sea_orm(string_value = "scoring")]
    Scoring,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::game_players::Entity")]
    GamePlayers,
}

impl Related<super::game_players::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::GamePlayers.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

impl fmt::Display for GameState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GameState::Waiting => write!(f, "waiting"),
            GameState::Started => write!(f, "started"),
            GameState::Completed => write!(f, "completed"),
        }
    }
}

impl fmt::Display for GamePhase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GamePhase::Bidding => write!(f, "bidding"),
            GamePhase::TrumpSelection => write!(f, "trump_selection"),
            GamePhase::Playing => write!(f, "playing"),
            GamePhase::Scoring => write!(f, "scoring"),
        }
    }
}
