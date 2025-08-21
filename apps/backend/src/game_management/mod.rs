//! Game management module
//!
//! This module contains the core game logic for the Nommie card game.

pub mod bidding;
pub mod rules;
pub mod scoring;
pub mod state;
pub mod tricks;

// Re-export commonly used rules constants and functions

use crate::game_management::rules::is_valid_card_format;
use crate::game_management::scoring::{calculate_round_points, has_exact_bid_bonus};
use crate::game_management::state::{
    build_game_snapshot, calculate_player_total_score, check_and_start_game,
};

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

use crate::dto::game_summary::{
    FinalRoundSummary, GameSummary, GameSummaryInfo, PlayerRoundResult, PlayerSummary,
    RoundBidSummary, RoundScoreSummary, RoundSummary, UserSummary,
};
use crate::dto::play_request::PlayRequest;
use crate::dto::trump_request::TrumpRequest;
use crate::entity::{game_players, game_rounds, games, round_bids, round_scores, users};
use crate::jwt::get_user;

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

    // Build game snapshot using the state module
    let game_snapshot = match build_game_snapshot(game, game_players, &db).await {
        Ok(snapshot) => snapshot,
        Err(e) => {
            return Ok(HttpResponse::InternalServerError()
                .content_type("application/json")
                .json(json!({
                    "error": "Failed to build game snapshot",
                    "details": e.to_string()
                })));
        }
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
#[allow(dead_code)]
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
