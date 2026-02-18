//! Authentication module for AI-Mentor SaaS platform.
//!
//! Handles user registration, login, JWT tokens, and Telegram account linking.

use anyhow::{bail, Context, Result};
use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use parking_lot::Mutex;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Arc;
use uuid::Uuid;

/// JWT claims structure
#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    /// User ID
    pub sub: String,
    /// Expiration timestamp (Unix)
    pub exp: i64,
    /// Issued at timestamp (Unix)
    pub iat: i64,
}

/// User data from database
#[derive(Debug, Clone)]
pub struct User {
    pub id: String,
    pub email: String,
    pub telegram_id: Option<String>,
    pub telegram_username: Option<String>,
    pub created_at: String,
}

/// Subscription status
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SubscriptionStatus {
    Trial,
    Active,
    Expired,
}

impl std::fmt::Display for SubscriptionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Trial => write!(f, "trial"),
            Self::Active => write!(f, "active"),
            Self::Expired => write!(f, "expired"),
        }
    }
}

impl std::str::FromStr for SubscriptionStatus {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "trial" => Ok(Self::Trial),
            "active" => Ok(Self::Active),
            "expired" => Ok(Self::Expired),
            _ => bail!("Unknown subscription status: {}", s),
        }
    }
}

/// Subscription data
#[derive(Debug, Clone)]
pub struct Subscription {
    pub user_id: String,
    pub status: SubscriptionStatus,
    pub trial_started_at: Option<String>,
    pub trial_tokens_used: i64,
    pub trial_tokens_limit: i64,
    pub paid_until: Option<String>,
    pub total_tokens_purchased: i64,
}

/// Pending Telegram link code
#[derive(Debug, Clone)]
pub struct TelegramLinkCode {
    pub code: String,
    pub user_id: String,
    pub expires_at: String,
}

/// Authentication manager
pub struct AuthManager {
    db: Arc<Mutex<Connection>>,
    jwt_secret: String,
    jwt_expiry_hours: i64,
}

impl AuthManager {
    /// Create new AuthManager with central database
    pub fn new(workspace_dir: &Path, jwt_secret: String) -> Result<Self> {
        let db_path = workspace_dir.join("central.db");

        // Ensure parent directory exists
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let conn = Connection::open(&db_path)
            .with_context(|| format!("Failed to open central DB at {:?}", db_path))?;

        // Initialize schema
        Self::init_schema(&conn)?;

        Ok(Self {
            db: Arc::new(Mutex::new(conn)),
            jwt_secret,
            jwt_expiry_hours: 24 * 7, // 7 days
        })
    }

    /// Initialize database schema
    fn init_schema(conn: &Connection) -> Result<()> {
        conn.execute_batch(
            r#"
            -- SQLite optimizations
            PRAGMA journal_mode = WAL;
            PRAGMA synchronous = NORMAL;
            PRAGMA foreign_keys = ON;

            -- Users table
            CREATE TABLE IF NOT EXISTS users (
                id TEXT PRIMARY KEY,
                email TEXT UNIQUE NOT NULL,
                password_hash TEXT NOT NULL,
                telegram_id TEXT UNIQUE,
                telegram_username TEXT,
                created_at TEXT NOT NULL,
                last_login TEXT
            );

            -- Subscriptions table
            CREATE TABLE IF NOT EXISTS subscriptions (
                user_id TEXT PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
                status TEXT NOT NULL DEFAULT 'trial',
                trial_started_at TEXT,
                trial_tokens_used INTEGER DEFAULT 0,
                trial_tokens_limit INTEGER DEFAULT 100000,
                paid_until TEXT,
                total_tokens_purchased INTEGER DEFAULT 0
            );

            -- Token transactions (billing history)
            CREATE TABLE IF NOT EXISTS token_transactions (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
                amount INTEGER NOT NULL,
                cost_cents INTEGER,
                price_cents INTEGER,
                provider TEXT,
                model TEXT,
                description TEXT,
                created_at TEXT NOT NULL
            );

            -- Pending Telegram link codes
            CREATE TABLE IF NOT EXISTS telegram_link_codes (
                code TEXT PRIMARY KEY,
                user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
                expires_at TEXT NOT NULL
            );

            -- Sessions (optional, for refresh tokens)
            CREATE TABLE IF NOT EXISTS sessions (
                token TEXT PRIMARY KEY,
                user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
                expires_at TEXT NOT NULL,
                created_at TEXT NOT NULL
            );

            -- Indexes
            CREATE INDEX IF NOT EXISTS idx_users_email ON users(email);
            CREATE INDEX IF NOT EXISTS idx_users_telegram_id ON users(telegram_id);
            CREATE INDEX IF NOT EXISTS idx_token_transactions_user ON token_transactions(user_id);
            CREATE INDEX IF NOT EXISTS idx_telegram_link_codes_user ON telegram_link_codes(user_id);
            "#,
        )
        .context("Failed to initialize auth schema")?;

        Ok(())
    }

    /// Register a new user
    pub fn register(&self, email: &str, password: &str) -> Result<(String, String)> {
        let email = email.trim().to_lowercase();

        // Validate email format (basic check)
        if !email.contains('@') || !email.contains('.') {
            bail!("Invalid email format");
        }

        // Validate password length
        if password.len() < 8 {
            bail!("Password must be at least 8 characters");
        }

        // Hash password
        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();
        let password_hash = argon2
            .hash_password(password.as_bytes(), &salt)
            .map_err(|e| anyhow::anyhow!("Failed to hash password: {}", e))?
            .to_string();

        let user_id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();

        let db = self.db.lock();

        // Check if email already exists
        let exists: bool = db
            .query_row(
                "SELECT 1 FROM users WHERE email = ?1",
                params![&email],
                |_| Ok(true),
            )
            .unwrap_or(false);

        if exists {
            bail!("Email already registered");
        }

        // Insert user
        db.execute(
            "INSERT INTO users (id, email, password_hash, created_at) VALUES (?1, ?2, ?3, ?4)",
            params![&user_id, &email, &password_hash, &now],
        )
        .context("Failed to create user")?;

        // Create subscription (trial by default)
        db.execute(
            "INSERT INTO subscriptions (user_id, status, trial_started_at) VALUES (?1, 'trial', ?2)",
            params![&user_id, &now],
        )
        .context("Failed to create subscription")?;

        drop(db);

        // Generate JWT
        let token = self.generate_jwt(&user_id)?;

        Ok((user_id, token))
    }

    /// Login with email and password
    pub fn login(&self, email: &str, password: &str) -> Result<(String, String)> {
        let email = email.trim().to_lowercase();

        let db = self.db.lock();

        let (user_id, password_hash): (String, String) = db
            .query_row(
                "SELECT id, password_hash FROM users WHERE email = ?1",
                params![&email],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .map_err(|_| anyhow::anyhow!("Invalid email or password"))?;

        // Verify password
        let parsed_hash =
            PasswordHash::new(&password_hash).map_err(|e| anyhow::anyhow!("Hash error: {}", e))?;

        Argon2::default()
            .verify_password(password.as_bytes(), &parsed_hash)
            .map_err(|_| anyhow::anyhow!("Invalid email or password"))?;

        // Update last login
        let now = Utc::now().to_rfc3339();
        db.execute(
            "UPDATE users SET last_login = ?1 WHERE id = ?2",
            params![&now, &user_id],
        )?;

        drop(db);

        // Generate JWT
        let token = self.generate_jwt(&user_id)?;

        Ok((user_id, token))
    }

    /// Generate JWT token for user
    fn generate_jwt(&self, user_id: &str) -> Result<String> {
        let now = Utc::now();
        let exp = now + Duration::hours(self.jwt_expiry_hours);

        let claims = Claims {
            sub: user_id.to_string(),
            exp: exp.timestamp(),
            iat: now.timestamp(),
        };

        let token = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(self.jwt_secret.as_bytes()),
        )
        .context("Failed to generate JWT")?;

        Ok(token)
    }

    /// Verify JWT token and return user ID
    pub fn verify_token(&self, token: &str) -> Result<String> {
        let token_data = decode::<Claims>(
            token,
            &DecodingKey::from_secret(self.jwt_secret.as_bytes()),
            &Validation::default(),
        )
        .map_err(|e| anyhow::anyhow!("Invalid token: {}", e))?;

        Ok(token_data.claims.sub)
    }

    /// Generate one-time Telegram link code
    pub fn generate_telegram_link(&self, user_id: &str) -> Result<String> {
        let code = format!("tg_{}", Uuid::new_v4().to_string().replace("-", "")[..16].to_string());
        let expires_at = (Utc::now() + Duration::hours(1)).to_rfc3339();

        let db = self.db.lock();

        // Remove any existing codes for this user
        db.execute(
            "DELETE FROM telegram_link_codes WHERE user_id = ?1",
            params![user_id],
        )?;

        // Insert new code
        db.execute(
            "INSERT INTO telegram_link_codes (code, user_id, expires_at) VALUES (?1, ?2, ?3)",
            params![&code, user_id, &expires_at],
        )
        .context("Failed to create Telegram link code")?;

        Ok(code)
    }

    /// Validate Telegram link code and return user ID
    pub fn validate_telegram_code(&self, code: &str) -> Result<String> {
        let db = self.db.lock();

        let (user_id, expires_at): (String, String) = db
            .query_row(
                "SELECT user_id, expires_at FROM telegram_link_codes WHERE code = ?1",
                params![code],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .map_err(|_| anyhow::anyhow!("Invalid or expired link code"))?;

        // Check expiration
        let expires = chrono::DateTime::parse_from_rfc3339(&expires_at)
            .map_err(|_| anyhow::anyhow!("Invalid expiration date"))?;

        if Utc::now() > expires {
            // Delete expired code
            db.execute(
                "DELETE FROM telegram_link_codes WHERE code = ?1",
                params![code],
            )?;
            bail!("Link code has expired");
        }

        // Delete used code
        db.execute(
            "DELETE FROM telegram_link_codes WHERE code = ?1",
            params![code],
        )?;

        Ok(user_id)
    }

    /// Link Telegram account to user
    pub fn link_telegram(
        &self,
        user_id: &str,
        telegram_id: &str,
        telegram_username: Option<&str>,
    ) -> Result<()> {
        let db = self.db.lock();

        // Check if Telegram ID is already linked to another account
        let existing: Option<String> = db
            .query_row(
                "SELECT id FROM users WHERE telegram_id = ?1 AND id != ?2",
                params![telegram_id, user_id],
                |row| row.get(0),
            )
            .ok();

        if existing.is_some() {
            bail!("This Telegram account is already linked to another user");
        }

        db.execute(
            "UPDATE users SET telegram_id = ?1, telegram_username = ?2 WHERE id = ?3",
            params![telegram_id, telegram_username, user_id],
        )
        .context("Failed to link Telegram account")?;

        Ok(())
    }

    /// Get user by ID
    pub fn get_user(&self, user_id: &str) -> Result<User> {
        let db = self.db.lock();

        db.query_row(
            "SELECT id, email, telegram_id, telegram_username, created_at FROM users WHERE id = ?1",
            params![user_id],
            |row| {
                Ok(User {
                    id: row.get(0)?,
                    email: row.get(1)?,
                    telegram_id: row.get(2)?,
                    telegram_username: row.get(3)?,
                    created_at: row.get(4)?,
                })
            },
        )
        .map_err(|_| anyhow::anyhow!("User not found"))
    }

    /// Get user by Telegram ID
    pub fn get_user_by_telegram(&self, telegram_id: &str) -> Result<User> {
        let db = self.db.lock();

        db.query_row(
            "SELECT id, email, telegram_id, telegram_username, created_at FROM users WHERE telegram_id = ?1",
            params![telegram_id],
            |row| {
                Ok(User {
                    id: row.get(0)?,
                    email: row.get(1)?,
                    telegram_id: row.get(2)?,
                    telegram_username: row.get(3)?,
                    created_at: row.get(4)?,
                })
            },
        )
        .map_err(|_| anyhow::anyhow!("User not found"))
    }

    /// Get subscription for user
    pub fn get_subscription(&self, user_id: &str) -> Result<Subscription> {
        let db = self.db.lock();

        db.query_row(
            r#"SELECT user_id, status, trial_started_at, trial_tokens_used,
                      trial_tokens_limit, paid_until, total_tokens_purchased
               FROM subscriptions WHERE user_id = ?1"#,
            params![user_id],
            |row| {
                let status_str: String = row.get(1)?;
                Ok(Subscription {
                    user_id: row.get(0)?,
                    status: status_str.parse().unwrap_or(SubscriptionStatus::Expired),
                    trial_started_at: row.get(2)?,
                    trial_tokens_used: row.get(3)?,
                    trial_tokens_limit: row.get(4)?,
                    paid_until: row.get(5)?,
                    total_tokens_purchased: row.get(6)?,
                })
            },
        )
        .map_err(|_| anyhow::anyhow!("Subscription not found"))
    }

    /// Check if Telegram account is connected
    pub fn is_telegram_connected(&self, user_id: &str) -> Result<bool> {
        let user = self.get_user(user_id)?;
        Ok(user.telegram_id.is_some())
    }

    /// Get database connection for advanced operations
    pub fn db(&self) -> Arc<Mutex<Connection>> {
        Arc::clone(&self.db)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn test_auth_manager() -> (TempDir, AuthManager) {
        let tmp = TempDir::new().unwrap();
        let auth = AuthManager::new(tmp.path(), "test_secret_key_123".to_string()).unwrap();
        (tmp, auth)
    }

    #[test]
    fn test_register_and_login() {
        let (_tmp, auth) = test_auth_manager();

        // Register
        let (user_id, token) = auth.register("test@example.com", "password123").unwrap();
        assert!(!user_id.is_empty());
        assert!(!token.is_empty());

        // Login
        let (login_user_id, login_token) = auth.login("test@example.com", "password123").unwrap();
        assert_eq!(user_id, login_user_id);
        assert!(!login_token.is_empty());

        // Verify token
        let verified_user_id = auth.verify_token(&login_token).unwrap();
        assert_eq!(user_id, verified_user_id);
    }

    #[test]
    fn test_duplicate_email() {
        let (_tmp, auth) = test_auth_manager();

        auth.register("test@example.com", "password123").unwrap();

        let result = auth.register("test@example.com", "password456");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("already registered"));
    }

    #[test]
    fn test_invalid_login() {
        let (_tmp, auth) = test_auth_manager();

        auth.register("test@example.com", "password123").unwrap();

        let result = auth.login("test@example.com", "wrongpassword");
        assert!(result.is_err());
    }

    #[test]
    fn test_telegram_link() {
        let (_tmp, auth) = test_auth_manager();

        let (user_id, _) = auth.register("test@example.com", "password123").unwrap();

        // Generate link code
        let code = auth.generate_telegram_link(&user_id).unwrap();
        assert!(code.starts_with("tg_"));

        // Validate code
        let validated_user_id = auth.validate_telegram_code(&code).unwrap();
        assert_eq!(user_id, validated_user_id);

        // Code should be consumed (single use)
        let result = auth.validate_telegram_code(&code);
        assert!(result.is_err());
    }

    #[test]
    fn test_link_telegram_account() {
        let (_tmp, auth) = test_auth_manager();

        let (user_id, _) = auth.register("test@example.com", "password123").unwrap();

        auth.link_telegram(&user_id, "123456789", Some("testuser"))
            .unwrap();

        let user = auth.get_user(&user_id).unwrap();
        assert_eq!(user.telegram_id, Some("123456789".to_string()));
        assert_eq!(user.telegram_username, Some("testuser".to_string()));

        // Get user by telegram ID
        let user_by_tg = auth.get_user_by_telegram("123456789").unwrap();
        assert_eq!(user_by_tg.id, user_id);
    }

    #[test]
    fn test_subscription_created_on_register() {
        let (_tmp, auth) = test_auth_manager();

        let (user_id, _) = auth.register("test@example.com", "password123").unwrap();

        let sub = auth.get_subscription(&user_id).unwrap();
        assert_eq!(sub.status, SubscriptionStatus::Trial);
        assert_eq!(sub.trial_tokens_used, 0);
        assert_eq!(sub.trial_tokens_limit, 100000);
    }
}
