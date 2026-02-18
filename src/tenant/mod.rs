//! Tenant module for AI-Mentor SaaS platform.
//!
//! Manages per-user isolated SQLite databases for storing profiles,
//! goals, conversation history, and feature settings.

use anyhow::{Context, Result};
use chrono::Utc;
use parking_lot::Mutex;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use uuid::Uuid;

/// User profile data from onboarding
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UserProfile {
    pub name: Option<String>,
    pub birthdate: Option<String>,
    pub birth_time: Option<String>,
    pub birth_place: Option<String>,
    pub mbti_type: Option<String>,
    pub use_esoteric: bool,
    pub bazi_chart: Option<JsonValue>,
    pub destiny_matrix: Option<JsonValue>,
    pub selected_features: Vec<String>,
    pub onboarding_completed: bool,
}

/// Goal data (SMART format)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Goal {
    pub id: String,
    pub original_text: String,
    pub smart_text: String,
    pub category: Option<String>,
    pub status: String,
    pub progress: i32,
    pub milestones: Vec<String>,
    pub notion_page_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// Conversation message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationMessage {
    pub id: String,
    pub role: String,
    pub content: String,
    pub tokens_used: Option<i32>,
    pub provider: Option<String>,
    pub created_at: String,
}

/// Feature setting
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureSetting {
    pub feature: String,
    pub enabled: bool,
    pub config: Option<JsonValue>,
}

/// Tenant database wrapper
pub struct TenantDb {
    conn: Mutex<Connection>,
    user_id: String,
    db_path: PathBuf,
}

impl TenantDb {
    /// Open or create tenant database
    pub fn open(base_path: &Path, user_id: &str) -> Result<Self> {
        let tenant_dir = base_path.join("tenants").join(user_id);
        std::fs::create_dir_all(&tenant_dir)?;

        let db_path = tenant_dir.join("brain.db");
        let conn = Connection::open(&db_path)
            .with_context(|| format!("Failed to open tenant DB at {:?}", db_path))?;

        Self::init_schema(&conn)?;

        Ok(Self {
            conn: Mutex::new(conn),
            user_id: user_id.to_string(),
            db_path,
        })
    }

    /// Initialize database schema
    fn init_schema(conn: &Connection) -> Result<()> {
        conn.execute_batch(
            r#"
            -- SQLite optimizations
            PRAGMA journal_mode = WAL;
            PRAGMA synchronous = NORMAL;

            -- User profile (key-value store)
            CREATE TABLE IF NOT EXISTS profile (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );

            -- Goals (SMART format)
            CREATE TABLE IF NOT EXISTS goals (
                id TEXT PRIMARY KEY,
                original_text TEXT NOT NULL,
                smart_text TEXT NOT NULL,
                category TEXT,
                status TEXT DEFAULT 'active',
                progress INTEGER DEFAULT 0,
                milestones TEXT,
                notion_page_id TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );

            -- Conversation history
            CREATE TABLE IF NOT EXISTS conversations (
                id TEXT PRIMARY KEY,
                role TEXT NOT NULL,
                content TEXT NOT NULL,
                tokens_used INTEGER,
                provider TEXT,
                created_at TEXT NOT NULL
            );

            -- Feature settings
            CREATE TABLE IF NOT EXISTS feature_settings (
                feature TEXT PRIMARY KEY,
                enabled INTEGER DEFAULT 1,
                config TEXT
            );

            -- Indexes
            CREATE INDEX IF NOT EXISTS idx_goals_status ON goals(status);
            CREATE INDEX IF NOT EXISTS idx_conversations_created ON conversations(created_at);
            "#,
        )
        .context("Failed to initialize tenant schema")?;

        Ok(())
    }

    /// Get user ID
    pub fn user_id(&self) -> &str {
        &self.user_id
    }

    /// Get database path
    pub fn db_path(&self) -> &Path {
        &self.db_path
    }

    // ===== Profile Operations =====

    /// Set profile value
    pub fn set_profile_value(&self, key: &str, value: &str) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        let conn = self.conn.lock();

        conn.execute(
            "INSERT OR REPLACE INTO profile (key, value, updated_at) VALUES (?1, ?2, ?3)",
            params![key, value, &now],
        )
        .context("Failed to set profile value")?;

        Ok(())
    }

    /// Get profile value
    pub fn get_profile_value(&self, key: &str) -> Result<Option<String>> {
        let conn = self.conn.lock();

        let result = conn.query_row(
            "SELECT value FROM profile WHERE key = ?1",
            params![key],
            |row| row.get(0),
        );

        match result {
            Ok(value) => Ok(Some(value)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Get full user profile
    pub fn get_profile(&self) -> Result<UserProfile> {
        let conn = self.conn.lock();

        let mut stmt = conn.prepare("SELECT key, value FROM profile")?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;

        let mut profile = UserProfile::default();

        for row in rows {
            let (key, value) = row?;
            match key.as_str() {
                "name" => profile.name = Some(value),
                "birthdate" => profile.birthdate = Some(value),
                "birth_time" => profile.birth_time = Some(value),
                "birth_place" => profile.birth_place = Some(value),
                "mbti_type" => profile.mbti_type = Some(value),
                "use_esoteric" => profile.use_esoteric = value == "true",
                "bazi_chart" => profile.bazi_chart = serde_json::from_str(&value).ok(),
                "destiny_matrix" => profile.destiny_matrix = serde_json::from_str(&value).ok(),
                "selected_features" => {
                    profile.selected_features = serde_json::from_str(&value).unwrap_or_default()
                }
                "onboarding_completed" => profile.onboarding_completed = value == "true",
                _ => {}
            }
        }

        Ok(profile)
    }

    /// Save full user profile
    pub fn save_profile(&self, profile: &UserProfile) -> Result<()> {
        if let Some(ref name) = profile.name {
            self.set_profile_value("name", name)?;
        }
        if let Some(ref birthdate) = profile.birthdate {
            self.set_profile_value("birthdate", birthdate)?;
        }
        if let Some(ref birth_time) = profile.birth_time {
            self.set_profile_value("birth_time", birth_time)?;
        }
        if let Some(ref birth_place) = profile.birth_place {
            self.set_profile_value("birth_place", birth_place)?;
        }
        if let Some(ref mbti_type) = profile.mbti_type {
            self.set_profile_value("mbti_type", mbti_type)?;
        }
        self.set_profile_value("use_esoteric", if profile.use_esoteric { "true" } else { "false" })?;
        if let Some(ref bazi) = profile.bazi_chart {
            self.set_profile_value("bazi_chart", &serde_json::to_string(bazi)?)?;
        }
        if let Some(ref matrix) = profile.destiny_matrix {
            self.set_profile_value("destiny_matrix", &serde_json::to_string(matrix)?)?;
        }
        if !profile.selected_features.is_empty() {
            self.set_profile_value(
                "selected_features",
                &serde_json::to_string(&profile.selected_features)?,
            )?;
        }
        self.set_profile_value(
            "onboarding_completed",
            if profile.onboarding_completed { "true" } else { "false" },
        )?;

        Ok(())
    }

    // ===== Goals Operations =====

    /// Create a new goal
    pub fn create_goal(&self, original_text: &str, smart_text: &str, category: Option<&str>) -> Result<Goal> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();

        let conn = self.conn.lock();

        conn.execute(
            r#"INSERT INTO goals (id, original_text, smart_text, category, status, progress, milestones, created_at, updated_at)
               VALUES (?1, ?2, ?3, ?4, 'active', 0, '[]', ?5, ?6)"#,
            params![&id, original_text, smart_text, category, &now, &now],
        )
        .context("Failed to create goal")?;

        Ok(Goal {
            id,
            original_text: original_text.to_string(),
            smart_text: smart_text.to_string(),
            category: category.map(String::from),
            status: "active".to_string(),
            progress: 0,
            milestones: vec![],
            notion_page_id: None,
            created_at: now.clone(),
            updated_at: now,
        })
    }

    /// Get all goals
    pub fn get_goals(&self, status: Option<&str>) -> Result<Vec<Goal>> {
        let conn = self.conn.lock();

        let query = match status {
            Some(_) => {
                "SELECT id, original_text, smart_text, category, status, progress, milestones, notion_page_id, created_at, updated_at FROM goals WHERE status = ?1 ORDER BY created_at DESC"
            }
            None => {
                "SELECT id, original_text, smart_text, category, status, progress, milestones, notion_page_id, created_at, updated_at FROM goals ORDER BY created_at DESC"
            }
        };

        let mut stmt = conn.prepare(query)?;

        let rows = if let Some(s) = status {
            stmt.query_map(params![s], Self::row_to_goal)?
        } else {
            stmt.query_map([], Self::row_to_goal)?
        };

        let mut goals = Vec::new();
        for row in rows {
            goals.push(row?);
        }

        Ok(goals)
    }

    fn row_to_goal(row: &rusqlite::Row) -> rusqlite::Result<Goal> {
        let milestones_str: String = row.get(6)?;
        let milestones: Vec<String> = serde_json::from_str(&milestones_str).unwrap_or_default();

        Ok(Goal {
            id: row.get(0)?,
            original_text: row.get(1)?,
            smart_text: row.get(2)?,
            category: row.get(3)?,
            status: row.get(4)?,
            progress: row.get(5)?,
            milestones,
            notion_page_id: row.get(7)?,
            created_at: row.get(8)?,
            updated_at: row.get(9)?,
        })
    }

    /// Get goal by ID
    pub fn get_goal(&self, goal_id: &str) -> Result<Option<Goal>> {
        let conn = self.conn.lock();

        let result = conn.query_row(
            "SELECT id, original_text, smart_text, category, status, progress, milestones, notion_page_id, created_at, updated_at FROM goals WHERE id = ?1",
            params![goal_id],
            Self::row_to_goal,
        );

        match result {
            Ok(goal) => Ok(Some(goal)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Update goal progress
    pub fn update_goal_progress(&self, goal_id: &str, progress: i32) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        let conn = self.conn.lock();

        let status = if progress >= 100 { "completed" } else { "active" };

        conn.execute(
            "UPDATE goals SET progress = ?1, status = ?2, updated_at = ?3 WHERE id = ?4",
            params![progress, status, &now, goal_id],
        )
        .context("Failed to update goal progress")?;

        Ok(())
    }

    /// Update goal milestones
    pub fn update_goal_milestones(&self, goal_id: &str, milestones: &[String]) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        let milestones_json = serde_json::to_string(milestones)?;
        let conn = self.conn.lock();

        conn.execute(
            "UPDATE goals SET milestones = ?1, updated_at = ?2 WHERE id = ?3",
            params![&milestones_json, &now, goal_id],
        )
        .context("Failed to update goal milestones")?;

        Ok(())
    }

    /// Set Notion page ID for goal
    pub fn set_goal_notion_id(&self, goal_id: &str, notion_page_id: &str) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        let conn = self.conn.lock();

        conn.execute(
            "UPDATE goals SET notion_page_id = ?1, updated_at = ?2 WHERE id = ?3",
            params![notion_page_id, &now, goal_id],
        )
        .context("Failed to set goal Notion ID")?;

        Ok(())
    }

    // ===== Conversation Operations =====

    /// Add conversation message
    pub fn add_message(
        &self,
        role: &str,
        content: &str,
        tokens_used: Option<i32>,
        provider: Option<&str>,
    ) -> Result<ConversationMessage> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();

        let conn = self.conn.lock();

        conn.execute(
            "INSERT INTO conversations (id, role, content, tokens_used, provider, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![&id, role, content, tokens_used, provider, &now],
        )
        .context("Failed to add message")?;

        Ok(ConversationMessage {
            id,
            role: role.to_string(),
            content: content.to_string(),
            tokens_used,
            provider: provider.map(String::from),
            created_at: now,
        })
    }

    /// Get recent conversation history
    pub fn get_conversation_history(&self, limit: usize) -> Result<Vec<ConversationMessage>> {
        let conn = self.conn.lock();

        let mut stmt = conn.prepare(
            "SELECT id, role, content, tokens_used, provider, created_at FROM conversations ORDER BY created_at DESC LIMIT ?1",
        )?;

        let rows = stmt.query_map(params![limit as i64], |row| {
            Ok(ConversationMessage {
                id: row.get(0)?,
                role: row.get(1)?,
                content: row.get(2)?,
                tokens_used: row.get(3)?,
                provider: row.get(4)?,
                created_at: row.get(5)?,
            })
        })?;

        let mut messages = Vec::new();
        for row in rows {
            messages.push(row?);
        }

        // Reverse to get chronological order
        messages.reverse();

        Ok(messages)
    }

    /// Clear conversation history
    pub fn clear_conversation_history(&self) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute("DELETE FROM conversations", [])
            .context("Failed to clear conversation history")?;
        Ok(())
    }

    // ===== Feature Settings =====

    /// Set feature enabled/disabled
    pub fn set_feature(&self, feature: &str, enabled: bool, config: Option<&JsonValue>) -> Result<()> {
        let conn = self.conn.lock();
        let config_str = config.map(|c| serde_json::to_string(c).unwrap_or_default());

        conn.execute(
            "INSERT OR REPLACE INTO feature_settings (feature, enabled, config) VALUES (?1, ?2, ?3)",
            params![feature, enabled as i32, config_str],
        )
        .context("Failed to set feature")?;

        Ok(())
    }

    /// Get feature setting
    pub fn get_feature(&self, feature: &str) -> Result<Option<FeatureSetting>> {
        let conn = self.conn.lock();

        let result = conn.query_row(
            "SELECT feature, enabled, config FROM feature_settings WHERE feature = ?1",
            params![feature],
            |row| {
                let config_str: Option<String> = row.get(2)?;
                let config = config_str.and_then(|s| serde_json::from_str(&s).ok());
                Ok(FeatureSetting {
                    feature: row.get(0)?,
                    enabled: row.get::<_, i32>(1)? != 0,
                    config,
                })
            },
        );

        match result {
            Ok(setting) => Ok(Some(setting)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Get all feature settings
    pub fn get_all_features(&self) -> Result<Vec<FeatureSetting>> {
        let conn = self.conn.lock();

        let mut stmt = conn.prepare("SELECT feature, enabled, config FROM feature_settings")?;

        let rows = stmt.query_map([], |row| {
            let config_str: Option<String> = row.get(2)?;
            let config = config_str.and_then(|s| serde_json::from_str(&s).ok());
            Ok(FeatureSetting {
                feature: row.get(0)?,
                enabled: row.get::<_, i32>(1)? != 0,
                config,
            })
        })?;

        let mut features = Vec::new();
        for row in rows {
            features.push(row?);
        }

        Ok(features)
    }

    /// Check if feature is enabled
    pub fn is_feature_enabled(&self, feature: &str) -> bool {
        self.get_feature(feature)
            .map(|f| f.map(|s| s.enabled).unwrap_or(false))
            .unwrap_or(false)
    }
}

/// Tenant manager for handling multiple user databases
pub struct TenantManager {
    base_path: PathBuf,
    tenants: Mutex<HashMap<String, Arc<TenantDb>>>,
}

impl TenantManager {
    /// Create new tenant manager
    pub fn new(workspace_dir: &Path) -> Self {
        Self {
            base_path: workspace_dir.to_path_buf(),
            tenants: Mutex::new(HashMap::new()),
        }
    }

    /// Get or create tenant database
    pub fn get_tenant(&self, user_id: &str) -> Result<Arc<TenantDb>> {
        let mut tenants = self.tenants.lock();

        if let Some(tenant) = tenants.get(user_id) {
            return Ok(Arc::clone(tenant));
        }

        let tenant = Arc::new(TenantDb::open(&self.base_path, user_id)?);
        tenants.insert(user_id.to_string(), Arc::clone(&tenant));

        Ok(tenant)
    }

    /// Check if tenant exists
    pub fn tenant_exists(&self, user_id: &str) -> bool {
        let tenant_dir = self.base_path.join("tenants").join(user_id);
        tenant_dir.join("brain.db").exists()
    }

    /// List all tenant IDs
    pub fn list_tenants(&self) -> Result<Vec<String>> {
        let tenants_dir = self.base_path.join("tenants");

        if !tenants_dir.exists() {
            return Ok(vec![]);
        }

        let mut tenant_ids = Vec::new();

        for entry in std::fs::read_dir(&tenants_dir)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                if let Some(name) = entry.file_name().to_str() {
                    // Check if brain.db exists
                    if entry.path().join("brain.db").exists() {
                        tenant_ids.push(name.to_string());
                    }
                }
            }
        }

        Ok(tenant_ids)
    }

    /// Delete tenant data
    pub fn delete_tenant(&self, user_id: &str) -> Result<()> {
        // Remove from cache
        {
            let mut tenants = self.tenants.lock();
            tenants.remove(user_id);
        }

        // Delete directory
        let tenant_dir = self.base_path.join("tenants").join(user_id);
        if tenant_dir.exists() {
            std::fs::remove_dir_all(&tenant_dir)
                .with_context(|| format!("Failed to delete tenant directory: {:?}", tenant_dir))?;
        }

        Ok(())
    }

    /// Get base path
    pub fn base_path(&self) -> &Path {
        &self.base_path
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn test_tenant_manager() -> (TempDir, TenantManager) {
        let tmp = TempDir::new().unwrap();
        let manager = TenantManager::new(tmp.path());
        (tmp, manager)
    }

    #[test]
    fn test_create_tenant() {
        let (_tmp, manager) = test_tenant_manager();

        let tenant = manager.get_tenant("user123").unwrap();
        assert_eq!(tenant.user_id(), "user123");
        assert!(manager.tenant_exists("user123"));
    }

    #[test]
    fn test_profile_operations() {
        let (_tmp, manager) = test_tenant_manager();
        let tenant = manager.get_tenant("user123").unwrap();

        // Set profile values
        tenant.set_profile_value("name", "Test User").unwrap();
        tenant.set_profile_value("mbti_type", "INTJ").unwrap();

        // Get profile values
        assert_eq!(
            tenant.get_profile_value("name").unwrap(),
            Some("Test User".to_string())
        );
        assert_eq!(
            tenant.get_profile_value("mbti_type").unwrap(),
            Some("INTJ".to_string())
        );

        // Get full profile
        let profile = tenant.get_profile().unwrap();
        assert_eq!(profile.name, Some("Test User".to_string()));
        assert_eq!(profile.mbti_type, Some("INTJ".to_string()));
    }

    #[test]
    fn test_save_and_load_profile() {
        let (_tmp, manager) = test_tenant_manager();
        let tenant = manager.get_tenant("user123").unwrap();

        let profile = UserProfile {
            name: Some("Test User".to_string()),
            birthdate: Some("1990-01-15".to_string()),
            mbti_type: Some("ENFP".to_string()),
            use_esoteric: true,
            selected_features: vec!["goals".to_string(), "content".to_string()],
            onboarding_completed: true,
            ..Default::default()
        };

        tenant.save_profile(&profile).unwrap();

        let loaded = tenant.get_profile().unwrap();
        assert_eq!(loaded.name, profile.name);
        assert_eq!(loaded.birthdate, profile.birthdate);
        assert_eq!(loaded.mbti_type, profile.mbti_type);
        assert_eq!(loaded.use_esoteric, profile.use_esoteric);
        assert_eq!(loaded.selected_features, profile.selected_features);
        assert!(loaded.onboarding_completed);
    }

    #[test]
    fn test_goals() {
        let (_tmp, manager) = test_tenant_manager();
        let tenant = manager.get_tenant("user123").unwrap();

        // Create goal
        let goal = tenant
            .create_goal(
                "I want to learn Rust",
                "I am a proficient Rust developer writing production code",
                Some("career"),
            )
            .unwrap();

        assert_eq!(goal.original_text, "I want to learn Rust");
        assert_eq!(goal.status, "active");
        assert_eq!(goal.progress, 0);

        // Get goals
        let goals = tenant.get_goals(Some("active")).unwrap();
        assert_eq!(goals.len(), 1);

        // Update progress
        tenant.update_goal_progress(&goal.id, 50).unwrap();
        let updated = tenant.get_goal(&goal.id).unwrap().unwrap();
        assert_eq!(updated.progress, 50);

        // Complete goal
        tenant.update_goal_progress(&goal.id, 100).unwrap();
        let completed = tenant.get_goal(&goal.id).unwrap().unwrap();
        assert_eq!(completed.status, "completed");
    }

    #[test]
    fn test_conversation_history() {
        let (_tmp, manager) = test_tenant_manager();
        let tenant = manager.get_tenant("user123").unwrap();

        // Add messages
        tenant.add_message("user", "Hello!", None, None).unwrap();
        tenant
            .add_message("assistant", "Hi there!", Some(50), Some("groq"))
            .unwrap();

        // Get history
        let history = tenant.get_conversation_history(10).unwrap();
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].role, "user");
        assert_eq!(history[1].role, "assistant");
    }

    #[test]
    fn test_feature_settings() {
        let (_tmp, manager) = test_tenant_manager();
        let tenant = manager.get_tenant("user123").unwrap();

        // Set feature
        tenant.set_feature("goals", true, None).unwrap();
        tenant.set_feature("content", false, None).unwrap();

        // Check features
        assert!(tenant.is_feature_enabled("goals"));
        assert!(!tenant.is_feature_enabled("content"));
        assert!(!tenant.is_feature_enabled("nonexistent"));
    }

    #[test]
    fn test_list_tenants() {
        let (_tmp, manager) = test_tenant_manager();

        // Create some tenants
        manager.get_tenant("user1").unwrap();
        manager.get_tenant("user2").unwrap();
        manager.get_tenant("user3").unwrap();

        let tenants = manager.list_tenants().unwrap();
        assert_eq!(tenants.len(), 3);
        assert!(tenants.contains(&"user1".to_string()));
        assert!(tenants.contains(&"user2".to_string()));
        assert!(tenants.contains(&"user3".to_string()));
    }

    #[test]
    fn test_delete_tenant() {
        let (_tmp, manager) = test_tenant_manager();

        manager.get_tenant("user123").unwrap();
        assert!(manager.tenant_exists("user123"));

        manager.delete_tenant("user123").unwrap();
        assert!(!manager.tenant_exists("user123"));
    }
}
