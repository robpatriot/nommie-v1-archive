//! Tricks module
//!
//! This module contains pure trick playing logic, card comparison,
//! and trick completion mechanisms that depend only on
//! in-memory domain types and std.

use crate::game_management::rules::get_card_rank_value;

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
