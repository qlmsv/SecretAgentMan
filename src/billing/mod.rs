//! Billing module for AI-Mentor SaaS platform.
//!
//! Handles token metering, cost calculation, and subscription management.

use anyhow::{Context, Result};
use chrono::{Duration, Utc};
use parking_lot::Mutex;
use rusqlite::{params, Connection};
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

/// Provider cost configuration (per 1M tokens)
#[derive(Debug, Clone)]
pub struct ProviderCost {
    pub name: String,
    pub model: String,
    pub input_cost_per_1m: f64,  // USD per 1M input tokens
    pub output_cost_per_1m: f64, // USD per 1M output tokens
}

impl ProviderCost {
    pub fn new(name: &str, model: &str, input_cost: f64, output_cost: f64) -> Self {
        Self {
            name: name.to_string(),
            model: model.to_string(),
            input_cost_per_1m: input_cost,
            output_cost_per_1m: output_cost,
        }
    }
}

/// Default provider costs (updated 2024)
pub fn default_provider_costs() -> HashMap<String, ProviderCost> {
    let mut costs = HashMap::new();

    // Groq - FREE tier
    costs.insert(
        "groq:llama-3.3-70b".to_string(),
        ProviderCost::new("groq", "llama-3.3-70b-versatile", 0.0, 0.0),
    );
    costs.insert(
        "groq:llama-3.1-8b".to_string(),
        ProviderCost::new("groq", "llama-3.1-8b-instant", 0.0, 0.0),
    );

    // DeepSeek V3
    costs.insert(
        "deepseek:v3".to_string(),
        ProviderCost::new("deepseek", "deepseek-chat", 0.14, 0.28),
    );

    // Google Gemini
    costs.insert(
        "google:gemini-2.0-flash".to_string(),
        ProviderCost::new("google", "gemini-2.0-flash-exp", 0.075, 0.30),
    );
    costs.insert(
        "google:gemini-1.5-flash".to_string(),
        ProviderCost::new("google", "gemini-1.5-flash", 0.075, 0.30),
    );

    // OpenRouter / Kimi
    costs.insert(
        "openrouter:kimi-k2.5".to_string(),
        ProviderCost::new("openrouter", "moonshotai/kimi-k2.5", 0.50, 0.50),
    );

    // Anthropic Claude
    costs.insert(
        "anthropic:claude-3.5-sonnet".to_string(),
        ProviderCost::new("anthropic", "claude-3-5-sonnet-20241022", 3.0, 15.0),
    );
    costs.insert(
        "anthropic:claude-3-haiku".to_string(),
        ProviderCost::new("anthropic", "claude-3-haiku-20240307", 0.25, 1.25),
    );

    // OpenAI (via OpenRouter or direct)
    costs.insert(
        "openai:gpt-4o-mini".to_string(),
        ProviderCost::new("openai", "gpt-4o-mini", 0.15, 0.60),
    );
    costs.insert(
        "openai:gpt-4o".to_string(),
        ProviderCost::new("openai", "gpt-4o", 2.50, 10.0),
    );

    costs
}

/// Access check result
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AccessResult {
    /// User has access
    Allowed,
    /// Trial tokens exhausted
    TrialExhausted,
    /// Trial period expired (3 days)
    TrialExpired,
    /// Subscription required
    SubscriptionRequired,
    /// User not found
    UserNotFound,
}

impl AccessResult {
    pub fn is_allowed(&self) -> bool {
        matches!(self, Self::Allowed)
    }
}

/// Token usage record
#[derive(Debug, Clone)]
pub struct TokenUsage {
    pub user_id: String,
    pub provider: String,
    pub model: String,
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub cost_cents: i64,
    pub price_cents: i64,
    pub created_at: String,
}

/// Token meter for billing
pub struct TokenMeter {
    db: Arc<Mutex<Connection>>,
    provider_costs: HashMap<String, ProviderCost>,
    markup_percent: f64, // e.g., 1.3 = 30% markup
    trial_days: i64,
    trial_token_limit: i64,
}

impl TokenMeter {
    /// Create new TokenMeter with shared database connection
    pub fn new(db: Arc<Mutex<Connection>>) -> Self {
        Self {
            db,
            provider_costs: default_provider_costs(),
            markup_percent: 1.3, // 30% markup
            trial_days: 3,
            trial_token_limit: 100_000,
        }
    }

    /// Create with custom configuration
    pub fn with_config(
        db: Arc<Mutex<Connection>>,
        markup_percent: f64,
        trial_days: i64,
        trial_token_limit: i64,
    ) -> Self {
        Self {
            db,
            provider_costs: default_provider_costs(),
            markup_percent,
            trial_days,
            trial_token_limit,
        }
    }

    /// Add or update provider cost
    pub fn set_provider_cost(&mut self, key: &str, cost: ProviderCost) {
        self.provider_costs.insert(key.to_string(), cost);
    }

    /// Calculate cost in cents for token usage
    pub fn calculate_cost(
        &self,
        provider: &str,
        model: &str,
        input_tokens: u32,
        output_tokens: u32,
    ) -> i64 {
        let key = format!("{}:{}", provider, model);

        // Try exact match first, then provider-only match
        let cost_config = self
            .provider_costs
            .get(&key)
            .or_else(|| {
                // Try to find by provider name only
                self.provider_costs
                    .values()
                    .find(|c| c.name == provider || key.starts_with(&format!("{}:", c.name)))
            })
            .cloned();

        match cost_config {
            Some(config) => {
                let input_cost = (input_tokens as f64 / 1_000_000.0) * config.input_cost_per_1m;
                let output_cost = (output_tokens as f64 / 1_000_000.0) * config.output_cost_per_1m;
                let total_usd = input_cost + output_cost;
                (total_usd * 100.0).round() as i64 // Convert to cents
            }
            None => {
                // Unknown provider - estimate conservatively
                let total_tokens = input_tokens + output_tokens;
                let estimated_usd = (total_tokens as f64 / 1_000_000.0) * 1.0; // $1 per 1M as fallback
                (estimated_usd * 100.0).round() as i64
            }
        }
    }

    /// Record token usage for a user
    pub fn record_usage(
        &self,
        user_id: &str,
        provider: &str,
        model: &str,
        input_tokens: u32,
        output_tokens: u32,
        description: Option<&str>,
    ) -> Result<TokenUsage> {
        let cost_cents = self.calculate_cost(provider, model, input_tokens, output_tokens);
        let price_cents = (cost_cents as f64 * self.markup_percent).round() as i64;
        let total_tokens = input_tokens as i64 + output_tokens as i64;

        let tx_id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();

        let db = self.db.lock();

        // Record transaction
        db.execute(
            r#"INSERT INTO token_transactions
               (id, user_id, amount, cost_cents, price_cents, provider, model, description, created_at)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)"#,
            params![
                &tx_id,
                user_id,
                -total_tokens, // Negative = usage
                cost_cents,
                price_cents,
                provider,
                model,
                description,
                &now
            ],
        )
        .context("Failed to record token transaction")?;

        // Update trial tokens used
        db.execute(
            "UPDATE subscriptions SET trial_tokens_used = trial_tokens_used + ?1 WHERE user_id = ?2",
            params![total_tokens, user_id],
        )
        .context("Failed to update trial tokens")?;

        Ok(TokenUsage {
            user_id: user_id.to_string(),
            provider: provider.to_string(),
            model: model.to_string(),
            input_tokens,
            output_tokens,
            cost_cents,
            price_cents,
            created_at: now,
        })
    }

    /// Check if user has access based on subscription status
    pub fn check_access(&self, user_id: &str) -> Result<AccessResult> {
        let db = self.db.lock();

        let result: Result<(String, Option<String>, i64, i64, Option<String>), _> = db.query_row(
            r#"SELECT status, trial_started_at, trial_tokens_used, trial_tokens_limit, paid_until
               FROM subscriptions WHERE user_id = ?1"#,
            params![user_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
        );

        match result {
            Ok((status, trial_started_at, trial_tokens_used, trial_tokens_limit, paid_until)) => {
                match status.as_str() {
                    "active" => {
                        // Check if paid subscription is still valid
                        if let Some(paid_until_str) = paid_until {
                            if let Ok(paid_until_dt) =
                                chrono::DateTime::parse_from_rfc3339(&paid_until_str)
                            {
                                if Utc::now() > paid_until_dt {
                                    // Subscription expired
                                    return Ok(AccessResult::SubscriptionRequired);
                                }
                            }
                        }
                        Ok(AccessResult::Allowed)
                    }
                    "trial" => {
                        // Check token limit
                        if trial_tokens_used >= trial_tokens_limit {
                            return Ok(AccessResult::TrialExhausted);
                        }

                        // Check trial duration (3 days)
                        if let Some(started_at) = trial_started_at {
                            if let Ok(started_dt) = chrono::DateTime::parse_from_rfc3339(&started_at)
                            {
                                let trial_end = started_dt + Duration::days(self.trial_days);
                                if Utc::now() > trial_end {
                                    return Ok(AccessResult::TrialExpired);
                                }
                            }
                        }

                        Ok(AccessResult::Allowed)
                    }
                    "expired" => Ok(AccessResult::SubscriptionRequired),
                    _ => Ok(AccessResult::SubscriptionRequired),
                }
            }
            Err(_) => Ok(AccessResult::UserNotFound),
        }
    }

    /// Get remaining trial tokens
    pub fn get_trial_remaining(&self, user_id: &str) -> Result<i64> {
        let db = self.db.lock();

        let (used, limit): (i64, i64) = db
            .query_row(
                "SELECT trial_tokens_used, trial_tokens_limit FROM subscriptions WHERE user_id = ?1",
                params![user_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .map_err(|_| anyhow::anyhow!("Subscription not found"))?;

        Ok((limit - used).max(0))
    }

    /// Get total tokens used by user
    pub fn get_total_usage(&self, user_id: &str) -> Result<i64> {
        let db = self.db.lock();

        let total: i64 = db
            .query_row(
                "SELECT COALESCE(SUM(ABS(amount)), 0) FROM token_transactions WHERE user_id = ?1 AND amount < 0",
                params![user_id],
                |row| row.get(0),
            )
            .unwrap_or(0);

        Ok(total)
    }

    /// Get usage history for user
    pub fn get_usage_history(&self, user_id: &str, limit: usize) -> Result<Vec<TokenUsage>> {
        let db = self.db.lock();

        let mut stmt = db.prepare(
            r#"SELECT user_id, provider, model, ABS(amount) as tokens, cost_cents, price_cents, created_at
               FROM token_transactions
               WHERE user_id = ?1 AND amount < 0
               ORDER BY created_at DESC
               LIMIT ?2"#,
        )?;

        let rows = stmt.query_map(params![user_id, limit as i64], |row| {
            Ok(TokenUsage {
                user_id: row.get(0)?,
                provider: row.get(1)?,
                model: row.get(2)?,
                input_tokens: 0,  // Not stored separately
                output_tokens: 0, // Not stored separately
                cost_cents: row.get(4)?,
                price_cents: row.get(5)?,
                created_at: row.get(6)?,
            })
        })?;

        let mut history = Vec::new();
        for row in rows {
            history.push(row?);
        }

        Ok(history)
    }

    /// Activate subscription (after payment)
    pub fn activate_subscription(&self, user_id: &str, days: i64) -> Result<()> {
        let paid_until = (Utc::now() + Duration::days(days)).to_rfc3339();

        let db = self.db.lock();

        db.execute(
            "UPDATE subscriptions SET status = 'active', paid_until = ?1 WHERE user_id = ?2",
            params![&paid_until, user_id],
        )
        .context("Failed to activate subscription")?;

        Ok(())
    }

    /// Add purchased tokens to user account
    pub fn add_tokens(&self, user_id: &str, tokens: i64, price_cents: i64) -> Result<()> {
        let tx_id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();

        let db = self.db.lock();

        // Record purchase transaction
        db.execute(
            r#"INSERT INTO token_transactions
               (id, user_id, amount, cost_cents, price_cents, description, created_at)
               VALUES (?1, ?2, ?3, 0, ?4, 'Token purchase', ?5)"#,
            params![&tx_id, user_id, tokens, price_cents, &now],
        )
        .context("Failed to record token purchase")?;

        // Update total purchased
        db.execute(
            "UPDATE subscriptions SET total_tokens_purchased = total_tokens_purchased + ?1 WHERE user_id = ?2",
            params![tokens, user_id],
        )
        .context("Failed to update purchased tokens")?;

        Ok(())
    }

    /// Get total cost (our cost) for a user
    pub fn get_total_cost(&self, user_id: &str) -> Result<i64> {
        let db = self.db.lock();

        let total: i64 = db
            .query_row(
                "SELECT COALESCE(SUM(cost_cents), 0) FROM token_transactions WHERE user_id = ?1 AND amount < 0",
                params![user_id],
                |row| row.get(0),
            )
            .unwrap_or(0);

        Ok(total)
    }

    /// Get total revenue (what users paid) for a user
    pub fn get_total_revenue(&self, user_id: &str) -> Result<i64> {
        let db = self.db.lock();

        let total: i64 = db
            .query_row(
                "SELECT COALESCE(SUM(price_cents), 0) FROM token_transactions WHERE user_id = ?1",
                params![user_id],
                |row| row.get(0),
            )
            .unwrap_or(0);

        Ok(total)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn test_db() -> (TempDir, Arc<Mutex<Connection>>) {
        let tmp = TempDir::new().unwrap();
        let db_path = tmp.path().join("test.db");
        let conn = Connection::open(&db_path).unwrap();

        // Create schema
        conn.execute_batch(
            r#"
            CREATE TABLE subscriptions (
                user_id TEXT PRIMARY KEY,
                status TEXT NOT NULL DEFAULT 'trial',
                trial_started_at TEXT,
                trial_tokens_used INTEGER DEFAULT 0,
                trial_tokens_limit INTEGER DEFAULT 100000,
                paid_until TEXT,
                total_tokens_purchased INTEGER DEFAULT 0
            );

            CREATE TABLE token_transactions (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                amount INTEGER NOT NULL,
                cost_cents INTEGER,
                price_cents INTEGER,
                provider TEXT,
                model TEXT,
                description TEXT,
                created_at TEXT NOT NULL
            );
            "#,
        )
        .unwrap();

        (tmp, Arc::new(Mutex::new(conn)))
    }

    fn create_test_user(db: &Arc<Mutex<Connection>>, user_id: &str) {
        let conn = db.lock();
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO subscriptions (user_id, status, trial_started_at) VALUES (?1, 'trial', ?2)",
            params![user_id, &now],
        )
        .unwrap();
    }

    #[test]
    fn test_calculate_cost_groq_free() {
        let (_tmp, db) = test_db();
        let meter = TokenMeter::new(db);

        let cost = meter.calculate_cost("groq", "llama-3.3-70b-versatile", 1000, 500);
        assert_eq!(cost, 0); // Groq is free
    }

    #[test]
    fn test_calculate_cost_anthropic() {
        let (_tmp, db) = test_db();
        let meter = TokenMeter::new(db);

        // 1M input tokens @ $3 + 1M output tokens @ $15 = $18 = 1800 cents
        // Note: Key must match exactly as "anthropic:claude-3.5-sonnet" in default_provider_costs()
        let cost = meter.calculate_cost("anthropic", "claude-3.5-sonnet", 1_000_000, 1_000_000);
        assert_eq!(cost, 1800);
    }

    #[test]
    fn test_record_usage() {
        let (_tmp, db) = test_db();
        create_test_user(&db, "user123");
        let meter = TokenMeter::new(db);

        let usage = meter
            .record_usage("user123", "groq", "llama-3.3-70b", 100, 50, Some("test"))
            .unwrap();

        assert_eq!(usage.user_id, "user123");
        assert_eq!(usage.input_tokens, 100);
        assert_eq!(usage.output_tokens, 50);
        assert_eq!(usage.cost_cents, 0); // Groq is free
    }

    #[test]
    fn test_check_access_trial() {
        let (_tmp, db) = test_db();
        create_test_user(&db, "user123");
        let meter = TokenMeter::new(db);

        let access = meter.check_access("user123").unwrap();
        assert_eq!(access, AccessResult::Allowed);
    }

    #[test]
    fn test_trial_token_exhaustion() {
        let (_tmp, db) = test_db();
        create_test_user(&db, "user123");

        // Set tokens used to limit
        {
            let conn = db.lock();
            conn.execute(
                "UPDATE subscriptions SET trial_tokens_used = trial_tokens_limit WHERE user_id = 'user123'",
                [],
            )
            .unwrap();
        }

        let meter = TokenMeter::new(db);
        let access = meter.check_access("user123").unwrap();
        assert_eq!(access, AccessResult::TrialExhausted);
    }

    #[test]
    fn test_activate_subscription() {
        let (_tmp, db) = test_db();
        create_test_user(&db, "user123");
        let meter = TokenMeter::new(db.clone());

        meter.activate_subscription("user123", 30).unwrap();

        let access = meter.check_access("user123").unwrap();
        assert_eq!(access, AccessResult::Allowed);

        // Verify status changed
        let conn = db.lock();
        let status: String = conn
            .query_row(
                "SELECT status FROM subscriptions WHERE user_id = 'user123'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(status, "active");
    }

    #[test]
    fn test_get_trial_remaining() {
        let (_tmp, db) = test_db();
        create_test_user(&db, "user123");
        let meter = TokenMeter::new(db);

        // Record some usage
        meter
            .record_usage("user123", "groq", "llama", 5000, 5000, None)
            .unwrap();

        let remaining = meter.get_trial_remaining("user123").unwrap();
        assert_eq!(remaining, 100_000 - 10_000);
    }
}
