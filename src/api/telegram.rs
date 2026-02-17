use axum::{
    routing::post,
    Router, Json, Extension,
    extract::Path,
    http::StatusCode,
    response::IntoResponse,
    middleware,
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;
use crate::api::middleware::{Claims, auth_middleware};
use crate::db::agent::AgentRepository;

#[derive(Deserialize)]
pub struct ConnectTelegramRequest {
    token: String,
}

pub fn router() -> Router {
    Router::new()
        .route("/:id/connect", post(connect_telegram))
        .layer(middleware::from_fn(auth_middleware))
}

async fn connect_telegram(
    Extension(pool): Extension<PgPool>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<Uuid>,
    Json(payload): Json<ConnectTelegramRequest>,
) -> impl IntoResponse {
    let repo = AgentRepository::new(pool.clone());
    let user_id = match Uuid::parse_str(&claims.sub) {
        Ok(id) => id,
        Err(_) => return (StatusCode::UNAUTHORIZED, "Invalid user ID").into_response(),
    };

    // 1. Verify agent ownership
    let agent = match repo.find_by_id(id).await {
        Ok(Some(a)) => {
            if a.user_id != user_id {
                return (StatusCode::FORBIDDEN, "Access denied").into_response();
            }
            a
        },
        Ok(None) => return (StatusCode::NOT_FOUND, "Agent not found").into_response(),
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "Database error").into_response(),
    };

    // 2. Update agent config with Telegram token
    let mut config = agent.config.0.clone();

    // Structure: channels: { telegram: { bot_token: "..." } }
    if let Some(channels) = config.get_mut("channels") {
        if let Some(channels_obj) = channels.as_object_mut() {
            channels_obj.insert("telegram".to_string(), serde_json::json!({
                "bot_token": payload.token,
                "allowed_users": ["*"] // Allow all for now, or could restrict
            }));
        } else {
             // overwriting if not object
             config["channels"] = serde_json::json!({
                "telegram": {
                    "bot_token": payload.token,
                    "allowed_users": ["*"]
                }
             });
        }
    } else {
        config["channels"] = serde_json::json!({
            "telegram": {
                "bot_token": payload.token,
                "allowed_users": ["*"]
            }
        });
    }

    match repo.update_config(id, config).await {
        Ok(_) => (StatusCode::OK, "Telegram connected").into_response(),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Failed to update config").into_response(),
    }
}
