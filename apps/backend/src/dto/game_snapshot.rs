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
    pub trump_chooser_id: Option<Uuid>,
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
    pub total_score: i32,
    pub hand: Option<Vec<String>>, // Cards in player's hand (only shown to the player themselves)
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
    pub cards_dealt: i32,
    pub bids: Vec<RoundBidSnapshot>,
    pub current_bidder_turn: Option<i32>,
    pub current_trick: Option<TrickSnapshot>,
    pub completed_tricks: Vec<TrickSnapshot>,
    pub current_player_turn: Option<Uuid>,
    pub round_scores: Vec<RoundScoreSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrickSnapshot {
    pub id: Uuid,
    pub trick_number: i32,
    pub winner_player_id: Option<Uuid>,
    pub plays: Vec<TrickPlaySnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrickPlaySnapshot {
    pub player_id: Uuid,
    pub card: String,
    pub play_order: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoundBidSnapshot {
    pub player_id: Uuid,
    pub bid: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoundScoreSnapshot {
    pub player_id: Uuid,
    pub tricks_won: i32,
    pub bid: i32,
    pub points: i32,
} 