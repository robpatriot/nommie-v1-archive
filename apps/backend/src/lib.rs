pub mod bootstrap;
pub mod dto;
pub mod entity;
pub mod game_management;
pub mod jwt;
pub mod user_management;

pub use bootstrap::{connect_and_migrate_from_env, init_tracing, load_dotenv};

use actix_web::web;

use game_management::{
    add_ai_player, create_game, delete_game, get_game_state, get_game_summary, get_games,
    join_game, mark_player_ready, play_card, submit_bid, submit_trump,
};
use jwt::{get_claims, get_user, JwtAuth};

/// Configure all routes for the application
pub fn configure_routes(cfg: &mut actix_web::web::ServiceConfig) {
    cfg.service(hello).service(
        web::scope("/api")
            .wrap(JwtAuth::new())
            .service(protected_route)
            .service(create_game)
            .service(get_games)
            .service(mark_player_ready)
            .service(add_ai_player)
            .service(join_game)
            .service(get_game_state)
            .service(get_game_summary)
            .service(submit_bid)
            .service(submit_trump)
            .service(play_card)
            .service(delete_game),
    );
}

#[actix_web::get("/")]
async fn hello() -> impl actix_web::Responder {
    "Hello, Nommie!"
}

#[actix_web::get("/protected")]
async fn protected_route(
    req: actix_web::HttpRequest,
) -> actix_web::Result<actix_web::HttpResponse> {
    // Extract claims and user from the request (set by JWT middleware)
    if let Some(claims) = get_claims(&req) {
        if let Some(user) = get_user(&req) {
            Ok(actix_web::HttpResponse::Ok()
                .content_type("application/json")
                .json(serde_json::json!({
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
            Ok(actix_web::HttpResponse::InternalServerError()
                .content_type("application/json")
                .json(serde_json::json!({
                    "error": "User not found"
                })))
        }
    } else {
        // This should never happen if middleware is working correctly
        Ok(actix_web::HttpResponse::Unauthorized()
            .content_type("application/json")
            .json(serde_json::json!({
                "error": "No claims found"
            })))
    }
}
