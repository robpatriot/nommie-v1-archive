use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "users")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    #[sea_orm(unique)]
    pub external_id: String,
    #[sea_orm(unique)]
    pub email: String,
    pub name: Option<String>,
    pub is_ai: bool,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
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
