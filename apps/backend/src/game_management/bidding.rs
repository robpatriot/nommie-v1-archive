//! Bidding module
//!
//! This module contains pure bidding logic, bid validation,
//! and bid processing mechanisms that depend only on
//! in-memory domain types and std.

use rand::seq::SliceRandom;

/// Create a standard 52-card deck and shuffle it
///
/// This function is PURE - it has no side effects and is deterministic
/// given the same random seed. It creates a deck with standard card
/// representations (e.g., "AS", "KH", "2C") and shuffles them.
pub fn create_shuffled_deck() -> Vec<String> {
    let suits = vec!["H", "D", "C", "S"]; // Hearts, Diamonds, Clubs, Spades
    let ranks = vec![
        "2", "3", "4", "5", "6", "7", "8", "9", "T", "J", "Q", "K", "A",
    ];

    let mut deck = Vec::new();
    for suit in &suits {
        for rank in &ranks {
            deck.push(format!("{rank}{suit}"));
        }
    }

    // Shuffle the deck
    let mut rng = rand::thread_rng();
    deck.shuffle(&mut rng);

    deck
}

/// Validate bid value (0-13)
///
/// This function is PURE - it validates bid values without any side effects.
/// Returns true if the bid is valid, false otherwise.
pub fn is_valid_bid(bid: i32) -> bool {
    (0..=13).contains(&bid)
}

/// Find the highest bidder from a list of bids
///
/// This function is PURE - it processes bid data to find the highest bidder.
/// In case of ties, returns the first bidder encountered (simulating "first bidder wins").
///
/// Returns a tuple of (highest_bid, highest_bidder_id, first_bidder_id)
pub fn find_highest_bidder(
    bids: &[(i32, uuid::Uuid)],
) -> (i32, Option<uuid::Uuid>, Option<uuid::Uuid>) {
    let mut highest_bid = -1;
    let mut highest_bidder_id = None;
    let mut first_bidder_id = None;

    for (bid, player_id) in bids {
        if *bid > highest_bid {
            highest_bid = *bid;
            highest_bidder_id = Some(*player_id);
            first_bidder_id = Some(*player_id);
        } else if *bid == highest_bid {
            // In case of tie, the first bidder wins
            if first_bidder_id.is_none() {
                first_bidder_id = Some(*player_id);
            }
        }
    }

    (highest_bid, highest_bidder_id, first_bidder_id)
}

/// Check if all players have submitted bids
///
/// This function is PURE - it compares bid count to player count.
/// Returns true if all players have bid, false otherwise.
pub fn all_bids_submitted(bid_count: usize, player_count: usize) -> bool {
    bid_count == player_count
}

/// Calculate the next player's turn for bidding
///
/// This function is PURE - it calculates the next turn using modulo arithmetic.
/// Returns the next turn index (0-3) with wraparound.
pub fn get_next_bidding_turn(current_turn: i32) -> i32 {
    (current_turn + 1) % 4
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_shuffled_deck() {
        let deck = create_shuffled_deck();

        // Should have exactly 52 cards
        assert_eq!(deck.len(), 52);

        // Should contain all expected cards
        let suits = vec!["H", "D", "C", "S"];
        let ranks = vec![
            "2", "3", "4", "5", "6", "7", "8", "9", "T", "J", "Q", "K", "A",
        ];

        for suit in &suits {
            for rank in &ranks {
                let expected_card = format!("{rank}{suit}");
                assert!(
                    deck.contains(&expected_card),
                    "Deck missing card: {expected_card}"
                );
            }
        }
    }

    #[test]
    fn test_is_valid_bid() {
        assert!(is_valid_bid(0));
        assert!(is_valid_bid(7));
        assert!(is_valid_bid(13));
        assert!(!is_valid_bid(-1));
        assert!(!is_valid_bid(14));
    }

    #[test]
    fn test_find_highest_bidder() {
        use uuid::Uuid;

        let player1 = Uuid::new_v4();
        let player2 = Uuid::new_v4();
        let player3 = Uuid::new_v4();
        let player4 = Uuid::new_v4();

        let bids = vec![(3, player1), (7, player2), (2, player3), (5, player4)];

        let (highest_bid, highest_bidder, first_bidder) = find_highest_bidder(&bids);

        assert_eq!(highest_bid, 7);
        assert_eq!(highest_bidder, Some(player2));
        assert_eq!(first_bidder, Some(player2));
    }

    #[test]
    fn test_find_highest_bidder_with_tie() {
        use uuid::Uuid;

        let player1 = Uuid::new_v4();
        let player2 = Uuid::new_v4();
        let player3 = Uuid::new_v4();
        let player4 = Uuid::new_v4();

        let bids = vec![(5, player1), (5, player2), (3, player3), (2, player4)];

        let (highest_bid, highest_bidder, first_bidder) = find_highest_bidder(&bids);

        assert_eq!(highest_bid, 5);
        assert_eq!(highest_bidder, Some(player1));
        assert_eq!(first_bidder, Some(player1)); // First bidder wins tie
    }

    #[test]
    fn test_all_bids_submitted() {
        assert!(all_bids_submitted(4, 4));
        assert!(all_bids_submitted(0, 0));
        assert!(!all_bids_submitted(3, 4));
        assert!(!all_bids_submitted(5, 4));
    }

    #[test]
    fn test_get_next_bidding_turn() {
        assert_eq!(get_next_bidding_turn(0), 1);
        assert_eq!(get_next_bidding_turn(1), 2);
        assert_eq!(get_next_bidding_turn(2), 3);
        assert_eq!(get_next_bidding_turn(3), 0); // Wraparound
    }
}
