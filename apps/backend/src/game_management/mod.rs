//! Game management module
//!
//! This module contains the core game logic for the Nommie card game.

pub mod bidding;
pub mod rules;
pub mod scoring;
pub mod state;
pub mod tricks;

// Re-export commonly used rules constants and functions

use crate::game_management::rules::{
    calculate_cards_dealt, get_card_rank_value, is_trump_suit, is_valid_card_format,
    MAX_CARDS_PER_ROUND, PLAYER_COUNT, TOTAL_ROUNDS,
};
use crate::game_management::scoring::{calculate_round_points, has_exact_bid_bonus};

use actix_web::{delete, get, post, web, HttpRequest, HttpResponse, Result as ActixResult};
use chrono::{DateTime, FixedOffset, Utc};

use sea_orm::sea_query::{LockType, Query};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, DatabaseTransaction, EntityTrait, Order,
    PaginatorTrait, QueryFilter, QueryOrder, QuerySelect, Set, TransactionTrait,
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
                                if let Err(e) =
                                    perform_ai_bid(game_id, current_player.id, bid_request, &db)
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
                                if let Err(e) = perform_ai_trump_selection(
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
                // Get all bids for this round to find the highest bidder
                if let Ok(round_bids) = round_bids::Entity::find()
                    .filter(round_bids::Column::RoundId.eq(current_round.id))
                    .all(&**db)
                    .await
                {
                    // Find the highest bidder
                    let mut highest_bid = -1;
                    let mut highest_bidder_id = None;

                    for bid in &round_bids {
                        if bid.bid > highest_bid {
                            highest_bid = bid.bid;
                            highest_bidder_id = Some(bid.player_id);
                        }
                    }

                    if let Some(highest_bidder_id) = highest_bidder_id {
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
                                    if let Err(e) = perform_ai_trump_selection(
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
            // Fetch all bids for this round to determine the highest bidder
            let round_bids = (round_bids::Entity::find()
                .filter(round_bids::Column::RoundId.eq(round.id))
                .all(&**db)
                .await)
                .unwrap_or_default();

            // Find the highest bid and the player who bid first in case of ties
            let mut highest_bid = -1;
            let mut trump_chooser_id = None;
            let mut first_bid_time = None;

            for bid in &round_bids {
                if bid.bid > highest_bid {
                    highest_bid = bid.bid;
                    trump_chooser_id = Some(bid.player_id);
                    // For now, we'll use the first bid we encounter as the "first" one
                    first_bid_time = Some(bid.id);
                } else if bid.bid == highest_bid {
                    // In case of tie, the first bidder wins
                    // Since we don't have timestamps, we'll use the first one we encounter
                    if first_bid_time.is_none() {
                        trump_chooser_id = Some(bid.player_id);
                        first_bid_time = Some(bid.id);
                    }
                }
            }
            trump_chooser_id
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

/// Helper function to submit a bid within a transaction
async fn submit_bid_transaction(
    game_id: Uuid,
    user_id: Uuid,
    bid_value: i32,
    txn: &DatabaseTransaction,
) -> Result<(), String> {
    // Lock the game row for update to prevent concurrent modifications
    let game = match games::Entity::find_by_id(game_id)
        .lock(LockType::Update)
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

    // Validate that the game is in the Bidding phase
    if game.phase != games::GamePhase::Bidding {
        return Err("Game is not in bidding phase".to_string());
    }

    // Lock the current round row for update
    let current_round = match game_rounds::Entity::find()
        .filter(game_rounds::Column::GameId.eq(game_id))
        .order_by_desc(game_rounds::Column::RoundNumber)
        .lock(LockType::Update)
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

    // Check if this player has already bid in this round (idempotency check)
    let existing_bid = match round_bids::Entity::find()
        .filter(round_bids::Column::RoundId.eq(current_round.id))
        .filter(round_bids::Column::PlayerId.eq(current_player.id))
        .one(txn)
        .await
    {
        Ok(Some(_)) => true,
        Ok(None) => false,
        Err(e) => {
            return Err(format!("Failed to check existing bid: {e}"));
        }
    };

    if existing_bid {
        return Err("You have already submitted a bid for this round".to_string());
    }

    // Check if it's this player's turn to bid
    let current_turn = game.current_turn.unwrap_or(0);
    if current_player.turn_order.unwrap_or(-1) != current_turn {
        return Err("It's not your turn to bid".to_string());
    }

    // Save the bid to the round_bids table
    let bid_id = Uuid::new_v4();
    let round_bid = round_bids::ActiveModel {
        id: Set(bid_id),
        round_id: Set(current_round.id),
        player_id: Set(current_player.id),
        bid: Set(bid_value),
    };

    match round_bid.insert(txn).await {
        Ok(_) => (),
        Err(e) => {
            return Err(format!("Failed to save bid: {e}"));
        }
    }

    // Check if all players have bid in this round
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

    let round_bids = match round_bids::Entity::find()
        .filter(round_bids::Column::RoundId.eq(current_round.id))
        .all(txn)
        .await
    {
        Ok(bids) => bids,
        Err(e) => {
            return Err(format!("Failed to fetch round bids: {e}"));
        }
    };

    let all_bids_submitted = round_bids.len() == all_players.len();

    if all_bids_submitted {
        // Find the highest bidder
        let mut highest_bid = -1;

        for bid in &round_bids {
            if bid.bid > highest_bid {
                highest_bid = bid.bid;
            }
        }

        // Transition the game to TrumpSelection phase
        let game_update = games::ActiveModel {
            id: Set(game.id),
            state: Set(game.state),
            phase: Set(games::GamePhase::TrumpSelection),
            current_turn: Set(Some(0)), // Reset turn for trump selection
            created_at: Set(game.created_at),
            updated_at: Set(chrono::Utc::now().into()),
            started_at: Set(game.started_at),
            completed_at: Set(game.completed_at),
        };

        match game_update.update(txn).await {
            Ok(_) => (),
            Err(e) => {
                return Err(format!("Failed to transition game phase: {e}"));
            }
        }
    } else {
        // Move to next player's turn
        let next_turn = (current_turn + 1) % 4;

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
        .transaction(|txn| Box::pin(submit_bid_transaction(game_id, user.id, bid_value, txn)))
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

/// Helper function to submit trump selection within a transaction
async fn submit_trump_transaction(
    game_id: Uuid,
    user_id: Uuid,
    trump_suit: String,
    txn: &DatabaseTransaction,
) -> Result<(), String> {
    // Lock the game row for update to prevent concurrent modifications
    let game = match games::Entity::find_by_id(game_id)
        .lock(LockType::Update)
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

    // Validate that the game is in the TrumpSelection phase
    if game.phase != games::GamePhase::TrumpSelection {
        return Err("Game is not in trump selection phase".to_string());
    }

    // Lock the current round row for update
    let current_round = match game_rounds::Entity::find()
        .filter(game_rounds::Column::GameId.eq(game_id))
        .order_by_desc(game_rounds::Column::RoundNumber)
        .lock(LockType::Update)
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

    // Check if trump has already been selected for this round (idempotency check)
    if current_round.trump_suit.is_some() {
        return Err("Trump has already been selected for this round".to_string());
    }

    // Fetch all bids for this round to determine the highest bidder
    let round_bids = match round_bids::Entity::find()
        .filter(round_bids::Column::RoundId.eq(current_round.id))
        .all(txn)
        .await
    {
        Ok(bids) => bids,
        Err(e) => {
            return Err(format!("Failed to fetch round bids: {e}"));
        }
    };

    // Find the highest bid and the player who bid first in case of ties
    let mut highest_bid = -1;
    let mut trump_chooser_id = None;
    let mut first_bid_time = None;

    for bid in &round_bids {
        if bid.bid > highest_bid {
            highest_bid = bid.bid;
            trump_chooser_id = Some(bid.player_id);
            // For now, we'll use the first bid we encounter as the "first" one
            // In a real implementation, you might want to add a timestamp to round_bids
            first_bid_time = Some(bid.id);
        } else if bid.bid == highest_bid {
            // In case of tie, the first bidder wins
            // Since we don't have timestamps, we'll use the first one we encounter
            if first_bid_time.is_none() {
                trump_chooser_id = Some(bid.player_id);
                first_bid_time = Some(bid.id);
            }
        }
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

    // Validate that the current player is the designated trump chooser
    if current_player.id != trump_chooser_id.unwrap_or_default() {
        return Err("Only the highest bidder can choose the trump suit".to_string());
    }

    // Update the round with the selected trump suit
    let round_update = game_rounds::ActiveModel {
        id: Set(current_round.id),
        game_id: Set(current_round.game_id),
        round_number: Set(current_round.round_number),
        dealer_player_id: Set(current_round.dealer_player_id),
        trump_suit: Set(Some(trump_suit)),
        cards_dealt: Set(current_round.cards_dealt),
        created_at: Set(current_round.created_at),
    };

    match round_update.update(txn).await {
        Ok(_) => (),
        Err(e) => {
            return Err(format!("Failed to update round with trump suit: {e}"));
        }
    }

    // Transition the game to Playing phase
    let game_update = games::ActiveModel {
        id: Set(game.id),
        state: Set(game.state),
        phase: Set(games::GamePhase::Playing),
        current_turn: Set(Some(0)), // Reset turn for playing
        created_at: Set(game.created_at),
        updated_at: Set(chrono::Utc::now().into()),
        started_at: Set(game.started_at),
        completed_at: Set(game.completed_at),
    };

    match game_update.update(txn).await {
        Ok(_) => (),
        Err(e) => {
            return Err(format!("Failed to transition game phase: {e}"));
        }
    }

    Ok(())
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
            Box::pin(submit_trump_transaction(
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

// Helper function to determine trick winner
async fn determine_trick_winner(
    trick_id: &Uuid,
    trump_suit: &Option<String>,
    db: &DatabaseConnection,
) -> Result<Uuid, String> {
    // Fetch all plays for this trick
    let plays = match trick_plays::Entity::find()
        .filter(trick_plays::Column::TrickId.eq(*trick_id))
        .order_by(trick_plays::Column::PlayOrder, Order::Asc)
        .all(db)
        .await
    {
        Ok(plays) => plays,
        Err(_) => return Err("Failed to fetch trick plays".to_string()),
    };

    if plays.is_empty() {
        return Err("No plays found for trick".to_string());
    }

    // Get the lead suit (suit of the first card played)
    let lead_card = &plays[0].card;
    let lead_suit = &lead_card[1..2];

    let mut winning_play = &plays[0];
    let mut highest_rank = get_card_rank_value(&lead_card[0..1]);

    for play in &plays[1..] {
        let card = &play.card;
        let rank = &card[0..1];
        let suit = &card[1..2];

        let card_rank = get_card_rank_value(rank);
        let is_trump = trump_suit.as_ref().is_some_and(|trump| suit == trump);
        let is_lead_suit = suit == lead_suit;

        // Trump beats non-trump
        if ((is_trump && !is_lead_suit)
            || (is_lead_suit && !is_trump_suit(&winning_play.card[1..2], trump_suit)))
            && (!is_trump_suit(&winning_play.card[1..2], trump_suit) || card_rank > highest_rank)
        {
            winning_play = play;
            highest_rank = card_rank;
        }
    }

    Ok(winning_play.player_id)
}

/// Create a standard 52-card deck and shuffle it
fn create_shuffled_deck() -> Vec<String> {
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
    use rand::seq::SliceRandom;
    let mut rng = rand::thread_rng();
    deck.shuffle(&mut rng);

    deck
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

#[cfg(test)]
mod tests {
    use crate::game_management::rules::{get_card_rank_value, is_trump_suit};

    // Test 1: Bidding rules
    #[test]
    fn test_bidding_rules_highest_bid_chooses_trump() {
        // Test that highest bidder becomes trump chooser

        // Create test bids with different values
        let bids = [3, 7, 2, 5];
        let mut highest_bid = -1;
        let mut highest_bidder_index = 0;

        for (i, &bid) in bids.iter().enumerate() {
            if bid > highest_bid {
                highest_bid = bid;
                highest_bidder_index = i;
            }
        }

        // Verify highest bidder (index 1 with bid 7) is selected
        assert_eq!(highest_bidder_index, 1);
        assert_eq!(highest_bid, 7);
    }

    #[test]
    fn test_bidding_rules_tie_resolution() {
        // Test tie resolution by "first highest in turn order"
        let bids = [5, 5, 3, 2]; // Two players bid 5

        // Simulate the logic from the production code
        let mut highest_bid = -1;
        let mut trump_chooser_index = 0;
        let mut first_bid_time = None;

        for (i, &bid) in bids.iter().enumerate() {
            if bid > highest_bid {
                highest_bid = bid;
                trump_chooser_index = i;
                first_bid_time = Some(i); // Use index as "time" for deterministic testing
            } else if bid == highest_bid {
                // In case of tie, the first bidder wins
                if first_bid_time.is_none() {
                    trump_chooser_index = i;
                    first_bid_time = Some(i);
                }
            }
        }

        // First player with bid 5 (index 0) should win the tie
        assert_eq!(trump_chooser_index, 0);
        assert_eq!(highest_bid, 5);
    }

    // Test 2: Trick resolution basics
    #[test]
    fn test_trick_resolution_highest_lead_suit_wins() {
        // Test that highest card in lead suit wins when no trump is played
        let trump_suit = None;
        let lead_suit = "H"; // Hearts

        // Create test plays: 7H, KH, 2H, 9H (all hearts, no trump)
        let plays = [
            ("7H", 0), // 7 of hearts, play order 0
            ("KH", 1), // King of hearts, play order 1
            ("2H", 2), // 2 of hearts, play order 2
            ("9H", 3), // 9 of hearts, play order 3
        ];

        // King of hearts should win (rank 13)
        let mut winning_play = &plays[0];
        let mut highest_rank = get_card_rank_value(&plays[0].0[0..1]);

        for play in &plays[1..] {
            let card = &play.0;
            let rank = &card[0..1];
            let suit = &card[1..2];

            let card_rank = get_card_rank_value(rank);
            let is_trump = trump_suit
                .as_ref()
                .is_some_and(|trump: &String| suit == trump);
            let is_lead_suit = suit == lead_suit;

            // Trump beats non-trump, but here no trump is played
            if is_lead_suit && !is_trump && card_rank > highest_rank {
                winning_play = play;
                highest_rank = card_rank;
            }
        }

        // King (rank 13) should win over 9, 7, 2
        assert_eq!(winning_play.0, "KH");
        assert_eq!(highest_rank, 13);
    }

    #[test]
    fn test_trick_resolution_trump_beats_lead_suit() {
        // Test that any trump beats non-trump cards
        let trump_suit = Some("S".to_string()); // Spades is trump
        let lead_suit = "H"; // Hearts is lead suit

        // Create test plays: AH (ace hearts), 2S (2 spades), 7H (7 hearts), KS (king spades)
        let plays = [
            ("AH", 0), // Ace of hearts, play order 0
            ("2S", 1), // 2 of spades (trump), play order 1
            ("7H", 2), // 7 of hearts, play order 2
            ("KS", 3), // King of spades (trump), play order 3
        ];

        // King of spades should win (highest trump)
        let mut winning_play = &plays[0];
        let mut highest_rank = get_card_rank_value(&plays[0].0[0..1]);

        for play in &plays[1..] {
            let card = &play.0;
            let rank = &card[0..1];
            let suit = &card[1..2];

            let card_rank = get_card_rank_value(rank);
            let is_trump = trump_suit
                .as_ref()
                .is_some_and(|trump: &String| suit == trump);
            let is_lead_suit = suit == lead_suit;

            // Trump beats non-trump
            if ((is_trump && !is_lead_suit) || (is_lead_suit && !is_trump_suit(suit, &trump_suit)))
                && (!is_trump_suit(&winning_play.0[1..2], &trump_suit) || card_rank > highest_rank)
            {
                winning_play = play;
                highest_rank = card_rank;
            }
        }

        // King of spades (trump) should win over ace of hearts (lead suit)
        assert_eq!(winning_play.0, "KS");
        assert_eq!(highest_rank, 13);
    }

    #[test]
    fn test_trick_resolution_cannot_follow_suit() {
        // Test case where some players can't follow suit
        let trump_suit = Some("D".to_string()); // Diamonds is trump
        let lead_suit = "H"; // Hearts is lead suit

        // Create test plays: 7H (7 hearts), 2D (2 diamonds - trump), 9C (9 clubs - can't follow), 3D (3 diamonds - trump)
        let plays = [
            ("7H", 0), // 7 of hearts, play order 0
            ("2D", 1), // 2 of diamonds (trump), play order 1
            ("9C", 2), // 9 of clubs (can't follow suit), play order 2
            ("3D", 3), // 3 of diamonds (trump), play order 3
        ];

        // 3 of diamonds should win (highest trump)
        let mut winning_play = &plays[0];
        let mut highest_rank = get_card_rank_value(&plays[0].0[0..1]);

        for play in &plays[1..] {
            let card = &play.0;
            let rank = &card[0..1];
            let suit = &card[1..2];

            let card_rank = get_card_rank_value(rank);
            let is_trump = trump_suit
                .as_ref()
                .is_some_and(|trump: &String| suit == trump);
            let is_lead_suit = suit == lead_suit;

            // Trump beats non-trump
            if ((is_trump && !is_lead_suit) || (is_lead_suit && !is_trump_suit(suit, &trump_suit)))
                && (!is_trump_suit(&winning_play.0[1..2], &trump_suit) || card_rank > highest_rank)
            {
                winning_play = play;
                highest_rank = card_rank;
            }
        }

        // 3 of diamonds (trump) should win over 2 of diamonds (trump) and 7 of hearts (lead suit)
        assert_eq!(winning_play.0, "3D");
        assert_eq!(highest_rank, 3);
    }

    // Helper function to test card rank calculation
    #[test]
    fn test_card_rank_values() {
        // Test that card ranks are calculated correctly
        assert_eq!(get_card_rank_value("2"), 2);
        assert_eq!(get_card_rank_value("T"), 10);
        assert_eq!(get_card_rank_value("J"), 11);
        assert_eq!(get_card_rank_value("Q"), 12);
        assert_eq!(get_card_rank_value("K"), 13);
        assert_eq!(get_card_rank_value("A"), 14);
    }

    // Helper function to test trump suit checking
    #[test]
    fn test_trump_suit_checking() {
        // Test trump suit identification
        let trump_suit = Some("H".to_string());

        assert!(is_trump_suit("H", &trump_suit));
        assert!(!is_trump_suit("S", &trump_suit));
        assert!(!is_trump_suit("D", &trump_suit));
        assert!(!is_trump_suit("C", &trump_suit));

        // Test with no trump
        let no_trump = None;
        assert!(!is_trump_suit("H", &no_trump));
        assert!(!is_trump_suit("S", &no_trump));
    }
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

// AI Helper Functions

/// Perform AI bidding action
async fn perform_ai_bid(
    game_id: Uuid,
    player_id: Uuid,
    bid_request: BidRequest,
    db: &DatabaseConnection,
) -> Result<(), String> {
    // Validate bid value (0-13)
    let bid_value = bid_request.bid;
    if !(0..=13).contains(&bid_value) {
        println!("[ERROR] perform_ai_bid: Invalid bid value: {bid_value}");
        return Err("Bid must be between 0 and 13".to_string());
    }

    // Fetch the game
    let game = match games::Entity::find_by_id(game_id).one(db).await {
        Ok(Some(game)) => game,
        Ok(None) => {
            println!("[ERROR] perform_ai_bid: Game not found: {game_id}");
            return Err("Game not found".to_string());
        }
        Err(e) => {
            println!("[ERROR] perform_ai_bid: Failed to fetch game: {e}");
            return Err(format!("Failed to fetch game: {e}"));
        }
    };

    // Validate that the game is in the Bidding phase
    if game.phase != games::GamePhase::Bidding {
        return Err("Game is not in bidding phase".to_string());
    }

    // Find the current round for this game (latest round)
    let current_round = match game_rounds::Entity::find()
        .filter(game_rounds::Column::GameId.eq(game_id))
        .order_by_desc(game_rounds::Column::RoundNumber)
        .one(db)
        .await
    {
        Ok(Some(round)) => round,
        Ok(None) => {
            println!("[ERROR] perform_ai_bid: No current round found for game: {game_id}",);
            return Err("No current round found".to_string());
        }
        Err(e) => {
            println!("[ERROR] perform_ai_bid: Failed to fetch current round: {e}",);
            return Err(format!("Failed to fetch current round: {e}"));
        }
    };

    // Check if this player has already bid in this round
    let existing_bid = match round_bids::Entity::find()
        .filter(round_bids::Column::RoundId.eq(current_round.id))
        .filter(round_bids::Column::PlayerId.eq(player_id))
        .one(db)
        .await
    {
        Ok(Some(_)) => true,
        Ok(None) => false,
        Err(e) => {
            println!("[ERROR] perform_ai_bid: Failed to check existing bid: {e}",);
            return Err(format!("Failed to check existing bid: {e}"));
        }
    };

    if existing_bid {
        return Err("Player has already submitted a bid for this round".to_string());
    }

    // Check if it's this player's turn to bid
    let current_turn = game.current_turn.unwrap_or(0);
    let current_player = match game_players::Entity::find()
        .filter(game_players::Column::GameId.eq(game_id))
        .filter(game_players::Column::Id.eq(player_id))
        .one(db)
        .await
    {
        Ok(Some(player)) => player,
        Ok(None) => {
            println!("[ERROR] perform_ai_bid: Player not found: {player_id}");
            return Err("Player not found".to_string());
        }
        Err(e) => {
            println!("[ERROR] perform_ai_bid: Failed to fetch player data: {e}");
            return Err(format!("Failed to fetch player data: {e}"));
        }
    };

    if current_player.turn_order.unwrap_or(-1) != current_turn {
        return Err("It's not this player's turn to bid".to_string());
    }

    // Save the bid to the round_bids table
    let bid_id = Uuid::new_v4();
    let round_bid = round_bids::ActiveModel {
        id: Set(bid_id),
        round_id: Set(current_round.id),
        player_id: Set(player_id),
        bid: Set(bid_value),
    };

    match round_bid.insert(db).await {
        Ok(_) => (),
        Err(e) => {
            return Err(format!("Failed to save bid: {e}"));
        }
    }

    // Check if all players have bid in this round
    let all_players = match game_players::Entity::find()
        .filter(game_players::Column::GameId.eq(game_id))
        .all(db)
        .await
    {
        Ok(players) => players,
        Err(e) => {
            println!("[ERROR] perform_ai_bid: Failed to fetch all players: {e}");
            return Err(format!("Failed to fetch all players: {e}"));
        }
    };

    let round_bids = match round_bids::Entity::find()
        .filter(round_bids::Column::RoundId.eq(current_round.id))
        .all(db)
        .await
    {
        Ok(bids) => bids,
        Err(e) => {
            println!("[ERROR] perform_ai_bid: Failed to fetch round bids: {e}");
            return Err(format!("Failed to fetch round bids: {e}"));
        }
    };

    let all_bids_submitted = round_bids.len() == all_players.len();

    if all_bids_submitted {
        // Transition the game to TrumpSelection phase
        let game_update = games::ActiveModel {
            id: Set(game.id),
            state: Set(game.state),
            phase: Set(games::GamePhase::TrumpSelection),
            current_turn: Set(Some(0)), // Reset turn for trump selection
            created_at: Set(game.created_at),
            updated_at: Set(chrono::Utc::now().into()),
            started_at: Set(game.started_at),
            completed_at: Set(game.completed_at),
        };

        match game_update.update(db).await {
            Ok(_) => (),
            Err(e) => {
                return Err(format!("Failed to transition game phase: {e}"));
            }
        }
    } else {
        // Move to next player's turn
        let next_turn = (current_turn + 1) % 4;

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

        match game_update.update(db).await {
            Ok(_) => (),
            Err(e) => {
                return Err(format!("Failed to update turn: {e}"));
            }
        }
    }

    Ok(())
}

/// Perform AI trump selection action
async fn perform_ai_trump_selection(
    game_id: Uuid,
    player_id: Uuid,
    trump_request: TrumpRequest,
    db: &DatabaseConnection,
) -> Result<(), String> {
    // Validate trump suit
    let trump_suit = &trump_request.trump_suit;
    let valid_suits = ["Spades", "Hearts", "Diamonds", "Clubs", "NoTrump"];
    if !valid_suits.contains(&trump_suit.as_str()) {
        println!("[ERROR] perform_ai_trump_selection: Invalid trump suit: {trump_suit}",);
        return Err(
            "Invalid trump suit. Must be one of: Spades, Hearts, Diamonds, Clubs, NoTrump"
                .to_string(),
        );
    }

    // Fetch the game
    let game = match games::Entity::find_by_id(game_id).one(db).await {
        Ok(Some(game)) => game,
        Ok(None) => {
            return Err("Game not found".to_string());
        }
        Err(e) => {
            return Err(format!("Failed to fetch game: {e}"));
        }
    };

    // Validate that the game is in the TrumpSelection phase
    if game.phase != games::GamePhase::TrumpSelection {
        return Err("Game is not in trump selection phase".to_string());
    }

    // Fetch the current round for this game (latest round)
    let current_round = match game_rounds::Entity::find()
        .filter(game_rounds::Column::GameId.eq(game_id))
        .order_by_desc(game_rounds::Column::RoundNumber)
        .one(db)
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

    // Check if trump has already been selected for this round
    if current_round.trump_suit.is_some() {
        return Err("Trump has already been selected for this round".to_string());
    }

    // Fetch all bids for this round to determine the highest bidder
    let round_bids = match round_bids::Entity::find()
        .filter(round_bids::Column::RoundId.eq(current_round.id))
        .all(db)
        .await
    {
        Ok(bids) => bids,
        Err(e) => {
            return Err(format!("Failed to fetch round bids: {e}"));
        }
    };

    // Find the highest bid and the player who bid first in case of ties
    let mut highest_bid = -1;
    let mut trump_chooser_id = None;
    let mut first_bid_time = None;

    for bid in &round_bids {
        if bid.bid > highest_bid {
            highest_bid = bid.bid;
            trump_chooser_id = Some(bid.player_id);
            first_bid_time = Some(bid.id);
        } else if bid.bid == highest_bid {
            // In case of tie, the first bidder wins
            if first_bid_time.is_none() {
                trump_chooser_id = Some(bid.player_id);
                first_bid_time = Some(bid.id);
            }
        }
    }

    // Validate that the current player is the designated trump chooser
    if player_id != trump_chooser_id.unwrap_or_default() {
        return Err("Only the highest bidder can choose the trump suit".to_string());
    }

    // Update the round with the trump suit
    let round_update = game_rounds::ActiveModel {
        id: Set(current_round.id),
        game_id: Set(current_round.game_id),
        round_number: Set(current_round.round_number),
        dealer_player_id: Set(current_round.dealer_player_id),
        trump_suit: Set(Some(trump_suit.clone())),
        cards_dealt: Set(current_round.cards_dealt),
        created_at: Set(current_round.created_at),
    };

    match round_update.update(db).await {
        Ok(_) => (),
        Err(e) => {
            return Err(format!("Failed to update round with trump suit: {e}"));
        }
    }

    // Transition the game to Playing phase
    let game_update = games::ActiveModel {
        id: Set(game.id),
        state: Set(game.state),
        phase: Set(games::GamePhase::Playing),
        current_turn: Set(Some(0)), // Reset turn for playing phase
        created_at: Set(game.created_at),
        updated_at: Set(chrono::Utc::now().into()),
        started_at: Set(game.started_at),
        completed_at: Set(game.completed_at),
    };

    match game_update.update(db).await {
        Ok(_) => (),
        Err(e) => {
            return Err(format!("Failed to transition game to playing phase: {e}"));
        }
    }

    Ok(())
}

/// Perform AI card play action
async fn perform_ai_card_play(
    game_id: Uuid,
    player_id: Uuid,
    play_request: PlayRequest,
    db: &DatabaseConnection,
) -> Result<(), String> {
    // Fetch the game
    let game = match games::Entity::find_by_id(game_id).one(db).await {
        Ok(Some(game)) => game,
        Ok(None) => return Err("Game not found".to_string()),
        Err(e) => return Err(format!("Failed to fetch game: {e}")),
    };

    // Validate that the game is in the Playing phase
    if game.phase != games::GamePhase::Playing {
        return Err("Game is not in playing phase".to_string());
    }

    // Fetch the current player's game_player record
    let current_player = match game_players::Entity::find()
        .filter(game_players::Column::GameId.eq(game_id))
        .filter(game_players::Column::Id.eq(player_id))
        .one(db)
        .await
    {
        Ok(Some(player)) => player,
        Ok(None) => return Err("Player not found".to_string()),
        Err(e) => return Err(format!("Failed to fetch player data: {e}")),
    };

    // Fetch the current round
    let current_round = match game_rounds::Entity::find()
        .filter(game_rounds::Column::GameId.eq(game_id))
        .order_by_desc(game_rounds::Column::RoundNumber)
        .one(db)
        .await
    {
        Ok(Some(round)) => round,
        Ok(None) => return Err("No current round found".to_string()),
        Err(e) => return Err(format!("Failed to fetch current round: {e}")),
    };

    // Get or create the current trick
    let current_trick = match round_tricks::Entity::find()
        .filter(round_tricks::Column::RoundId.eq(current_round.id))
        .order_by_desc(round_tricks::Column::TrickNumber)
        .one(db)
        .await
    {
        Ok(Some(trick)) => {
            // Check if this trick is complete (has 4 plays)
            let trick_plays_count = match trick_plays::Entity::find()
                .filter(trick_plays::Column::TrickId.eq(trick.id))
                .count(db)
                .await
            {
                Ok(count) => count,
                Err(_) => return Err("Failed to count trick plays".to_string()),
            };

            if trick_plays_count >= 4 {
                // Create a new trick
                let new_trick_number = trick.trick_number + 1;
                let new_trick = round_tricks::ActiveModel {
                    id: Set(Uuid::new_v4()),
                    round_id: Set(current_round.id),
                    trick_number: Set(new_trick_number),
                    winner_player_id: Set(None),
                    created_at: Set(chrono::Utc::now().into()),
                };

                match new_trick.insert(db).await {
                    Ok(inserted_trick) => inserted_trick,
                    Err(e) => return Err(format!("Failed to create new trick: {e}")),
                }
            } else {
                trick
            }
        }
        Ok(None) => {
            // Create the first trick
            let first_trick = round_tricks::ActiveModel {
                id: Set(Uuid::new_v4()),
                round_id: Set(current_round.id),
                trick_number: Set(1),
                winner_player_id: Set(None),
                created_at: Set(chrono::Utc::now().into()),
            };

            match first_trick.insert(db).await {
                Ok(inserted_trick) => inserted_trick,
                Err(e) => return Err(format!("Failed to create first trick: {e}")),
            }
        }
        Err(e) => return Err(format!("Failed to fetch current trick: {e}")),
    };

    // Check if it's the current player's turn
    let current_turn = game.current_turn.unwrap_or(0);
    if current_player.turn_order.unwrap_or(-1) != current_turn {
        return Err("It's not this player's turn to play".to_string());
    }

    // Validate the card format (e.g., "5S", "AH", "KD")
    let card = &play_request.card;
    if !is_valid_card_format(card) {
        return Err("Invalid card format. Use format like '5S', 'AH', 'KD'".to_string());
    }

    // Get the play order for this trick
    let play_order = match trick_plays::Entity::find()
        .filter(trick_plays::Column::TrickId.eq(current_trick.id))
        .count(db)
        .await
    {
        Ok(count) => count as i32 + 1,
        Err(_) => return Err("Failed to determine play order".to_string()),
    };

    // Store the card play
    let trick_play = trick_plays::ActiveModel {
        id: Set(Uuid::new_v4()),
        trick_id: Set(current_trick.id),
        player_id: Set(player_id),
        card: Set(card.clone()),
        play_order: Set(play_order),
    };

    match trick_play.insert(db).await {
        Ok(_) => (),
        Err(e) => return Err(format!("Failed to store card play: {e}")),
    }

    // Check if this was the 4th card played
    if play_order == 4 {
        // Determine the winner of the trick
        let winner_player_id =
            match determine_trick_winner(&current_trick.id, &current_round.trump_suit, db).await {
                Ok(winner_id) => winner_id,
                Err(e) => return Err(format!("Failed to determine trick winner: {e}")),
            };

        // Update the trick with the winner
        let mut trick_update: round_tricks::ActiveModel = current_trick.into();
        trick_update.winner_player_id = Set(Some(winner_player_id));
        match trick_update.update(db).await {
            Ok(_) => (),
            Err(e) => return Err(format!("Failed to update trick winner: {e}")),
        }

        // Check if this was the last trick of the round
        let total_tricks_in_round = match round_tricks::Entity::find()
            .filter(round_tricks::Column::RoundId.eq(current_round.id))
            .count(db)
            .await
        {
            Ok(count) => count,
            Err(_) => return Err("Failed to count tricks in round".to_string()),
        };

        // Check if we've played all tricks for this round (based on cards_dealt)
        if total_tricks_in_round >= current_round.cards_dealt as u64 {
            // Calculate scores for the round
            if let Err(e) = calculate_round_scores(&current_round.id, db).await {
                return Err(format!("Failed to calculate round scores: {e}"));
            }

            // Create the next round
            if let Err(e) = create_next_round(&game_id, db).await {
                return Err(format!("Failed to create next round: {e}"));
            }

            // Transition back to bidding phase
            let game_update = games::ActiveModel {
                id: Set(game.id),
                state: Set(game.state),
                phase: Set(games::GamePhase::Bidding),
                current_turn: Set(None),
                created_at: Set(game.created_at),
                updated_at: Set(chrono::Utc::now().into()),
                started_at: Set(game.started_at),
                completed_at: Set(game.completed_at),
            };

            match game_update.update(db).await {
                Ok(_) => (),
                Err(e) => return Err(format!("Failed to transition to bidding phase: {e}")),
            }
        } else {
            // Start next trick with the winner leading
            let next_turn = match game_players::Entity::find()
                .filter(game_players::Column::GameId.eq(game_id))
                .filter(game_players::Column::Id.eq(winner_player_id))
                .one(db)
                .await
            {
                Ok(Some(winner_player)) => winner_player.turn_order.unwrap_or(0),
                _ => 0,
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

            match game_update.update(db).await {
                Ok(_) => (),
                Err(e) => return Err(format!("Failed to update turn order: {e}")),
            }
        }
    } else {
        // Move to next player's turn
        let next_turn = (current_turn + 1) % 4;
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

        match game_update.update(db).await {
            Ok(_) => (),
            Err(e) => return Err(format!("Failed to update turn order: {e}")),
        }
    }

    Ok(())
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

    // Lock the game row for update to prevent concurrent modifications
    let game = match games::Entity::find_by_id(game_id)
        .lock(LockType::Update)
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

    // Lock the current round row for update
    let current_round = match game_rounds::Entity::find()
        .filter(game_rounds::Column::GameId.eq(game_id))
        .order_by_desc(game_rounds::Column::RoundNumber)
        .lock(LockType::Update)
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

    // Check if it's this player's turn to play
    let current_turn = game.current_turn.unwrap_or(0);
    if current_player.turn_order.unwrap_or(-1) != current_turn {
        return Err("It's not your turn to play".to_string());
    }

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
    let lead_suit = if is_first_play {
        None
    } else {
        // Extract suit from the first card played
        let first_card = &trick_plays[0].card;
        if first_card.ends_with("s") {
            Some(first_card[..first_card.len() - 1].to_string())
        } else {
            Some(first_card.to_string())
        }
    };

    // Enforce follow-suit rule if not the first play
    if let Some(lead_suit_ref) = lead_suit.as_ref() {
        let card_suit = if card.ends_with("s") {
            &card[..card.len() - 1]
        } else {
            &card
        };

        if card_suit != lead_suit_ref {
            // Check if player has any cards of the lead suit
            let has_lead_suit = player_hand.iter().any(|h| {
                let h_suit = if h.card.ends_with("s") {
                    &h.card[..h.card.len() - 1]
                } else {
                    &h.card
                };
                h_suit == lead_suit_ref
            });

            if has_lead_suit {
                return Err("You must follow suit if possible".to_string());
            }
        }
    }

    // Record the card play
    let play_id = Uuid::new_v4();
    let play_order = trick_plays.len() as i32;
    let trick_play = trick_plays::ActiveModel {
        id: Set(play_id),
        trick_id: Set(current_trick.id),
        player_id: Set(current_player.id),
        card: Set(card.clone()),
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
        .filter(round_hands::Column::Card.eq(card.clone()))
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

    // Check if all players have played in this trick
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

    let all_played = trick_plays.len() + 1 == all_players.len(); // +1 for the current play

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
        let next_turn = (current_turn + 1) % PLAYER_COUNT as i32;

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
