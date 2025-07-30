use actix_web::{
    dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform},
    error::ErrorUnauthorized,
    http::header,
    Error, HttpMessage, HttpRequest, HttpResponse,
};
use actix_web::body::EitherBody;
use futures_util::future::{ready, LocalBoxFuture, Ready};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use std::env;
use std::rc::Rc;
use chrono;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: String, // Subject (user ID)
    pub exp: usize,  // Expiration time
    pub iat: usize,  // Issued at
}

#[derive(Clone)]
pub struct JwtAuth;

impl JwtAuth {
    pub fn new() -> Self {
        Self
    }

    fn get_jwt_secret() -> String {
        env::var("AUTH_SECRET").unwrap_or_else(|_| {
            eprintln!("Warning: AUTH_SECRET not set, using default secret");
            "your-secret-key".to_string()
        })
    }

    pub fn create_token(user_id: &str) -> Result<String, jsonwebtoken::errors::Error> {
        let secret = Self::get_jwt_secret();
        let expiration = chrono::Utc::now()
            .checked_add_signed(chrono::Duration::hours(24))
            .expect("valid timestamp")
            .timestamp() as usize;

        let claims = Claims {
            sub: user_id.to_string(),
            exp: expiration,
            iat: chrono::Utc::now().timestamp() as usize,
        };

        encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(secret.as_ref()),
        )
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
        }))
    }
}

pub struct JwtAuthMiddleware<S> {
    service: Rc<S>,
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
                            // Add claims to request extensions
                            req.extensions_mut().insert(claims);
                            let res = svc.call(req).await?;
                            Ok(res.map_into_left_body())
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