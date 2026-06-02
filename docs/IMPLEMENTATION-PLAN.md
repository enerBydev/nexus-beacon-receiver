# NEXUS Beacon Receiver — Implementation Plan

## Overview

This plan covers ALL remaining work to get global telemetry operational, across
TWO separate codebases:

1. **NEXUS-AI-Gateway** — the proxy (existing repo, this branch)
2. **nexus-beacon-receiver** — the CF Worker (new, developed in-tree)

Each phase has explicit **commit/push milestones** — the points at which code is
safe to commit and push to its respective remote.

---

## Phase 0: Bug Fixes (DONE ✅)

**Already applied and verified.**

| Fix | Files | Status |
|-----|-------|--------|
| Gauge `nexus_unique_users_today` updates after each fingerprint | `src/telemetry/mod.rs` | ✅ Done |
| `telemetry_disabled_reason` field + correct log messages | `src/config.rs`, `src/main.rs`, `src/telemetry/mod.rs` | ✅ Done |

**Not yet committed.** These changes are in the working tree but uncommitted.

---

## Phase 1: Expand ClientType Detection

**Goal**: Detect Cline, Aider, Continue, Codex, Cursor, Windsurf, Copilot in addition
to the existing Claude Code / Anthropic SDK / CustomScript / Unknown.

**Files modified** (NEXUS repo):
- `src/telemetry/fingerprint.rs` — expand enum, add detection rules, add tests
- No other files need changes (metrics, store, beacon all use `.to_string()` dynamically)

**Steps**:

1. Expand `ClientType` enum from 4 → 13 variants (see [CLIENT-TYPES.md](./CLIENT-TYPES.md))
2. Update `Display` impl with new labels
3. Add detection rules in `classify_client_type()` — priority-ordered:
   - Aider BEFORE anthropic (Aider wraps the SDK, so its UA contains both)
   - Codex before generic originator header
   - Closed-source tools best-effort (Cursor, Windsurf, Copilot)
4. Add ~12 new `#[test]` functions
5. Add `OpenAISDK` variant for `openai-python/` UA pattern
6. Run `cargo test` — all existing + new tests must pass
7. Run `cargo clippy -- -D warnings` — clean
8. Run `cargo fmt` — clean

**Verification**: `cargo test` passes with all new ClientType tests.

### 🏁 COMMIT/ PUSH MILESTONE: Phase 1 Complete

**NEXUS-AI-Gateway repo** → commit to `feat/85-autonomous-git-sync-system` branch:

```
feat: expand ClientType detection — classify Cline, Aider, Continue, Codex, Cursor, Windsurf, Copilot

Includes bug fixes from Phase 0:
- fix: update nexus_unique_users_today gauge after each fingerprint record
- fix: add telemetry_disabled_reason to Config for accurate log messages
```

**This is committable AND pushable to PR#86.** At this point:
- ✅ All NEXUS code is correct and tested
- ✅ No breaking changes (new enum variants are additive)
- ✅ Beacon code still has `#![allow(dead_code)]` — it won't break anything
- ✅ The 2 bug fixes are included

---

## Phase 2: Wire Beacon in NEXUS

**Goal**: Make the existing `beacon.rs` code actually execute — spawn a periodic task
in main.rs that sends daily stats to the CF Worker.

**Files modified** (NEXUS repo):
- `src/main.rs` — spawn daily beacon task (~30 lines)
- `src/telemetry/beacon.rs` — remove `#![allow(dead_code)]`, add `auth_token` to `BeaconConfig`, update `send_beacon()` to send auth header
- `src/config.rs` — add `beacon_auth_token: Option<String>` field + env var parsing
- `Cargo.toml` — add `hostname` crate dependency

**Steps**:

1. Add `hostname` crate to `Cargo.toml` (for `compute_instance_id`)
2. Add `beacon_auth_token: Option<String>` to `Config` struct in `config.rs`
3. Load `BEACON_AUTH_TOKEN` env var in both `from_map()` and `from_env_with_path()`
4. Remove `#![allow(dead_code)]` from `beacon.rs`
5. Add `auth_token: String` field to `BeaconConfig`
6. Update `send_beacon()` to include `Authorization: Bearer <token>` header
7. In `main.rs`, after telemetry init, spawn the beacon task:
   - Only if `telemetry_beacon_url` is set AND telemetry context is Some
   - First beacon after 5 minutes (warm-up period)
   - Then every 24 hours
   - Queries `get_daily_stats(1)` from SQLite
   - Calls `send_beacon()` with the latest day's stats
8. Add `#[cfg(test)]` for beacon auth token config loading
9. Run `cargo test` — all pass
10. Run `cargo clippy -- -D warnings` — clean

**Verification**:
- Build succeeds
- With `TELEMETRY_BEACON_URL=https://webhook.site/xxx` set, proxy starts without error
- After 5 minutes, a POST appears at the webhook URL with correct JSON payload

### 🏁 COMMIT/ PUSH MILESTONE: Phase 2 Complete

**NEXUS-AI-Gateway repo** → commit to same branch:

```
feat: wire telemetry beacon — daily POST to configured endpoint

- Spawn periodic beacon task in main.rs (24h interval, 5min warm-up)
- Add BEACON_AUTH_TOKEN env var for authenticating to receiver
- Remove dead_code allow from beacon.rs — now fully wired
- Add auth_token field to BeaconConfig + Authorization header in send_beacon
```

**This is committable AND pushable to PR#86.** At this point:
- ✅ NEXUS can send beacons to any HTTPS endpoint
- ✅ The CF Worker doesn't need to exist yet — beacons will fail gracefully
  (debug-level log, no impact on proxy functionality)
- ✅ `TELEMETRY_BEACON_URL` defaults to not set — no beacons unless configured

---

## Phase 3: Create CF Worker (nexus-beacon-receiver)

**Goal**: Build the Rust/WASM Cloudflare Worker that receives beacons, stores in D1,
and serves the global stats API.

**Files created** (inside `nexus-beacon-receiver/`, NOT tracked by NEXUS git):
- `Cargo.toml` — project definition
- `wrangler.toml` — CF Worker config
- `schema.sql` — D1 database schema
- `src/lib.rs` — Worker code (~200 lines)
- `README.md` — public documentation

**Steps**:

1. Initialize project with `wasm-pack` or manual Cargo.toml
2. Add dependencies: `worker = "0.8"`, `serde`, `serde_json`, `chrono`
3. Create `wrangler.toml` with D1 binding
4. Create `schema.sql` with `beacons` + `daily_global_stats` tables
5. Implement `src/lib.rs`:
   - `POST /v1/beacon` — validate auth, parse payload, upsert into D1
   - `GET /v1/stats` — query D1, build global aggregation JSON
   - `GET /v1/stats/summary` — lightweight endpoint for badges/widgets
   - CORS headers for `*.enerby.dev`
6. Create D1 database: `wrangler d1 create nexus-beacon-db`
7. Run schema: `wrangler d1 execute nexus-beacon-db --file=schema.sql`
8. Test locally: `wrangler dev`
9. Set secrets: `wrangler secret put BEACON_AUTH_TOKEN`
10. Deploy: `wrangler deploy`
11. Write `README.md`

**Verification**:
- `wrangler dev` runs without errors
- `curl POST /v1/beacon` with valid payload → 200 OK
- `curl GET /v1/stats` → JSON with aggregated data
- `curl POST /v1/beacon` without auth → 401
- `curl POST /v1/beacon` with duplicate `(instance_id, date)` → upserts (200, not error)

### 🏁 COMMIT/ PUSH MILESTONE: Phase 3 Complete

**nexus-beacon-receiver** → This is a SEPARATE repo. Steps:

1. Create new GitHub repo: `enerBydev/nexus-beacon-receiver` (public)
2. `cd nexus-beacon-receiver && git init`
3. Add all files, commit:
   ```
   feat: initial release — NEXUS global telemetry receiver

   CF Worker (Rust/WASM) that receives daily beacons from NEXUS-AI-Gateway
   instances, stores in D1, and serves aggregated global usage statistics.
   ```
4. `git remote add origin git@github.com:enerBydev/nexus-beacon-receiver.git`
5. `git push -u origin main`

**Remove from NEXUS .gitignore**: After pushing to its own repo, the
`nexus-beacon-receiver/` directory in NEXUS's tree becomes just a local working
copy. The .gitignore entry stays — NEXUS doesn't track the Worker's code.

**The Worker is pushable to its own repo when**:
- ✅ All 3 endpoints work (POST beacon, GET stats, GET summary)
- ✅ Auth token validation works
- ✅ D1 read/write works
- ✅ CORS headers present
- ✅ `wrangler deploy` succeeds

---

## Phase 4: Integration Test (End-to-End)

**Goal**: Verify the complete pipeline: NEXUS → CF Worker → API response.

**Steps**:

1. Deploy NEXUS locally with:
   ```bash
   TELEMETRY_ENABLED=true
   TELEMETRY_BEACON_URL=https://nexus-beacon-receiver.<your-subdomain>.workers.dev/v1/beacon
   BEACON_AUTH_TOKEN=<the-secret-you-set-in-wrangler>
   ```
2. Send requests to NEXUS (with different User-Agents to test ClientType)
3. Wait for the 5-minute warm-up + first beacon
4. Check CF Worker logs: `wrangler tail`
5. Verify beacon was received and stored in D1
6. `curl https://nexus-beacon-receiver.<subdomain>.workers.dev/v1/stats`
7. Verify response contains the data from step 2
8. Test from enerby.dev: fetch `/v1/stats/summary` and display

**Verification**:
- Beacon arrives at CF Worker ✅
- D1 has data ✅
- `/v1/stats` returns correct aggregated JSON ✅
- `/v1/stats/summary` returns lightweight numbers ✅

### 🏁 FINAL MILESTONE: System Operational

At this point the full pipeline is live:

```
NEXUS instances worldwide → daily HTTPS beacon → CF Worker → D1
                                                          ↓
                                              GET /v1/stats → consumers
```

No further commits needed for this phase — it's a verification step.

---

## Phase 5: Consumers (Future, Not in This Plan)

These are ideas documented for reference but NOT part of the current implementation:

| Consumer | What It Shows | How |
|----------|-------------|-----|
| enerby.dev portfolio | "X active NEXUS instances globally" | `fetch("/v1/stats/summary")` |
| GitHub README badge | `![](https://beacon.enerby.dev/v1/stats/summary)` | Shields.io endpoint |
| Social proof | "Used in 30+ countries by 85+ developers" | `/v1/stats` data |
| Business intelligence | Model popularity, tool adoption, version distribution | `/v1/stats` full response |

---

## Commit/Push Decision Matrix

| When | What | Where | Commit? | Push? |
|------|------|-------|---------|-------|
| After Phase 0 | Bug fixes (gauge + log) | NEXUS working tree | Yes | Yes → PR#86 |
| After Phase 1 | ClientType expansion | NEXUS working tree | Yes | Yes → PR#86 |
| After Phase 2 | Beacon wiring | NEXUS working tree | Yes | Yes → PR#86 |
| During Phase 3 | CF Worker code | nexus-beacon-receiver/ | **No** (gitignored) | **No** |
| After Phase 3 | CF Worker complete | **New repo** | Yes | Yes → `enerBydev/nexus-beacon-receiver` |
| Phase 4 | Integration test | N/A | No code changes | No push |

**Key insight**: Phases 0-2 are NEXUS changes → push to PR#86. Phase 3 is a new
service → push to its own repo. The `nexus-beacon-receiver/` directory in NEXUS's
tree is always gitignored — it's a local workspace, not a submodule.

---

## Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|-----------|
| workers-rs D1 bindings have bugs | Low | High | Test locally with `wrangler dev` before deploy |
| WASM binary exceeds CF size limit | Low | Medium | `opt-level = "s"` + LTO; limit is 10MB compressed |
| Beacon auth token leaked | Medium | Low | Rotate via `wrangler secret put`; payload has zero PII anyway |
| D1 free tier exceeded | Very Low | Low | 5GB storage, 5M reads/day — years of data at projected scale |
| Cold start latency | Low | Low | Edge is fast; Rust WASM ~50ms cold start; beacon is async |
| Aider misclassified as AnthropicSDK | Medium | Low | Priority-ordered detection: check "aider" before "anthropic" |
| Closed-source tools change UA format | Medium | Low | Best-effort detection; falls back to Unknown gracefully |
