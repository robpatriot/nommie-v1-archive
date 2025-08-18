use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameSummary {
    pub game: GameSummaryInfo,
    pub players: Vec<PlayerSummary>,
    pub rounds: Vec<RoundSummary>,
    pub final_round: FinalRoundSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameSummaryInfo {
    pub id: Uuid,
    pub state: String,
    pub created_at: DateTime<FixedOffset>,
    pub updated_at: DateTime<FixedOffset>,
    pub started_at: Option<DateTime<FixedOffset>>,
    pub completed_at: DateTime<FixedOffset>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerSummary {
    pub id: Uuid,
    pub user_id: Uuid,
    pub turn_order: Option<i32>,
    pub is_ai: bool,
    pub final_score: i32,
    pub rank: i32,
    pub user: UserSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserSummary {
    pub id: Uuid,
    pub email: String,
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoundSummary {
    pub round_number: i32,
    pub cards_dealt: i32,
    pub trump_suit: Option<String>,
    pub dealer_player_id: Option<Uuid>,
    pub player_results: Vec<PlayerRoundResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerRoundResult {
    pub player_id: Uuid,
    pub bid: i32,
    pub tricks_won: i32,
    pub points: i32,
    pub bonus: bool, // true if bid matches tricks won
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FinalRoundSummary {
    pub round_number: i32,
    pub cards_dealt: i32,
    pub trump_suit: Option<String>,
    pub dealer_player_id: Option<Uuid>,
    pub bids: Vec<RoundBidSummary>,
    pub tricks_won: Vec<RoundScoreSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoundBidSummary {
    pub player_id: Uuid,
    pub bid: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoundScoreSummary {
    pub player_id: Uuid,
    pub tricks_won: i32,
    pub bid: i32,
    pub points: i32,
}
