use crate::memory::{Memory, MemoryCategory, MemoryEntry};
use crate::memory::embeddings::EmbeddingProvider;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;
use std::sync::Arc;
use anyhow::Result;

pub struct PostgresMemory {
    agent_id: Uuid,
    pool: PgPool,
    embedder: Arc<dyn EmbeddingProvider>,
    vector_weight: f32,
    keyword_weight: f32,
}

impl PostgresMemory {
    pub fn new(
        agent_id: Uuid,
        pool: PgPool,
        embedder: Arc<dyn EmbeddingProvider>,
    ) -> Self {
        Self {
            agent_id,
            pool,
            embedder,
            vector_weight: 0.7,
            keyword_weight: 0.3,
        }
    }

    async fn get_embedding(&self, text: &str) -> Result<Vec<f32>> {
        self.embedder.embed_one(text).await
    }
}

#[async_trait]
impl Memory for PostgresMemory {
    fn name(&self) -> &str {
        "postgres"
    }

    async fn store(&self, key: &str, content: &str, category: MemoryCategory) -> Result<()> {
        let embedding = self.get_embedding(content).await?;
        let category_str = category.to_string();

        let embedding_vec = format!("{:?}", embedding);

        sqlx::query(
            "INSERT INTO memories (agent_id, key, content, category, embedding)
             VALUES ($1, $2, $3, $4, $5::vector)
             ON CONFLICT (agent_id, key) DO UPDATE SET
                content = EXCLUDED.content,
                category = EXCLUDED.category,
                embedding = EXCLUDED.embedding,
                created_at = NOW()"
        )
        .bind(self.agent_id)
        .bind(key)
        .bind(content)
        .bind(category_str)
        .bind(embedding_vec)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn recall(&self, query: &str, limit: usize) -> Result<Vec<MemoryEntry>> {
        let query_embedding = self.get_embedding(query).await?;
        let embedding_str = format!("{:?}", query_embedding);

        let rows = sqlx::query_as::<_, (Uuid, String, String, String, DateTime<Utc>)>(
            "SELECT id, key, content, category, created_at
             FROM memories
             WHERE agent_id = $1
             ORDER BY embedding <=> $2::vector
             LIMIT $3"
        )
        .bind(self.agent_id)
        .bind(embedding_str)
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await?;

        let entries = rows.into_iter().map(|(id, key, content, category_str, created_at)| {
             let category = match category_str.as_str() {
                 "core" => MemoryCategory::Core,
                 "daily" => MemoryCategory::Daily,
                 "conversation" => MemoryCategory::Conversation,
                 other => MemoryCategory::Custom(other.to_string()),
             };

             MemoryEntry {
                 id: id.to_string(),
                 key,
                 content,
                 category,
                 timestamp: created_at.to_rfc3339(),
                 session_id: None,
                 score: None,
             }
        }).collect();

        Ok(entries)
    }

    async fn get(&self, key: &str) -> Result<Option<MemoryEntry>> {
        let row = sqlx::query_as::<_, (Uuid, String, String, String, DateTime<Utc>)>(
            "SELECT id, key, content, category, created_at FROM memories WHERE agent_id = $1 AND key = $2"
        )
        .bind(self.agent_id)
        .bind(key)
        .fetch_optional(&self.pool)
        .await?;

        if let Some((id, key, content, category_str, created_at)) = row {
             let category = match category_str.as_str() {
                 "core" => MemoryCategory::Core,
                 "daily" => MemoryCategory::Daily,
                 "conversation" => MemoryCategory::Conversation,
                 other => MemoryCategory::Custom(other.to_string()),
             };

             Ok(Some(MemoryEntry {
                 id: id.to_string(),
                 key,
                 content,
                 category,
                 timestamp: created_at.to_rfc3339(),
                 session_id: None,
                 score: None,
             }))
        } else {
            Ok(None)
        }
    }

    async fn list(&self, category: Option<&MemoryCategory>) -> Result<Vec<MemoryEntry>> {
        let query_str = if category.is_some() {
            "SELECT id, key, content, category, created_at FROM memories WHERE agent_id = $1 AND category = $2 ORDER BY created_at DESC"
        } else {
            "SELECT id, key, content, category, created_at FROM memories WHERE agent_id = $1 ORDER BY created_at DESC"
        };

        let mut query = sqlx::query_as::<_, (Uuid, String, String, String, DateTime<Utc>)>(query_str)
            .bind(self.agent_id);

        if let Some(cat) = category {
            query = query.bind(cat.to_string());
        }

        let rows = query.fetch_all(&self.pool).await?;

        let entries = rows.into_iter().map(|(id, key, content, category_str, created_at)| {
             let category = match category_str.as_str() {
                 "core" => MemoryCategory::Core,
                 "daily" => MemoryCategory::Daily,
                 "conversation" => MemoryCategory::Conversation,
                 other => MemoryCategory::Custom(other.to_string()),
             };

             MemoryEntry {
                 id: id.to_string(),
                 key,
                 content,
                 category,
                 timestamp: created_at.to_rfc3339(),
                 session_id: None,
                 score: None,
             }
        }).collect();

        Ok(entries)
    }

    async fn forget(&self, key: &str) -> Result<bool> {
        let result = sqlx::query(
            "DELETE FROM memories WHERE agent_id = $1 AND key = $2"
        )
        .bind(self.agent_id)
        .bind(key)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    async fn count(&self) -> Result<usize> {
         let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM memories WHERE agent_id = $1")
            .bind(self.agent_id)
            .fetch_one(&self.pool)
            .await?;

         Ok(count.0 as usize)
    }

    async fn health_check(&self) -> bool {
        sqlx::query("SELECT 1").execute(&self.pool).await.is_ok()
    }
}
