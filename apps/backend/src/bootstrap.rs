use std::env;
use std::sync::OnceLock;

use migration::{Migrator, MigratorTrait};
use sea_orm::{Database, DatabaseConnection};
use tokio::sync::OnceCell;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

static DOTENV_INIT: OnceLock<()> = OnceLock::new();
static TRACING_INIT: OnceLock<()> = OnceLock::new();
static DB_CONNECTION: OnceCell<DatabaseConnection> = OnceCell::const_new();

/// Load environment variables from .env file exactly once (safe to call anywhere)
pub fn load_dotenv() {
    DOTENV_INIT.get_or_init(|| {
        let _ = dotenv::dotenv();
    });
}

/// Initialize tracing exactly once (safe to call anywhere)
pub fn init_tracing() {
    TRACING_INIT.get_or_init(|| {
        let filter = EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| EnvFilter::new("info,actix_web=info,sea_orm=info"));

        let is_production =
            env::var("RUST_ENV").unwrap_or_else(|_| "development".to_string()) == "production";

        if is_production {
            tracing_subscriber::registry()
                .with(filter)
                .with(tracing_subscriber::fmt::layer().json())
                .init();
        } else {
            tracing_subscriber::registry()
                .with(filter)
                .with(tracing_subscriber::fmt::layer().pretty())
                .init();
        }
    });
}

/// Connect to database and run migrations exactly once; returns a cheap clone thereafter.
pub async fn connect_and_migrate_from_env() -> DatabaseConnection {
    // Ensure env is loaded even if caller forgot.
    load_dotenv();

    DB_CONNECTION
        .get_or_init(|| async {
            let database_url =
                env::var("DATABASE_URL").expect("DATABASE_URL must be set before starting backend");

            info!("Starting Nommie backend server…");
            info!("Database URL: {}", redact_db_url(&database_url));

            let db: DatabaseConnection = Database::connect(&database_url)
                .await
                .expect("DB connect failed");

            info!("Connected to database successfully!");

            Migrator::up(&db, None).await.expect("Migrator::up failed");

            info!("Database migrations completed successfully!");

            db
        })
        .await
        .clone()
}

/// Helper to log a DB URL without credentials.
fn redact_db_url(url: &str) -> String {
    if let Some(at_pos) = url.rfind('@') {
        if let Some(colon_pos) = url[..at_pos].rfind(':') {
            if url[..colon_pos].contains("//") {
                let mut s = String::with_capacity(url.len());
                s.push_str(&url[..(colon_pos + 1)]);
                s.push_str("***");
                s.push_str(&url[at_pos..]);
                return s;
            }
        }
    }
    url.to_string()
}
