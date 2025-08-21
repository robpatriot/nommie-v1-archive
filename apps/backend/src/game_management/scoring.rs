//! Scoring module
//!
//! This module contains pure scoring calculation logic and point calculation
//! helpers for the Nommie card game.

/// Calculate points for a round based on tricks won and bid
///
/// Points calculation:
/// - 1 point per trick won
/// - 10 point bonus if bid exactly matches tricks won
///
/// # Arguments
/// * `tricks_won` - Number of tricks won by the player
/// * `bid` - Number of tricks the player bid
///
/// # Returns
/// * Total points for the round
pub fn calculate_round_points(tricks_won: i32, bid: i32) -> i32 {
    let base_points = tricks_won;
    let bonus = if tricks_won == bid { 10 } else { 0 };
    base_points + bonus
}

/// Calculate total score for a player across multiple rounds
///
/// This is a pure calculation function that takes pre-fetched data
/// and returns the calculated total score without any database operations.
///
/// # Arguments
/// * `round_data` - Vector of tuples containing (tricks_won, bid) for each round
///
/// # Returns
/// * Total calculated score
pub fn calculate_total_score_from_rounds(round_data: &[(i32, i32)]) -> i32 {
    round_data
        .iter()
        .map(|(tricks_won, bid)| calculate_round_points(*tricks_won, *bid))
        .sum()
}

/// Determine if a player gets a bonus for exact bid
///
/// # Arguments
/// * `tricks_won` - Number of tricks won by the player
/// * `bid` - Number of tricks the player bid
///
/// # Returns
/// * True if the player gets the 10-point bonus
pub fn has_exact_bid_bonus(tricks_won: i32, bid: i32) -> bool {
    tricks_won == bid
}

/// Calculate the bonus amount for exact bid
///
/// # Arguments
/// * `tricks_won` - Number of tricks won by the player
/// * `bid` - Number of tricks the player bid
///
/// # Returns
/// * Bonus amount (10 if exact match, 0 otherwise)
pub fn calculate_bonus_amount(tricks_won: i32, bid: i32) -> i32 {
    if has_exact_bid_bonus(tricks_won, bid) {
        10
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Test scoring point calculations
    #[test]
    fn test_calculate_round_points_exact_bid() {
        // Test that exact bid gives +10 bonus
        let tricks_won = 5;
        let bid = 5;

        let points = calculate_round_points(tricks_won, bid);

        // Should get 5 + 10 = 15 points for exact bid
        assert_eq!(points, 15);
    }

    #[test]
    fn test_calculate_round_points_no_bonus() {
        // Test that no bonus is given for inexact bid
        let tricks_won = 3;
        let bid = 5;

        let points = calculate_round_points(tricks_won, bid);

        // Should get only 3 points (no bonus)
        assert_eq!(points, 3);
    }

    #[test]
    fn test_calculate_round_points_zero_bid_exact() {
        // Test edge case: zero bid with zero tricks
        let tricks_won = 0;
        let bid = 0;

        let points = calculate_round_points(tricks_won, bid);

        // Should get 0 + 10 = 10 points for exact zero bid
        assert_eq!(points, 10);
    }

    #[test]
    fn test_calculate_round_points_over_bid() {
        // Test case where player wins more tricks than bid
        let tricks_won = 7;
        let bid = 5;

        let points = calculate_round_points(tricks_won, bid);

        // Should get 7 points (no bonus for over-bidding)
        assert_eq!(points, 7);
    }

    #[test]
    fn test_calculate_round_points_under_bid() {
        // Test case where player wins fewer tricks than bid
        let tricks_won = 2;
        let bid = 5;

        let points = calculate_round_points(tricks_won, bid);

        // Should get 2 points (no bonus for under-bidding)
        assert_eq!(points, 2);
    }

    // Test bonus calculation functions
    #[test]
    fn test_has_exact_bid_bonus() {
        assert!(has_exact_bid_bonus(5, 5));
        assert!(has_exact_bid_bonus(0, 0));
        assert!(!has_exact_bid_bonus(3, 5));
        assert!(!has_exact_bid_bonus(7, 5));
    }

    #[test]
    fn test_calculate_bonus_amount() {
        assert_eq!(calculate_bonus_amount(5, 5), 10);
        assert_eq!(calculate_bonus_amount(0, 0), 10);
        assert_eq!(calculate_bonus_amount(3, 5), 0);
        assert_eq!(calculate_bonus_amount(7, 5), 0);
    }

    // Test total score calculation
    #[test]
    fn test_calculate_total_score_from_rounds() {
        let round_data = vec![
            (5, 5), // Round 1: exact bid, 15 points
            (3, 5), // Round 2: under bid, 3 points
            (7, 7), // Round 3: exact bid, 17 points
            (2, 4), // Round 4: under bid, 2 points
        ];

        let total_score = calculate_total_score_from_rounds(&round_data);

        // 15 + 3 + 17 + 2 = 37 points
        assert_eq!(total_score, 37);
    }

    #[test]
    fn test_calculate_total_score_from_rounds_empty() {
        let round_data: Vec<(i32, i32)> = vec![];
        let total_score = calculate_total_score_from_rounds(&round_data);
        assert_eq!(total_score, 0);
    }

    #[test]
    fn test_calculate_total_score_from_rounds_single_round() {
        let round_data = vec![(6, 6)]; // Single round with exact bid
        let total_score = calculate_total_score_from_rounds(&round_data);
        assert_eq!(total_score, 16); // 6 + 10 bonus
    }
}
