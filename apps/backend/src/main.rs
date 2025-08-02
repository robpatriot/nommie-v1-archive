use actix_web::{get, App, HttpServer, Responder, web, HttpResponse, Result as ActixResult, HttpRequest};
use actix_cors::Cors;
use dotenv::dotenv;
use sea_orm::{Database, DatabaseConnection};
use std::env;
use serde_json::json;

mod entity;
mod jwt;
mod user_management;
mod game_management;
mod dto;

use migration::Migrator;
use migration::MigratorTrait;
use jwt::{JwtAuth, get_claims, get_user};
use game_management::{create_game, get_games, mark_player_ready, add_ai_player, join_game, get_game_state, submit_bid};

//extern crate sea_query;

#[get("/")]
async fn hello() -> impl Responder {
    "Hello, Nommie!"
}

#[get("/protected")]
async fn protected_route(req: HttpRequest) -> ActixResult<HttpResponse> {
    // Extract claims and user from the request (set by JWT middleware)
    if let Some(claims) = get_claims(&req) {
        if let Some(user) = get_user(&req) {
            Ok(HttpResponse::Ok()
                .content_type("application/json")
                .json(json!({
                    "message": "Access granted to protected route",
                    "user": {
                        "id": user.id,
                        "external_id": user.external_id,
                        "email": user.email,
                        "name": user.name,
                        "created_at": user.created_at
                    },
                    "token_info": {
                        "sub": claims.sub,
                        "email": claims.email,
                        "issued_at": claims.iat,
                        "expires_at": claims.exp
                    }
                })))
        } else {
            Ok(HttpResponse::InternalServerError()
                .content_type("application/json")
                .json(json!({
                    "error": "User not found in request"
                })))
        }
    } else {
        // This should never happen if middleware is working correctly
        Ok(HttpResponse::Unauthorized()
            .content_type("application/json")
            .json(json!({
                "error": "No claims found"
            })))
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Load environment variables from .env file
    dotenv().ok();
    
    // Get database URL from environment
    let database_url = env::var("DATABASE_URL").unwrap_or_else(|_| {
        println!("Warning: DATABASE_URL not set, using default");
        "postgres://nommie_user:pineconescamping@localhost:5432/nommie".to_string()
    });
    
    println!("Starting Nommie backend server...");
    println!("Database URL: {}", database_url);
    
    // Connect to database
    let db: DatabaseConnection = Database::connect(&database_url)
        .await
        .expect("Failed to connect to database");
    
    println!("Connected to database successfully!");
    
    // Run migrations
    Migrator::up(&db, None)
        .await
        .expect("Failed to run migrations");
    
    println!("Database migrations completed successfully!");
    
    // Start the HTTP server
    HttpServer::new(move || {
        // Configure CORS
        let frontend_origin = env::var("CORS_ALLOWED_ORIGIN").unwrap_or_else(|_| {
            println!("Warning: CORS_ALLOWED_ORIGIN not set, using default");
            "http://localhost:3000".to_string()
        });
        
        let cors = Cors::default()
            .allowed_origin(&frontend_origin)
            .allowed_methods(vec!["GET", "POST", "PUT", "DELETE", "OPTIONS"])
            .allowed_headers(vec![http::header::AUTHORIZATION, http::header::ACCEPT])
            .allowed_header(http::header::CONTENT_TYPE)
            .supports_credentials()
            .max_age(3600);

        App::new()
            .wrap(cors)
            .app_data(web::Data::new(db.clone()))
            .service(hello)
            .service(web::scope("/api")
                .wrap(JwtAuth::new(db.clone()))
                .service(protected_route)
                .service(create_game)
                .service(get_games)
                .service(mark_player_ready)
                .service(add_ai_player)
                .service(join_game)
                .service(get_game_state)
                .service(submit_bid)
                )
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}
