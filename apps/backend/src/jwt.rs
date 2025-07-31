use actix_web::{
    dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform},
    http::header,
    Error, HttpMessage, HttpRequest, HttpResponse,
};
use actix_web::body::EitherBody;
use futures_util::future::{ready, LocalBoxFuture, Ready};
use jsonwebtoken::{decode, DecodingKey, Validation};
use serde::{Deserialize, Serialize};
use std::env;
use std::rc::Rc;

use sea_orm::DatabaseConnection;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: String, // Subject (user ID)
    pub email: String, // User email
    pub exp: usize,  // Expiration time
    pub iat: usize,  // Issued at
}

#[derive(Clone)]
pub struct JwtAuth {
    db: DatabaseConnection,
}

impl JwtAuth {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    fn get_jwt_secret() -> String {
        env::var("AUTH_SECRET").unwrap_or_else(|_| {
            eprintln!("Warning: AUTH_SECRET not set, using default secret");
            "your-secret-key".to_string()
        })
    }



    fn verify_token(token: &str) -> Result<Claims, jsonwebtoken::errors::Error> {
        let secret = Self::get_jwt_secret();
        let result = decode::<Claims>(
            token,
            &DecodingKey::from_secret(secret.as_ref()),
            &Validation::default(),
        );
        result.map(|token_data| token_data.claims)
    }
}

impl<S, B> Transform<S, ServiceRequest> for JwtAuth
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type InitError = ();
    type Transform = JwtAuthMiddleware<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(JwtAuthMiddleware {
            service: Rc::new(service),
            db: self.db.clone(),
        }))
    }
}

pub struct JwtAuthMiddleware<S> {
    service: Rc<S>,
    db: DatabaseConnection,
}

impl<S, B> Service<ServiceRequest> for JwtAuthMiddleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let svc = self.service.clone();

        let db = self.db.clone();
        Box::pin(async move {
            // Extract the Authorization header
            let auth_header = req
                .headers()
                .get(header::AUTHORIZATION)
                .and_then(|h| h.to_str().ok())
                .and_then(|h| h.strip_prefix("Bearer "));

            match auth_header {
                Some(token) => {
                    // Verify the JWT token
                    match JwtAuth::verify_token(token) {
                        Ok(claims) => {
                            // Ensure user exists in database
                            match crate::user_management::ensure_user_exists(&db, &claims).await {
                                Ok(user) => {
                                    // Add claims and user to request extensions
                                    req.extensions_mut().insert(claims);
                                    req.extensions_mut().insert(user);
                                    let res = svc.call(req).await?;
                                    Ok(res.map_into_left_body())
                                }
                                Err(_) => {
                                    let (req, _pl) = req.into_parts();
                                    let resp = HttpResponse::InternalServerError()
                                        .content_type("application/json")
                                        .json(serde_json::json!({"error": "Failed to ensure user exists"}));
                                    Ok(ServiceResponse::new(req, resp).map_into_right_body())
                                }
                            }
                        }
                        Err(_) => {
                            let (req, _pl) = req.into_parts();
                            let resp = HttpResponse::Unauthorized()
                                .content_type("application/json")
                                .json(serde_json::json!({"error": "Invalid token"}));
                            Ok(ServiceResponse::new(req, resp).map_into_right_body())
                        }
                    }
                }
                None => {
                    let (req, _pl) = req.into_parts();
                    let resp = HttpResponse::Unauthorized()
                        .content_type("application/json")
                        .json(serde_json::json!({"error": "Missing Authorization header"}));
                    Ok(ServiceResponse::new(req, resp).map_into_right_body())
                }
            }
        })
    }
}

// Helper function to extract claims from request
pub fn get_claims(req: &HttpRequest) -> Option<Claims> {
    req.extensions().get::<Claims>().cloned()
}

// Helper function to extract user from request
pub fn get_user(req: &HttpRequest) -> Option<crate::entity::users::Model> {
    req.extensions().get::<crate::entity::users::Model>().cloned()
} 