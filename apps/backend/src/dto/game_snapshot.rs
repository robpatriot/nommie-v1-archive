use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, FixedOffset};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameSnapshot {
    pub game: GameInfo,
    pub players: Vec<PlayerSnapshot>,
    pub current_round: Option<RoundSnapshot>,
    pub player_count: usize,
    pub max_players: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameInfo {
    pub id: Uuid,
    pub state: String,
    pub phase: String,
    pub current_turn: Option<i32>,
    pub created_at: DateTime<FixedOffset>,
    pub updated_at: DateTime<FixedOffset>,
    pub started_at: Option<DateTime<FixedOffset>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerSnapshot {
    pub id: Uuid,
    pub user_id: Uuid,
    pub turn_order: Option<i32>,
    pub is_ready: bool,
    pub is_ai: bool,
    pub user: UserSnapshot,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserSnapshot {
    pub id: Uuid,
    pub email: String,
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoundSnapshot {
    pub id: Uuid,
    pub round_number: i32,
    pub phase: String,
    pub dealer_player_id: Option<Uuid>,
    pub trump_suit: Option<String>,
    pub bids: Vec<RoundBidSnapshot>,
    pub current_bidder_turn: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoundBidSnapshot {
    pub player_id: Uuid,
    pub bid: i32,
} 