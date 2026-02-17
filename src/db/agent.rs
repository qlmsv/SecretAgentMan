use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool, types::Json};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use anyhow::Result;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Agent {
    pub id: Uuid,
    pub user_id: Uuid,
    pub name: String,
    pub config: Json<serde_json::Value>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub struct AgentRepository {
    pool: PgPool,
}

impl AgentRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create(&self, user_id: Uuid, name: &str, config: serde_json::Value) -> Result<Agent> {
        let agent = sqlx::query_as::<_, Agent>(
            "INSERT INTO agents (user_id, name, config) VALUES ($1, $2, $3) RETURNING *"
        )
        .bind(user_id)
        .bind(name)
        .bind(Json(config))
        .fetch_one(&self.pool)
        .await?;

        Ok(agent)
    }

    pub async fn find_by_user_id(&self, user_id: Uuid) -> Result<Vec<Agent>> {
        let agents = sqlx::query_as::<_, Agent>(
            "SELECT * FROM agents WHERE user_id = $1 ORDER BY created_at DESC"
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(agents)
    }

    pub async fn find_by_id(&self, id: Uuid) -> Result<Option<Agent>> {
        let agent = sqlx::query_as::<_, Agent>(
            "SELECT * FROM agents WHERE id = $1"
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(agent)
    }

    pub async fn update_config(&self, id: Uuid, config: serde_json::Value) -> Result<Agent> {
        let agent = sqlx::query_as::<_, Agent>(
            "UPDATE agents SET config = $1, updated_at = NOW() WHERE id = $2 RETURNING *"
        )
        .bind(Json(config))
        .bind(id)
        .fetch_one(&self.pool)
        .await?;

        Ok(agent)
    }
}
