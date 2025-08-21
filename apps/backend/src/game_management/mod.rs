//! Game management: thin orchestration + cross-cutting helpers.
//! Domain logic lives in `rules`, `bidding`, `tricks`, `scoring`, `state`.
//! HTTP handlers are defined in `routes::game` and wired via configure_routes.

pub mod bidding;
pub mod orchestration;
pub mod rules;
pub mod scoring;
pub mod state;
pub mod tricks;

use uuid::Uuid;

/// Perform AI card play action
#[allow(dead_code)]
async fn perform_ai_card_play(
    _game_id: Uuid,
    _player_id: Uuid,
    _play_request: crate::dto::play_request::PlayRequest,
    _db: &sea_orm::DatabaseConnection,
) -> Result<(), String> {
    // TODO: Implement AI card play logic
    // This function was temporarily removed during bidding refactor
    // and needs to be properly implemented
    Err("AI card play not yet implemented".to_string())
}

/// Helper function to play a card within a transaction
pub(crate) async fn play_card_transaction(
    game_id: Uuid,
    user_id: Uuid,
    card: String,
    txn: &sea_orm::DatabaseTransaction,
) -> Result<(), String> {
    // Validate the card format first
    if !crate::game_management::rules::is_valid_card_format(&card) {
        return Err("Invalid card format. Use format like '5S', 'AH', 'KD'".to_string());
    }

    // Delegate to the orchestration module for all trick logic
    crate::game_management::orchestration::play_card(game_id, user_id, &card, txn).await
}

#[cfg(test)]
mod tests {
    // Bidding tests have been moved to bidding.rs
}
