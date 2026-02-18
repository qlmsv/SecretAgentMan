//! Auth API handlers for AI-Mentor SaaS platform.
//!
//! Provides REST endpoints for user registration, login, and Telegram account linking.

use super::AppState;
use crate::auth::AuthManager;
use anyhow::Error as AnyhowError;
use axum::{
    extract::State,
    http::{header, HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};

// ══════════════════════════════════════════════════════════════════════════════
// REQUEST/RESPONSE TYPES
// ══════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct RegisterResponse {
    pub user_id: String,
    pub token: String,
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub user_id: String,
    pub token: String,
}

#[derive(Debug, Serialize)]
pub struct TelegramLinkResponse {
    pub link: String,
    pub code: String,
    pub expires_in_seconds: u64,
}

#[derive(Debug, Serialize)]
pub struct TelegramStatusResponse {
    pub connected: bool,
    pub telegram_username: Option<String>,
    pub telegram_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct UsageResponse {
    pub status: String,
    pub trial_tokens_remaining: Option<i64>,
    pub total_tokens_used: i64,
    pub total_cost_cents: i64,
}

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

// ══════════════════════════════════════════════════════════════════════════════
// HANDLERS
// ══════════════════════════════════════════════════════════════════════════════

/// POST /api/auth/register
/// Register a new user with email and password
pub async fn handle_register(
    State(state): State<AppState>,
    Json(body): Json<RegisterRequest>,
) -> impl IntoResponse {
    let auth_manager = match &state.auth_manager {
        Some(am) => am,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "Auth not configured".to_string(),
                }),
            )
                .into_response();
        }
    };

    let result: Result<(String, String), AnyhowError> =
        auth_manager.register(&body.email, &body.password);

    match result {
        Ok((user_id, token)) => (
            StatusCode::CREATED,
            Json(RegisterResponse { user_id, token }),
        )
            .into_response(),
        Err(e) => {
            let error_msg = e.to_string();
            let status = if error_msg.contains("already registered") {
                StatusCode::CONFLICT
            } else if error_msg.contains("Invalid email") || error_msg.contains("at least 8") {
                StatusCode::BAD_REQUEST
            } else {
                StatusCode::INTERNAL_SERVER_ERROR
            };
            (status, Json(ErrorResponse { error: error_msg })).into_response()
        }
    }
}

/// POST /api/auth/login
/// Login with email and password, returns JWT token
pub async fn handle_login(
    State(state): State<AppState>,
    Json(body): Json<LoginRequest>,
) -> impl IntoResponse {
    let auth_manager = match &state.auth_manager {
        Some(am) => am,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "Auth not configured".to_string(),
                }),
            )
                .into_response();
        }
    };

    match auth_manager.login(&body.email, &body.password) {
        Ok((user_id, token)) => (StatusCode::OK, Json(LoginResponse { user_id, token })).into_response(),
        Err(_e) => (
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                error: "Invalid email or password".to_string(),
            }),
        )
            .into_response(),
    }
}

/// GET /api/auth/telegram-link
/// Generate a one-time link for connecting Telegram account
/// Requires: Authorization: Bearer <token>
pub async fn handle_telegram_link(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let auth_manager = match &state.auth_manager {
        Some(am) => am,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "Auth not configured".to_string(),
                }),
            )
                .into_response();
        }
    };

    // Extract and verify JWT token
    let user_id = match extract_user_id(auth_manager.as_ref(), &headers) {
        Ok(id) => id,
        Err(response) => return response,
    };

    // Generate link code
    let result: Result<String, AnyhowError> =
        auth_manager.generate_telegram_link(&user_id);

    match result {
        Ok(code) => {
            // Get bot username from environment
            let bot_username = std::env::var("ZEROCLAW_TELEGRAM_BOT_USERNAME")
                .unwrap_or_else(|_| "AIAssistantBot".to_string());
            let link = format!("https://t.me/{}?start={}", bot_username, code);

            (
                StatusCode::OK,
                Json(TelegramLinkResponse {
                    link,
                    code,
                    expires_in_seconds: 3600, // 1 hour
                }),
            )
                .into_response()
        }
        Err(e) => {
            let error_msg = e.to_string();
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse { error: error_msg }),
            )
                .into_response()
        }
    }
}

/// GET /api/auth/telegram-status
/// Check if Telegram account is connected
/// Requires: Authorization: Bearer <token>
pub async fn handle_telegram_status(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let auth_manager = match &state.auth_manager {
        Some(am) => am,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "Auth not configured".to_string(),
                }),
            )
                .into_response();
        }
    };

    // Extract and verify JWT token
    let user_id = match extract_user_id(auth_manager.as_ref(), &headers) {
        Ok(id) => id,
        Err(response) => return response,
    };

    // Get user info
    match auth_manager.get_user(&user_id) {
        Ok(user) => (
            StatusCode::OK,
            Json(TelegramStatusResponse {
                connected: user.telegram_id.is_some(),
                telegram_username: user.telegram_username,
                telegram_id: user.telegram_id,
            }),
        )
            .into_response(),
        Err(_e) => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "User not found".to_string(),
            }),
        )
            .into_response(),
    }
}

/// GET /api/usage
/// Get token usage statistics
/// Requires: Authorization: Bearer <token>
pub async fn handle_usage(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let auth_manager = match &state.auth_manager {
        Some(am) => am,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "Auth not configured".to_string(),
                }),
            )
                .into_response();
        }
    };

    let token_meter = match &state.token_meter {
        Some(tm) => tm,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "Billing not configured".to_string(),
                }),
            )
                .into_response();
        }
    };

    // Extract and verify JWT token
    let user_id = match extract_user_id(auth_manager.as_ref(), &headers) {
        Ok(id) => id,
        Err(response) => return response,
    };

    // Get subscription status
    let subscription = match auth_manager.get_subscription(&user_id) {
        Ok(sub) => sub,
        Err(_) => {
            return (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: "Subscription not found".to_string(),
                }),
            )
                .into_response()
        }
    };

    let trial_tokens_remaining = if subscription.status.to_string() == "trial" {
        Some(token_meter.get_trial_remaining(&user_id).unwrap_or(0))
    } else {
        None
    };

    let total_tokens_used = token_meter.get_total_usage(&user_id).unwrap_or(0);
    let total_cost_cents = token_meter.get_total_cost(&user_id).unwrap_or(0);

    (
        StatusCode::OK,
        Json(UsageResponse {
            status: subscription.status.to_string(),
            trial_tokens_remaining,
            total_tokens_used,
            total_cost_cents,
        }),
    )
        .into_response()
}

// ══════════════════════════════════════════════════════════════════════════════
// HELPER FUNCTIONS
// ══════════════════════════════════════════════════════════════════════════════

/// Extract user ID from Authorization header
fn extract_user_id(
    auth_manager: &AuthManager,
    headers: &HeaderMap,
) -> Result<String, axum::response::Response> {
    let auth_header = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| {
            (
                StatusCode::UNAUTHORIZED,
                Json(ErrorResponse {
                    error: "Missing Authorization header".to_string(),
                }),
            )
                .into_response()
        })?;

    let token = auth_header.strip_prefix("Bearer ").ok_or_else(|| {
        (
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                error: "Invalid Authorization format. Expected: Bearer <token>".to_string(),
            }),
        )
            .into_response()
    })?;

    auth_manager.verify_token(token).map_err(|_| {
        (
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                error: "Invalid or expired token".to_string(),
            }),
        )
            .into_response()
    })
}

/// Validate Telegram link code (called from Telegram bot)
pub fn validate_telegram_code(auth_manager: &AuthManager, code: &str) -> Result<String, String> {
    auth_manager
        .validate_telegram_code(code)
        .map_err(|e: AnyhowError| e.to_string())
}

/// Link Telegram account (called from Telegram bot)
pub fn link_telegram_account(
    auth_manager: &AuthManager,
    user_id: &str,
    telegram_id: &str,
    telegram_username: Option<&str>,
) -> Result<(), String> {
    auth_manager
        .link_telegram(user_id, telegram_id, telegram_username)
        .map_err(|e: AnyhowError| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_response_serialization() {
        let error = ErrorResponse {
            error: "Test error".to_string(),
        };
        let json = serde_json::to_string(&error).unwrap();
        assert!(json.contains("Test error"));
    }

    #[test]
    fn test_register_response_serialization() {
        let response = RegisterResponse {
            user_id: "user123".to_string(),
            token: "jwt_token".to_string(),
        };
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("user123"));
        assert!(json.contains("jwt_token"));
    }
}
