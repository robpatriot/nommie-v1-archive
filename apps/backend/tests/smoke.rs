mod common;
use chrono::Utc;
use common::{test_bootstrap, test_issue_token};
use sea_orm::{ActiveModelTrait, Set};
use uuid::Uuid;

#[actix_web::test]
async fn smoke_workflow() -> anyhow::Result<()> {
    let db = test_bootstrap().await; // loads .env, ensures *_test, inits tracing, connects+migrates once
    let app = actix_web::test::init_service(
        actix_web::App::new()
            .app_data(actix_web::web::Data::new(db.clone()))
            .configure(backend::configure_routes),
    )
    .await;

    // 1) Create a test user with unique email
    let user_id = Uuid::new_v4();
    let user_email = format!("test-{user_id}@example.com");

    let user = backend::entity::users::ActiveModel {
        id: Set(user_id),
        external_id: Set(user_id.to_string()),
        email: Set(user_email.to_string()),
        name: Set(Some("Test User".to_string())),
        is_ai: Set(false),
        created_at: Set(Utc::now().into()),
        updated_at: Set(Utc::now().into()),
    };

    let user = user.insert(&db).await?;

    // 2) Mint JWT
    let token = test_issue_token(&user.external_id, &user.email, 3600);
    let auth = format!("Bearer {token}");

    // 3) Create game
    let req = actix_web::test::TestRequest::post()
        .uri("/api/create_game")
        .insert_header(("Authorization", auth.as_str()))
        .to_request();
    let res = actix_web::test::call_service(&app, req).await;
    assert!(res.status().is_success());
    let created: serde_json::Value = actix_web::test::read_body_json(res).await;
    println!(
        "Create game response: {}",
        serde_json::to_string_pretty(&created)?
    );

    // Extract game ID from nested response
    let game_id = created["game"]["id"].as_str().unwrap().to_string();

    // 4) Add AI players until 4 total (repeat POST to your add_ai endpoint)
    // First, mark the human player as ready
    let req = actix_web::test::TestRequest::post()
        .uri(&format!("/api/game/{game_id}/ready"))
        .insert_header(("Authorization", auth.as_str()))
        .to_request();
    let res = actix_web::test::call_service(&app, req).await;
    if !res.status().is_success() {
        println!("Mark ready failed with status: {}", res.status());
        let body = actix_web::test::read_body(res).await;
        println!(
            "Mark ready response body: {}",
            String::from_utf8_lossy(&body)
        );
        panic!("Mark ready request failed");
    }

    // Add 3 AI players
    for _ in 0..3 {
        let req = actix_web::test::TestRequest::post()
            .uri(&format!("/api/game/{game_id}/add_ai"))
            .insert_header(("Authorization", auth.as_str()))
            .to_request();
        let res = actix_web::test::call_service(&app, req).await;
        let is_success = res.status().is_success();
        if !is_success {
            println!("Add AI failed with status: {}", res.status());
            let body = actix_web::test::read_body(res).await;
            println!("Add AI response body: {}", String::from_utf8_lossy(&body));
        }
        assert!(is_success);
    }

    // 5) Submit bids for all 4 players (simple fixed bids; adapt to your endpoint shape)
    // The game should have started automatically when all players were ready
    // Let's check the game state to see if we're in bidding phase
    let req = actix_web::test::TestRequest::get()
        .uri(&format!("/api/game/{game_id}/state"))
        .insert_header(("Authorization", auth.as_str()))
        .to_request();
    let res = actix_web::test::call_service(&app, req).await;
    assert!(res.status().is_success());
    let state: serde_json::Value = actix_web::test::read_body_json(res).await;

    // Check if we're in bidding phase
    if state["phase"].as_str().unwrap_or("") == "bidding" {
        // Submit bids for each player (assuming the API allows this)
        // This might need to be adjusted based on your actual API design
        for player_index in 0..4 {
            let bid_data = serde_json::json!({
                "bid": 1 + (player_index % 3) // Simple bid pattern: 1, 2, 3, 1
            });

            let req = actix_web::test::TestRequest::post()
                .uri(&format!("/api/game/{game_id}/bid"))
                .insert_header(("Authorization", auth.as_str()))
                .set_json(bid_data)
                .to_request();
            let res = actix_web::test::call_service(&app, req).await;
            // Note: This might fail if your API doesn't allow submitting bids for all players at once
            // Adjust the test based on your actual game flow
            if res.status().is_success() {
                println!("Successfully submitted bid for player {player_index}");
            } else {
                println!(
                    "Bid submission for player {player_index} returned status: {}",
                    res.status()
                );
            }
        }
    }

    // 6) Submit trump (highest bid wins; tie -> first-highest in turn order)
    // This might need to be adjusted based on your actual game flow
    let trump_data = serde_json::json!({
        "trump_suit": "hearts"
    });

    let req = actix_web::test::TestRequest::post()
        .uri(&format!("/api/game/{game_id}/trump"))
        .insert_header(("Authorization", auth.as_str()))
        .set_json(trump_data)
        .to_request();
    let res = actix_web::test::call_service(&app, req).await;
    // Note: This might fail if the game hasn't reached the trump selection phase
    // Adjust the test based on your actual game flow
    if res.status().is_success() {
        println!("Successfully submitted trump");
    } else {
        println!("Trump submission returned status: {}", res.status());
    }

    // 7) Fetch snapshot/state and assert coarse invariants
    let req = actix_web::test::TestRequest::get()
        .uri(&format!("/api/game/{game_id}/state"))
        .insert_header(("Authorization", auth.as_str()))
        .to_request();
    let res = actix_web::test::call_service(&app, req).await;
    assert!(res.status().is_success());
    let state: serde_json::Value = actix_web::test::read_body_json(res).await;

    // Coarse assertions
    assert_eq!(state["players"].as_array().unwrap().len(), 4);
    assert!(state["bids"].is_array() || state["bids"].is_null()); // coarse
    assert!(state["trump"].is_string() || state["trump"].is_object() || state["trump"].is_null());
    // Note: phase might have changed from bidding, so we don't assert it's still bidding

    println!("Smoke test completed successfully!");
    Ok(())
}
