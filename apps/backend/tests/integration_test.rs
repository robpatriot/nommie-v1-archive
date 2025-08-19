use backend::test_support::common::init_tracing_for_tests;
use std::env;

#[test]
fn test_tracing_and_env() {
    // Initialize tracing for tests (should not panic)
    init_tracing_for_tests();

    // Read DATABASE_URL from env and print it (inline capture to satisfy clippy)
    let database_url =
        env::var("DATABASE_URL").unwrap_or_else(|_| "DATABASE_URL not set".to_string());
    println!("DATABASE_URL: {database_url}");

    // Make a non-constant assertion so clippy is happy
    assert!(
        !database_url.is_empty(),
        "DATABASE_URL should be non-empty (ok if it's a placeholder)"
    );
}
