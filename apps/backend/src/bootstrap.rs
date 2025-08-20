use migration::Migrator;
use migration::MigratorTrait;
use sea_orm::{Database, DatabaseConnection};
use std::env;
use std::sync::OnceLock;
use tokio::sync::OnceCell;
use tracing::{info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

static DOTENV_INIT: OnceLock<()> = OnceLock::new();
static TRACING_INIT: OnceLock<()> = OnceLock::new();
static DB_CONNECTION: OnceCell<DatabaseConnection> = OnceCell::const_new();

/// Load environment variables from .env file exactly once
pub fn load_dotenv() {
    DOTENV_INIT.get_or_init(|| {
        dotenv::dotenv().ok();
    });
}

/// Initialize tracing exactly once
pub fn init_tracing() {
    TRACING_INIT.get_or_init(|| {
        let filter = EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| EnvFilter::new("info,actix_web=info,sea_orm=info"));

        let is_production =
            env::var("RUST_ENV").unwrap_or_else(|_| "development".to_string()) == "production";

        if is_production {
            // JSON formatter for production
            tracing_subscriber::registry()
                .with(filter)
                .with(tracing_subscriber::fmt::layer().json())
                .init();
        } else {
            // Pretty formatter for development
            tracing_subscriber::registry()
                .with(filter)
                .with(tracing_subscriber::fmt::layer().pretty())
                .init();
        }
    });
}

/// Connect to database and run migrations exactly once, return a cheap clone thereafter
pub async fn connect_and_migrate_from_env() -> DatabaseConnection {
    DB_CONNECTION
        .get_or_init(|| async {
            // Get database URL from environment
            let database_url = env::var("DATABASE_URL").unwrap_or_else(|_| {
                warn!("Warning: DATABASE_URL not set, using default");
                "postgres://nommie_user:pineconescamping@localhost:5432/nommie".to_string()
            });

            info!("Starting Nommie backend server...");
            info!("Database URL: {}", database_url);

            // Connect to database
            let db: DatabaseConnection = Database::connect(&database_url)
                .await
                .expect("Failed to connect to database");

            info!("Connected to database successfully!");

            // Run migrations
            Migrator::up(&db, None)
                .await
                .expect("Failed to run migrations");

            info!("Database migrations completed successfully!");

            db
        })
        .await
        .clone()
}
