//! Tricks module
//!
//! This module contains trick playing logic, card comparison,
//! and trick completion mechanisms that depend only on
//! in-memory domain types and std.

use crate::game_management::rules::get_card_rank_value;
use uuid::Uuid;

// Pure domain types for trick logic
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub(crate) struct TrickState {
    plays: Vec<(String, Uuid)>, // (card, player_id) tuples
}

/// Result of pure trick advancement logic
///
/// This type contains all the information needed to advance a trick
/// after a card play, without any database dependencies.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub(crate) struct TrickAdvancement {
    /// Whether the trick is complete (all players have played)
    pub trick_complete: bool,
    /// The winner of the trick (if complete)
    pub winner_user_id: Option<Uuid>,
    /// The next player to lead (if trick complete)
    pub next_leader_user_id: Option<Uuid>,
    /// Whether the round is complete and should advance to scoring
    pub round_complete: bool,
    /// The next turn index for the game
    pub next_turn: i32,
}

#[derive(Debug)]
#[allow(dead_code)]
pub(crate) struct ApplyPlayOutcome {
    card_to_remove: String,
}

#[derive(Debug)]
#[allow(dead_code)]
pub(crate) enum ApplyPlayError {
    InvalidCardFormat,
    CardNotInHand,
    FollowSuitViolation,
}

/// Apply a single play to the current trick state (pure logic, no DB I/O)
///
/// This function is PURE - it applies a card play to the current trick state
/// without any side effects or database operations. It validates the play
/// against game rules and returns the card that should be removed from the player's hand.
///
/// # Arguments
/// * `card` - The card being played
/// * `current_trick` - The current state of the trick
/// * `player_hand` - The player's current hand
///
/// # Returns
/// * `Ok(ApplyPlayOutcome)` - The card to remove from hand
/// * `Err(ApplyPlayError)` - Validation error if the play is invalid
#[allow(dead_code)]
pub(crate) fn apply_play_logic(
    card: &str,
    current_trick: &TrickState,
    player_hand: &[String],
) -> Result<ApplyPlayOutcome, ApplyPlayError> {
    // Validate card format
    if card.len() < 2 {
        return Err(ApplyPlayError::InvalidCardFormat);
    }

    // Check if card is in hand
    if !player_hand.contains(&card.to_string()) {
        return Err(ApplyPlayError::CardNotInHand);
    }

    // Validate follow-suit rule if this isn't the first play
    if !current_trick.plays.is_empty() {
        let lead_suit = get_lead_suit_from_trick(&current_trick.plays).unwrap();
        let card_suit = &card[1..2];

        if card_suit != lead_suit && can_follow_suit(player_hand, &lead_suit) {
            return Err(ApplyPlayError::FollowSuitViolation);
        }
    }

    Ok(ApplyPlayOutcome {
        card_to_remove: card.to_string(),
    })
}

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
pub fn get_lead_suit_from_trick(plays: &[(String, Uuid)]) -> Option<String> {
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

/// Determine trick advancement after a card play (pure logic, no DB I/O)
///
/// This function is PURE - it determines all advancement outcomes based on
/// the current trick state, without any side effects or database operations.
/// It reuses existing pure helpers to determine winners, completion status,
/// and next player turns.
///
/// # Arguments
/// * `trick_plays` - The current plays in the trick (card, player_id) tuples
/// * `player_count` - Total number of players in the game
/// * `current_turn` - Current turn index in the game
/// * `trick_number` - Current trick number in the round
/// * `cards_per_player` - Number of cards dealt per player this round
/// * `trump_suit` - The trump suit for this round (if any)
///
/// # Returns
/// * `TrickAdvancement` - Complete advancement information for the caller to persist
pub(crate) fn advance_trick_logic(
    trick_plays: &[(String, Uuid)],
    player_count: usize,
    current_turn: i32,
    trick_number: i32,
    cards_per_player: i32,
    trump_suit: &Option<String>,
) -> TrickAdvancement {
    let trick_complete = is_trick_complete(trick_plays.len(), player_count);

    if trick_complete {
        // Determine the winner of the trick
        let winner_user_id = determine_trick_winner(trick_plays, trump_suit).ok();

        // Check if this was the last trick of the round
        let total_tricks = cards_per_player;
        let round_complete = trick_number == total_tricks;

        if round_complete {
            // Round is complete, next turn will be 0 (first player)
            TrickAdvancement {
                trick_complete: true,
                winner_user_id,
                next_leader_user_id: None, // Not relevant for round completion
                round_complete: true,
                next_turn: 0,
            }
        } else {
            // Move to next player's turn (the winner of the trick)
            // Note: We need to find the winner's turn order, but this function is pure
            // so we'll return the winner's ID and let the caller handle the mapping
            TrickAdvancement {
                trick_complete: true,
                winner_user_id,
                next_leader_user_id: winner_user_id,
                round_complete: false,
                next_turn: -1, // Caller needs to map winner_user_id to turn order
            }
        }
    } else {
        // Move to next player's turn
        let next_turn = get_next_trick_turn(current_turn);

        TrickAdvancement {
            trick_complete: false,
            winner_user_id: None,
            next_leader_user_id: None,
            round_complete: false,
            next_turn,
        }
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
    fn test_advance_trick_logic_trick_not_complete() {
        let plays = vec![
            ("7H".to_string(), uuid::Uuid::new_v4()),
            ("KH".to_string(), uuid::Uuid::new_v4()),
        ];

        let advancement = advance_trick_logic(
            &plays, 4,     // 4 players
            1,     // Current turn (player 2)
            1,     // Trick 1
            13,    // 13 cards per player
            &None, // No trump
        );

        assert!(!advancement.trick_complete);
        assert_eq!(advancement.next_turn, 2); // Next player's turn
        assert!(!advancement.round_complete);
    }

    #[test]
    fn test_advance_trick_logic_trick_complete_not_round_end() {
        let plays = vec![
            ("7H".to_string(), uuid::Uuid::new_v4()),
            ("KH".to_string(), uuid::Uuid::new_v4()),
            ("2H".to_string(), uuid::Uuid::new_v4()),
            ("9H".to_string(), uuid::Uuid::new_v4()),
        ];

        let advancement = advance_trick_logic(
            &plays, 4,     // 4 players
            1,     // Current turn (player 2)
            1,     // Trick 1
            13,    // 13 cards per player
            &None, // No trump
        );

        assert!(advancement.trick_complete);
        assert!(!advancement.round_complete);
        assert_eq!(advancement.next_turn, -1); // Caller needs to map winner to turn
    }

    #[test]
    fn test_advance_trick_logic_round_complete() {
        let plays = vec![
            ("7H".to_string(), uuid::Uuid::new_v4()),
            ("KH".to_string(), uuid::Uuid::new_v4()),
            ("2H".to_string(), uuid::Uuid::new_v4()),
            ("9H".to_string(), uuid::Uuid::new_v4()),
        ];

        let advancement = advance_trick_logic(
            &plays, 4,     // 4 players
            1,     // Current turn (player 2)
            13,    // Trick 13 (last trick of round)
            13,    // 13 cards per player
            &None, // No trump
        );

        assert!(advancement.trick_complete);
        assert!(advancement.round_complete);
        assert_eq!(advancement.next_turn, 0); // Reset to first player
    }

    #[test]
    fn test_advance_trick_logic_with_trump() {
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

        let advancement = advance_trick_logic(
            &plays,
            4,                      // 4 players
            1,                      // Current turn (player 2)
            1,                      // Trick 1
            13,                     // 13 cards per player
            &Some("S".to_string()), // Spades is trump
        );

        assert!(advancement.trick_complete);
        assert_eq!(advancement.winner_user_id, Some(player4)); // King of spades should win (highest trump)
        assert!(!advancement.round_complete);
    }
}
