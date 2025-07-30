use actix_web::{get, post, App, HttpServer, Responder, web, HttpResponse, Result as ActixResult, HttpRequest};
use actix_cors::Cors;
use dotenv::dotenv;
use sea_orm::{Database, DatabaseConnection, EntityTrait, DbErr};
use std::env;
use serde_json::json;

mod entity;
mod jwt;

use migration::Migrator;
use migration::MigratorTrait;
use entity::users::{Entity as Users, Model as User};
use jwt::{JwtAuth, get_claims};

#[get("/")]
async fn hello() -> impl Responder {
    "Hello, Nommie!"
}

#[get("/users")]
async fn get_users(db: web::Data<DatabaseConnection>) -> ActixResult<HttpResponse> {
    // Fetch all users from the database
    let users: Result<Vec<User>, DbErr> = Users::find()
        .all(db.get_ref())
        .await;
    
    match users {
        Ok(users) => {
            // Convert users to JSON and return success response
            let users_json = json!({
                "users": users,
                "count": users.len()
            });
            
            Ok(HttpResponse::Ok()
                .content_type("application/json")
                .json(users_json))
        }
        Err(e) => {
            // Log the error and return an error response
            eprintln!("Database error: {}", e);
            Ok(HttpResponse::InternalServerError()
                .content_type("application/json")
                .json(json!({
                    "error": "Failed to fetch users",
                    "message": "Database query failed"
                })))
        }
    }
}

#[post("/login")]
async fn login() -> ActixResult<HttpResponse> {
    // In a real application, you would validate credentials here
    // For demo purposes, we'll create a token for a test user
    match JwtAuth::create_token("test-user-123") {
        Ok(token) => {
            Ok(HttpResponse::Ok()
                .content_type("application/json")
                .json(json!({
                    "token": token,
                    "message": "Login successful"
                })))
        }
        Err(_) => {
            Ok(HttpResponse::InternalServerError()
                .content_type("application/json")
                .json(json!({
                    "error": "Failed to create token"
                })))
        }
    }
}

#[get("/protected")]
async fn protected_route(req: HttpRequest) -> ActixResult<HttpResponse> {
    // Extract claims from the request (set by JWT middleware)
    if let Some(claims) = get_claims(&req) {
        Ok(HttpResponse::Ok()
            .content_type("application/json")
            .json(json!({
                "message": "Access granted to protected route",
                "user_id": claims.sub,
                "issued_at": claims.iat,
                "expires_at": claims.exp
            })))
    } else {
        // This should never happen if middleware is working correctly
        Ok(HttpResponse::Unauthorized()
            .content_type("application/json")
            .json(json!({
                "error": "No claims found"
            })))
    }
}

async fn query_users(db: &DatabaseConnection) -> Result<(), Box<dyn std::error::Error>> {
    println!("Querying users table...");
    
    // Fetch all users from the database
    let users: Vec<User> = Users::find()
        .all(db)
        .await?;
    
    println!("Found {} users:", users.len());
    
    for user in users {
        println!("  User ID: {}", user.id);
        println!("  Email: {}", user.email);
        println!("  Name: {}", user.name.as_deref().unwrap_or("Not set"));
        println!("  Created: {}", user.created_at);
        println!("  Updated: {}", user.updated_at);
        println!("  ---");
    }
    
    Ok(())
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
    
    // Query users table
    if let Err(e) = query_users(&db).await {
        eprintln!("Error querying users: {}", e);
    }
    
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
            .service(get_users)
            .service(login)
            .service(web::scope("/api")
                .wrap(JwtAuth::new())
                .service(protected_route)
                )
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}
