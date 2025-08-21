//! Game orchestration module
//!
//! This module contains database-coupled orchestration logic for game operations.
//! It handles database transactions, entity management, and coordinates between
//! pure domain logic modules.

use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseTransaction, EntityTrait, QueryFilter, QueryOrder,
    QuerySelect, Set,
};
use uuid::Uuid;

use crate::entity::{game_players, game_rounds, games, round_hands, round_tricks, trick_plays};
use crate::game_management::tricks;

/// Validate that a play is legal according to game rules:
/// - Player must own the card
/// - Must follow suit if possible
/// - Must be player's turn
/// - Game must be in playing phase
pub(crate) async fn validate_play(
    game_id: Uuid,
    user_id: Uuid,
    card: &str,
    txn: &DatabaseTransaction,
) -> Result<(), String> {
    // Lock the game row for update to prevent concurrent modifications
    let game = match games::Entity::find_by_id(game_id)
        .lock(sea_orm::sea_query::LockType::Update)
        .one(txn)
        .await
    {
        Ok(Some(game)) => game,
        Ok(None) => {
            return Err("Game not found".to_string());
        }
        Err(e) => {
            return Err(format!("Failed to fetch game: {e}"));
        }
    };

    // Validate that the game is in the Playing phase
    if game.phase != games::GamePhase::Playing {
        return Err("Game is not in playing phase".to_string());
    }

    // Fetch the current player's game_player record
    let current_player = match game_players::Entity::find()
        .filter(game_players::Column::GameId.eq(game_id))
        .filter(game_players::Column::UserId.eq(user_id))
        .one(txn)
        .await
    {
        Ok(Some(player)) => player,
        Ok(None) => {
            return Err("You are not a participant in this game".to_string());
        }
        Err(e) => {
            return Err(format!("Failed to fetch player data: {e}"));
        }
    };

    // Check if it's this player's turn to play
    let current_turn = game.current_turn.unwrap_or(0);
    if current_player.turn_order.unwrap_or(-1) != current_turn {
        return Err("It's not your turn to play".to_string());
    }

    // Lock the current round row for update
    let current_round = match game_rounds::Entity::find()
        .filter(game_rounds::Column::GameId.eq(game_id))
        .order_by_desc(game_rounds::Column::RoundNumber)
        .lock(sea_orm::sea_query::LockType::Update)
        .one(txn)
        .await
    {
        Ok(Some(round)) => round,
        Ok(None) => {
            return Err("No current round found".to_string());
        }
        Err(e) => {
            return Err(format!("Failed to fetch current round: {e}"));
        }
    };

    // Check if the player has the card in their hand
    let player_hand = match round_hands::Entity::find()
        .filter(round_hands::Column::RoundId.eq(current_round.id))
        .filter(round_hands::Column::PlayerId.eq(current_player.id))
        .all(txn)
        .await
    {
        Ok(hand) => hand,
        Err(e) => {
            return Err(format!("Failed to fetch player hand: {e}"));
        }
    };

    let has_card = player_hand.iter().any(|h| h.card == card);
    if !has_card {
        return Err("You don't have that card in your hand".to_string());
    }

    // Get the current trick
    let current_trick = match round_tricks::Entity::find()
        .filter(round_tricks::Column::RoundId.eq(current_round.id))
        .order_by_desc(round_tricks::Column::TrickNumber)
        .one(txn)
        .await
    {
        Ok(Some(trick)) => trick,
        Ok(None) => {
            return Err("No current trick found".to_string());
        }
        Err(e) => {
            return Err(format!("Failed to fetch current trick: {e}"));
        }
    };

    // Check if this player has already played in this trick (idempotency check)
    let existing_play = match trick_plays::Entity::find()
        .filter(trick_plays::Column::TrickId.eq(current_trick.id))
        .filter(trick_plays::Column::PlayerId.eq(current_player.id))
        .one(txn)
        .await
    {
        Ok(Some(_)) => true,
        Ok(None) => false,
        Err(e) => {
            return Err(format!("Failed to check existing play: {e}"));
        }
    };

    if existing_play {
        return Err("You have already played a card in this trick".to_string());
    }

    // Check if this is the first play in the trick (to determine lead suit)
    let trick_plays = match trick_plays::Entity::find()
        .filter(trick_plays::Column::TrickId.eq(current_trick.id))
        .all(txn)
        .await
    {
        Ok(plays) => plays,
        Err(e) => {
            return Err(format!("Failed to fetch trick plays: {e}"));
        }
    };

    let is_first_play = trick_plays.is_empty();
    if !is_first_play {
        // Enforce follow-suit rule if not the first play
        let lead_suit = if let Some(first_play) = trick_plays.first() {
            if first_play.card.len() >= 2 {
                &first_play.card[1..2]
            } else {
                return Err("Invalid first card format".to_string());
            }
        } else {
            return Err("No first play found".to_string());
        };

        let card_suit = if card.len() >= 2 {
            &card[1..2]
        } else {
            return Err("Invalid card format".to_string());
        };

        if card_suit != lead_suit {
            // Check if player has any cards of the lead suit
            let has_lead_suit = player_hand.iter().any(|h| {
                if h.card.len() >= 2 {
                    &h.card[1..2] == lead_suit
                } else {
                    false
                }
            });

            if has_lead_suit {
                return Err("You must follow suit if possible".to_string());
            }
        }
    }

    Ok(())
}

/// Apply a card play to the current trick
///
/// This function records the card play and removes it from the player's hand.
/// It does not handle turn progression or trick completion.
pub(crate) async fn apply_play(
    game_id: Uuid,
    user_id: Uuid,
    card: &str,
    txn: &DatabaseTransaction,
) -> Result<(), String> {
    // Lock the current round row for update
    let current_round = match game_rounds::Entity::find()
        .filter(game_rounds::Column::GameId.eq(game_id))
        .order_by_desc(game_rounds::Column::RoundNumber)
        .lock(sea_orm::sea_query::LockType::Update)
        .one(txn)
        .await
    {
        Ok(Some(round)) => round,
        Ok(None) => {
            return Err("No current round found".to_string());
        }
        Err(e) => {
            return Err(format!("Failed to fetch current round: {e}"));
        }
    };

    // Fetch the current player's game_player record
    let current_player = match game_players::Entity::find()
        .filter(game_players::Column::GameId.eq(game_id))
        .filter(game_players::Column::UserId.eq(user_id))
        .one(txn)
        .await
    {
        Ok(Some(player)) => player,
        Ok(None) => {
            return Err("You are not a participant in this game".to_string());
        }
        Err(e) => {
            return Err(format!("Failed to fetch player data: {e}"));
        }
    };

    // Get the current trick
    let current_trick = match round_tricks::Entity::find()
        .filter(round_tricks::Column::RoundId.eq(current_round.id))
        .order_by_desc(round_tricks::Column::TrickNumber)
        .one(txn)
        .await
    {
        Ok(Some(trick)) => trick,
        Ok(None) => {
            return Err("No current trick found".to_string());
        }
        Err(e) => {
            return Err(format!("Failed to fetch current trick: {e}"));
        }
    };

    // Record the card play
    let play_model = trick_plays::ActiveModel {
        id: Set(Uuid::new_v4()),
        trick_id: Set(current_trick.id),
        player_id: Set(current_player.id),
        card: Set(card.to_string()),
        play_order: Set(0), // This will be set by the caller
    };

    if let Err(e) = play_model.insert(txn).await {
        return Err(format!("Failed to record card play: {e}"));
    }

    // Remove the card from the player's hand
    let hand_to_remove = match round_hands::Entity::find()
        .filter(round_hands::Column::RoundId.eq(current_round.id))
        .filter(round_hands::Column::PlayerId.eq(current_player.id))
        .filter(round_hands::Column::Card.eq(card))
        .one(txn)
        .await
    {
        Ok(Some(hand)) => hand,
        Ok(None) => {
            return Err("Card not found in player's hand".to_string());
        }
        Err(e) => {
            return Err(format!("Failed to find card in hand: {e}"));
        }
    };

    if let Err(e) = round_hands::Entity::delete_by_id(hand_to_remove.id)
        .exec(txn)
        .await
    {
        return Err(format!("Failed to remove card from hand: {e}"));
    }

    Ok(())
}

/// Check if a trick is complete and handle progression
///
/// This function checks if all players have played in the current trick
/// and handles either starting the next trick or completing the round.
pub(crate) async fn maybe_advance_after_play(
    game_id: Uuid,
    txn: &DatabaseTransaction,
) -> Result<(), String> {
    // Lock the game row for update
    let game = match games::Entity::find_by_id(game_id)
        .lock(sea_orm::sea_query::LockType::Update)
        .one(txn)
        .await
    {
        Ok(Some(game)) => game,
        Ok(None) => {
            return Err("Game not found".to_string());
        }
        Err(e) => {
            return Err(format!("Failed to fetch game: {e}"));
        }
    };

    // Lock the current round row for update
    let current_round = match game_rounds::Entity::find()
        .filter(game_rounds::Column::GameId.eq(game_id))
        .order_by_desc(game_rounds::Column::RoundNumber)
        .lock(sea_orm::sea_query::LockType::Update)
        .one(txn)
        .await
    {
        Ok(Some(round)) => round,
        Ok(None) => {
            return Err("No current round found".to_string());
        }
        Err(e) => {
            return Err(format!("Failed to fetch current round: {e}"));
        }
    };

    // Get the current trick
    let current_trick = match round_tricks::Entity::find()
        .filter(round_tricks::Column::RoundId.eq(current_round.id))
        .order_by_desc(round_tricks::Column::TrickNumber)
        .one(txn)
        .await
    {
        Ok(Some(trick)) => trick,
        Ok(None) => {
            return Err("No current trick found".to_string());
        }
        Err(e) => {
            return Err(format!("Failed to fetch current trick: {e}"));
        }
    };

    // Get all players in the game
    let all_players = match game_players::Entity::find()
        .filter(game_players::Column::GameId.eq(game_id))
        .all(txn)
        .await
    {
        Ok(players) => players,
        Err(e) => {
            return Err(format!("Failed to fetch all players: {e}"));
        }
    };

    // Get current trick plays
    let trick_plays = match trick_plays::Entity::find()
        .filter(trick_plays::Column::TrickId.eq(current_trick.id))
        .all(txn)
        .await
    {
        Ok(plays) => plays,
        Err(e) => {
            return Err(format!("Failed to fetch trick plays: {e}"));
        }
    };

    // Use pure logic to determine advancement
    let current_turn = game.current_turn.unwrap_or(0);
    let cards_per_player = current_round.cards_dealt;

    // Convert trick plays to pure domain format
    let pure_trick_plays: Vec<(String, Uuid)> = trick_plays
        .iter()
        .map(|p| (p.card.clone(), p.player_id))
        .collect();

    let advancement = tricks::advance_trick_logic(
        &pure_trick_plays,
        all_players.len(),
        current_turn,
        current_trick.trick_number,
        cards_per_player,
        &current_round.trump_suit,
    );

    if advancement.trick_complete {
        // Record the trick winner
        if let Some(winner_id) = advancement.winner_user_id {
            let winner_player = all_players
                .iter()
                .find(|p| p.user_id == winner_id)
                .ok_or("Winner not found in player list")?;

            let mut trick_model: round_tricks::ActiveModel = current_trick.clone().into();
            trick_model.winner_player_id = Set(Some(winner_player.id));
            if let Err(e) = trick_model.update(txn).await {
                return Err(format!("Failed to update trick winner: {e}"));
            }
        }

        if advancement.round_complete {
            // Round is complete, advance to scoring
            let mut game_model: games::ActiveModel = game.into();
            game_model.phase = Set(games::GamePhase::Scoring);
            game_model.current_turn = Set(Some(0));
            if let Err(e) = game_model.update(txn).await {
                return Err(format!("Failed to update game phase: {e}"));
            }
        } else {
            // Start next trick
            let next_trick_number = current_trick.trick_number + 1;
            let next_trick_model = round_tricks::ActiveModel {
                id: Set(Uuid::new_v4()),
                round_id: Set(current_round.id),
                trick_number: Set(next_trick_number),
                winner_player_id: Set(None),
                created_at: Set(chrono::Utc::now().into()),
            };

            if let Err(e) = next_trick_model.insert(txn).await {
                return Err(format!("Failed to create next trick: {e}"));
            }

            // Move to next player's turn using state module
            if let Err(e) =
                crate::game_management::state::set_next_player(&game, advancement.next_turn, txn)
                    .await
            {
                return Err(format!("Failed to update turn: {e}"));
            }
        }
    } else {
        // Move to next player's turn using state module
        if let Err(e) =
            crate::game_management::state::set_next_player(&game, advancement.next_turn, txn).await
        {
            return Err(format!("Failed to update turn: {e}"));
        }
    }

    Ok(())
}

/// Play a card and handle all trick logic
///
/// This is the main entry point for playing a card. It validates the play,
/// applies it, and handles trick progression.
pub(crate) async fn play_card(
    game_id: Uuid,
    user_id: Uuid,
    card: &str,
    txn: &DatabaseTransaction,
) -> Result<(), String> {
    // Validate the play
    validate_play(game_id, user_id, card, txn).await?;

    // Apply the play
    apply_play(game_id, user_id, card, txn).await?;

    // Handle progression
    maybe_advance_after_play(game_id, txn).await?;

    Ok(())
}
