use actix_web::{get, post, web, HttpResponse, HttpRequest, Result as ActixResult};
use sea_orm::{DatabaseConnection, ActiveModelTrait, Set, EntityTrait, QueryFilter, ColumnTrait};
use sea_orm_migration::prelude::Query;
use serde_json::json;
use chrono::{Utc, DateTime, FixedOffset};
use uuid::Uuid;

use crate::entity::{games, game_players, users};
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
        current_turn: Set(None),
        created_at: Set(now),
        updated_at: Set(now),
        started_at: Set(None),
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
    let _game_player_result = match game_player.insert(&**db).await {
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

    // Return the created game and its players
    Ok(HttpResponse::Ok()
        .content_type("application/json")
        .json(json!({
            "game": game_result,
            "game_players": game_players
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
        let game_players: Vec<_> = all_game_players.iter()
            .filter(|gp| gp.game_id == game.id)
            .collect();
        
        let player_count = game_players.len();
        let is_player_in_game = game_players.iter().any(|gp| gp.user_id == user.id);
        
        games_list.push(json!({
            "id": game.id,
            "state": game.state,
            "player_count": player_count,
            "max_players": 4, // Assuming 4 players max for now
            "is_player_in_game": is_player_in_game
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
    let all_game_players = match game_players::Entity::find()
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

    // Only proceed if exactly 4 players are in the game
    if all_game_players.len() == 4 {
        // Check if all players are ready (AI players are automatically ready)
        // We need to fetch user details to check if they are AI
        let mut all_ready = true;
        for game_player in &all_game_players {
            let user = match users::Entity::find_by_id(game_player.user_id).one(&**db).await {
                Ok(Some(user)) => user,
                Ok(None) => {
                    all_ready = false;
                    break;
                }
                Err(_) => {
                    all_ready = false;
                    break;
                }
            };
            
            if !game_player.is_ready && !user.is_ai {
                all_ready = false;
                break;
            }
        }
        
        if all_ready {
            // Start the game
            let now: DateTime<FixedOffset> = Utc::now().into();
            let mut game_model: games::ActiveModel = game.into();
            game_model.state = Set(games::GameState::Started);
            game_model.started_at = Set(Some(now));
            game_model.updated_at = Set(now);

            match game_model.update(&**db).await {
                Ok(_) => {
                    return Ok(HttpResponse::Ok()
                        .content_type("application/json")
                        .json(json!({
                            "success": true,
                            "message": "Player marked as ready and game started",
                            "game_player": updated_game_player,
                            "game_started": true
                        })));
                }
                Err(e) => {
                    return Ok(HttpResponse::InternalServerError()
                        .content_type("application/json")
                        .json(json!({
                            "error": "Failed to start game",
                            "details": e.to_string()
                        })));
                }
            }
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
        .filter(users::Column::Id.not_in_subquery(
            Query::select()
                .column(game_players::Column::UserId)
                .from(game_players::Entity)
                .and_where(game_players::Column::GameId.eq(game_id))
                .to_owned()
        ))
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
        is_ready: Set(true), // AI players are automatically ready
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
    let updated_players = match game_players::Entity::find()
        .filter(game_players::Column::GameId.eq(game_id))
        .all(&**db)
        .await
    {
        Ok(players) => players,
        Err(e) => {
            return Ok(HttpResponse::InternalServerError()
                .content_type("application/json")
                .json(json!({
                    "error": "Failed to fetch updated game players",
                    "details": e.to_string()
                })));
        }
    };

    let game_started = if updated_players.len() == 4 {
        // Check if all players are ready (AI players are automatically ready)
        // We need to fetch user details to check if they are AI
        let mut all_ready = true;
        for game_player in &updated_players {
            let user = match users::Entity::find_by_id(game_player.user_id).one(&**db).await {
                Ok(Some(user)) => user,
                Ok(None) => {
                    all_ready = false;
                    break;
                }
                Err(_) => {
                    all_ready = false;
                    break;
                }
            };
            
            if !game_player.is_ready && !user.is_ai {
                all_ready = false;
                break;
            }
        }
        
        if all_ready {
            // Start the game
            let now: DateTime<FixedOffset> = Utc::now().into();
            let mut game_model: games::ActiveModel = game.into();
            game_model.state = Set(games::GameState::Started);
            game_model.started_at = Set(Some(now));
            game_model.updated_at = Set(now);

            match game_model.update(&**db).await {
                Ok(_) => true,
                Err(_) => false,
            }
        } else {
            false
        }
    } else {
        false
    };

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

    // Fetch user details for all players
    let mut players_with_details = Vec::new();
    for game_player in &game_players {
        let user = match users::Entity::find_by_id(game_player.user_id).one(&**db).await {
            Ok(Some(user)) => user,
            Ok(None) => continue, // Skip if user not found
            Err(_) => continue, // Skip on error
        };

        players_with_details.push(json!({
            "id": game_player.id,
            "user_id": game_player.user_id,
            "turn_order": game_player.turn_order,
            "is_ready": game_player.is_ready,
            "is_ai": user.is_ai,
            "user": {
                "id": user.id,
                "email": user.email,
                "name": user.name
            }
        }));
    }

    // Sort players by turn order
    players_with_details.sort_by(|a, b| {
        let a_order = a["turn_order"].as_i64().unwrap_or(-1);
        let b_order = b["turn_order"].as_i64().unwrap_or(-1);
        a_order.cmp(&b_order)
    });

    Ok(HttpResponse::Ok()
        .content_type("application/json")
        .json(json!({
            "game": {
                "id": game.id,
                "state": game.state,
                "current_turn": game.current_turn,
                "created_at": game.created_at,
                "updated_at": game.updated_at,
                "started_at": game.started_at
            },
            "players": players_with_details,
            "player_count": game_players.len(),
            "max_players": 4
        })))
} 