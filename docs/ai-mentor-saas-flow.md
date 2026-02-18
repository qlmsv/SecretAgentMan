# AI-Mentor SaaS Platform - Architecture & Flow

## Overview

AI-Mentor is a personal AI assistant SaaS built on ZeroClaw, focused on personal development, goal setting, and esoteric practices (Bazi, Destiny Matrix).

**Business Model**: Token-based billing with markup. Users pay for tokens consumed during AI interactions.

---

## System Architecture

```
                    ┌─────────────────────────────────────┐
                    │         Frontend (Next.js)          │
                    │  /register → /login → /connect-tg   │
                    └───────────────┬─────────────────────┘
                                    │ REST API
                                    ▼
┌───────────────────────────────────────────────────────────────────┐
│                     ZeroClaw Gateway (Rust/Axum)                  │
│  ┌─────────────┐  ┌─────────────┐  ┌───────────────────────────┐  │
│  │ Auth API    │  │ Payment API │  │ Telegram Webhook          │  │
│  │ /register   │  │ /packages   │  │ /telegram                 │  │
│  │ /login      │  │ /webhook    │  │                           │  │
│  │ /tg-link    │  │ /create     │  │                           │  │
│  └─────────────┘  └─────────────┘  └───────────────────────────┘  │
│                                                                   │
│  ┌───────────────────────────────────────────────────────────┐   │
│  │                    Core Components                         │   │
│  │  ┌──────────────┐ ┌──────────────┐ ┌────────────────────┐ │   │
│  │  │ AuthManager  │ │ TokenMeter   │ │ TenantManager      │ │   │
│  │  │ (JWT, Argon2)│ │ (Usage/Bill) │ │ (Per-user DBs)     │ │   │
│  │  └──────────────┘ └──────────────┘ └────────────────────┘ │   │
│  └───────────────────────────────────────────────────────────┘   │
└───────────────────────────────────────────────────────────────────┘
          │                    │                    │
          ▼                    ▼                    ▼
    ┌──────────┐        ┌──────────┐        ┌──────────────┐
    │Central DB│        │Tenant DBs│        │LLM Providers │
    │(users,   │        │per-user  │        │Groq/DeepSeek/│
    │billing)  │        │SQLite    │        │Gemini/Claude │
    └──────────┘        └──────────┘        └──────────────┘
```

---

## Part 1: Authentication Flow

### Registration Flow

```
User                    Frontend               Gateway            AuthManager
 │                         │                      │                    │
 │──(1) Fill form─────────>│                      │                    │
 │                         │──(2) POST /register─>│                    │
 │                         │                      │──(3) hash_password>│
 │                         │                      │     create_user    │
 │                         │                      │<─(4) user_id, jwt──│
 │<─(5) JWT cookie + tg link───────────────────────                    │
```

**Endpoint**: `POST /api/auth/register`

```json
// Request
{
  "email": "user@example.com",
  "password": "securepass123"
}

// Response (200 OK)
{
  "user_id": "usr_abc123",
  "token": "eyJ..."
}
```

**Implementation** ([auth/mod.rs:103-140](src/auth/mod.rs#L103-L140)):
1. Validate email format
2. Check email not already registered
3. Hash password with Argon2 (memory-hard, secure)
4. Generate UUID for user_id
5. Insert into `users` table
6. Create initial subscription (trial status)
7. Generate JWT token (24h expiry)

### Login Flow

**Endpoint**: `POST /api/auth/login`

```json
// Request
{
  "email": "user@example.com",
  "password": "securepass123"
}

// Response (200 OK)
{
  "user_id": "usr_abc123",
  "token": "eyJ..."
}
```

**Implementation**:
1. Look up user by email
2. Verify password against Argon2 hash
3. Generate new JWT token
4. Update `last_login` timestamp

### Telegram Linking Flow

```
User           Frontend          Gateway         Bot           Central DB
 │                │                 │              │                │
 │──(1) Click─────>                 │              │                │
 │   "Connect TG" │                 │              │                │
 │                │──(2) GET /tg-link>             │                │
 │                │<──(3) one-time code────────────│                │
 │                │                 │              │                │
 │<──(4) t.me/Bot?start=code────────│              │                │
 │                │                 │              │                │
 │──(5) Click link, open Telegram──>│              │                │
 │                │                 │──(6) /start code>             │
 │                │                 │              │──(7) validate──>│
 │                │                 │              │<─(8) user_id────│
 │                │                 │              │──(9) UPDATE────>│
 │<──(10) "Connected!" message──────────────────────                │
```

**Endpoint**: `GET /api/auth/telegram-link`
- Requires `Authorization: Bearer {jwt}`
- Returns `{ "link": "https://t.me/BotName?start={one_time_code}" }`

**Endpoint**: `GET /api/auth/telegram-status`
- Returns `{ "connected": true/false, "telegram_username": "..." }`

---

## Part 2: Payment Flow (Cryptomus)

### Token Packages

Defined in [payment_handlers.rs:122-127](src/gateway/payment_handlers.rs#L122-L127):

| Package | Tokens | Price |
|---------|--------|-------|
| 100k    | 100,000 | $5 |
| 500k    | 500,000 | $20 |
| 1m      | 1,000,000 | $35 |
| 5m      | 5,000,000 | $150 |

### Payment Creation Flow

```
User        Frontend       Gateway        Cryptomus API
 │             │              │                 │
 │──(1) Select package────────>                 │
 │             │──(2) POST /payment/create──────>
 │             │      (package, Bearer JWT)     │
 │             │              │──(3) create payment>
 │             │              │<─(4) payment_url───│
 │             │<─(5) {payment_url, order_id}───│
 │<──(6) Redirect to payment URL────────────────│
```

**Endpoint**: `POST /api/payment/create`

```json
// Request (Authorization: Bearer {jwt})
{
  "package": "100k"
}

// Response
{
  "payment_url": "https://pay.cryptomus.com/...",
  "order_id": "user_usr123_pkg_100k"
}
```

### Webhook Processing Flow

```
Cryptomus         Gateway                 TokenMeter
    │                │                        │
    │──(1) POST /payment/webhook──────────────>
    │    {uuid, order_id, status, sign}       │
    │                │                        │
    │                │──(2) verify signature──>│
    │                │                        │
    │                │──(3) parse order_id────>│
    │                │    user_usr123_pkg_100k│
    │                │                        │
    │                │──(4) if paid:          │
    │                │    add_tokens(100k)────>│
    │                │    activate_subscription│
    │<──(5) 200 OK {"success": true}───────────│
```

**Endpoint**: `POST /api/payment/webhook`

**Signature Verification** ([payment_handlers.rs:139-166](src/gateway/payment_handlers.rs#L139-L166)):
```
signature = MD5(base64(json_sorted_by_keys) + api_key)
```

**Payment Statuses**:
- `paid` / `paid_over` → Success → Activate tokens
- `process` / `confirm` → Pending
- `cancel` / `fail` → Failed

---

## Part 3: Token Metering & Billing

### TokenMeter Component

Location: [billing/mod.rs](src/billing/mod.rs)

**Key Methods**:

1. **`record_usage()`** - Track token consumption
```rust
pub fn record_usage(
    user_id: &str,
    provider: &str,
    model: &str,
    input_tokens: i64,
    output_tokens: i64,
) -> Result<()>
```

2. **`add_tokens()`** - Credit tokens after purchase
```rust
pub fn add_tokens(
    user_id: &str,
    tokens: i64,
    price_cents: i64,
) -> Result<()>
```

3. **`check_access()`** - Verify user can use AI
```rust
pub fn check_access(user_id: &str) -> Result<AccessResult>
```

4. **`activate_subscription()`** - Enable paid access
```rust
pub fn activate_subscription(user_id: &str, days: i64) -> Result<()>
```

### Trial System

- **Trial tokens**: 100,000 (~50 messages)
- **Trial duration**: Unlimited time, limited tokens
- **After trial**: Must purchase tokens via Cryptomus

### Cost Calculation

Provider costs defined in [billing/mod.rs:20-35](src/billing/mod.rs#L20-L35):

| Provider | Model | Input $/1M | Output $/1M |
|----------|-------|------------|-------------|
| Groq | llama-3.3-70b | $0.00 | $0.00 |
| DeepSeek | deepseek-v3 | $0.14 | $0.28 |
| Google | gemini-2.0-flash | $0.075 | $0.30 |
| Anthropic | claude-3.5-sonnet | $3.00 | $15.00 |

---

## Part 4: Tools & Features

### Esoteric Tools

Location: [tools/esoteric.rs](src/tools/esoteric.rs)

**Actions**:
- `calculate_bazi` - Chinese Four Pillars astrology
- `calculate_destiny_matrix` - 22 arcana numerology
- `store_mbti` - Store MBTI profile
- `get_profile` - Retrieve stored esoteric data

**Bazi Calculation**:
- 10 Heavenly Stems (天干)
- 12 Earthly Branches (地支)
- Four Pillars: Year, Month, Day, Hour

**Destiny Matrix**:
- Sum all digits of birthdate
- Reduce to 1-22 (arcana number)
- Each arcana has specific meaning

### Goals Tool

Location: [tools/goals.rs](src/tools/goals.rs)

**Actions**:
- `create` - Create SMART goal (transformed to first person present tense)
- `list` - List active goals
- `update` - Update goal progress
- `delete` - Remove goal
- `decompose` - Break into milestones

**SMART Transformation Example**:
```
Input: "I want to lose 10 kg"
Output: "I am at my target weight, having lost 10 kg through healthy lifestyle"
```

---

## Part 5: Database Schema

### Central Database (`workspace/central.db`)

```sql
-- Users (auth)
CREATE TABLE users (
    id TEXT PRIMARY KEY,
    email TEXT UNIQUE NOT NULL,
    password_hash TEXT NOT NULL,
    telegram_id TEXT UNIQUE,
    telegram_username TEXT,
    created_at TEXT NOT NULL,
    last_login TEXT
);

-- Subscriptions
CREATE TABLE subscriptions (
    user_id TEXT PRIMARY KEY,
    status TEXT DEFAULT 'trial',  -- trial, active, expired
    trial_tokens_used INTEGER DEFAULT 0,
    trial_tokens_limit INTEGER DEFAULT 100000,
    paid_until TEXT,
    total_tokens_purchased INTEGER DEFAULT 0
);

-- Billing transactions
CREATE TABLE token_transactions (
    id TEXT PRIMARY KEY,
    user_id TEXT,
    amount INTEGER NOT NULL,        -- +purchase / -usage
    cost_cents INTEGER,
    price_cents INTEGER,
    provider TEXT,
    model TEXT,
    description TEXT,
    created_at TEXT NOT NULL
);

-- Telegram link codes
CREATE TABLE telegram_link_codes (
    code TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    created_at TEXT NOT NULL,
    expires_at TEXT NOT NULL
);
```

### Per-Tenant Database (`workspace/tenants/{user_id}/brain.db`)

```sql
-- User profile
CREATE TABLE profile (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    updated_at TEXT
);

-- Goals
CREATE TABLE goals (
    id TEXT PRIMARY KEY,
    original_text TEXT NOT NULL,
    smart_text TEXT NOT NULL,
    category TEXT,
    status TEXT DEFAULT 'active',
    progress INTEGER DEFAULT 0,
    created_at TEXT NOT NULL
);

-- Conversation history
CREATE TABLE conversations (
    id TEXT PRIMARY KEY,
    role TEXT NOT NULL,
    content TEXT NOT NULL,
    tokens_used INTEGER,
    created_at TEXT NOT NULL
);
```

---

## Part 6: Environment Variables

### Required for Production

```bash
# JWT Secret (generate with: openssl rand -hex 32)
JWT_SECRET=your-256-bit-secret-key

# Telegram Bot
ZEROCLAW_TELEGRAM_BOT_TOKEN=123456:ABC-DEF...

# Cryptomus Payments
CRYPTOMUS_API_KEY=your-cryptomus-api-key
CRYPTOMUS_MERCHANT_ID=your-merchant-id

# LLM Providers (at least one required)
GROQ_API_KEY=gsk_...           # Free tier
GOOGLE_API_KEY=AIza...         # Gemini
OPENROUTER_API_KEY=sk-or-...   # Multi-provider
ANTHROPIC_API_KEY=sk-ant-...   # Claude (expensive)
```

### Optional

```bash
# Server binding
PORT=8080
ZEROCLAW_ALLOW_PUBLIC_BIND=true

# Observability
OTLP_ENDPOINT=https://otlp.example.com

# Workspace
ZEROCLAW_WORKSPACE=/app/workspace
```

---

## Part 7: API Reference

### Auth Endpoints

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| POST | `/api/auth/register` | No | Create account |
| POST | `/api/auth/login` | No | Get JWT token |
| GET | `/api/auth/telegram-link` | JWT | Get TG link code |
| GET | `/api/auth/telegram-status` | JWT | Check TG connection |
| GET | `/api/usage` | JWT | Get token usage stats |

### Payment Endpoints

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| GET | `/api/payment/packages` | No | List token packages |
| POST | `/api/payment/create` | JWT | Create payment link |
| POST | `/api/payment/webhook` | Signature | Cryptomus callback |

### Response Codes

| Code | Meaning |
|------|---------|
| 200 | Success |
| 400 | Bad request (invalid data) |
| 401 | Unauthorized (invalid/missing JWT) |
| 409 | Conflict (email exists) |
| 503 | Service unavailable (payments not configured) |

---

## Part 8: Deployment

### Docker Build

```bash
docker build -t ai-mentor .
```

### Run with Docker Compose

```yaml
version: '3.8'
services:
  ai-mentor:
    image: ai-mentor
    ports:
      - "8080:8080"
    environment:
      - JWT_SECRET=${JWT_SECRET}
      - ZEROCLAW_TELEGRAM_BOT_TOKEN=${TG_TOKEN}
      - CRYPTOMUS_API_KEY=${CRYPTOMUS_KEY}
      - CRYPTOMUS_MERCHANT_ID=${CRYPTOMUS_ID}
      - GROQ_API_KEY=${GROQ_KEY}
    volumes:
      - workspace:/app/workspace

volumes:
  workspace:
```

### Render.com Deployment

See `render.yaml` in repository root.

---

## Part 9: Security Considerations

1. **Password Storage**: Argon2id with secure parameters
2. **JWT**: 24h expiry, HS256 signing
3. **Webhook Signatures**: MD5 verification for Cryptomus
4. **Rate Limiting**: Via Axum middleware (configurable)
5. **Token Metering**: Prevents abuse, enforces limits

---

## Verification Checklist

- [ ] Register user → Check `users` table
- [ ] Login → Verify JWT works
- [ ] Generate TG link → Click → Bot receives `/start`
- [ ] Send message → Verify token counting
- [ ] Purchase tokens → Cryptomus webhook → Tokens added
- [ ] Calculate Bazi → Verify 4 pillars
- [ ] Create goal → Verify SMART transformation
