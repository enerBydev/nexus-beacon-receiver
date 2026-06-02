# NEXUS Beacon Receiver — CF Worker Design

## Overview

A Cloudflare Worker written in Rust (compiled to WASM via `workers-rs`) that:
1. Receives daily beacon POSTs from NEXUS instances worldwide
2. Stores them in D1 (Cloudflare's SQLite)
3. Exposes a public GET API for consuming aggregated global stats

## Why Rust/WASM Instead of TypeScript

| Factor | Rust/WASM | TypeScript |
|--------|-----------|-----------|
| Language consistency | Same as NEXUS — single mental model | Context switch |
| Type safety | Compile-time, guaranteed | Runtime with JSDoc |
| JSON parsing | serde — zero-cost deserialization | JSON.parse — runtime |
| D1 bindings | `workers-rs` 0.8.3 with full D1 support | Native CF support |
| Performance | ~2x faster on CPU-bound work | Sufficient for I/O-bound |
| Binary size | ~1-2 MB WASM | ~50 KB JS |
| Cold start | ~50ms | ~5ms |
| Ecosystem | Smaller but growing | Massive |
| Debugging | `wrangler tail` + `log!()` | `console.log` native |

Verdict: Rust makes sense because the Worker is simple (~200 lines), type safety
catches bugs at compile time, and we keep a single language across the stack.

## workers-rs Status

| Aspect | Status |
|--------|--------|
| Crate version | 0.8.3 (released 2026-05-09) |
| D1 support | ✅ Full — `D1Database`, `D1PreparedStatement`, session support |
| KV support | ✅ Available |
| Fetch API | ✅ Via `worker::Fetch` |
| wrangler compat | ✅ `wrangler dev`, `wrangler deploy` |
| GitHub | `cloudflare/workers-rs` — actively maintained |

## Project Structure

```
nexus-beacon-receiver/
├── Cargo.toml              # workers-rs + serde + chrono
├── wrangler.toml           # CF config (name, compatibility_date, D1 binding)
├── schema.sql              # D1 schema — run once with `wrangler d1 execute`
├── src/
│   └── lib.rs              # Worker entry point (~200 lines)
├── docs/                   # This documentation
│   ├── ARCHITECTURE.md
│   ├── PROBLEM-STATEMENT.md
│   ├── CLIENT-TYPES.md
│   ├── NEXUS-CHANGES.md
│   ├── WORKER-DESIGN.md     # ← you are here
│   └── IMPLEMENTATION-PLAN.md
└── README.md               # Public-facing README
```

## Files to Create

| File | Purpose | Approximate Size |
|------|---------|-----------------|
| `Cargo.toml` | Rust project definition with workers-rs dependency | ~20 lines |
| `wrangler.toml` | CF Worker configuration: name, D1 binding, compatibility | ~15 lines |
| `schema.sql` | D1 database schema (2 tables + indexes) | ~30 lines |
| `src/lib.rs` | Worker entry point — all HTTP handling, D1 queries | ~200 lines |
| `README.md` | Public documentation — purpose, API, deployment | ~80 lines |

### Why each file exists

- **Cargo.toml**: Required for Rust compilation to WASM. Dependencies: `worker` (workers-rs),
  `worker-sys`, `serde`, `serde_json`, `chrono`.
- **wrangler.toml**: CF Workers require this for deployment. Defines the worker name,
  D1 database binding, compatibility date, and build command.
- **schema.sql**: D1 databases are created empty. This script initializes the tables.
  Run once with `wrangler d1 execute nexus-beacon-db --file=schema.sql`.
- **src/lib.rs**: Single-file worker. CF Workers are typically small; 200 lines covers
  routing, validation, D1 queries, and CORS. No need for multi-file modules.
- **README.md**: The repo is public. People need to understand what it does, how to
  deploy it, and what API it exposes.

---

## D1 Schema

### Table: `beacons` (raw beacon storage)

```sql
CREATE TABLE IF NOT EXISTS beacons (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    instance_id TEXT NOT NULL,            -- HMAC-SHA256(secret, hostname) — 32 hex chars
    version TEXT NOT NULL,                -- NEXUS version e.g. "0.17.4"
    date TEXT NOT NULL,                   -- YYYY-MM-DD
    total_requests INTEGER NOT NULL,      -- Aggregated count
    unique_fingerprints INTEGER NOT NULL, -- Count of unique users that day
    models_used TEXT NOT NULL DEFAULT '{}',  -- JSON: {"claude-sonnet-4-6": 1200, ...}
    client_types TEXT NOT NULL DEFAULT '{}', -- JSON: {"claude_code": 2, "sdk": 1, ...}
    avg_message_count REAL NOT NULL DEFAULT 0,
    tool_use_ratio REAL NOT NULL DEFAULT 0,
    received_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(instance_id, date)             -- One beacon per instance per day
);
```

### Table: `daily_global_stats` (pre-aggregated for fast reads)

```sql
CREATE TABLE IF NOT EXISTS daily_global_stats (
    date TEXT PRIMARY KEY,                -- YYYY-MM-DD
    total_instances INTEGER NOT NULL DEFAULT 0,  -- COUNT(DISTINCT instance_id)
    total_requests INTEGER NOT NULL DEFAULT 0,   -- SUM(total_requests)
    total_unique_users INTEGER NOT NULL DEFAULT 0, -- SUM(unique_fingerprints)
    models_used TEXT NOT NULL DEFAULT '{}',      -- Aggregated JSON across instances
    client_types TEXT NOT NULL DEFAULT '{}',     -- Aggregated JSON across instances
    avg_message_count REAL NOT NULL DEFAULT 0,   -- Weighted average
    tool_use_ratio REAL NOT NULL DEFAULT 0,      -- Weighted average
    versions TEXT NOT NULL DEFAULT '{}',         -- JSON: {"0.17.4": 5, "0.16.0": 2}
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
```

### Indexes

```sql
CREATE INDEX IF NOT EXISTS idx_beacons_instance ON beacons(instance_id);
CREATE INDEX IF NOT EXISTS idx_beacons_date ON beacons(date);
```

### Why two tables instead of one

`beacons` stores raw per-instance data. `daily_global_stats` stores pre-aggregated
global data. This is critical for performance:

- `GET /v1/stats` queries `daily_global_stats` — single row per day, instant response
- `POST /v1/beacon` writes to `beacons` AND upserts `daily_global_stats`
- Without pre-aggregation, every GET would need `GROUP BY date` + JSON merge across
  hundreds of rows — slow on D1's edge SQLite

The upsert logic on beacon POST merges the new instance's stats into the global
aggregation. If a beacon for the same `(instance_id, date)` already exists, it
replaces the old one (idempotent).

---

## API Specification

### POST /v1/beacon

Receives a telemetry beacon from a NEXUS instance.

**Request:**

```
POST /v1/beacon
Content-Type: application/json
Authorization: Bearer <BEACON_AUTH_TOKEN>

{
  "instance_id": "a3f7c2...8e0a4c",
  "version": "0.17.4",
  "date": "2026-06-01",
  "stats": {
    "total_requests": 1500,
    "unique_fingerprints": 3,
    "models_used": {"claude-sonnet-4-6": 1200, "claude-opus-4-5": 300},
    "client_types": {"claude_code": 2, "sdk": 1},
    "avg_message_count": 12.3,
    "tool_use_ratio": 0.78
  }
}
```

**Response (200 OK):**

```json
{"status": "ok"}
```

**Response (401 Unauthorized):**

```json
{"error": "invalid_auth_token"}
```

**Response (400 Bad Request):**

```json
{"error": "invalid_payload", "detail": "missing field: stats"}
```

**Validation rules:**

1. `Authorization: Bearer <token>` must match `BEACON_AUTH_TOKEN` secret
2. `instance_id` must be non-empty and ≤128 chars
3. `version` must be non-empty
4. `date` must match `YYYY-MM-DD` format
5. `stats.total_requests` must be ≥ 0
6. `stats.unique_fingerprints` must be ≥ 0
7. Payload size must be ≤ 4KB
8. Deduplication: if `(instance_id, date)` already exists, replace (upsert)

**Rate limiting:**

- One beacon per instance per day is the expected cadence
- D1 UNIQUE constraint on `(instance_id, date)` provides natural dedup
- CF Workers has built-in DDoS protection

---

### GET /v1/stats

Returns global aggregated statistics for the last N days.

**Request:**

```
GET /v1/stats?days=7
Authorization: Bearer <API_READ_TOKEN>  (optional, configurable)
```

**Response (200 OK):**

```json
{
  "generated_at": "2026-06-01T12:00:00Z",
  "period_days": 7,
  "global": {
    "total_instances": 42,
    "total_requests": 150000,
    "total_unique_users": 85,
    "avg_message_count": 12.5,
    "tool_use_ratio": 0.78
  },
  "models_used": {
    "claude-sonnet-4-6": 120000,
    "claude-opus-4-5": 25000,
    "claude-haiku-4-5": 5000
  },
  "client_types": {
    "claude_code": 30,
    "cline": 5,
    "aider": 3,
    "sdk": 2,
    "script": 1,
    "unknown": 1
  },
  "versions": {
    "0.17.4": 25,
    "0.16.0": 10,
    "0.15.0": 7
  },
  "daily": [
    {
      "date": "2026-06-01",
      "total_instances": 40,
      "total_requests": 22000,
      "total_unique_users": 80
    },
    {
      "date": "2026-05-31",
      "total_instances": 38,
      "total_requests": 21000,
      "total_unique_users": 75
    }
  ]
}
```

**Query parameters:**

| Param | Default | Range | Description |
|-------|---------|-------|-------------|
| `days` | 7 | 1–90 | How many days of history to return |

**CORS:** Allowed origins: `*.enerby.dev`, `localhost:*`

---

### GET /v1/stats/summary

Lightweight endpoint for embedding (badges, widgets).

**Request:**

```
GET /v1/stats/summary
```

**Response (200 OK):**

```json
{
  "total_instances": 42,
  "total_requests": 150000,
  "total_unique_users": 85,
  "period": "7d"
}
```

This is a subset of `/v1/stats` — just the headline numbers. Designed for:
- Portfolio widgets: "NEXUS: 42 active instances, 150K requests"
- GitHub badges: `![](https://beacon.enerby.dev/v1/stats/summary)`
- Social proof: "Used by 85+ developers globally"

---

## src/lib.rs Design (single file, ~200 lines)

### Structure

```rust
// --- Imports ---
use worker::*;
use serde::{Deserialize, Serialize};
use chrono::NaiveDate;

// --- Types ---
struct BeaconPayload { ... }         // Deserialize from POST
struct GlobalStats { ... }           // Serialize for GET response
struct DailySummary { ... }          // Serialize for daily array

// --- Entry Point ---
#[event(fetch)]
async fn main(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    let router = Router::new()
        .post("/v1/beacon", handle_beacon_post)
        .get("/v1/stats", handle_stats_get)
        .get("/v1/stats/summary", handle_summary_get)
        .run(req, env)
        .await
}

// --- Handlers ---
async fn handle_beacon_post(req: Request, ctx: RouteContext<Env>) -> Result<Response> {
    // 1. Validate auth token
    // 2. Parse JSON body
    // 3. Validate fields
    // 4. Upsert into beacons table
    // 5. Upsert into daily_global_stats (merge)
    // 6. Return 200 {"status": "ok"}
}

async fn handle_stats_get(req: Request, ctx: RouteContext<Env>) -> Result<Response> {
    // 1. Parse ?days=N parameter
    // 2. Query daily_global_stats
    // 3. Query raw beacons for models/client_types aggregation
    // 4. Build response JSON
    // 5. Return with CORS headers
}

async fn handle_summary_get(req: Request, ctx: RouteContext<Env>) -> Result<Response> {
    // 1. Query latest 7 days from daily_global_stats
    // 2. Sum up headline numbers
    // 3. Return lightweight JSON
}

// --- Helpers ---
fn validate_auth(req: &Request, env: &Env) -> Result<()> { ... }
fn cors_headers() -> Headers { ... }
fn merge_json_maps(existing: &str, incoming: &str) -> String { ... }
```

### Error handling

All errors return JSON:

```json
{"error": "description"}
```

With appropriate HTTP status codes: 400, 401, 404, 500.

---

## wrangler.toml Design

```toml
name = "nexus-beacon-receiver"
main = "src/lib.rs"
compatibility_date = "2026-05-09"
compatibility_flags = ["nodejs_compat"]

[[d1_databases]]
binding = "DB"
database_name = "nexus-beacon-db"
database_id = "<generated-by-wrangler-d1-create>"

[vars]
CORS_ORIGINS = "https://enerby.dev,https://www.enerby.dev"

# Secrets (set via `wrangler secret put`):
# - BEACON_AUTH_TOKEN    — required for POST /v1/beacon
# - API_READ_TOKEN       — optional for GET /v1/stats (if auth is enabled)
```

---

## Cargo.toml Design

```toml
[package]
name = "nexus-beacon-receiver"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
worker = "0.8"
worker-sys = "0.8"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
chrono = { version = "0.4", features = ["serde"] }

[profile.release]
opt-level = "s"   # Optimize for size (WASM)
lto = true
