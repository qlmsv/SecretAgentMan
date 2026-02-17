use axum::{
    routing::post,
    Router, Json, Extension,
    http::StatusCode,
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use argon2::{
    password_hash::{
        PasswordHash, PasswordHasher, PasswordVerifier, SaltString
    },
    Argon2
};
use rand::rngs::OsRng;
use jsonwebtoken::{encode, Header, EncodingKey};
use crate::db::user::UserRepository;

#[derive(Deserialize)]
pub struct AuthRequest {
    email: String,
    password: String,
}

#[derive(Serialize)]
pub struct AuthResponse {
    token: String,
    user_id: String,
}

#[derive(Serialize, Deserialize)]
struct Claims {
    sub: String, // user_id
    exp: usize,
}

pub fn router() -> Router {
    Router::new()
        .route("/register", post(register))
        .route("/login", post(login))
}

async fn register(
    Extension(pool): Extension<PgPool>,
    Json(payload): Json<AuthRequest>,
) -> impl IntoResponse {
    let repo = UserRepository::new(pool);

    // Check if user exists
    if let Ok(Some(_)) = repo.find_by_email(&payload.email).await {
        return (StatusCode::CONFLICT, "User already exists").into_response();
    }

    // Hash password
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let password_hash = match argon2.hash_password(payload.password.as_bytes(), &salt) {
        Ok(h) => h.to_string(),
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "Hashing failed").into_response(),
    };

    // Create user
    match repo.create(&payload.email, &password_hash).await {
        Ok(user) => {
            // Generate token
            let expiration = (chrono::Utc::now() + chrono::Duration::hours(24)).timestamp() as usize;
            let claims = Claims {
                sub: user.id.to_string(),
                exp: expiration,
            };

            let secret = std::env::var("JWT_SECRET").unwrap_or_else(|_| "secret".to_string());
            let token = match encode(&Header::default(), &claims, &EncodingKey::from_secret(secret.as_bytes())) {
                Ok(t) => t,
                Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "Token generation failed").into_response(),
            };

            (StatusCode::CREATED, Json(AuthResponse {
                token,
                user_id: user.id.to_string(),
            })).into_response()
        },
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Failed to create user").into_response(),
    }
}

async fn login(
    Extension(pool): Extension<PgPool>,
    Json(payload): Json<AuthRequest>,
) -> impl IntoResponse {
    let repo = UserRepository::new(pool);

    match repo.find_by_email(&payload.email).await {
        Ok(Some(user)) => {
            let parsed_hash = match PasswordHash::new(&user.password_hash) {
                Ok(h) => h,
                Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "Invalid hash stored").into_response(),
            };

            if Argon2::default().verify_password(payload.password.as_bytes(), &parsed_hash).is_ok() {
                let expiration = (chrono::Utc::now() + chrono::Duration::hours(24)).timestamp() as usize;
                let claims = Claims {
                    sub: user.id.to_string(),
                    exp: expiration,
                };

                let secret = std::env::var("JWT_SECRET").unwrap_or_else(|_| "secret".to_string());
                let token = match encode(&Header::default(), &claims, &EncodingKey::from_secret(secret.as_bytes())) {
                    Ok(t) => t,
                    Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "Token generation failed").into_response(),
                };

                (StatusCode::OK, Json(AuthResponse {
                    token,
                    user_id: user.id.to_string(),
                })).into_response()
            } else {
                (StatusCode::UNAUTHORIZED, "Invalid credentials").into_response()
            }
        },
        Ok(None) => (StatusCode::UNAUTHORIZED, "Invalid credentials").into_response(),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Database error").into_response(),
    }
}
