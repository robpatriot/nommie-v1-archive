//! Game management module
//!
//! This module contains the core game logic for the Nommie card game.

pub mod bidding;
pub mod rules;
pub mod scoring;
pub mod state;
pub mod tricks;

// Re-export commonly used rules constants and functions

use crate::game_management::bidding::create_shuffled_deck;
use crate::game_management::rules::{
    calculate_cards_dealt, is_valid_card_format, MAX_CARDS_PER_ROUND, PLAYER_COUNT, TOTAL_ROUNDS,
};
use crate::game_management::scoring::{calculate_round_points, has_exact_bid_bonus};

use actix_web::{delete, get, post, web, HttpRequest, HttpResponse, Result as ActixResult};
use chrono::{DateTime, FixedOffset, Utc};

use sea_orm::sea_query::Query;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, DatabaseTransaction, EntityTrait, Order,
    QueryFilter, QueryOrder, Set, TransactionTrait,
};
use serde_json::json;
use uuid::Uuid;

use crate::dto::bid_request::BidRequest;
use crate::dto::game_snapshot::{
    GameInfo, GameSnapshot, PlayerSnapshot, RoundBidSnapshot, RoundScoreSnapshot, RoundSnapshot,
    TrickPlaySnapshot, TrickSnapshot, UserSnapshot,
};
use crate::dto::game_summary::{
    FinalRoundSummary, GameSummary, GameSummaryInfo, PlayerRoundResult, PlayerSummary,
    RoundBidSummary, RoundScoreSummary, RoundSummary, UserSummary,
};
use crate::dto::play_request::PlayRequest;
use crate::dto::trump_request::TrumpRequest;
use crate::entity::{
    game_players, game_rounds, games, round_bids, round_hands, round_scores, round_tricks,
    trick_plays, users,
};
use crate::jwt::get_user;

/// Helper function to check if all players are ready and start the game if so
async fn check_and_start_game(game: games::Model, db: &DatabaseConnection) -> Result<bool, String> {
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

#[post("/create_game")]
pub async fn create_game(
    req: HttpRequest,
    db: web::Data<DatabaseConnection>,
) -> ActixResult<HttpResponse> {
    // Extract user from JWT authentication
    let user = match get_user(&req) {
        Some(user) => user,
        None => {
            return Ok(HttpResponse::Unauthorized()
                .content_type("application/json")
                .json(json!({
                    "error": "User not authenticated"
                })));
        }
    };

    // Create a new game
    let game_id = Uuid::new_v4();
    let now: DateTime<FixedOffset> = Utc::now().into();

    let game = games::ActiveModel {
        id: Set(game_id),
        state: Set(games::GameState::Waiting),
        phase: Set(games::GamePhase::Bidding),
        current_turn: Set(None),
        created_at: Set(now),
        updated_at: Set(now),
        started_at: Set(None),
        completed_at: Set(None),
    };

    // Insert the game into the database
    let game_result = match game.insert(&**db).await {
        Ok(game) => game,
        Err(e) => {
            return Ok(HttpResponse::InternalServerError()
                .content_type("application/json")
                .json(json!({
                    "error": "Failed to create game",
                    "details": e.to_string()
                })));
        }
    };

    // Create a game_players entry linking the user to the game
    let game_player_id = Uuid::new_v4();
    let game_player = game_players::ActiveModel {
        id: Set(game_player_id),
        game_id: Set(game_id),
        user_id: Set(user.id),
        turn_order: Set(Some(0)), // First player gets turn order 0
        is_ready: Set(false),
    };

    // Insert the game player into the database
    let game_player_result = match game_player.insert(&**db).await {
        Ok(game_player) => game_player,
        Err(e) => {
            return Ok(HttpResponse::InternalServerError()
                .content_type("application/json")
                .json(json!({
                    "error": "Failed to create game player",
                    "details": e.to_string()
                })));
        }
    };

    // Return the created game and its player
    Ok(HttpResponse::Ok()
        .content_type("application/json")
        .json(json!({
            "game": game_result,
            "game_players": vec![game_player_result]
        })))
}

#[get("/games")]
pub async fn get_games(
    req: HttpRequest,
    db: web::Data<DatabaseConnection>,
) -> ActixResult<HttpResponse> {
    // Extract user from JWT authentication
    let user = match get_user(&req) {
        Some(user) => user,
        None => {
            return Ok(HttpResponse::Unauthorized()
                .content_type("application/json")
                .json(json!({
                    "error": "User not authenticated"
                })));
        }
    };

    // Fetch all games
    let all_games = match games::Entity::find().all(&**db).await {
        Ok(games) => games,
        Err(e) => {
            return Ok(HttpResponse::InternalServerError()
                .content_type("application/json")
                .json(json!({
                    "error": "Failed to fetch games",
                    "details": e.to_string()
                })));
        }
    };

    // Fetch all game players
    let all_game_players = match game_players::Entity::find().all(&**db).await {
        Ok(players) => players,
        Err(e) => {
            return Ok(HttpResponse::InternalServerError()
                .content_type("application/json")
                .json(json!({
                    "error": "Failed to fetch game players",
                    "details": e.to_string()
                })));
        }
    };

    // Process the results to get game info with player counts
    let mut games_list = Vec::new();

    for game in all_games {
        let game_players: Vec<_> = all_game_players
            .iter()
            .filter(|gp| gp.game_id == game.id)
            .collect();

        let player_count = game_players.len();
        let is_player_in_game = game_players.iter().any(|gp| gp.user_id == user.id);

        // Check if current user is the creator (turn_order 0)
        let is_creator = game_players
            .iter()
            .any(|gp| gp.user_id == user.id && gp.turn_order == Some(0));

        games_list.push(json!({
            "id": game.id,
            "state": game.state,
            "player_count": player_count,
            "max_players": 4, // Assuming 4 players max for now
            "is_player_in_game": is_player_in_game,
            "is_creator": is_creator
        }));
    }

    Ok(HttpResponse::Ok()
        .content_type("application/json")
        .json(json!({
            "games": games_list
        })))
}

#[post("/game/{game_id}/ready")]
pub async fn mark_player_ready(
    req: HttpRequest,
    path: web::Path<String>,
    db: web::Data<DatabaseConnection>,
) -> ActixResult<HttpResponse> {
    // Extract user from JWT authentication
    let user = match get_user(&req) {
        Some(user) => user,
        None => {
            return Ok(HttpResponse::Unauthorized()
                .content_type("application/json")
                .json(json!({
                    "error": "User not authenticated"
                })));
        }
    };

    // Parse game ID from path
    let game_id = match Uuid::parse_str(&path.into_inner()) {
        Ok(id) => id,
        Err(_) => {
            return Ok(HttpResponse::BadRequest()
                .content_type("application/json")
                .json(json!({
                    "error": "Invalid game ID format"
                })));
        }
    };

    // Fetch the game to check its state
    let game = match games::Entity::find_by_id(game_id).one(&**db).await {
        Ok(Some(game)) => game,
        Ok(None) => {
            return Ok(HttpResponse::NotFound()
                .content_type("application/json")
                .json(json!({
                    "error": "Game not found"
                })));
        }
        Err(e) => {
            return Ok(HttpResponse::InternalServerError()
                .content_type("application/json")
                .json(json!({
                    "error": "Failed to fetch game",
                    "details": e.to_string()
                })));
        }
    };

    // Check if game is in waiting state
    if game.state != games::GameState::Waiting {
        return Ok(HttpResponse::BadRequest()
            .content_type("application/json")
            .json(json!({
                "error": "Game is not in waiting state"
            })));
    }

    // Find the game player record for this user
    let game_player = match game_players::Entity::find()
        .filter(game_players::Column::GameId.eq(game_id))
        .filter(game_players::Column::UserId.eq(user.id))
        .one(&**db)
        .await
    {
        Ok(Some(game_player)) => game_player,
        Ok(None) => {
            return Ok(HttpResponse::NotFound()
                .content_type("application/json")
                .json(json!({
                    "error": "Player not found in game"
                })));
        }
        Err(e) => {
            return Ok(HttpResponse::InternalServerError()
                .content_type("application/json")
                .json(json!({
                    "error": "Failed to fetch game player",
                    "details": e.to_string()
                })));
        }
    };

    // Check if player is already ready
    if game_player.is_ready {
        return Ok(HttpResponse::BadRequest()
            .content_type("application/json")
            .json(json!({
                "error": "Player is already ready"
            })));
    }

    // Update the game player to mark as ready
    let mut game_player_model: game_players::ActiveModel = game_player.into();
    game_player_model.is_ready = Set(true);

    let updated_game_player = match game_player_model.update(&**db).await {
        Ok(game_player) => game_player,
        Err(e) => {
            return Ok(HttpResponse::InternalServerError()
                .content_type("application/json")
                .json(json!({
                    "error": "Failed to update player readiness",
                    "details": e.to_string()
                })));
        }
    };

    // Check if all players are ready and start the game if so
    match check_and_start_game(game, &db).await {
        Ok(true) => {
            return Ok(HttpResponse::Ok()
                .content_type("application/json")
                .json(json!({
                    "success": true,
                    "message": "Player marked as ready and game started",
                    "game_player": updated_game_player,
                    "game_started": true
                })));
        }
        Ok(false) => {
            // Game not started, continue to normal response
        }
        Err(e) => {
            return Ok(HttpResponse::InternalServerError()
                .content_type("application/json")
                .json(json!({
                    "error": "Failed to start game",
                    "details": e
                })));
        }
    }

    Ok(HttpResponse::Ok()
        .content_type("application/json")
        .json(json!({
            "success": true,
            "message": "Player marked as ready",
            "game_player": updated_game_player,
            "game_started": false
        })))
}

#[post("/game/{game_id}/add_ai")]
pub async fn add_ai_player(
    req: HttpRequest,
    path: web::Path<String>,
    db: web::Data<DatabaseConnection>,
) -> ActixResult<HttpResponse> {
    // Extract user from JWT authentication
    let user = match get_user(&req) {
        Some(user) => user,
        None => {
            return Ok(HttpResponse::Unauthorized()
                .content_type("application/json")
                .json(json!({
                    "error": "User not authenticated"
                })));
        }
    };

    // Parse game ID from path
    let game_id = match Uuid::parse_str(&path.into_inner()) {
        Ok(id) => id,
        Err(_) => {
            return Ok(HttpResponse::BadRequest()
                .content_type("application/json")
                .json(json!({
                    "error": "Invalid game ID format"
                })));
        }
    };

    // Fetch the game to check its state
    let game = match games::Entity::find_by_id(game_id).one(&**db).await {
        Ok(Some(game)) => game,
        Ok(None) => {
            return Ok(HttpResponse::NotFound()
                .content_type("application/json")
                .json(json!({
                    "error": "Game not found"
                })));
        }
        Err(e) => {
            return Ok(HttpResponse::InternalServerError()
                .content_type("application/json")
                .json(json!({
                    "error": "Failed to fetch game",
                    "details": e.to_string()
                })));
        }
    };

    // Check if game is in waiting state
    if game.state != games::GameState::Waiting {
        return Ok(HttpResponse::BadRequest()
            .content_type("application/json")
            .json(json!({
                "error": "Game is not in waiting state"
            })));
    }

    // Check if user is in the game
    let user_in_game = match game_players::Entity::find()
        .filter(game_players::Column::GameId.eq(game_id))
        .filter(game_players::Column::UserId.eq(user.id))
        .one(&**db)
        .await
    {
        Ok(Some(_)) => true,
        Ok(None) => false,
        Err(e) => {
            return Ok(HttpResponse::InternalServerError()
                .content_type("application/json")
                .json(json!({
                    "error": "Failed to check if user is in game",
                    "details": e.to_string()
                })));
        }
    };

    if !user_in_game {
        return Ok(HttpResponse::Forbidden()
            .content_type("application/json")
            .json(json!({
                "error": "User is not in this game"
            })));
    }

    // Get current player count
    let current_players = match game_players::Entity::find()
        .filter(game_players::Column::GameId.eq(game_id))
        .all(&**db)
        .await
    {
        Ok(players) => players,
        Err(e) => {
            return Ok(HttpResponse::InternalServerError()
                .content_type("application/json")
                .json(json!({
                    "error": "Failed to fetch game players",
                    "details": e.to_string()
                })));
        }
    };

    // Check if game is full (max 4 players)
    if current_players.len() >= 4 {
        return Ok(HttpResponse::BadRequest()
            .content_type("application/json")
            .json(json!({
                "error": "Game is full"
            })));
    }

    // Find an available AI user that's not already in this game
    let ai_user = match users::Entity::find()
        .filter(users::Column::IsAi.eq(true))
        .filter(users::Column::Email.like("__ai+%@nommie.dev"))
        .filter(
            users::Column::Id.not_in_subquery(
                Query::select()
                    .column(game_players::Column::UserId)
                    .from(game_players::Entity)
                    .and_where(game_players::Column::GameId.eq(game_id))
                    .to_owned(),
            ),
        )
        .one(&**db)
        .await
    {
        Ok(Some(user)) => user,
        Ok(None) => {
            return Ok(HttpResponse::InternalServerError()
                .content_type("application/json")
                .json(json!({
                    "error": "No AI users available"
                })));
        }
        Err(e) => {
            return Ok(HttpResponse::InternalServerError()
                .content_type("application/json")
                .json(json!({
                    "error": "Failed to fetch AI user",
                    "details": e.to_string()
                })));
        }
    };

    // Create AI game player
    let ai_game_player_id = Uuid::new_v4();
    let ai_game_player = game_players::ActiveModel {
        id: Set(ai_game_player_id),
        game_id: Set(game_id),
        user_id: Set(ai_user.id),
        turn_order: Set(Some(current_players.len() as i32)), // Assign next available turn order
        is_ready: Set(true),                                 // AI players are automatically ready
    };

    // Insert the AI game player into the database
    let ai_game_player_result = match ai_game_player.insert(&**db).await {
        Ok(game_player) => game_player,
        Err(e) => {
            return Ok(HttpResponse::InternalServerError()
                .content_type("application/json")
                .json(json!({
                    "error": "Failed to add AI player",
                    "details": e.to_string()
                })));
        }
    };

    // Check if game should start (all players ready)
    let game_started = (check_and_start_game(game, &db).await).unwrap_or_default();

    Ok(HttpResponse::Ok()
        .content_type("application/json")
        .json(json!({
            "success": true,
            "message": "AI player added successfully",
            "ai_player": ai_game_player_result,
            "game_started": game_started
        })))
}

#[post("/join_game")]
pub async fn join_game(
    req: HttpRequest,
    query: web::Query<std::collections::HashMap<String, String>>,
    db: web::Data<DatabaseConnection>,
) -> ActixResult<HttpResponse> {
    // Extract user from JWT authentication
    let user = match get_user(&req) {
        Some(user) => user,
        None => {
            return Ok(HttpResponse::Unauthorized()
                .content_type("application/json")
                .json(json!({
                    "error": "User not authenticated"
                })));
        }
    };

    // Get game_id from query parameters
    let game_id = match query.get("game_id") {
        Some(id) => match Uuid::parse_str(id) {
            Ok(uuid) => uuid,
            Err(_) => {
                return Ok(HttpResponse::BadRequest()
                    .content_type("application/json")
                    .json(json!({
                        "error": "Invalid game ID format"
                    })));
            }
        },
        None => {
            return Ok(HttpResponse::BadRequest()
                .content_type("application/json")
                .json(json!({
                    "error": "Game ID is required"
                })));
        }
    };

    // Fetch the game to check its state
    let game = match games::Entity::find_by_id(game_id).one(&**db).await {
        Ok(Some(game)) => game,
        Ok(None) => {
            return Ok(HttpResponse::NotFound()
                .content_type("application/json")
                .json(json!({
                    "error": "Game not found"
                })));
        }
        Err(e) => {
            return Ok(HttpResponse::InternalServerError()
                .content_type("application/json")
                .json(json!({
                    "error": "Failed to fetch game",
                    "details": e.to_string()
                })));
        }
    };

    // Check if game is in waiting state
    if game.state != games::GameState::Waiting {
        return Ok(HttpResponse::BadRequest()
            .content_type("application/json")
            .json(json!({
                "error": "Game is not in waiting state"
            })));
    }

    // Check if user is already in the game
    let user_already_in_game = match game_players::Entity::find()
        .filter(game_players::Column::GameId.eq(game_id))
        .filter(game_players::Column::UserId.eq(user.id))
        .one(&**db)
        .await
    {
        Ok(Some(_)) => true,
        Ok(None) => false,
        Err(e) => {
            return Ok(HttpResponse::InternalServerError()
                .content_type("application/json")
                .json(json!({
                    "error": "Failed to check if user is already in game",
                    "details": e.to_string()
                })));
        }
    };

    if user_already_in_game {
        return Ok(HttpResponse::BadRequest()
            .content_type("application/json")
            .json(json!({
                "error": "User is already in this game"
            })));
    }

    // Get current player count
    let current_players = match game_players::Entity::find()
        .filter(game_players::Column::GameId.eq(game_id))
        .all(&**db)
        .await
    {
        Ok(players) => players,
        Err(e) => {
            return Ok(HttpResponse::InternalServerError()
                .content_type("application/json")
                .json(json!({
                    "error": "Failed to fetch game players",
                    "details": e.to_string()
                })));
        }
    };

    // Check if game is full (max 4 players)
    if current_players.len() >= 4 {
        return Ok(HttpResponse::BadRequest()
            .content_type("application/json")
            .json(json!({
                "error": "Game is full"
            })));
    }

    // Assign turn order based on current player count
    let turn_order = current_players.len() as i32;

    // Create game player entry
    let game_player_id = Uuid::new_v4();
    let game_player = game_players::ActiveModel {
        id: Set(game_player_id),
        game_id: Set(game_id),
        user_id: Set(user.id),
        turn_order: Set(Some(turn_order)),
        is_ready: Set(false),
    };

    // Insert the game player into the database
    let game_player_result = match game_player.insert(&**db).await {
        Ok(game_player) => game_player,
        Err(e) => {
            return Ok(HttpResponse::InternalServerError()
                .content_type("application/json")
                .json(json!({
                    "error": "Failed to join game",
                    "details": e.to_string()
                })));
        }
    };

    Ok(HttpResponse::Ok()
        .content_type("application/json")
        .json(json!({
            "success": true,
            "message": "Successfully joined game",
            "game_player": game_player_result,
            "turn_order": turn_order
        })))
}

#[get("/game/{game_id}/state")]
pub async fn get_game_state(
    req: HttpRequest,
    path: web::Path<String>,
    db: web::Data<DatabaseConnection>,
) -> ActixResult<HttpResponse> {
    // Extract user from JWT authentication
    let user = match get_user(&req) {
        Some(user) => user,
        None => {
            return Ok(HttpResponse::Unauthorized()
                .content_type("application/json")
                .json(json!({
                    "error": "User not authenticated"
                })));
        }
    };

    // Parse game ID from path
    let game_id = match Uuid::parse_str(&path.into_inner()) {
        Ok(id) => id,
        Err(_) => {
            return Ok(HttpResponse::BadRequest()
                .content_type("application/json")
                .json(json!({
                    "error": "Invalid game ID format"
                })));
        }
    };

    // Fetch the game
    let game = match games::Entity::find_by_id(game_id).one(&**db).await {
        Ok(Some(game)) => game,
        Ok(None) => {
            return Ok(HttpResponse::NotFound()
                .content_type("application/json")
                .json(json!({
                    "error": "Game not found"
                })));
        }
        Err(e) => {
            return Ok(HttpResponse::InternalServerError()
                .content_type("application/json")
                .json(json!({
                    "error": "Failed to fetch game",
                    "details": e.to_string()
                })));
        }
    };

    // Reject completed games - use /summary endpoint instead
    if game.state == games::GameState::Completed {
        return Ok(HttpResponse::BadRequest()
            .content_type("application/json")
            .json(json!({
                "error": "Game is completed. Use /api/game/{id}/summary for game summary."
            })));
    }

    // Check if user is a participant in this game
    let user_in_game = match game_players::Entity::find()
        .filter(game_players::Column::GameId.eq(game_id))
        .filter(game_players::Column::UserId.eq(user.id))
        .one(&**db)
        .await
    {
        Ok(Some(_)) => true,
        Ok(None) => false,
        Err(e) => {
            return Ok(HttpResponse::InternalServerError()
                .content_type("application/json")
                .json(json!({
                    "error": "Failed to check user participation",
                    "details": e.to_string()
                })));
        }
    };

    if !user_in_game {
        return Ok(HttpResponse::Forbidden()
            .content_type("application/json")
            .json(json!({
                "error": "Access denied. You are not a participant in this game."
            })));
    }

    // Fetch all game players for this game
    let game_players = match game_players::Entity::find()
        .filter(game_players::Column::GameId.eq(game_id))
        .all(&**db)
        .await
    {
        Ok(players) => players,
        Err(e) => {
            return Ok(HttpResponse::InternalServerError()
                .content_type("application/json")
                .json(json!({
                    "error": "Failed to fetch game players",
                    "details": e.to_string()
                })));
        }
    };

    // AI BEHAVIOR LOGIC
    // Check if it's currently an AI player's turn and they haven't acted yet
    if let Some(current_turn) = game.current_turn {
        // Find the current player
        if let Some(current_player) = game_players
            .iter()
            .find(|p| p.turn_order == Some(current_turn))
        {
            // Get the user for this player
            if let Ok(Some(current_user)) = users::Entity::find_by_id(current_player.user_id)
                .one(&**db)
                .await
            {
                // If it's an AI player's turn, perform their action
                if current_user.is_ai {
                    // Check if they have already acted in this phase
                    let has_acted = match game.phase {
                        games::GamePhase::Bidding => {
                            // Check if they have already bid in the current round
                            if let Ok(Some(current_round)) = game_rounds::Entity::find()
                                .filter(game_rounds::Column::GameId.eq(game_id))
                                .order_by_desc(game_rounds::Column::RoundNumber)
                                .one(&**db)
                                .await
                            {
                                matches!(
                                    round_bids::Entity::find()
                                        .filter(round_bids::Column::RoundId.eq(current_round.id))
                                        .filter(round_bids::Column::PlayerId.eq(current_player.id))
                                        .one(&**db)
                                        .await,
                                    Ok(Some(_))
                                )
                            } else {
                                false
                            }
                        }
                        games::GamePhase::TrumpSelection => {
                            // Check if trump has already been selected
                            if let Ok(Some(current_round)) = game_rounds::Entity::find()
                                .filter(game_rounds::Column::GameId.eq(game_id))
                                .order_by_desc(game_rounds::Column::RoundNumber)
                                .one(&**db)
                                .await
                            {
                                current_round.trump_suit.is_some()
                            } else {
                                false
                            }
                        }
                        games::GamePhase::Playing => {
                            // Check if they have already played in the current trick
                            if let Ok(Some(current_round)) = game_rounds::Entity::find()
                                .filter(game_rounds::Column::GameId.eq(game_id))
                                .order_by_desc(game_rounds::Column::RoundNumber)
                                .one(&**db)
                                .await
                            {
                                if let Ok(Some(current_trick)) = round_tricks::Entity::find()
                                    .filter(round_tricks::Column::RoundId.eq(current_round.id))
                                    .order_by_desc(round_tricks::Column::TrickNumber)
                                    .one(&**db)
                                    .await
                                {
                                    matches!(
                                        trick_plays::Entity::find()
                                            .filter(
                                                trick_plays::Column::TrickId.eq(current_trick.id)
                                            )
                                            .filter(
                                                trick_plays::Column::PlayerId.eq(current_player.id)
                                            )
                                            .one(&**db)
                                            .await,
                                        Ok(Some(_))
                                    )
                                } else {
                                    false
                                }
                            } else {
                                false
                            }
                        }
                        _ => false,
                    };

                    // If AI hasn't acted yet, perform their action
                    if !has_acted {
                        match game.phase {
                            games::GamePhase::Bidding => {
                                // AI bidding: random bid between 0 and 13
                                let ai_bid = rand::random::<i32>() % 14;
                                let bid_request = BidRequest { bid: ai_bid };

                                // Call the bidding logic internally
                                if let Err(e) = bidding::perform_ai_bid(
                                    game_id,
                                    current_player.id,
                                    bid_request,
                                    &db,
                                )
                                .await
                                {
                                    eprintln!("[ERROR] get_game_state: AI bid failed: {e}");
                                }
                            }
                            games::GamePhase::TrumpSelection => {
                                // AI trump selection: random suit
                                let trump_suits =
                                    ["Spades", "Hearts", "Diamonds", "Clubs", "NoTrump"];
                                let ai_trump = trump_suits
                                    [rand::random::<usize>() % trump_suits.len()]
                                .to_string();
                                let trump_request = TrumpRequest {
                                    trump_suit: ai_trump.clone(),
                                };

                                // Call the trump selection logic internally
                                if let Err(e) = bidding::perform_ai_trump_selection(
                                    game_id,
                                    current_player.id,
                                    trump_request,
                                    &db,
                                )
                                .await
                                {
                                    eprintln!(
                                        "[ERROR] get_game_state: AI trump selection failed: {e}",
                                    );
                                }
                            }
                            games::GamePhase::Playing => {
                                // AI card play: follow suit if possible, otherwise random
                                if let Ok(Some(current_round)) = game_rounds::Entity::find()
                                    .filter(game_rounds::Column::GameId.eq(game_id))
                                    .order_by_desc(game_rounds::Column::RoundNumber)
                                    .one(&**db)
                                    .await
                                {
                                    if let Ok(Some(current_trick)) = round_tricks::Entity::find()
                                        .filter(round_tricks::Column::RoundId.eq(current_round.id))
                                        .order_by_desc(round_tricks::Column::TrickNumber)
                                        .one(&**db)
                                        .await
                                    {
                                        // Get AI's hand
                                        let ai_hand = match round_hands::Entity::find()
                                            .filter(
                                                round_hands::Column::RoundId.eq(current_round.id),
                                            )
                                            .filter(
                                                round_hands::Column::PlayerId.eq(current_player.id),
                                            )
                                            .all(&**db)
                                            .await
                                        {
                                            Ok(cards) => cards
                                                .into_iter()
                                                .map(|card| card.card)
                                                .collect::<Vec<String>>(),
                                            Err(_) => Vec::new(),
                                        };

                                        // Get the lead suit from the first card played
                                        let lead_suit = if let Ok(plays) =
                                            trick_plays::Entity::find()
                                                .filter(
                                                    trick_plays::Column::TrickId
                                                        .eq(current_trick.id),
                                                )
                                                .order_by(
                                                    trick_plays::Column::PlayOrder,
                                                    Order::Asc,
                                                )
                                                .all(&**db)
                                                .await
                                        {
                                            if let Some(first_play) = plays.first() {
                                                // Extract suit from card (e.g., "5S" -> "S")
                                                if first_play.card.len() >= 2 {
                                                    Some(
                                                        first_play.card
                                                            [first_play.card.len() - 1..]
                                                            .to_string(),
                                                    )
                                                } else {
                                                    None
                                                }
                                            } else {
                                                None
                                            }
                                        } else {
                                            None
                                        };

                                        // Choose card to play
                                        let ai_card = if let Some(lead_suit) = lead_suit {
                                            // Must follow suit if possible
                                            let cards_of_lead_suit: Vec<String> = ai_hand
                                                .iter()
                                                .filter(|card| {
                                                    card.len() >= 2
                                                        && card[card.len() - 1..] == lead_suit
                                                })
                                                .cloned()
                                                .collect();

                                            if !cards_of_lead_suit.is_empty() {
                                                // Play a random card of the lead suit
                                                cards_of_lead_suit[rand::random::<usize>()
                                                    % cards_of_lead_suit.len()]
                                                .clone()
                                            } else {
                                                // Can play any card
                                                ai_hand[rand::random::<usize>() % ai_hand.len()]
                                                    .clone()
                                            }
                                        } else {
                                            // No lead suit (first to play), play any card
                                            ai_hand[rand::random::<usize>() % ai_hand.len()].clone()
                                        };

                                        let play_request = PlayRequest {
                                            card: ai_card.clone(),
                                        };

                                        // Call the card play logic internally
                                        if let Err(e) = perform_ai_card_play(
                                            game_id,
                                            current_player.id,
                                            play_request,
                                            &db,
                                        )
                                        .await
                                        {
                                            eprintln!(
                                                "[ERROR] get_game_state: AI card play failed: {e}",
                                            );
                                        }
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
    }

    // SPECIAL HANDLING FOR TRUMP SELECTION PHASE
    // In trump selection phase, the highest bidder (not the current turn player) should choose trump
    if game.phase == games::GamePhase::TrumpSelection {
        // Get the current round
        if let Ok(Some(current_round)) = game_rounds::Entity::find()
            .filter(game_rounds::Column::GameId.eq(game_id))
            .order_by_desc(game_rounds::Column::RoundNumber)
            .one(&**db)
            .await
        {
            // Check if trump has already been selected
            if current_round.trump_suit.is_none() {
                // Get the highest bidder using the bidding module
                if let Ok(Some(highest_bidder_id)) =
                    bidding::resolve_highest_bidder(current_round.id, &db).await
                {
                    // Get the highest bidder's player info
                    if let Ok(Some(highest_bidder)) =
                        game_players::Entity::find_by_id(highest_bidder_id)
                            .one(&**db)
                            .await
                    {
                        // Get the user info to check if they're AI
                        if let Ok(Some(user_info)) =
                            users::Entity::find_by_id(highest_bidder.user_id)
                                .one(&**db)
                                .await
                        {
                            if user_info.is_ai {
                                // AI trump selection: random suit
                                let trump_suits =
                                    ["Spades", "Hearts", "Diamonds", "Clubs", "NoTrump"];
                                let ai_trump = trump_suits
                                    [rand::random::<usize>() % trump_suits.len()]
                                .to_string();
                                let trump_request = TrumpRequest {
                                    trump_suit: ai_trump.clone(),
                                };

                                // Call the trump selection logic internally
                                if let Err(e) = bidding::perform_ai_trump_selection(
                                    game_id,
                                    highest_bidder_id,
                                    trump_request,
                                    &db,
                                )
                                .await
                                {
                                    eprintln!(
                                        "[ERROR] get_game_state: AI trump selection failed: {e}",
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Fetch user details for all players and build PlayerSnapshot instances
    let mut players_with_details = Vec::new();
    for game_player in &game_players {
        let user = match users::Entity::find_by_id(game_player.user_id)
            .one(&**db)
            .await
        {
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
        let total_score = (calculate_player_total_score(&game_player.id, &game_id, &db).await)
            .unwrap_or_default();

        // Fetch player's hand for the current round (only if there is a current round)
        let mut player_hand = None;
        if let Ok(Some(current_round)) = game_rounds::Entity::find()
            .filter(game_rounds::Column::GameId.eq(game_id))
            .order_by_desc(game_rounds::Column::RoundNumber)
            .one(&**db)
            .await
        {
            // Only show hand to the authenticated player
            if game_player.user_id == user.id {
                let hand_cards = match round_hands::Entity::find()
                    .filter(round_hands::Column::RoundId.eq(current_round.id))
                    .filter(round_hands::Column::PlayerId.eq(game_player.id))
                    .all(&**db)
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
        .filter(game_rounds::Column::GameId.eq(game_id))
        .order_by_desc(game_rounds::Column::RoundNumber)
        .one(&**db)
        .await
    {
        Ok(Some(round)) => {
            // Fetch bids for this round
            let round_bids = (round_bids::Entity::find()
                .filter(round_bids::Column::RoundId.eq(round.id))
                .all(&**db)
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
                .all(&**db)
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
                    .all(&**db)
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
                .all(&**db)
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
            bidding::resolve_highest_bidder(round.id, &db)
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

    Ok(HttpResponse::Ok()
        .content_type("application/json")
        .json(game_snapshot))
}

#[post("/game/{id}/bid")]
pub async fn submit_bid(
    req: HttpRequest,
    path: web::Path<String>,
    bid_data: web::Json<BidRequest>,
    db: web::Data<DatabaseConnection>,
) -> ActixResult<HttpResponse> {
    // Extract user from JWT authentication
    let user = match get_user(&req) {
        Some(user) => user,
        None => {
            return Ok(HttpResponse::Unauthorized()
                .content_type("application/json")
                .json(json!({
                    "error": "User not authenticated"
                })));
        }
    };

    // Parse game ID from path
    let game_id = match path.parse::<Uuid>() {
        Ok(id) => id,
        Err(_) => {
            return Ok(HttpResponse::BadRequest()
                .content_type("application/json")
                .json(json!({
                    "error": "Invalid game ID format"
                })));
        }
    };

    // Validate bid value (0-13)
    let bid_value = bid_data.bid;
    if !(0..=13).contains(&bid_value) {
        return Ok(HttpResponse::BadRequest()
            .content_type("application/json")
            .json(json!({
                "error": "Bid must be between 0 and 13"
            })));
    }

    // Execute the entire operation in a transaction with row locks
    let result = db
        .transaction(|txn| {
            Box::pin(bidding::submit_bid_transaction(
                game_id, user.id, bid_value, txn,
            ))
        })
        .await;

    match result {
        Ok(_) => Ok(HttpResponse::Ok()
            .content_type("application/json")
            .json(json!({
                "message": "Bid submitted successfully",
                "bid": bid_value
            }))),
        Err(e) => Ok(HttpResponse::BadRequest()
            .content_type("application/json")
            .json(json!({
                "error": e.to_string()
            }))),
    }
}

#[post("/game/{id}/trump")]
pub async fn submit_trump(
    req: HttpRequest,
    path: web::Path<String>,
    trump_data: web::Json<TrumpRequest>,
    db: web::Data<DatabaseConnection>,
) -> ActixResult<HttpResponse> {
    // Extract user from JWT authentication
    let user = match get_user(&req) {
        Some(user) => user,
        None => {
            return Ok(HttpResponse::Unauthorized()
                .content_type("application/json")
                .json(json!({
                    "error": "User not authenticated"
                })));
        }
    };

    // Parse game ID from path
    let game_id = match path.parse::<Uuid>() {
        Ok(id) => id,
        Err(_) => {
            return Ok(HttpResponse::BadRequest()
                .content_type("application/json")
                .json(json!({
                    "error": "Invalid game ID format"
                })));
        }
    };

    // Validate trump suit
    let trump_suit = &trump_data.trump_suit;
    let valid_suits = ["Spades", "Hearts", "Diamonds", "Clubs", "NoTrump"];
    if !valid_suits.contains(&trump_suit.as_str()) {
        return Ok(HttpResponse::BadRequest()
            .content_type("application/json")
            .json(json!({
                "error": "Invalid trump suit. Must be one of: Spades, Hearts, Diamonds, Clubs, NoTrump"
            })));
    }

    // Execute the entire operation in a transaction with row locks
    let result = db
        .transaction(|txn| {
            Box::pin(bidding::submit_trump_transaction(
                game_id,
                user.id,
                trump_suit.clone(),
                txn,
            ))
        })
        .await;

    match result {
        Ok(_) => Ok(HttpResponse::Ok()
            .content_type("application/json")
            .json(json!({
                "message": "Trump suit selected successfully",
                "trump_suit": trump_suit,
                "phase": "playing"
            }))),
        Err(e) => Ok(HttpResponse::BadRequest()
            .content_type("application/json")
            .json(json!({
                "error": e.to_string()
            }))),
    }
}

#[post("/game/{id}/play")]
pub async fn play_card(
    req: HttpRequest,
    path: web::Path<String>,
    play_data: web::Json<PlayRequest>,
    db: web::Data<DatabaseConnection>,
) -> ActixResult<HttpResponse> {
    // Extract user from JWT authentication
    let user = match get_user(&req) {
        Some(user) => user,
        None => {
            return Ok(HttpResponse::Unauthorized()
                .content_type("application/json")
                .json(json!({
                    "error": "User not authenticated"
                })));
        }
    };

    // Parse game ID from path
    let game_id = match path.parse::<Uuid>() {
        Ok(id) => id,
        Err(_) => {
            return Ok(HttpResponse::BadRequest()
                .content_type("application/json")
                .json(json!({
                    "error": "Invalid game ID format"
                })));
        }
    };

    // Extract the card from the request
    let card = play_data.card.clone();

    // Execute the entire operation in a transaction with row locks
    let result = db
        .transaction(|txn| Box::pin(play_card_transaction(game_id, user.id, card.clone(), txn)))
        .await;

    match result {
        Ok(_) => Ok(HttpResponse::Ok()
            .content_type("application/json")
            .json(json!({
                "message": "Card played successfully"
            }))),
        Err(e) => Ok(HttpResponse::BadRequest()
            .content_type("application/json")
            .json(json!({
                "error": e.to_string()
            }))),
    }
}

/// Deal cards to players for a round
async fn deal_cards_to_players(
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
async fn calculate_round_scores(round_id: &Uuid, db: &DatabaseConnection) -> Result<(), String> {
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

        // Create round score record
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
async fn create_next_round(game_id: &Uuid, db: &DatabaseConnection) -> Result<(), String> {
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
async fn calculate_player_total_score(
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

        // Get the player's bid for this round
        let bid = match round_bids::Entity::find()
            .filter(round_bids::Column::RoundId.eq(round.id))
            .filter(round_bids::Column::PlayerId.eq(*player_id))
            .one(db)
            .await
        {
            Ok(Some(bid)) => bid.bid,
            Ok(None) => 0, // No bid for this round
            Err(_) => 0,   // Default to 0 on error
        };

        // Calculate points: 1 point per trick + 10 point bonus if bid matches tricks won
        let points = calculate_round_points(round_scores.tricks_won, bid);
        total_score += points;
    }

    Ok(total_score)
}

#[get("/game/{game_id}/summary")]
pub async fn get_game_summary(
    req: HttpRequest,
    path: web::Path<String>,
    db: web::Data<DatabaseConnection>,
) -> ActixResult<HttpResponse> {
    // Extract user from JWT authentication
    let user = match get_user(&req) {
        Some(user) => user,
        None => {
            return Ok(HttpResponse::Unauthorized()
                .content_type("application/json")
                .json(json!({
                    "error": "User not authenticated"
                })));
        }
    };

    // Parse game ID from path
    let game_id = match Uuid::parse_str(&path.into_inner()) {
        Ok(id) => id,
        Err(_) => {
            return Ok(HttpResponse::BadRequest()
                .content_type("application/json")
                .json(json!({
                    "error": "Invalid game ID format"
                })));
        }
    };

    // Fetch the game
    let game = match games::Entity::find_by_id(game_id).one(&**db).await {
        Ok(Some(game)) => game,
        Ok(None) => {
            return Ok(HttpResponse::NotFound()
                .content_type("application/json")
                .json(json!({
                    "error": "Game not found"
                })));
        }
        Err(e) => {
            return Ok(HttpResponse::InternalServerError()
                .content_type("application/json")
                .json(json!({
                    "error": "Failed to fetch game",
                    "details": e.to_string()
                })));
        }
    };

    // Only allow access to completed games
    if game.state != games::GameState::Completed {
        return Ok(HttpResponse::BadRequest()
            .content_type("application/json")
            .json(json!({
                "error": "Game is not completed. Use /api/game/{id}/state for active games."
            })));
    }

    // Check if user is a participant in this game
    let user_in_game = match game_players::Entity::find()
        .filter(game_players::Column::GameId.eq(game_id))
        .filter(game_players::Column::UserId.eq(user.id))
        .one(&**db)
        .await
    {
        Ok(Some(_)) => true,
        Ok(None) => false,
        Err(e) => {
            return Ok(HttpResponse::InternalServerError()
                .content_type("application/json")
                .json(json!({
                    "error": "Failed to check user participation",
                    "details": e.to_string()
                })));
        }
    };

    if !user_in_game {
        return Ok(HttpResponse::Forbidden()
            .content_type("application/json")
            .json(json!({
                "error": "Access denied. You are not a participant in this game."
            })));
    }

    // Fetch all game players for this game
    let game_players = match game_players::Entity::find()
        .filter(game_players::Column::GameId.eq(game_id))
        .all(&**db)
        .await
    {
        Ok(players) => players,
        Err(e) => {
            return Ok(HttpResponse::InternalServerError()
                .content_type("application/json")
                .json(json!({
                    "error": "Failed to fetch game players",
                    "details": e.to_string()
                })));
        }
    };

    // Fetch user details for all players and build PlayerSummary instances
    let mut players_with_details = Vec::new();
    for game_player in &game_players {
        let user = match users::Entity::find_by_id(game_player.user_id)
            .one(&**db)
            .await
        {
            Ok(Some(user)) => user,
            Ok(None) => continue, // Skip if user not found
            Err(_) => continue,   // Skip on error
        };

        let user_summary = UserSummary {
            id: user.id,
            email: user.email,
            name: user.name,
        };

        // Calculate total score for this player
        let final_score = (calculate_player_total_score(&game_player.id, &game_id, &db).await)
            .unwrap_or_default();

        let player_summary = PlayerSummary {
            id: game_player.id,
            user_id: game_player.user_id,
            turn_order: game_player.turn_order,
            is_ai: user.is_ai,
            final_score,
            rank: 0, // Will be set after sorting
            user: user_summary,
        };

        players_with_details.push(player_summary);
    }

    // Sort players by final score (descending) and assign ranks with tie support
    players_with_details.sort_by(|a, b| b.final_score.cmp(&a.final_score));

    // Assign ranks with tie support
    let mut current_rank = 1;
    let mut current_score = None;
    for player in &mut players_with_details {
        if let Some(score) = current_score {
            if player.final_score < score {
                current_rank += 1;
            }
        }
        player.rank = current_rank;
        current_score = Some(player.final_score);
    }

    // Sort players back by turn order for consistent display
    players_with_details.sort_by(|a, b| {
        let a_order = a.turn_order.unwrap_or(-1);
        let b_order = b.turn_order.unwrap_or(-1);
        a_order.cmp(&b_order)
    });

    // Build GameSummaryInfo
    let game_summary_info = GameSummaryInfo {
        id: game.id,
        state: game.state.to_string(),
        created_at: game.created_at,
        updated_at: game.updated_at,
        started_at: game.started_at,
        completed_at: game
            .completed_at
            .unwrap_or_else(|| chrono::Utc::now().into()),
    };

    // Fetch all rounds for round-by-round breakdown
    let all_rounds = match game_rounds::Entity::find()
        .filter(game_rounds::Column::GameId.eq(game_id))
        .order_by(game_rounds::Column::RoundNumber, Order::Asc)
        .all(&**db)
        .await
    {
        Ok(rounds) => rounds,
        Err(e) => {
            return Ok(HttpResponse::InternalServerError()
                .content_type("application/json")
                .json(json!({
                    "error": "Failed to fetch game rounds",
                    "details": e.to_string()
                })));
        }
    };

    // Build round-by-round breakdown
    let mut rounds_summary = Vec::new();
    for round in &all_rounds {
        // Fetch bids for this round
        let round_bids = (round_bids::Entity::find()
            .filter(round_bids::Column::RoundId.eq(round.id))
            .all(&**db)
            .await)
            .unwrap_or_default();

        // Fetch scores for this round
        let round_scores = (round_scores::Entity::find()
            .filter(round_scores::Column::RoundId.eq(round.id))
            .all(&**db)
            .await)
            .unwrap_or_default();

        // Build player results for this round
        let mut player_results = Vec::new();
        for player in &players_with_details {
            let bid = round_bids
                .iter()
                .find(|b| b.player_id == player.id)
                .map(|b| b.bid)
                .unwrap_or(0);

            let score = round_scores
                .iter()
                .find(|s| s.player_id == player.id)
                .map(|s| s.tricks_won)
                .unwrap_or(0);

            let bonus = has_exact_bid_bonus(score, bid) && bid > 0;
            let points = calculate_round_points(score, bid);

            player_results.push(PlayerRoundResult {
                player_id: player.id,
                bid,
                tricks_won: score,
                points,
                bonus,
            });
        }

        rounds_summary.push(RoundSummary {
            round_number: round.round_number,
            cards_dealt: round.cards_dealt,
            trump_suit: round.trump_suit.clone(),
            dealer_player_id: round.dealer_player_id,
            player_results,
        });
    }

    // Build final round summary (last round)
    let final_round = if let Some(last_round) = all_rounds.last() {
        let final_bids = (round_bids::Entity::find()
            .filter(round_bids::Column::RoundId.eq(last_round.id))
            .all(&**db)
            .await)
            .unwrap_or_default();

        let final_scores = (round_scores::Entity::find()
            .filter(round_scores::Column::RoundId.eq(last_round.id))
            .all(&**db)
            .await)
            .unwrap_or_default();

        let final_bid_summaries: Vec<RoundBidSummary> = final_bids
            .iter()
            .map(|bid| RoundBidSummary {
                player_id: bid.player_id,
                bid: bid.bid,
            })
            .collect();

        let final_score_summaries: Vec<RoundScoreSummary> = final_scores
            .iter()
            .map(|score| {
                let bid = final_bid_summaries
                    .iter()
                    .find(|b| b.player_id == score.player_id)
                    .map(|b| b.bid)
                    .unwrap_or(0);
                let points = calculate_round_points(score.tricks_won, bid);

                RoundScoreSummary {
                    player_id: score.player_id,
                    tricks_won: score.tricks_won,
                    bid,
                    points,
                }
            })
            .collect();

        FinalRoundSummary {
            round_number: last_round.round_number,
            cards_dealt: last_round.cards_dealt,
            trump_suit: last_round.trump_suit.clone(),
            dealer_player_id: last_round.dealer_player_id,
            bids: final_bid_summaries,
            tricks_won: final_score_summaries,
        }
    } else {
        return Ok(HttpResponse::InternalServerError()
            .content_type("application/json")
            .json(json!({
                "error": "No rounds found for completed game"
            })));
    };

    // Build GameSummary
    let game_summary = GameSummary {
        game: game_summary_info,
        players: players_with_details,
        rounds: rounds_summary,
        final_round,
    };

    Ok(HttpResponse::Ok()
        .content_type("application/json")
        .json(game_summary))
}

/// Perform AI card play action
async fn perform_ai_card_play(
    _game_id: Uuid,
    _player_id: Uuid,
    _play_request: PlayRequest,
    _db: &DatabaseConnection,
) -> Result<(), String> {
    // TODO: Implement AI card play logic
    // This function was temporarily removed during bidding refactor
    // and needs to be properly implemented
    Err("AI card play not yet implemented".to_string())
}

#[delete("/game/{game_id}")]
pub async fn delete_game(
    req: HttpRequest,
    path: web::Path<String>,
    db: web::Data<DatabaseConnection>,
) -> ActixResult<HttpResponse> {
    // Extract user from JWT authentication
    let user = match get_user(&req) {
        Some(user) => user,
        None => {
            return Ok(HttpResponse::Unauthorized()
                .content_type("application/json")
                .json(json!({
                    "error": "User not authenticated"
                })));
        }
    };

    // Parse game ID from path
    let game_id = match Uuid::parse_str(&path.into_inner()) {
        Ok(id) => id,
        Err(_) => {
            return Ok(HttpResponse::BadRequest()
                .content_type("application/json")
                .json(json!({
                    "error": "Invalid game ID format"
                })));
        }
    };

    // Check if user is a participant in the game
    let user_in_game = match game_players::Entity::find()
        .filter(game_players::Column::GameId.eq(game_id))
        .filter(game_players::Column::UserId.eq(user.id))
        .one(&**db)
        .await
    {
        Ok(Some(_)) => true,
        Ok(None) => false,
        Err(e) => {
            return Ok(HttpResponse::InternalServerError()
                .content_type("application/json")
                .json(json!({
                    "error": "Failed to check if user is in game",
                    "details": e.to_string()
                })));
        }
    };

    if !user_in_game {
        return Ok(HttpResponse::Forbidden()
            .content_type("application/json")
            .json(json!({
                "error": "User is not a participant in this game"
            })));
    }

    // Delete the game and all associated data
    // Due to foreign key constraints with CASCADE, deleting the game will automatically delete:
    // - game_players
    // - game_rounds
    // - round_bids
    // - round_tricks
    // - trick_plays
    // - round_scores
    // - round_hands

    match games::Entity::delete_by_id(game_id).exec(&**db).await {
        Ok(_) => Ok(HttpResponse::Ok()
            .content_type("application/json")
            .json(json!({
                "success": true,
                "message": "Game deleted successfully"
            }))),
        Err(e) => Ok(HttpResponse::InternalServerError()
            .content_type("application/json")
            .json(json!({
                "error": "Failed to delete game",
                "details": e.to_string()
            }))),
    }
}

/// Helper function to play a card within a transaction
async fn play_card_transaction(
    game_id: Uuid,
    user_id: Uuid,
    card: String,
    txn: &DatabaseTransaction,
) -> Result<(), String> {
    // Validate the card format first
    if !is_valid_card_format(&card) {
        return Err("Invalid card format. Use format like '5S', 'AH', 'KD'".to_string());
    }

    // Delegate to the tricks module for all trick logic
    crate::game_management::tricks::play_card(game_id, user_id, &card, txn).await
}

#[cfg(test)]
mod tests {

    // Bidding tests have been moved to bidding.rs
}
