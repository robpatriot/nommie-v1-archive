use actix_cors::Cors;
use actix_web::{web, App, HttpServer};
use std::env;
use tracing::warn;
use tracing_actix_web::TracingLogger;

// Import bootstrap functions and route configurator
use backend::{configure_routes, connect_and_migrate_from_env, init_tracing, load_dotenv};

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Use bootstrap functions for idempotent initialization
    load_dotenv();
    init_tracing();
    let db = connect_and_migrate_from_env().await;

    // Start the HTTP server
    HttpServer::new(move || {
        // Configure CORS
        let frontend_origin = env::var("CORS_ALLOWED_ORIGIN").unwrap_or_else(|_| {
            warn!("Warning: CORS_ALLOWED_ORIGIN not set, using default");
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
            .wrap(TracingLogger::default())
            .app_data(web::Data::new(db.clone()))
            .configure(configure_routes)
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}
