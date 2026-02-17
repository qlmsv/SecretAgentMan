use axum::{
    routing::{get, post},
    Router, Json, Extension,
    extract::Path,
    http::StatusCode,
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;
use crate::db::agent::{AgentRepository, Agent};
use crate::api::middleware::Claims;

#[derive(Deserialize)]
pub struct CreateAgentRequest {
    name: String,
    config: serde_json::Value,
}

#[derive(Serialize)]
pub struct AgentResponse {
    id: Uuid,
    name: String,
    config: serde_json::Value,
    created_at: String,
}

impl From<Agent> for AgentResponse {
    fn from(agent: Agent) -> Self {
        Self {
            id: agent.id,
            name: agent.name,
            config: agent.config.0,
            created_at: agent.created_at.to_rfc3339(),
        }
    }
}

#[derive(Deserialize)]
pub struct ChatRequest {
    message: String,
}

#[derive(Serialize)]
pub struct ChatResponse {
    response: String,
}

pub fn router() -> Router {
    Router::new()
        .route("/", get(list_agents).post(create_agent))
        .route("/:id", get(get_agent))
        .route("/:id/chat", post(chat))
}

async fn list_agents(
    Extension(pool): Extension<PgPool>,
    Extension(claims): Extension<Claims>,
) -> impl IntoResponse {
    let repo = AgentRepository::new(pool);
    let user_id = match Uuid::parse_str(&claims.sub) {
        Ok(id) => id,
        Err(_) => return (StatusCode::UNAUTHORIZED, "Invalid user ID").into_response(),
    };

    match repo.find_by_user_id(user_id).await {
        Ok(agents) => {
            let response: Vec<AgentResponse> = agents.into_iter().map(AgentResponse::from).collect();
            Json(response).into_response()
        },
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Database error").into_response(),
    }
}

async fn create_agent(
    Extension(pool): Extension<PgPool>,
    Extension(claims): Extension<Claims>,
    Json(payload): Json<CreateAgentRequest>,
) -> impl IntoResponse {
    let repo = AgentRepository::new(pool);
    let user_id = match Uuid::parse_str(&claims.sub) {
        Ok(id) => id,
        Err(_) => return (StatusCode::UNAUTHORIZED, "Invalid user ID").into_response(),
    };

    match repo.create(user_id, &payload.name, payload.config).await {
        Ok(agent) => (StatusCode::CREATED, Json(AgentResponse::from(agent))).into_response(),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Failed to create agent").into_response(),
    }
}

async fn get_agent(
    Extension(pool): Extension<PgPool>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    let repo = AgentRepository::new(pool);
    let user_id = match Uuid::parse_str(&claims.sub) {
        Ok(id) => id,
        Err(_) => return (StatusCode::UNAUTHORIZED, "Invalid user ID").into_response(),
    };

    match repo.find_by_id(id).await {
        Ok(Some(agent)) => {
            if agent.user_id != user_id {
                return (StatusCode::FORBIDDEN, "Access denied").into_response();
            }
            Json(AgentResponse::from(agent)).into_response()
        },
        Ok(None) => (StatusCode::NOT_FOUND, "Agent not found").into_response(),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Database error").into_response(),
    }
}

async fn chat(
    Extension(pool): Extension<PgPool>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<Uuid>,
    Json(payload): Json<ChatRequest>,
) -> impl IntoResponse {
    let repo = AgentRepository::new(pool.clone());
    let user_id = match Uuid::parse_str(&claims.sub) {
        Ok(id) => id,
        Err(_) => return (StatusCode::UNAUTHORIZED, "Invalid user ID").into_response(),
    };

    let agent_entity = match repo.find_by_id(id).await {
        Ok(Some(a)) => {
            if a.user_id != user_id {
                return (StatusCode::FORBIDDEN, "Access denied").into_response();
            }
            a
        },
        Ok(None) => return (StatusCode::NOT_FOUND, "Agent not found").into_response(),
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "Database error").into_response(),
    };

    // Construct Config
    let mut config = crate::config::Config::default();

    // Setup Memory
    let embedder = crate::memory::embeddings::create_embedding_provider(
        &config.memory.embedding_provider,
        None,
        &config.memory.embedding_model,
        config.memory.embedding_dimensions,
    );

    let memory = std::sync::Arc::new(crate::memory::postgres::PostgresMemory::new(
        agent_entity.id,
        pool.clone(),
        std::sync::Arc::from(embedder),
    ));

    // Create Agent
    let mut agent = match crate::agent::agent::Agent::from_config(&config) {
        Ok(a) => a,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to create agent: {}", e)).into_response(),
    };

    agent.set_memory(memory.clone());

    // Load History
    use crate::memory::Memory;
    let history_entries = memory.list(Some(&crate::memory::MemoryCategory::Custom("history".into()))).await.unwrap_or_default();

    let mut history_entries = history_entries;
    history_entries.reverse(); // Convert DESC to ASC

    let history: Vec<crate::providers::ConversationMessage> = history_entries.iter().filter_map(|e| {
        let parts: Vec<&str> = e.key.split('_').collect();
        if parts.len() < 3 { return None; }
        let role = parts[2];
        let content = e.content.clone();

        match role {
            "user" => Some(crate::providers::ConversationMessage::Chat(crate::providers::ChatMessage::user(content))),
            "assistant" => Some(crate::providers::ConversationMessage::Chat(crate::providers::ChatMessage::assistant(content))),
            _ => None,
        }
    }).collect();

    agent.set_history(history);

    // Run Turn
    let response_text = match agent.turn(&payload.message).await {
        Ok(s) => s,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, format!("Agent error: {}", e)).into_response(),
    };

    // Save new messages to history
    let timestamp = chrono::Utc::now().timestamp_micros();
    let user_key = format!("hist_{}_user", timestamp);
    let _ = memory.store(&user_key, &payload.message, crate::memory::MemoryCategory::Custom("history".into())).await;

    let timestamp2 = chrono::Utc::now().timestamp_micros() + 1;
    let asst_key = format!("hist_{}_assistant", timestamp2);
    let _ = memory.store(&asst_key, &response_text, crate::memory::MemoryCategory::Custom("history".into())).await;

    (StatusCode::OK, Json(ChatResponse { response: response_text })).into_response()
}
