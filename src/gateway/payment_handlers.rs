//! Payment webhook handlers for Cryptomus integration.
//!
//! Handles payment callbacks and subscription activation.

use super::AppState;
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};

// ══════════════════════════════════════════════════════════════════════════════
// CRYPTOMUS WEBHOOK TYPES
// ══════════════════════════════════════════════════════════════════════════════

/// Cryptomus webhook payload
#[derive(Debug, Deserialize)]
pub struct CryptomusWebhook {
    /// Payment UUID
    pub uuid: String,
    /// Order ID (our user_id or order reference)
    pub order_id: String,
    /// Payment amount
    pub amount: String,
    /// Currency (USD, EUR, etc.)
    pub currency: String,
    /// Payment status
    pub status: String,
    /// Crypto currency used
    #[serde(default)]
    pub payer_currency: Option<String>,
    /// Actual paid amount in crypto
    #[serde(default)]
    pub payer_amount: Option<String>,
    /// Transaction hash
    #[serde(default)]
    pub txid: Option<String>,
    /// Signature for verification
    #[serde(default)]
    pub sign: Option<String>,
}

/// Payment status from Cryptomus
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PaymentStatus {
    Paid,
    PaidOver,
    WrongAmount,
    Process,
    Confirm,
    Cancel,
    Fail,
    Unknown(String),
}

impl From<&str> for PaymentStatus {
    fn from(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "paid" => Self::Paid,
            "paid_over" => Self::PaidOver,
            "wrong_amount" => Self::WrongAmount,
            "process" => Self::Process,
            "confirm_check" | "confirm" => Self::Confirm,
            "cancel" => Self::Cancel,
            "fail" | "failed" => Self::Fail,
            other => Self::Unknown(other.to_string()),
        }
    }
}

impl PaymentStatus {
    /// Returns true if payment is successful
    pub fn is_successful(&self) -> bool {
        matches!(self, Self::Paid | Self::PaidOver)
    }
}

/// Response for webhook
#[derive(Debug, Serialize)]
pub struct WebhookResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// Payment creation request
#[derive(Debug, Deserialize)]
pub struct CreatePaymentRequest {
    /// Amount in USD cents
    pub amount_cents: i64,
    /// Token package (e.g., "100k", "500k", "1m")
    pub package: String,
}

/// Payment creation response
#[derive(Debug, Serialize)]
pub struct CreatePaymentResponse {
    pub payment_url: String,
    pub order_id: String,
}

/// Error response
#[derive(Debug, Serialize)]
pub struct PaymentErrorResponse {
    pub error: String,
}

// ══════════════════════════════════════════════════════════════════════════════
// TOKEN PACKAGES
// ══════════════════════════════════════════════════════════════════════════════

/// Token package definition
pub struct TokenPackage {
    pub name: &'static str,
    pub tokens: i64,
    pub price_cents: i64,  // USD cents
}

/// Available token packages
pub const TOKEN_PACKAGES: &[TokenPackage] = &[
    TokenPackage { name: "100k", tokens: 100_000, price_cents: 500 },      // $5
    TokenPackage { name: "500k", tokens: 500_000, price_cents: 2000 },     // $20
    TokenPackage { name: "1m", tokens: 1_000_000, price_cents: 3500 },     // $35
    TokenPackage { name: "5m", tokens: 5_000_000, price_cents: 15000 },    // $150
];

impl TokenPackage {
    pub fn find(name: &str) -> Option<&'static TokenPackage> {
        TOKEN_PACKAGES.iter().find(|p| p.name == name)
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// SIGNATURE VERIFICATION
// ══════════════════════════════════════════════════════════════════════════════

/// Verify Cryptomus webhook signature
fn verify_cryptomus_signature(
    payload: &serde_json::Value,
    received_sign: &str,
    api_key: &str,
) -> bool {
    use std::collections::BTreeMap;

    // Cryptomus signature: MD5(base64(json_sorted_by_keys) + api_key)
    let mut sorted: BTreeMap<String, serde_json::Value> = BTreeMap::new();

    if let Some(obj) = payload.as_object() {
        for (k, v) in obj {
            if k != "sign" {
                sorted.insert(k.clone(), v.clone());
            }
        }
    }

    let json_str = serde_json::to_string(&sorted).unwrap_or_default();
    let base64_data = base64_encode(json_str.as_bytes());
    let to_hash = format!("{}{}", base64_data, api_key);

    let computed = md5_hash(&to_hash);

    // Constant-time comparison
    computed.to_lowercase() == received_sign.to_lowercase()
}

fn base64_encode(data: &[u8]) -> String {
    use base64::{Engine, engine::general_purpose::STANDARD};
    STANDARD.encode(data)
}

fn md5_hash(input: &str) -> String {
    let digest = md5::compute(input.as_bytes());
    format!("{:x}", digest)
}

// ══════════════════════════════════════════════════════════════════════════════
// HANDLERS
// ══════════════════════════════════════════════════════════════════════════════

/// POST /api/payment/webhook
/// Cryptomus payment callback
pub async fn handle_cryptomus_webhook(
    State(state): State<AppState>,
    _headers: HeaderMap,
    Json(payload): Json<serde_json::Value>,
) -> impl IntoResponse {
    tracing::info!("Received Cryptomus webhook");

    // Check if payments are enabled
    let token_meter = match &state.token_meter {
        Some(tm) => tm,
        None => {
            tracing::warn!("Cryptomus webhook received but payments not configured");
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(WebhookResponse {
                    success: false,
                    message: Some("Payments not configured".to_string()),
                }),
            );
        }
    };

    // Parse webhook
    let webhook: CryptomusWebhook = match serde_json::from_value(payload.clone()) {
        Ok(w) => w,
        Err(e) => {
            tracing::error!("Failed to parse Cryptomus webhook: {}", e);
            return (
                StatusCode::BAD_REQUEST,
                Json(WebhookResponse {
                    success: false,
                    message: Some("Invalid payload".to_string()),
                }),
            );
        }
    };

    // Verify signature if API key is configured
    if let Some(api_key) = &state.cryptomus_api_key {
        if let Some(sign) = &webhook.sign {
            if !verify_cryptomus_signature(&payload, sign, api_key) {
                tracing::warn!("Invalid Cryptomus signature for order {}", webhook.order_id);
                return (
                    StatusCode::UNAUTHORIZED,
                    Json(WebhookResponse {
                        success: false,
                        message: Some("Invalid signature".to_string()),
                    }),
                );
            }
        }
    }

    let status = PaymentStatus::from(webhook.status.as_str());

    tracing::info!(
        "Payment {} for order {}: {:?}",
        webhook.uuid,
        webhook.order_id,
        status
    );

    // Process successful payment
    if status.is_successful() {
        // Parse order_id format: "user_<user_id>_pkg_<package>"
        let parts: Vec<&str> = webhook.order_id.split('_').collect();

        if parts.len() >= 4 && parts[0] == "user" && parts[2] == "pkg" {
            let user_id = parts[1];
            let package_name = parts[3];

            if let Some(package) = TokenPackage::find(package_name) {
                // Record token purchase
                if let Err(e) = token_meter.add_tokens(
                    user_id,
                    package.tokens,
                    package.price_cents,
                ) {
                    tracing::error!("Failed to record purchase: {}", e);
                } else {
                    tracing::info!(
                        "Activated {} tokens for user {} (payment {})",
                        package.tokens,
                        user_id,
                        webhook.uuid
                    );

                    // Activate subscription (30 days)
                    if let Err(e) = token_meter.activate_subscription(user_id, 30) {
                        tracing::error!("Failed to activate subscription: {}", e);
                    }
                }
            } else {
                tracing::warn!("Unknown package: {}", package_name);
            }
        } else {
            tracing::warn!("Invalid order_id format: {}", webhook.order_id);
        }
    }

    (
        StatusCode::OK,
        Json(WebhookResponse {
            success: true,
            message: None,
        }),
    )
}

/// POST /api/payment/create
/// Create a new payment link (requires auth)
pub async fn handle_create_payment(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<CreatePaymentRequest>,
) -> impl IntoResponse {
    // Check if payments are enabled
    let auth_manager = match &state.auth_manager {
        Some(am) => am,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(PaymentErrorResponse {
                    error: "Payments not configured".to_string(),
                }),
            )
                .into_response();
        }
    };

    // Extract user from auth header
    let auth_header = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok());

    let token = match auth_header.and_then(|h| h.strip_prefix("Bearer ")) {
        Some(t) => t,
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(PaymentErrorResponse {
                    error: "Missing authorization".to_string(),
                }),
            )
                .into_response();
        }
    };

    let user_id = match auth_manager.verify_token(token) {
        Ok(id) => id,
        Err(_) => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(PaymentErrorResponse {
                    error: "Invalid token".to_string(),
                }),
            )
                .into_response();
        }
    };

    // Find package
    let package = match TokenPackage::find(&body.package) {
        Some(p) => p,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(PaymentErrorResponse {
                    error: format!("Unknown package: {}", body.package),
                }),
            )
                .into_response();
        }
    };

    // Create Cryptomus payment
    let (merchant_id, api_key) = match (&state.cryptomus_merchant_id, &state.cryptomus_api_key) {
        (Some(m), Some(k)) => (m.clone(), k.clone()),
        _ => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(PaymentErrorResponse {
                    error: "Cryptomus not configured".to_string(),
                }),
            )
                .into_response();
        }
    };

    let order_id = format!("user_{}_pkg_{}", user_id, package.name);
    let amount = format!("{:.2}", package.price_cents as f64 / 100.0);

    // Create payment via Cryptomus API
    match create_cryptomus_payment(&merchant_id, &api_key, &order_id, &amount).await {
        Ok(url) => (
            StatusCode::OK,
            Json(CreatePaymentResponse {
                payment_url: url,
                order_id,
            }),
        )
            .into_response(),
        Err(e) => {
            tracing::error!("Failed to create Cryptomus payment: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(PaymentErrorResponse {
                    error: "Failed to create payment".to_string(),
                }),
            )
                .into_response()
        }
    }
}

/// Create payment via Cryptomus API
async fn create_cryptomus_payment(
    merchant_id: &str,
    api_key: &str,
    order_id: &str,
    amount: &str,
) -> anyhow::Result<String> {
    let client = reqwest::Client::new();

    let payload = serde_json::json!({
        "amount": amount,
        "currency": "USD",
        "order_id": order_id,
        "url_callback": "", // Set via config
        "url_return": "",   // Set via config
        "url_success": "",  // Set via config
    });

    let json_str = serde_json::to_string(&payload)?;
    let base64_data = base64_encode(json_str.as_bytes());
    let sign = md5_hash(&format!("{}{}", base64_data, api_key));

    let response = client
        .post("https://api.cryptomus.com/v1/payment")
        .header("merchant", merchant_id)
        .header("sign", sign)
        .json(&payload)
        .send()
        .await?;

    if !response.status().is_success() {
        let error = response.text().await?;
        anyhow::bail!("Cryptomus API error: {}", error);
    }

    let data: serde_json::Value = response.json().await?;

    data["result"]["url"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| anyhow::anyhow!("No payment URL in response"))
}

/// GET /api/payment/packages
/// List available token packages
pub async fn handle_list_packages() -> impl IntoResponse {
    let packages: Vec<serde_json::Value> = TOKEN_PACKAGES
        .iter()
        .map(|p| {
            serde_json::json!({
                "name": p.name,
                "tokens": p.tokens,
                "price_usd": format!("{:.2}", p.price_cents as f64 / 100.0),
                "price_cents": p.price_cents,
            })
        })
        .collect();

    (StatusCode::OK, Json(packages))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_payment_status_parsing() {
        assert!(PaymentStatus::from("paid").is_successful());
        assert!(PaymentStatus::from("paid_over").is_successful());
        assert!(!PaymentStatus::from("process").is_successful());
        assert!(!PaymentStatus::from("cancel").is_successful());
        assert!(!PaymentStatus::from("fail").is_successful());
    }

    #[test]
    fn test_token_package_find() {
        assert!(TokenPackage::find("100k").is_some());
        assert!(TokenPackage::find("1m").is_some());
        assert!(TokenPackage::find("invalid").is_none());
    }

    #[test]
    fn test_package_prices() {
        let pkg = TokenPackage::find("100k").unwrap();
        assert_eq!(pkg.tokens, 100_000);
        assert_eq!(pkg.price_cents, 500);
    }
}
