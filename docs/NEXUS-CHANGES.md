# NEXUS Beacon Receiver — Changes to NEXUS Codebase

## Overview

Two categories of changes are needed in the NEXUS-AI-Gateway codebase:

1. **Expand ClientType detection** — classify AI tools beyond Claude Code
2. **Wire the beacon** — spawn a periodic task that sends daily stats to the CF Worker

Plus the 2 bug fixes already applied (gauge update + disabled reason log).

## .rs Files Involved

### Files to MODIFY

| File | What Changes | Lines Affected | Risk |
|------|-------------|----------------|------|
| `src/telemetry/fingerprint.rs` | Expand `ClientType` enum from 4→13 variants, add detection rules, add tests | ~80 lines changed/added | Medium — enum expansion affects serialization |
| `src/main.rs` | Spawn daily beacon task (~30 lines) | ~30 lines added | Low — additive change, guarded by TELEMETRY_BEACON_URL |
| `src/telemetry/mod.rs` | Bug fixes already applied (gauge update, log message) | Already done ✅ | Done |
| `src/config.rs` | Bug fix already applied (telemetry_disabled_reason) | Already done ✅ | Done |

### Files that DO NOT change

| File | Why No Change Needed |
|------|---------------------|
| `src/telemetry/beacon.rs` | Already fully implemented — `send_beacon()`, `validate_beacon_url()`, `compute_instance_id()` all work. Only needs `#![allow(dead_code)]` removed once wired. |
| `src/telemetry/store.rs` | No changes — `DailyStatsEntry` serialization already works for beacon payload |
| `src/telemetry/metrics.rs` | No changes — `record_client_type_request()` uses `client_type.to_string()` dynamically |
| `src/proxy/mod.rs` | No changes — telemetry capture call already exists |

---

## Change 1: Expand ClientType Enum

### File: `src/telemetry/fingerprint.rs`

### Current enum (4 variants)

```rust
pub enum ClientType {
    ClaudeCode,
    AnthropicSDK,
    CustomScript,
    Unknown,
}
```

### New enum (13 variants)

```rust
pub enum ClientType {
    // AI Coding Tools
    ClaudeCode,
    Cline,
    Aider,
    Continue,
    Codex,
    Cursor,
    Windsurf,
    Copilot,
    // SDKs
    AnthropicSDK,
    OpenAISDK,
    // Generic
    CustomScript,
    AnotherProxy,
    Unknown,
}
```

### `Display` impl changes

Every new variant needs a `to_string()` label. See [CLIENT-TYPES.md](./CLIENT-TYPES.md)
for the full label mapping.

### `classify_client_type()` function changes

Current: 3 if-branches + 1 script loop + 1 beta header check.

New: priority-ordered detection chain. The critical ordering is:

1. **Aider** must be checked BEFORE `anthropic` (Aider uses anthropic SDK, so its UA
   contains both "aider" and "anthropic-python" — "aider" must win)
2. **Codex** must be checked before generic `originator` header check
3. Closed-source tools (Cursor, Windsurf, Copilot) are best-effort
4. Fallback chain: exact UA → secondary headers → script signatures → Unknown

### New test cases

~12 new `#[test]` functions covering each ClientType variant. See
[CLIENT-TYPES.md](./CLIENT-TYPES.md) section "Test Cases Required" for the full list.

### Impact on existing code

- **Prometheus**: `metrics::record_client_type_request()` uses `client_type.to_string()`
  as a label — new variants produce new labels automatically. No code change needed.
- **SQLite**: `store::record_fingerprint()` stores `client_type.to_string()` as text —
  new variants are just new strings. No schema change needed.
- **Beacon**: `beacon::BeaconStats.client_types` is a JSON value from SQLite — new
  types appear as new keys. No code change needed.
- **/analytics**: Returns JSON from SQLite — new types appear automatically.

**Risk**: Prometheus cardinality. 13 ClientType labels × existing metric = 13 time series.
This is well within Prometheus limits (thousands is fine).

---

## Change 2: Wire the Beacon in main.rs

### File: `src/main.rs`

### What to add

After the telemetry context initialization (line ~156), spawn a daily beacon task:

```rust
// v0.18.0: Spawn daily telemetry beacon (if TELEMETRY_BEACON_URL is set)
if let Some(ref beacon_url) = config.telemetry_beacon_url {
    if let Some(ref ctx) = telemetry_ctx {
        let beacon_config = crate::telemetry::beacon::BeaconConfig {
            url: beacon_url.clone(),
            instance_id: crate::telemetry::beacon::compute_instance_id(
                &hostname::get().unwrap_or_default().to_string_lossy(),
                ctx.secret.as_bytes(),
            ),
            version: env!("CARGO_PKG_VERSION").to_string(),
        };
        let beacon_store = ctx.store.clone();
        let beacon_auth_token = config.beacon_auth_token.clone(); // new config field
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(24 * 3600));
            // First beacon after 5 minutes (let the proxy warm up)
            tokio::time::sleep(Duration::from_secs(300)).await;
            loop {
                interval.tick().await;
                // Query today's stats from SQLite
                let stats = tokio::task::spawn_blocking({
                    let store = beacon_store.clone();
                    move || store.get_daily_stats(1)
                }).await;
                if let Ok(Ok(entries)) = stats {
                    if let Some(today) = entries.first() {
                        // Validate URL before sending
                        if crate::telemetry::beacon::validate_beacon_url(&beacon_config.url).is_ok() {
                            match crate::telemetry::beacon::send_beacon(&beacon_config, today).await {
                                Ok(()) => tracing::info!("📡 Telemetry beacon sent"),
                                Err(e) => tracing::debug!("📡 Telemetry beacon failed: {e}"),
                            }
                        }
                    }
                }
            }
        });
        tracing::info!("📡 Telemetry beacon: will send daily to {}", beacon_url);
    }
}
```

### Dependencies

- `hostname` crate — for getting the machine hostname (for instance_id)
- `BEACON_AUTH_TOKEN` env var — for authenticating to the CF Worker

### File: `src/config.rs`

Add one new field:

```rust
pub beacon_auth_token: Option<String>,
```

Loaded from:

```rust
let beacon_auth_token = env::var("BEACON_AUTH_TOKEN").ok().filter(|t| !t.is_empty());
```

### File: `src/telemetry/beacon.rs`

Remove `#![allow(dead_code)]` once the beacon is wired.

Update `send_beacon()` to include the auth token in a header:

```rust
// Add Authorization header
let response = client
    .post(&config.url)
    .header("Authorization", format!("Bearer {}", config.auth_token))
    .json(&payload)
    .send()
    .await
    .context("sending telemetry beacon")?;
```

This means `BeaconConfig` gets a new field:

```rust
pub struct BeaconConfig {
    pub url: String,
    pub instance_id: String,
    pub version: String,
    pub auth_token: String,  // NEW
}
```

---

## Change 3: Bug Fixes (ALREADY APPLIED)

### Bug 1: `nexus_unique_users_today` gauge never updated after init

**File**: `src/telemetry/mod.rs` — `record_async()` function

**Fix applied**: After each successful `record_fingerprint()`, query
`get_unique_fingerprint_count_today()` and call `metrics::record_unique_users(count)`.

### Bug 2: Misleading log "TELEMETRY_ENABLED=false" when guard auto-disabled it

**Files**: `src/config.rs`, `src/main.rs`, `src/telemetry/mod.rs`

**Fix applied**:
- `Config` now has `telemetry_disabled_reason: Option<String>`
- The $HOME guard sets this field instead of emitting an invisible warning
- main.rs logs the reason after the tracing subscriber is initialized
- init() now says "📊 Telemetry: disabled" (neutral, no false attribution)

---

## Summary of All .rs Changes

| File | Status | Nature of Change |
|------|--------|-----------------|
| `src/telemetry/fingerprint.rs` | **PENDING** | Expand ClientType enum + detection logic + tests |
| `src/main.rs` | **PENDING** | Spawn daily beacon task + auth token pass-through |
| `src/config.rs` | **DONE** ✅ | telemetry_disabled_reason field + beacon_auth_token |
| `src/telemetry/mod.rs` | **DONE** ✅ | Gauge update in record_async + neutral log message |
| `src/telemetry/beacon.rs` | **PENDING** | Remove dead_code, add auth_token to BeaconConfig + send_beacon |
| `src/telemetry/metrics.rs` | No change | — |
| `src/telemetry/store.rs` | No change | — |
| `src/proxy/mod.rs` | No change | — |
