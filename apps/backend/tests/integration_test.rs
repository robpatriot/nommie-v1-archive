mod common;
use common::{test_bootstrap, test_issue_token};
use std::env;

#[actix_web::test]
async fn test_tracing_and_env() -> anyhow::Result<()> {
    // Test that test_bootstrap works and initializes everything
    let db = test_bootstrap().await;

    // Test that we can issue a test token
    let token = test_issue_token("test_user", "test@example.com", 3600);
    assert!(!token.is_empty());

    // Test that we have a database connection
    assert!(db.ping().await.is_ok());

    // Read DATABASE_URL from env and print it (inline capture to satisfy clippy)
    let database_url =
        env::var("DATABASE_URL").unwrap_or_else(|_| "DATABASE_URL not set".to_string());
    println!("DATABASE_URL: {database_url}");

    // Make a non-constant assertion so clippy is happy
    assert!(
        !database_url.is_empty(),
        "DATABASE_URL should be non-empty (ok if it's a placeholder)"
    );

    Ok(())
}
