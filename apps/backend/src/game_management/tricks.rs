//! Tricks module
//!
//! This module contains trick playing logic, card comparison,
//! and trick completion mechanisms that depend only on
//! in-memory domain types and std.

use crate::game_management::rules::get_card_rank_value;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseTransaction, EntityTrait, QueryFilter, QueryOrder,
    QuerySelect, Set,
};
use uuid::Uuid;

use crate::entity::{game_players, game_rounds, games, round_hands, round_tricks, trick_plays};

/// Determine the winner of a trick based on card plays and trump suit
///
/// This function is PURE - it determines the trick winner based on card comparison
/// without any side effects. It follows standard trick-taking rules:
/// - Trump beats non-trump
/// - Within the same category (trump or non-trump), highest rank wins
/// - Lead suit is determined by the first card played
///
/// Returns the player ID of the trick winner.
pub fn determine_trick_winner(
    plays: &[(String, uuid::Uuid)], // (card, player_id) tuples
    trump_suit: &Option<String>,
) -> Result<uuid::Uuid, String> {
    if plays.is_empty() {
        return Err("No plays found for trick".to_string());
    }

    // Get the lead suit (suit of the first card played)
    let lead_card = &plays[0].0;
    let lead_suit = if lead_card.len() >= 2 {
        &lead_card[1..2]
    } else {
        return Err("Invalid card format".to_string());
    };

    let mut winning_play = &plays[0];
    let mut winning_rank = get_card_rank_value(&plays[0].0[0..1]);
    let mut winning_is_trump = trump_suit
        .as_ref()
        .is_some_and(|trump| &plays[0].0[1..2] == trump);
    let mut winning_follows_lead = &plays[0].0[1..2] == lead_suit;

    for play in &plays[1..] {
        let card = &play.0;
        if card.len() < 2 {
            continue; // Skip invalid cards
        }

        let rank = &card[0..1];
        let suit = &card[1..2];

        let card_rank = get_card_rank_value(rank);
        let is_trump = trump_suit.as_ref().is_some_and(|trump| suit == trump);
        let follows_lead = suit == lead_suit;

        // Determine if this card should win
        let should_win = if is_trump && !winning_is_trump {
            // Trump beats non-trump
            true
        } else if !is_trump && winning_is_trump {
            // Non-trump cannot beat trump
            false
        } else if is_trump && winning_is_trump {
            // Both are trump, highest rank wins
            card_rank > winning_rank
        } else if follows_lead && winning_follows_lead {
            // Both follow lead suit, highest rank wins
            card_rank > winning_rank
        } else if follows_lead && !winning_follows_lead {
            // This follows lead suit, current winner doesn't
            true
        } else {
            // Neither follows lead suit, neither is trump, cannot win
            false
        };

        if should_win {
            winning_play = play;
            winning_rank = card_rank;
            winning_is_trump = is_trump;
            winning_follows_lead = follows_lead;
        }
    }

    Ok(winning_play.1)
}

/// Check if a player can follow suit
///
/// This function is PURE - it checks if a player has cards of the lead suit
/// without any side effects. Returns true if the player can follow suit.
pub fn can_follow_suit(player_hand: &[String], lead_suit: &str) -> bool {
    player_hand.iter().any(|card| {
        if card.len() >= 2 {
            &card[1..2] == lead_suit
        } else {
            false
        }
    })
}

/// Get the next player's turn after a trick
///
/// This function is PURE - it calculates the next turn using modulo arithmetic.
/// Returns the next turn index (0-3) with wraparound.
pub fn get_next_trick_turn(current_turn: i32) -> i32 {
    (current_turn + 1) % 4
}

/// Check if a trick is complete (all 4 players have played)
///
/// This function is PURE - it checks if the trick has the expected number of plays.
/// Returns true if the trick is complete.
pub fn is_trick_complete(play_count: usize, player_count: usize) -> bool {
    play_count >= player_count
}

/// Get the lead suit from the first card played in a trick
///
/// This function is PURE - it extracts the suit from a card string.
/// Returns the lead suit as a string, or None if the card format is invalid.
pub fn get_lead_suit_from_trick(plays: &[(String, uuid::Uuid)]) -> Option<String> {
    if let Some((first_card, _)) = plays.first() {
        if first_card.len() >= 2 {
            Some(first_card[1..2].to_string())
        } else {
            None
        }
    } else {
        None
    }
}

/// Validate that a card play follows the follow-suit rule
///
/// This function is PURE - it validates card plays against game rules.
/// Returns true if the play is valid, false if it violates follow-suit.
pub fn validate_follow_suit_rule(card: &str, lead_suit: &str, player_hand: &[String]) -> bool {
    let card_suit = if card.len() >= 2 {
        &card[1..2]
    } else {
        return false;
    };

    // If playing the lead suit, it's always valid
    if card_suit == lead_suit {
        return true;
    }

    // If not playing the lead suit, check if player has any cards of lead suit
    let has_lead_suit = player_hand.iter().any(|h| {
        if h.len() >= 2 {
            &h[1..2] == lead_suit
        } else {
            false
        }
    });

    // If player has lead suit cards, they must follow suit
    !has_lead_suit
}

/// Validate a card play for a specific player
///
/// This function validates that a play is legal according to game rules:
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

    // Get or create the current trick
    let current_trick = match round_tricks::Entity::find()
        .filter(round_tricks::Column::RoundId.eq(current_round.id))
        .order_by_desc(round_tricks::Column::TrickNumber)
        .one(txn)
        .await
    {
        Ok(Some(trick)) => trick,
        Ok(None) => {
            // Create a new trick if none exists
            let trick_id = Uuid::new_v4();
            let new_trick = round_tricks::ActiveModel {
                id: Set(trick_id),
                round_id: Set(current_round.id),
                trick_number: Set(1),
                winner_player_id: Set(None),
                created_at: Set(chrono::Utc::now().into()),
            };

            match new_trick.insert(txn).await {
                Ok(trick) => trick,
                Err(e) => {
                    return Err(format!("Failed to create new trick: {e}"));
                }
            }
        }
        Err(e) => {
            return Err(format!("Failed to fetch current trick: {e}"));
        }
    };

    // Get existing trick plays to determine play order
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

    // Record the card play
    let play_id = Uuid::new_v4();
    let play_order = trick_plays.len() as i32;
    let trick_play = trick_plays::ActiveModel {
        id: Set(play_id),
        trick_id: Set(current_trick.id),
        player_id: Set(current_player.id),
        card: Set(card.to_string()),
        play_order: Set(play_order),
    };

    match trick_play.insert(txn).await {
        Ok(_) => (),
        Err(e) => {
            return Err(format!("Failed to record card play: {e}"));
        }
    }

    // Remove the card from the player's hand
    let card_to_remove = match round_hands::Entity::find()
        .filter(round_hands::Column::RoundId.eq(current_round.id))
        .filter(round_hands::Column::PlayerId.eq(current_player.id))
        .filter(round_hands::Column::Card.eq(card))
        .one(txn)
        .await
    {
        Ok(Some(hand_card)) => hand_card,
        Ok(None) => {
            return Err("Card not found in hand".to_string());
        }
        Err(e) => {
            return Err(format!("Failed to find card in hand: {e}"));
        }
    };

    match round_hands::Entity::delete_by_id(card_to_remove.id)
        .exec(txn)
        .await
    {
        Ok(_) => (),
        Err(e) => {
            return Err(format!("Failed to remove card from hand: {e}"));
        }
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

    let all_played = trick_plays.len() == all_players.len();

    if all_played {
        // Determine the winner of the trick
        let mut winning_player_id = None;
        let mut highest_value = -1;

        for play in &trick_plays {
            let card_value = get_card_rank_value(&play.card[0..1]);
            if card_value > highest_value {
                highest_value = card_value;
                winning_player_id = Some(play.player_id);
            }
        }

        // Update the trick with the winner
        let trick_update = round_tricks::ActiveModel {
            id: Set(current_trick.id),
            round_id: Set(current_trick.round_id),
            trick_number: Set(current_trick.trick_number),
            winner_player_id: Set(winning_player_id),
            created_at: Set(current_trick.created_at),
        };

        match trick_update.update(txn).await {
            Ok(_) => (),
            Err(e) => {
                return Err(format!("Failed to update trick winner: {e}"));
            }
        }

        // Check if this was the last trick of the round
        let cards_per_player = current_round.cards_dealt;
        let total_tricks = cards_per_player;
        let current_trick_number = current_trick.trick_number;

        if current_trick_number == total_tricks {
            // Round is complete, transition to scoring phase
            let game_update = games::ActiveModel {
                id: Set(game.id),
                state: Set(game.state),
                phase: Set(games::GamePhase::Scoring),
                current_turn: Set(None),
                created_at: Set(game.created_at),
                updated_at: Set(chrono::Utc::now().into()),
                started_at: Set(game.started_at),
                completed_at: Set(game.completed_at),
            };

            match game_update.update(txn).await {
                Ok(_) => (),
                Err(e) => {
                    return Err(format!("Failed to transition to scoring phase: {e}"));
                }
            }
        } else {
            // Move to next player's turn (the winner of the trick)
            let next_turn = match game_players::Entity::find()
                .filter(game_players::Column::GameId.eq(game_id))
                .filter(game_players::Column::Id.eq(winning_player_id.unwrap()))
                .one(txn)
                .await
            {
                Ok(Some(player)) => player.turn_order.unwrap_or(0),
                Ok(None) => 0,
                Err(_) => 0,
            };

            let game_update = games::ActiveModel {
                id: Set(game.id),
                state: Set(game.state),
                phase: Set(game.phase),
                current_turn: Set(Some(next_turn)),
                created_at: Set(game.created_at),
                updated_at: Set(chrono::Utc::now().into()),
                started_at: Set(game.started_at),
                completed_at: Set(game.completed_at),
            };

            match game_update.update(txn).await {
                Ok(_) => (),
                Err(e) => {
                    return Err(format!("Failed to update turn: {e}"));
                }
            }
        }
    } else {
        // Move to next player's turn
        let current_turn = game.current_turn.unwrap_or(0);
        let next_turn = (current_turn + 1) % all_players.len() as i32;

        let game_update = games::ActiveModel {
            id: Set(game.id),
            state: Set(game.state),
            phase: Set(game.phase),
            current_turn: Set(Some(next_turn)),
            created_at: Set(game.created_at),
            updated_at: Set(chrono::Utc::now().into()),
            started_at: Set(game.started_at),
            completed_at: Set(game.completed_at),
        };

        match game_update.update(txn).await {
            Ok(_) => (),
            Err(e) => {
                return Err(format!("Failed to update turn: {e}"));
            }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_determine_trick_winner_highest_lead_suit() {
        let player1 = uuid::Uuid::new_v4();
        let player2 = uuid::Uuid::new_v4();
        let player3 = uuid::Uuid::new_v4();
        let player4 = uuid::Uuid::new_v4();

        let plays = vec![
            ("7H".to_string(), player1), // 7 of hearts
            ("KH".to_string(), player2), // King of hearts
            ("2H".to_string(), player3), // 2 of hearts
            ("9H".to_string(), player4), // 9 of hearts
        ];
        let trump_suit = None;

        let winner = determine_trick_winner(&plays, &trump_suit).unwrap();
        assert_eq!(winner, player2); // King of hearts should win
    }

    #[test]
    fn test_determine_trick_winner_trump_beats_lead() {
        let player1 = uuid::Uuid::new_v4();
        let player2 = uuid::Uuid::new_v4();
        let player3 = uuid::Uuid::new_v4();
        let player4 = uuid::Uuid::new_v4();

        let plays = vec![
            ("AH".to_string(), player1), // Ace of hearts
            ("2S".to_string(), player2), // 2 of spades (trump)
            ("7H".to_string(), player3), // 7 of hearts
            ("KS".to_string(), player4), // King of spades (trump)
        ];
        let trump_suit = Some("S".to_string());

        let winner = determine_trick_winner(&plays, &trump_suit).unwrap();
        assert_eq!(winner, player4); // King of spades should win (highest trump)
    }

    #[test]
    fn test_determine_trick_winner_empty_plays() {
        let plays: Vec<(String, uuid::Uuid)> = vec![];
        let trump_suit = None;

        let result = determine_trick_winner(&plays, &trump_suit);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "No plays found for trick");
    }

    #[test]
    fn test_can_follow_suit() {
        let hand = vec!["AS".to_string(), "KH".to_string(), "2C".to_string()];

        assert!(can_follow_suit(&hand, "S"));
        assert!(can_follow_suit(&hand, "H"));
        assert!(can_follow_suit(&hand, "C"));
        assert!(!can_follow_suit(&hand, "D"));
    }

    #[test]
    fn test_get_next_trick_turn() {
        assert_eq!(get_next_trick_turn(0), 1);
        assert_eq!(get_next_trick_turn(1), 2);
        assert_eq!(get_next_trick_turn(2), 3);
        assert_eq!(get_next_trick_turn(3), 0); // Wraparound
    }

    #[test]
    fn test_is_trick_complete() {
        assert!(is_trick_complete(4, 4));
        assert!(is_trick_complete(5, 4)); // More plays than players
        assert!(!is_trick_complete(3, 4));
        assert!(!is_trick_complete(0, 4));
    }

    #[test]
    fn test_get_lead_suit_from_trick() {
        let plays = vec![
            ("7H".to_string(), uuid::Uuid::new_v4()),
            ("KH".to_string(), uuid::Uuid::new_v4()),
        ];

        let lead_suit = get_lead_suit_from_trick(&plays);
        assert_eq!(lead_suit, Some("H".to_string()));
    }

    #[test]
    fn test_get_lead_suit_from_empty_trick() {
        let plays: Vec<(String, uuid::Uuid)> = vec![];
        let lead_suit = get_lead_suit_from_trick(&plays);
        assert_eq!(lead_suit, None);
    }

    #[test]
    fn test_validate_follow_suit_rule() {
        let hand = vec!["AS".to_string(), "KH".to_string(), "2C".to_string()];

        // Playing lead suit is always valid
        assert!(validate_follow_suit_rule("AS", "S", &hand));

        // Playing different suit when no lead suit cards available is valid
        assert!(validate_follow_suit_rule("2C", "D", &hand));

        // Playing different suit when lead suit cards available is invalid
        assert!(!validate_follow_suit_rule("2C", "S", &hand));
    }
}
