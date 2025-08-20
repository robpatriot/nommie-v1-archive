//! Game rules module
//!
//! This module contains pure game rules, validation logic,
//! and rule enforcement mechanisms that depend only on
//! in-memory domain types and std.

/// Total number of rounds in a game
pub const TOTAL_ROUNDS: i32 = 26;

/// Number of players in a game
pub const PLAYER_COUNT: usize = 4;

/// Number of rounds with 2 cards (rounds 12-15)
pub const TWO_CARD_ROUNDS: i32 = 4;

/// Round number where 2-card rounds start
pub const TWO_CARD_ROUNDS_START: i32 = 12;

/// Round number where 2-card rounds end
pub const TWO_CARD_ROUNDS_END: i32 = 15;

/// Round number where card count starts increasing again
pub const INCREASING_CARDS_START: i32 = 16;

/// Maximum cards dealt in a round
pub const MAX_CARDS_PER_ROUND: i32 = 13;

/// Minimum cards dealt in a round
pub const MIN_CARDS_PER_ROUND: i32 = 2;

/// Valid card ranks in order from lowest to highest
pub const VALID_RANKS: [&str; 13] = [
    "2", "3", "4", "5", "6", "7", "8", "9", "T", "J", "Q", "K", "A",
];

/// Valid card suits
pub const VALID_SUITS: [&str; 4] = ["S", "H", "D", "C"];

/// Card rank values for comparison (2=2, 3=3, ..., A=14)
pub const CARD_RANK_VALUES: [i32; 13] = [2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14];

/// Calculate the number of cards to deal for a given round number
///
/// Round sequence: 13 → 12 → 11 → ... → 3 → 2 → 2 → 2 → 2 → 3 → 4 → ... → 13
/// - Rounds 1-11: 13 cards down to 3 cards (14 - round_number)
/// - Rounds 12-15: 4 rounds of 2 cards
/// - Rounds 16-26: 3 cards up to 13 cards (round_number - 15 + 2)
pub fn calculate_cards_dealt(round_number: i32) -> i32 {
    if round_number <= 11 {
        // Rounds 1-11: 13 cards down to 3 cards
        14 - round_number
    } else if round_number <= 15 {
        // Rounds 12-15: 4 rounds of 2 cards
        2
    } else {
        // Rounds 16-26: 3 cards up to 13 cards
        round_number - 15 + 2
    }
}

/// Get the next round number in sequence
/// Returns None if the game is complete (after round 26)
pub fn get_next_round_number(current_round: i32) -> Option<i32> {
    if current_round >= TOTAL_ROUNDS {
        None
    } else {
        Some(current_round + 1)
    }
}

/// Get the previous round number in sequence
/// Returns None if this is the first round
pub fn get_previous_round_number(current_round: i32) -> Option<i32> {
    if current_round <= 1 {
        None
    } else {
        Some(current_round - 1)
    }
}

/// Check if a round number is valid (1-26)
pub fn is_valid_round_number(round_number: i32) -> bool {
    (1..=TOTAL_ROUNDS).contains(&round_number)
}

/// Check if a round is a 2-card round
pub fn is_two_card_round(round_number: i32) -> bool {
    (TWO_CARD_ROUNDS_START..=TWO_CARD_ROUNDS_END).contains(&round_number)
}

/// Get the next player index in turn order (with wraparound)
pub fn get_next_player_index(current_player: usize) -> usize {
    (current_player + 1) % PLAYER_COUNT
}

/// Get the previous player index in turn order (with wraparound)
pub fn get_previous_player_index(current_player: usize) -> usize {
    if current_player == 0 {
        PLAYER_COUNT - 1
    } else {
        current_player - 1
    }
}

/// Get the next dealer index (with wraparound)
pub fn get_next_dealer_index(current_dealer: usize) -> usize {
    (current_dealer + 1) % PLAYER_COUNT
}

/// Get the dealer index for a given round number
/// Dealer rotates each round, starting with player 0
pub fn get_dealer_index_for_round(round_number: i32) -> usize {
    ((round_number - 1) % PLAYER_COUNT as i32) as usize
}

/// Get the canonical player index from a turn order value
/// This converts database turn_order to 0-based player index
pub fn canonical_player_index(turn_order: i32) -> usize {
    (turn_order % PLAYER_COUNT as i32) as usize
}

/// Get the turn order value from a canonical player index
/// This converts 0-based player index to database turn_order
pub fn turn_order_from_index(player_index: usize) -> i32 {
    player_index as i32
}

/// Validate card format (e.g., "AS", "KH", "2C")
/// Card must be exactly 2 characters: rank + suit
pub fn is_valid_card_format(card: &str) -> bool {
    if card.len() != 2 {
        return false;
    }

    let rank = &card[0..1];
    let suit = &card[1..2];

    VALID_RANKS.contains(&rank) && VALID_SUITS.contains(&suit)
}

/// Get card rank value for comparison (2=2, 3=3, ..., A=14)
pub fn get_card_rank_value(rank: &str) -> i32 {
    match rank {
        "2" => 2,
        "3" => 3,
        "4" => 4,
        "5" => 5,
        "6" => 6,
        "7" => 7,
        "8" => 8,
        "9" => 9,
        "T" => 10,
        "J" => 11,
        "Q" => 12,
        "K" => 13,
        "A" => 14,
        _ => 0,
    }
}

/// Check if a suit is the trump suit
pub fn is_trump_suit(suit: &str, trump_suit: &Option<String>) -> bool {
    trump_suit.as_ref().is_some_and(|trump| suit == trump)
}

/// Get the lead suit from a card
pub fn get_card_suit(card: &str) -> Option<&str> {
    if card.len() >= 2 {
        Some(&card[1..2])
    } else {
        None
    }
}

/// Get the rank from a card
pub fn get_card_rank(card: &str) -> Option<&str> {
    if !card.is_empty() {
        Some(&card[0..1])
    } else {
        None
    }
}

/// Check if a card follows suit (matches the lead suit)
pub fn follows_suit(card: &str, lead_suit: &str) -> bool {
    get_card_suit(card) == Some(lead_suit)
}

/// Check if a card is trump
pub fn is_trump_card(card: &str, trump_suit: &Option<String>) -> bool {
    get_card_suit(card).is_some_and(|suit| is_trump_suit(suit, trump_suit))
}

/// Compare two cards to determine which wins
/// Returns:
/// - Some(Ordering::Greater) if card1 wins
/// - Some(Ordering::Less) if card2 wins  
/// - Some(Ordering::Equal) if cards are equal
/// - None if cards cannot be compared (different suits, no trump)
pub fn compare_cards(
    card1: &str,
    card2: &str,
    lead_suit: &str,
    trump_suit: &Option<String>,
) -> Option<std::cmp::Ordering> {
    let suit1 = get_card_suit(card1)?;
    let suit2 = get_card_suit(card2)?;
    let rank1 = get_card_rank(card1)?;
    let rank2 = get_card_rank(card2)?;

    let is_trump1 = is_trump_suit(suit1, trump_suit);
    let is_trump2 = is_trump_suit(suit2, trump_suit);
    let follows_lead1 = suit1 == lead_suit;
    let follows_lead2 = suit2 == lead_suit;

    // Trump beats non-trump
    if is_trump1 && !is_trump2 {
        return Some(std::cmp::Ordering::Greater);
    }
    if !is_trump1 && is_trump2 {
        return Some(std::cmp::Ordering::Less);
    }

    // If both are trump or both follow lead, compare ranks
    if (is_trump1 && is_trump2) || (follows_lead1 && follows_lead2) {
        let value1 = get_card_rank_value(rank1);
        let value2 = get_card_rank_value(rank2);
        return Some(value1.cmp(&value2));
    }

    // Cards cannot be compared (different non-trump suits)
    None
}

/// Get the round sequence as a vector for testing/validation
pub fn get_round_sequence() -> Vec<i32> {
    (1..=TOTAL_ROUNDS).collect()
}

/// Get the card counts for each round as a vector for testing/validation
pub fn get_round_card_counts() -> Vec<i32> {
    (1..=TOTAL_ROUNDS).map(calculate_cards_dealt).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_round_sequence_length() {
        assert_eq!(get_round_sequence().len(), TOTAL_ROUNDS as usize);
    }

    #[test]
    fn test_round_sequence_bounds() {
        let sequence = get_round_sequence();
        assert_eq!(sequence.first(), Some(&1));
        assert_eq!(sequence.last(), Some(&TOTAL_ROUNDS));
    }

    #[test]
    fn test_round_sequence_strictly_decreasing_then_increasing() {
        let card_counts = get_round_card_counts();

        // Rounds 1-11: strictly decreasing (13 → 12 → 11 → ... → 3)
        for i in 0..10 {
            assert!(card_counts[i] > card_counts[i + 1]);
        }

        // Rounds 12-15: all 2 cards
        for count in card_counts.iter().take(15).skip(11) {
            assert_eq!(*count, 2);
        }

        // Rounds 16-26: strictly increasing (3 → 4 → ... → 13)
        for i in 15..25 {
            assert!(card_counts[i] < card_counts[i + 1]);
        }
    }

    #[test]
    fn test_exactly_four_two_card_rounds() {
        let two_card_count = get_round_card_counts()
            .iter()
            .filter(|&&count| count == 2)
            .count();
        assert_eq!(two_card_count, TWO_CARD_ROUNDS as usize);
    }

    #[test]
    fn test_round_sequence_starts_and_ends_with_13() {
        let card_counts = get_round_card_counts();
        assert_eq!(card_counts[0], MAX_CARDS_PER_ROUND); // Round 1: 13 cards
        assert_eq!(card_counts[25], MAX_CARDS_PER_ROUND); // Round 26: 13 cards
    }

    #[test]
    fn test_turn_order_wraparound() {
        assert_eq!(get_next_player_index(0), 1);
        assert_eq!(get_next_player_index(1), 2);
        assert_eq!(get_next_player_index(2), 3);
        assert_eq!(get_next_player_index(3), 0); // Wraparound

        assert_eq!(get_previous_player_index(0), 3); // Wraparound
        assert_eq!(get_previous_player_index(1), 0);
        assert_eq!(get_previous_player_index(2), 1);
        assert_eq!(get_previous_player_index(3), 2);
    }

    #[test]
    fn test_dealer_rotation() {
        assert_eq!(get_dealer_index_for_round(1), 0);
        assert_eq!(get_dealer_index_for_round(2), 1);
        assert_eq!(get_dealer_index_for_round(3), 2);
        assert_eq!(get_dealer_index_for_round(4), 3);
        assert_eq!(get_dealer_index_for_round(5), 0); // Wraparound
        assert_eq!(get_dealer_index_for_round(26), 1); // Last round
    }

    #[test]
    fn test_canonical_player_index() {
        assert_eq!(canonical_player_index(0), 0);
        assert_eq!(canonical_player_index(1), 1);
        assert_eq!(canonical_player_index(2), 2);
        assert_eq!(canonical_player_index(3), 3);
        assert_eq!(canonical_player_index(4), 0); // Wraparound
        assert_eq!(canonical_player_index(7), 3); // Wraparound
    }

    #[test]
    fn test_turn_order_from_index() {
        assert_eq!(turn_order_from_index(0), 0);
        assert_eq!(turn_order_from_index(1), 1);
        assert_eq!(turn_order_from_index(2), 2);
        assert_eq!(turn_order_from_index(3), 3);
    }

    #[test]
    fn test_card_validation() {
        assert!(is_valid_card_format("AS"));
        assert!(is_valid_card_format("KH"));
        assert!(is_valid_card_format("2C"));
        assert!(is_valid_card_format("TD"));

        assert!(!is_valid_card_format("A")); // Too short
        assert!(!is_valid_card_format("ASS")); // Too long
        assert!(!is_valid_card_format("1S")); // Invalid rank
        assert!(!is_valid_card_format("AX")); // Invalid suit
    }

    #[test]
    fn test_card_rank_values() {
        assert_eq!(get_card_rank_value("2"), 2);
        assert_eq!(get_card_rank_value("9"), 9);
        assert_eq!(get_card_rank_value("T"), 10);
        assert_eq!(get_card_rank_value("J"), 11);
        assert_eq!(get_card_rank_value("Q"), 12);
        assert_eq!(get_card_rank_value("K"), 13);
        assert_eq!(get_card_rank_value("A"), 14);
        assert_eq!(get_card_rank_value("X"), 0); // Invalid rank
    }

    #[test]
    fn test_trump_suit_checking() {
        assert!(is_trump_suit("H", &Some("H".to_string())));
        assert!(!is_trump_suit("S", &Some("H".to_string())));
        assert!(!is_trump_suit("H", &None));
    }

    #[test]
    fn test_card_suit_and_rank_extraction() {
        assert_eq!(get_card_suit("AS"), Some("S"));
        assert_eq!(get_card_rank("AS"), Some("A"));
        assert_eq!(get_card_suit(""), None);
        assert_eq!(get_card_rank(""), None);
    }

    #[test]
    fn test_follows_suit() {
        assert!(follows_suit("AS", "S"));
        assert!(!follows_suit("AH", "S"));
    }

    #[test]
    fn test_is_trump_card() {
        assert!(is_trump_card("AH", &Some("H".to_string())));
        assert!(!is_trump_card("AS", &Some("H".to_string())));
        assert!(!is_trump_card("AH", &None));
    }

    #[test]
    fn test_compare_cards() {
        // Same suit, different ranks
        assert_eq!(
            compare_cards("AS", "KS", "S", &None),
            Some(std::cmp::Ordering::Greater)
        );
        assert_eq!(
            compare_cards("KS", "AS", "S", &None),
            Some(std::cmp::Ordering::Less)
        );

        // Trump vs non-trump
        assert_eq!(
            compare_cards("AH", "AS", "S", &Some("H".to_string())),
            Some(std::cmp::Ordering::Greater)
        );

        // Different non-trump suits (cannot compare)
        assert_eq!(compare_cards("AS", "KH", "S", &None), None);
    }

    #[test]
    fn test_round_number_validation() {
        assert!(is_valid_round_number(1));
        assert!(is_valid_round_number(13));
        assert!(is_valid_round_number(26));
        assert!(!is_valid_round_number(0));
        assert!(!is_valid_round_number(27));
    }

    #[test]
    fn test_two_card_rounds() {
        assert!(!is_two_card_round(11));
        assert!(is_two_card_round(12));
        assert!(is_two_card_round(13));
        assert!(is_two_card_round(14));
        assert!(is_two_card_round(15));
        assert!(!is_two_card_round(16));
    }

    #[test]
    fn test_next_round_number() {
        assert_eq!(get_next_round_number(1), Some(2));
        assert_eq!(get_next_round_number(25), Some(26));
        assert_eq!(get_next_round_number(26), None);
    }

    #[test]
    fn test_previous_round_number() {
        assert_eq!(get_previous_round_number(2), Some(1));
        assert_eq!(get_previous_round_number(26), Some(25));
        assert_eq!(get_previous_round_number(1), None);
    }
}
