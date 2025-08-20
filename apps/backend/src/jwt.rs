use actix_web::body::EitherBody;
use actix_web::{
    dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform},
    http::header,
    web, Error, HttpMessage, HttpRequest, HttpResponse,
};
use futures_util::future::{ready, LocalBoxFuture, Ready};
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use std::env;
use std::rc::Rc;
use std::sync::OnceLock;

use sea_orm::DatabaseConnection;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: String,   // Subject (user ID)
    pub email: String, // User email
    pub exp: usize,    // Expiration time
    pub iat: usize,    // Issued at
}

pub struct JwtConfig {
    pub alg: Algorithm,
    pub secret: String,
    pub issuer: Option<String>,
    pub audience: Option<String>,
    pub default_ttl_secs: i64,
    pub enc_key: EncodingKey,
    pub dec_key: DecodingKey,
}

static JWT_CFG: OnceLock<JwtConfig> = OnceLock::new();

fn get_jwt_config() -> &'static JwtConfig {
    JWT_CFG.get_or_init(|| {
        let secret = env::var("JWT_SECRET")
            .or_else(|_| env::var("AUTH_SECRET"))
            .expect("JWT_SECRET or AUTH_SECRET must be set");

        let issuer = env::var("JWT_ISSUER").ok();
        let audience = env::var("JWT_AUDIENCE").ok();
        let default_ttl_secs = env::var("JWT_TTL_SECS")
            .unwrap_or_else(|_| "3600".to_string())
            .parse()
            .expect("JWT_TTL_SECS must be a valid integer");

        let alg = Algorithm::HS256;
        let enc_key = EncodingKey::from_secret(secret.as_ref());
        let dec_key = DecodingKey::from_secret(secret.as_ref());

        JwtConfig {
            alg,
            secret,
            issuer,
            audience,
            default_ttl_secs,
            enc_key,
            dec_key,
        }
    })
}

/// Issue a JWT token with custom TTL
pub fn issue_token_with_ttl(sub: &str, email: &str, ttl_secs: i64) -> anyhow::Result<String> {
    let now = chrono::Utc::now().timestamp() as usize;
    let exp = (now as i64 + ttl_secs) as usize;

    let claims = Claims {
        sub: sub.to_string(),
        email: email.to_string(),
        iat: now,
        exp,
    };

    // Note: To support issuer/audience in claims, we'd need to extend the Claims struct
    // For now, keeping the existing Claims structure for backward compatibility

    issue_token(&claims)
}

/// Issue a JWT token from claims
pub fn issue_token(claims: &Claims) -> anyhow::Result<String> {
    let cfg = get_jwt_config();
    let header = Header::new(cfg.alg);

    let token = encode(&header, claims, &cfg.enc_key)
        .map_err(|e| anyhow::anyhow!("Failed to encode JWT token: {}", e))?;

    Ok(token)
}

/// Test-only helper that issues a token without error handling
pub fn issue_test_token(sub: &str, email: &str, ttl_secs: i64) -> String {
    issue_token_with_ttl(sub, email, ttl_secs).expect("failed to issue test token")
}

#[derive(Clone)]
pub struct JwtAuth;

impl Default for JwtAuth {
    fn default() -> Self {
        Self::new()
    }
}

impl JwtAuth {
    pub fn new() -> Self {
        Self
    }

    fn verify_token(token: &str) -> Result<Claims, jsonwebtoken::errors::Error> {
        let cfg = get_jwt_config();
        let mut validation = Validation::new(cfg.alg);

        // Set issuer and audience validation if configured
        if let Some(ref issuer) = cfg.issuer {
            validation.iss = Some(std::collections::HashSet::from([issuer.clone()]));
        }
        if let Some(ref audience) = cfg.audience {
            validation.aud = Some(std::collections::HashSet::from([audience.clone()]));
        }

        let result = decode::<Claims>(token, &cfg.dec_key, &validation);
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
                            // Get database connection from request data
                            let db = match req.app_data::<web::Data<DatabaseConnection>>() {
                                Some(db) => db.get_ref().clone(),
                                None => {
                                    let (req, _pl) = req.into_parts();
                                    let resp = HttpResponse::InternalServerError()
                                        .content_type("application/json")
                                        .json(serde_json::json!({"error": "Database connection not available"}));
                                    return Ok(
                                        ServiceResponse::new(req, resp).map_into_right_body()
                                    );
                                }
                            };

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
    req.extensions()
        .get::<crate::entity::users::Model>()
        .cloned()
}
