use dotenv;
use migration::{Migrator, MigratorTrait};
use sea_orm::{Database, DatabaseConnection};
use std::env;
use std::sync::Once;
use tokio::sync::OnceCell;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

static INIT: Once = Once::new();
static ENSURE_TEST_DB_ONCE: Once = Once::new();
static DB_CONNECTION: OnceCell<DatabaseConnection> = OnceCell::const_new();

pub fn init_tracing_for_tests() {
    INIT.call_once(|| {
        let filter = EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| EnvFilter::new("info,actix_web=info,sea_orm=info"));
        tracing_subscriber::registry()
            .with(filter)
            .with(tracing_subscriber::fmt::layer().pretty())
            .init();
    });
}

/// Test-only JWT helper that issues a signed JWT using the same secret/algorithm/claims as production
pub fn test_issue_token(sub: &str, email: &str, ttl_seconds: i64) -> String {
    crate::jwt::issue_test_token(sub, email, ttl_seconds)
}

/// Test bootstrap that loads .env, ensures *_test database, inits tracing, connects+migrates once
pub async fn test_bootstrap() -> DatabaseConnection {
    load_dotenv();
    ensure_test_db();
    init_tracing_for_tests();
    connect_and_migrate_from_env().await
}

fn load_dotenv() {
    let _ = dotenv::dotenv();
}

fn ensure_test_db() {
    ENSURE_TEST_DB_ONCE.call_once(|| {
        let url = env::var("DATABASE_URL").expect("DATABASE_URL is required for tests");

        // Parse the database name from the last path segment before "?"
        let db_name = if let Some(query_pos) = url.find('?') {
            &url[..query_pos]
        } else {
            &url
        };

        let db_name = if let Some(last_slash) = db_name.rfind('/') {
            &db_name[last_slash + 1..]
        } else {
            db_name
        };

        assert!(
            db_name.ends_with("_test"),
            "Refusing to run unless DATABASE_URL points to a *_test database. Current: {}",
            redact_db_url(&url)
        );
    });
}

async fn connect_and_migrate_from_env() -> DatabaseConnection {
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
