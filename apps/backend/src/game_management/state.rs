//! Game state management module
//!
//! This module contains logic for managing game state transitions,
//! player readiness, and game lifecycle management.

use chrono::{DateTime, FixedOffset, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, Order, QueryFilter, QueryOrder,
    Set,
};
use uuid::Uuid;

use crate::dto::game_snapshot::{
    GameInfo, GameSnapshot, PlayerSnapshot, RoundBidSnapshot, RoundScoreSnapshot, RoundSnapshot,
    TrickPlaySnapshot, TrickSnapshot, UserSnapshot,
};
use crate::entity::{
    game_players, game_rounds, games, round_bids, round_hands, round_scores, round_tricks,
    trick_plays, users,
};
use crate::game_management::bidding::create_shuffled_deck;
use crate::game_management::rules::{
    calculate_cards_dealt, MAX_CARDS_PER_ROUND, PLAYER_COUNT, TOTAL_ROUNDS,
};
use crate::game_management::scoring::calculate_round_points;

/// Helper function to check if all players are ready and start the game if so
pub(crate) async fn check_and_start_game(
    game: games::Model,
    db: &DatabaseConnection,
) -> Result<bool, String> {
    // Fetch all players for this game
    let players = match game_players::Entity::find()
        .filter(game_players::Column::GameId.eq(game.id))
        .all(db)
        .await
    {
        Ok(players) => players,
        Err(_) => return Err("Failed to fetch game players".to_string()),
    };

    // Only proceed if exactly 4 players are in the game
    if players.len() == PLAYER_COUNT {
        // Check if all players are ready
        let all_ready = players.iter().all(|game_player| game_player.is_ready);

        if all_ready {
            // Start the game
            let now: DateTime<FixedOffset> = Utc::now().into();
            let game_id = game.id; // Extract game_id before moving game
            let mut game_model: games::ActiveModel = game.into();
            game_model.state = Set(games::GameState::Started);
            game_model.phase = Set(games::GamePhase::Bidding);
            game_model.current_turn = Set(Some(0)); // Start with player 0
            game_model.started_at = Set(Some(now));
            game_model.updated_at = Set(now);

            match game_model.update(db).await {
                Ok(_) => {
                    // Create the first round
                    let round_id = Uuid::new_v4();
                    let first_round = game_rounds::ActiveModel {
                        id: Set(round_id),
                        game_id: Set(game_id),
                        round_number: Set(1),
                        dealer_player_id: Set(None), // Will be set later
                        trump_suit: Set(None),
                        cards_dealt: Set(MAX_CARDS_PER_ROUND), // First round deals 13 cards
                        created_at: Set(now),
                    };

                    match first_round.insert(db).await {
                        Ok(_) => {
                            // Deal cards to players for the first round
                            match deal_cards_to_players(&round_id, MAX_CARDS_PER_ROUND, db).await {
                                Ok(_) => Ok(true),
                                Err(e) => Err(format!("Failed to deal cards: {e}")),
                            }
                        }
                        Err(_) => Err("Failed to create first round".to_string()),
                    }
                }
                Err(_) => Err("Failed to start game".to_string()),
            }
        } else {
            Ok(false)
        }
    } else {
        Ok(false)
    }
}

/// Deal cards to players for a round
pub(crate) async fn deal_cards_to_players(
    round_id: &Uuid,
    cards_dealt: i32,
    db: &DatabaseConnection,
) -> Result<(), String> {
    // Get all players in the game
    let round = match game_rounds::Entity::find_by_id(*round_id).one(db).await {
        Ok(Some(round)) => round,
        Ok(None) => return Err("Round not found".to_string()),
        Err(_) => return Err("Failed to fetch round".to_string()),
    };

    let players = match game_players::Entity::find()
        .filter(game_players::Column::GameId.eq(round.game_id))
        .all(db)
        .await
    {
        Ok(players) => players,
        Err(_) => return Err("Failed to fetch game players".to_string()),
    };

    // Create and shuffle the deck
    let deck = create_shuffled_deck();

    // Calculate total cards needed
    let total_cards_needed = cards_dealt * players.len() as i32;
    if total_cards_needed > PLAYER_COUNT as i32 * MAX_CARDS_PER_ROUND {
        return Err("Not enough cards in deck".to_string());
    }

    // Deal cards to each player
    for (player_index, player) in players.iter().enumerate() {
        for card_index in 0..cards_dealt {
            let card_index_in_deck = (player_index * cards_dealt as usize) + card_index as usize;
            if card_index_in_deck >= deck.len() {
                return Err("Not enough cards in deck".to_string());
            }

            let card = deck[card_index_in_deck].clone();

            // Store the card in round_hands table
            let round_hand = round_hands::ActiveModel {
                id: Set(Uuid::new_v4()),
                round_id: Set(*round_id),
                player_id: Set(player.id),
                card: Set(card),
            };

            match round_hand.insert(db).await {
                Ok(_) => (),
                Err(_) => return Err("Failed to store card in round_hands".to_string()),
            }
        }
    }

    Ok(())
}

/// Calculate scores for a round and update player totals
#[allow(dead_code)]
pub(crate) async fn calculate_round_scores(
    round_id: &Uuid,
    db: &DatabaseConnection,
) -> Result<(), String> {
    // Get all players in the game
    let round = match game_rounds::Entity::find_by_id(*round_id).one(db).await {
        Ok(Some(round)) => round,
        Ok(None) => return Err("Round not found".to_string()),
        Err(_) => return Err("Failed to fetch round".to_string()),
    };

    let players = match game_players::Entity::find()
        .filter(game_players::Column::GameId.eq(round.game_id))
        .all(db)
        .await
    {
        Ok(players) => players,
        Err(_) => return Err("Failed to fetch game players".to_string()),
    };

    // Count tricks won by each player
    let tricks_won = match round_tricks::Entity::find()
        .filter(round_tricks::Column::RoundId.eq(*round_id))
        .all(db)
        .await
    {
        Ok(tricks) => tricks,
        Err(_) => return Err("Failed to fetch round tricks".to_string()),
    };

    // Create a map of player_id -> tricks won
    let mut player_tricks: std::collections::HashMap<Uuid, i32> = std::collections::HashMap::new();
    for trick in tricks_won {
        if let Some(winner_id) = trick.winner_player_id {
            *player_tricks.entry(winner_id).or_insert(0) += 1;
        }
    }

    // Create round scores and update player totals
    for player in &players {
        let tricks_won = player_tricks.get(&player.id).unwrap_or(&0);

        let round_score = round_scores::ActiveModel {
            id: Set(Uuid::new_v4()),
            round_id: Set(*round_id),
            player_id: Set(player.id),
            tricks_won: Set(*tricks_won),
        };

        match round_score.insert(db).await {
            Ok(_) => (),
            Err(_) => return Err("Failed to create round score".to_string()),
        }

        // Note: Round scores are stored in round_scores table
        // Total score is now calculated dynamically from round scores
    }

    Ok(())
}

/// Create the next round for a game
#[allow(dead_code)]
pub(crate) async fn create_next_round(
    game_id: &Uuid,
    db: &DatabaseConnection,
) -> Result<(), String> {
    // Get the current round number
    let current_round = match game_rounds::Entity::find()
        .filter(game_rounds::Column::GameId.eq(*game_id))
        .order_by_desc(game_rounds::Column::RoundNumber)
        .one(db)
        .await
    {
        Ok(Some(round)) => round,
        Ok(None) => return Err("No current round found".to_string()),
        Err(_) => return Err("Failed to fetch current round".to_string()),
    };

    let next_round_number = current_round.round_number + 1;

    // Check if game is complete (26 rounds)
    if next_round_number > TOTAL_ROUNDS {
        // Mark game as completed
        let game = match games::Entity::find_by_id(*game_id).one(db).await {
            Ok(Some(game)) => game,
            Ok(None) => return Err("Game not found".to_string()),
            Err(_) => return Err("Failed to fetch game".to_string()),
        };

        let now: DateTime<FixedOffset> = Utc::now().into();
        let mut game_update: games::ActiveModel = game.into();
        game_update.state = Set(games::GameState::Completed);
        game_update.phase = Set(games::GamePhase::Bidding);
        game_update.completed_at = Set(Some(now));
        game_update.updated_at = Set(now);

        match game_update.update(db).await {
            Ok(_) => return Ok(()),
            Err(_) => return Err("Failed to mark game as completed".to_string()),
        }
    }

    // Calculate cards to deal for the next round
    let cards_dealt = calculate_cards_dealt(next_round_number);

    // Get all players to determine the next dealer
    let players = match game_players::Entity::find()
        .filter(game_players::Column::GameId.eq(*game_id))
        .order_by(game_players::Column::TurnOrder, Order::Asc)
        .all(db)
        .await
    {
        Ok(players) => players,
        Err(_) => return Err("Failed to fetch game players".to_string()),
    };

    // Determine next dealer (rotate through players)
    let next_dealer = if let Some(current_dealer) = current_round.dealer_player_id {
        let current_dealer_index = players
            .iter()
            .position(|p| p.id == current_dealer)
            .unwrap_or(0);
        let next_dealer_index = (current_dealer_index + 1) % PLAYER_COUNT;
        Some(players[next_dealer_index].id)
    } else {
        // If no current dealer, start with the first player
        players.first().map(|p| p.id)
    };

    // Create the next round
    let next_round_id = Uuid::new_v4();
    let next_round = game_rounds::ActiveModel {
        id: Set(next_round_id),
        game_id: Set(*game_id),
        round_number: Set(next_round_number),
        dealer_player_id: Set(next_dealer),
        trump_suit: Set(None),
        cards_dealt: Set(cards_dealt),
        created_at: Set(chrono::Utc::now().into()),
    };

    match next_round.insert(db).await {
        Ok(_) => {
            // Update game state to bidding phase and set current turn
            let game = match games::Entity::find_by_id(*game_id).one(db).await {
                Ok(Some(game)) => game,
                Ok(None) => return Err("Game not found".to_string()),
                Err(_) => return Err("Failed to fetch game".to_string()),
            };

            let mut game_update: games::ActiveModel = game.into();
            game_update.phase = Set(games::GamePhase::Bidding);
            game_update.current_turn = Set(Some(0)); // Start bidding with player 0
            game_update.updated_at = Set(chrono::Utc::now().into());

            match game_update.update(db).await {
                Ok(_) => {
                    // Deal cards to players for the new round
                    match deal_cards_to_players(&next_round_id, cards_dealt, db).await {
                        Ok(_) => Ok(()),
                        Err(e) => Err(format!("Failed to deal cards: {e}")),
                    }
                }
                Err(_) => Err("Failed to update game state".to_string()),
            }
        }
        Err(_) => Err("Failed to create next round".to_string()),
    }
}

/// Calculate total score for a player based on their round scores
pub(crate) async fn calculate_player_total_score(
    player_id: &Uuid,
    game_id: &Uuid,
    db: &DatabaseConnection,
) -> Result<i32, String> {
    // Get all rounds for this game
    let rounds = match game_rounds::Entity::find()
        .filter(game_rounds::Column::GameId.eq(*game_id))
        .all(db)
        .await
    {
        Ok(rounds) => rounds,
        Err(_) => return Err("Failed to fetch game rounds".to_string()),
    };

    let mut total_score = 0;

    for round in rounds {
        // Get round scores for this player
        let round_scores = match round_scores::Entity::find()
            .filter(round_scores::Column::RoundId.eq(round.id))
            .filter(round_scores::Column::PlayerId.eq(*player_id))
            .one(db)
            .await
        {
            Ok(Some(score)) => score,
            Ok(None) => continue, // No score for this round
            Err(_) => continue,   // Skip on error
        };

        total_score += round_scores.tricks_won;
    }

    Ok(total_score)
}

/// Build a game snapshot from the current game state
pub(crate) async fn build_game_snapshot(
    game: games::Model,
    game_players: Vec<game_players::Model>,
    db: &DatabaseConnection,
) -> Result<GameSnapshot, String> {
    // Fetch user details for all players and build PlayerSnapshot instances
    let mut players_with_details = Vec::new();
    for game_player in &game_players {
        let user = match users::Entity::find_by_id(game_player.user_id).one(db).await {
            Ok(Some(user)) => user,
            Ok(None) => continue, // Skip if user not found
            Err(_) => continue,   // Skip on error
        };

        let user_snapshot = UserSnapshot {
            id: user.id,
            email: user.email,
            name: user.name,
        };

        // Calculate total score for this player
        let total_score =
            (calculate_player_total_score(&game_player.id, &game.id, db).await).unwrap_or_default();

        // Fetch player's hand for the current round (only if there is a current round)
        let mut player_hand = None;
        if let Ok(Some(current_round)) = game_rounds::Entity::find()
            .filter(game_rounds::Column::GameId.eq(game.id))
            .order_by_desc(game_rounds::Column::RoundNumber)
            .one(db)
            .await
        {
            // Only show hand to the authenticated player
            if game_player.user_id == user.id {
                let hand_cards = match round_hands::Entity::find()
                    .filter(round_hands::Column::RoundId.eq(current_round.id))
                    .filter(round_hands::Column::PlayerId.eq(game_player.id))
                    .all(db)
                    .await
                {
                    Ok(cards) => cards.into_iter().map(|card| card.card).collect(),
                    Err(_) => Vec::new(),
                };
                player_hand = Some(hand_cards);
            }
        }

        let player_snapshot = PlayerSnapshot {
            id: game_player.id,
            user_id: game_player.user_id,
            turn_order: game_player.turn_order,
            is_ready: game_player.is_ready,
            is_ai: user.is_ai,
            total_score,
            hand: player_hand,
            user: user_snapshot,
        };

        players_with_details.push(player_snapshot);
    }

    // Sort players by turn order
    players_with_details.sort_by(|a, b| {
        let a_order = a.turn_order.unwrap_or(-1);
        let b_order = b.turn_order.unwrap_or(-1);
        a_order.cmp(&b_order)
    });

    // Build GameInfo
    let game_info = GameInfo {
        id: game.id,
        state: game.state.to_string(),
        phase: game.phase.to_string(),
        current_turn: game.current_turn,
        created_at: game.created_at,
        updated_at: game.updated_at,
        started_at: game.started_at,
    };

    // Fetch current round information
    let current_round = match game_rounds::Entity::find()
        .filter(game_rounds::Column::GameId.eq(game.id))
        .order_by_desc(game_rounds::Column::RoundNumber)
        .one(db)
        .await
    {
        Ok(Some(round)) => {
            // Fetch bids for this round
            let round_bids = (round_bids::Entity::find()
                .filter(round_bids::Column::RoundId.eq(round.id))
                .all(db)
                .await)
                .unwrap_or_default();

            let bid_snapshots: Vec<RoundBidSnapshot> = round_bids
                .iter()
                .map(|bid| RoundBidSnapshot {
                    player_id: bid.player_id,
                    bid: bid.bid,
                })
                .collect();

            // Fetch tricks for this round
            let round_tricks = (round_tricks::Entity::find()
                .filter(round_tricks::Column::RoundId.eq(round.id))
                .order_by(round_tricks::Column::TrickNumber, Order::Asc)
                .all(db)
                .await)
                .unwrap_or_default();

            // Build trick snapshots
            let mut completed_tricks = Vec::new();
            let mut current_trick = None;

            for trick in &round_tricks {
                // Fetch plays for this trick
                let trick_plays = (trick_plays::Entity::find()
                    .filter(trick_plays::Column::TrickId.eq(trick.id))
                    .order_by(trick_plays::Column::PlayOrder, Order::Asc)
                    .all(db)
                    .await)
                    .unwrap_or_default();

                let play_snapshots: Vec<TrickPlaySnapshot> = trick_plays
                    .iter()
                    .map(|play| TrickPlaySnapshot {
                        player_id: play.player_id,
                        card: play.card.clone(),
                        play_order: play.play_order,
                    })
                    .collect();

                let trick_snapshot = TrickSnapshot {
                    id: trick.id,
                    trick_number: trick.trick_number,
                    winner_player_id: trick.winner_player_id,
                    plays: play_snapshots,
                };

                // If trick has a winner, it's completed
                if trick.winner_player_id.is_some() {
                    completed_tricks.push(trick_snapshot);
                } else {
                    // This is the current trick
                    current_trick = Some(trick_snapshot);
                }
            }

            // Determine current player turn for playing and bidding phases
            let current_player_turn = if game.phase == games::GamePhase::Playing
                || game.phase == games::GamePhase::Bidding
            {
                if let Some(turn_order) = game.current_turn {
                    // Find the player with this turn order
                    players_with_details
                        .iter()
                        .find(|p| p.turn_order == Some(turn_order))
                        .map(|p| p.id)
                } else {
                    // For bidding phase, if no current turn is set, default to player 0
                    if game.phase == games::GamePhase::Bidding {
                        players_with_details
                            .iter()
                            .find(|p| p.turn_order == Some(0))
                            .map(|p| p.id)
                    } else {
                        None
                    }
                }
            } else {
                None
            };

            // Fetch round scores for this round
            let round_scores = (round_scores::Entity::find()
                .filter(round_scores::Column::RoundId.eq(round.id))
                .all(db)
                .await)
                .unwrap_or_default();

            // Build round score snapshots with calculated points
            let mut round_score_snapshots = Vec::new();
            for score in &round_scores {
                // Find the corresponding bid for this player
                let bid = bid_snapshots
                    .iter()
                    .find(|b| b.player_id == score.player_id)
                    .map(|b| b.bid)
                    .unwrap_or(0);

                // Calculate points: 1 point per trick + 10 point bonus if bid matches tricks won
                let points = calculate_round_points(score.tricks_won, bid);

                round_score_snapshots.push(RoundScoreSnapshot {
                    player_id: score.player_id,
                    tricks_won: score.tricks_won,
                    bid,
                    points,
                });
            }

            Some(RoundSnapshot {
                id: round.id,
                round_number: round.round_number,
                phase: game.phase.to_string(),
                dealer_player_id: round.dealer_player_id,
                trump_suit: round.trump_suit.clone(),
                cards_dealt: round.cards_dealt,
                bids: bid_snapshots,
                current_bidder_turn: game.current_turn,
                current_trick,
                completed_tricks,
                current_player_turn,
                round_scores: round_score_snapshots,
            })
        }
        Ok(None) => None,
        Err(_) => None,
    };

    // Calculate trump chooser if in TrumpSelection phase
    let trump_chooser_id = if game.phase == games::GamePhase::TrumpSelection {
        if let Some(round) = &current_round {
            // Get the highest bidder using the bidding module
            crate::game_management::bidding::resolve_highest_bidder(round.id, db)
                .await
                .unwrap_or(None)
        } else {
            None
        }
    } else {
        None
    };

    // Build GameSnapshot
    let game_snapshot = GameSnapshot {
        game: game_info,
        players: players_with_details,
        current_round,
        player_count: game_players.len(),
        max_players: 4,
        trump_chooser_id,
    };

    Ok(game_snapshot)
}

/// Assert that the game is in the expected phase
#[allow(dead_code)]
pub(crate) fn assert_phase(
    game: &games::Model,
    expected_phase: games::GamePhase,
) -> Result<(), String> {
    if game.phase != expected_phase {
        Err(format!(
            "Game is in {:?} phase, expected {:?}",
            game.phase, expected_phase
        ))
    } else {
        Ok(())
    }
}

/// Advance the game to the next phase
pub(crate) async fn advance_phase(
    game: &games::Model,
    next_phase: games::GamePhase,
    db: &(impl sea_orm::ConnectionTrait + std::marker::Send),
) -> Result<(), String> {
    let mut game_update: games::ActiveModel = game.clone().into();
    game_update.phase = Set(next_phase);
    game_update.updated_at = Set(Utc::now().into());

    match game_update.update(db).await {
        Ok(_) => Ok(()),
        Err(e) => Err(format!("Failed to advance phase: {e}")),
    }
}

/// Set the next player's turn
pub(crate) async fn set_next_player(
    game: &games::Model,
    next_turn: i32,
    db: &(impl sea_orm::ConnectionTrait + std::marker::Send),
) -> Result<(), String> {
    let mut game_update: games::ActiveModel = game.clone().into();
    game_update.current_turn = Set(Some(next_turn));
    game_update.updated_at = Set(Utc::now().into());

    match game_update.update(db).await {
        Ok(_) => Ok(()),
        Err(e) => Err(format!("Failed to set next player: {e}")),
    }
}

/// Start the next trick if ready (when all players have played in current trick)
#[allow(dead_code)]
pub(crate) async fn start_next_trick_if_ready(
    _game: &games::Model,
    round_id: Uuid,
    db: &DatabaseConnection,
) -> Result<bool, String> {
    // Check if all players have played in the current trick
    let current_trick = match round_tricks::Entity::find()
        .filter(round_tricks::Column::RoundId.eq(round_id))
        .order_by_desc(round_tricks::Column::TrickNumber)
        .one(db)
        .await
    {
        Ok(Some(trick)) => trick,
        Ok(None) => return Ok(false), // No current trick
        Err(_) => return Err("Failed to fetch current trick".to_string()),
    };

    let plays = match trick_plays::Entity::find()
        .filter(trick_plays::Column::TrickId.eq(current_trick.id))
        .all(db)
        .await
    {
        Ok(plays) => plays,
        Err(_) => return Err("Failed to fetch trick plays".to_string()),
    };

    // If all players have played, start next trick
    if plays.len() == PLAYER_COUNT {
        // Create next trick
        let next_trick = round_tricks::ActiveModel {
            id: Set(Uuid::new_v4()),
            round_id: Set(round_id),
            trick_number: Set(current_trick.trick_number + 1),
            winner_player_id: Set(None),
            created_at: Set(Utc::now().into()),
        };

        match next_trick.insert(db).await {
            Ok(_) => Ok(true),
            Err(_) => Err("Failed to create next trick".to_string()),
        }
    } else {
        Ok(false)
    }
}

/// Start the next round if ready (when current round is complete)
#[allow(dead_code)]
pub(crate) async fn start_next_round_if_ready(
    game: &games::Model,
    db: &DatabaseConnection,
) -> Result<bool, String> {
    // Check if current round is complete
    let current_round = match game_rounds::Entity::find()
        .filter(game_rounds::Column::GameId.eq(game.id))
        .order_by_desc(game_rounds::Column::RoundNumber)
        .one(db)
        .await
    {
        Ok(Some(round)) => round,
        Ok(None) => return Ok(false), // No current round
        Err(_) => return Err("Failed to fetch current round".to_string()),
    };

    // Check if all tricks in the round are complete
    let tricks = match round_tricks::Entity::find()
        .filter(round_tricks::Column::RoundId.eq(current_round.id))
        .all(db)
        .await
    {
        Ok(tricks) => tricks,
        Err(_) => return Err("Failed to fetch round tricks".to_string()),
    };

    let expected_tricks = current_round.cards_dealt;
    let completed_tricks = tricks
        .iter()
        .filter(|t| t.winner_player_id.is_some())
        .count();

    // If all tricks are complete, start next round
    if completed_tricks == expected_tricks as usize {
        create_next_round(&game.id, db).await?;
        Ok(true)
    } else {
        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_assert_phase() {
        let game = games::Model {
            id: Uuid::new_v4(),
            state: games::GameState::Started,
            phase: games::GamePhase::Bidding,
            current_turn: Some(0),
            created_at: Utc::now().into(),
            updated_at: Utc::now().into(),
            started_at: Some(Utc::now().into()),
            completed_at: None,
        };

        // Should succeed for correct phase
        assert!(assert_phase(&game, games::GamePhase::Bidding).is_ok());

        // Should fail for wrong phase
        assert!(assert_phase(&game, games::GamePhase::Playing).is_err());
    }
}
