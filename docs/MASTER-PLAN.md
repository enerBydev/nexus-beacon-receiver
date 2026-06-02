# NEXUS Beacon Receiver — Master Implementation & Development Plan

> **Scope**: Everything needed to build, deploy, and maintain `nexus-beacon-receiver` —
> a Cloudflare Worker (Rust/WASM) that receives daily telemetry beacons from NEXUS-AI-Gateway
> instances worldwide, stores in D1, and serves global usage stats.
>
> **Companion docs**: [ARCHITECTURE.md](./ARCHITECTURE.md) · [PROBLEM-STATEMENT.md](./PROBLEM-STATEMENT.md) ·
> [CLIENT-TYPES.md](./CLIENT-TYPES.md) · [NEXUS-CHANGES.md](./NEXUS-CHANGES.md) ·
> [WORKER-DESIGN.md](./WORKER-DESIGN.md)

---

## Part I: Manifesto Compatibility Analysis

The user's engineering manifesto specifies patterns for a Rust+Dioxus fullstack monorepo.
Not everything applies to a ~200-line CF Worker. This analysis filters what fits.

### ✅ ADOPT — Directly Compatible

| Manifesto Principle | How It Applies to beacon-receiver | Implementation |
|---------------------|----------------------------------|----------------|
| **KISS (strict)** | Worker is ~200 LOC. Single file. No over-engineering. | `src/lib.rs` only — no module splitting needed |
| **DRY** | JSON merge logic reused between POST and GET handlers | `merge_json_maps()` helper function |
| **Zero-Cost Abstraction (ZCA)** | serde deserialization at compile time, no runtime reflection | `#[derive(Deserialize, Serialize)]` on all types |
| **Conventional Commits** | Exact same format as NEXUS | `feat:`, `fix:`, `chore:`, etc. |
| **TDD desde cero** | Write tests first for each handler | `#[cfg(test)]` module + `wrangler dev` integration tests |
| **Docs as Code** | This plan + README + API docs in code comments | All docs in `docs/` + README.md |
| **Git Flow avanzado** | Branching + hooks + CI/CD + auto-version | Adapted from NEXUS (see Part II) |
| **Semantic Versioning** | 3-file sync adapted: VERSION + Cargo.toml + src/lib.rs | Same as NEXUS |
| **Rule of Three as DRY↔KISS contract** | If a pattern appears 3 times → extract. Before that → keep inline. | Applied during implementation |
| **Single Source of Truth** | Config in `wrangler.toml` + env secrets, not scattered | One config source |
| **Repetition conscious > premature abstraction** | 200 LOC — no traits, no trait objects, no strategy pattern needed | Just functions and structs |
| **Code Review via CodeRabbit** | PR feedback loop until clean | Same workflow as NEXUS |

### ⚠️ ADAPT — Partially Compatible (Modified for CF Worker context)

| Manifesto Principle | Original Form | Adapted Form | Why |
|---------------------|---------------|--------------|-----|
| **Hexagonal + Clean Architecture** | Domain/Adapters/Ports layers | Flat single-file with clear handler→helper separation | 200 LOC doesn't justify 5 directories |
| **DDD (Repository Pattern)** | Trait-based repository with impls | Direct D1 queries in handlers (no trait needed) | One storage backend (D1), one query pattern |
| **Strategy Pattern** | Runtime algorithm selection | Compile-time function selection (just 3 handlers) | No strategies to swap at runtime |
| **Atomic Design + Design System** | Component hierarchy | N/A — no UI | Worker is API-only |
| **Mobile-First** | Responsive CSS | N/A — no UI | Worker is API-only |
| **Provisioning: justfile** | `just` task runner | Taskfile.yaml (go-task) — same as NEXUS | Consistency with existing repo |
| **release-plz** | Automated Rust crate releases | Manual `task full-release` — CF Workers don't use crates.io | Worker deploys via `wrangler deploy`, not cargo publish |
| **dependabot** | cargo + github-actions ecosystem | cargo only (no github-actions deps monitoring needed) | Simpler dependency surface |
| **post-merge auto-rebuild** | systemd service restart | `wrangler deploy` on merge to main | CF Workers deploy differently |

### ❌ DISCARD — Not Applicable

| Manifesto Principle | Why Not Applicable |
|---------------------|-------------------|
| **Dioxus 0.7 patterns** | No frontend — this is an edge API |
| **WASM-specific UI concerns** | Our WASM runs server-side (CF Worker), not browser |
| **TailwindCSS v4** | No CSS needed |
| **systemd service management** | CF Workers are serverless — no daemon |
| **git-sync daemon (3-layer sync)** | CF Workers deploy via `wrangler`, not local binary |
| **binary sync (md5, ~/.cargo/bin, ~/.local/bin)** | No local binary to sync |
| **cargo audit** | No crate publication — dependency surface is tiny (worker, serde, chrono) |
| **wiremock / deps-monitor** | No pinned test dependencies |
| **Dioxus/VSCode extension development** | No IDE extension |
| **prompt_cache / tokenizer patterns** | No AI model interaction |

---

## Part II: CI/CD Replication — NEXUS → beacon-receiver Mapping

### NEXUS CI/CD Stack (analyzed from repo)

| Component | NEXUS Implementation | beacon-receiver Adaptation |
|-----------|----------------------|---------------------------|
| **GitHub Actions CI** | 4 jobs: security-scan, test, lint, format, build + release | 3 jobs: lint, format, build-wasm (no security-scan — no cargo-audit needed for 3-dep project) |
| **GitHub Actions Auto-Version** | workflow_run after CI → analyze commits → bump → tag → release | Same pattern adapted: bump → tag → `wrangler deploy` instead of binary release |
| **Git Hooks (5)** | pre-commit, commit-msg, pre-push, post-commit, post-merge | 4 hooks: pre-commit, commit-msg, pre-push, post-commit (no post-merge — no local binary) |
| **Taskfile.yaml** | ~40 tasks (build, test, lint, install, service, version) | ~20 tasks adapted for CF Worker (build-wasm, dev, deploy, version) |
| **Version Scripts** | auto-version.sh, bump-version.sh, increment-version.sh | Same 3 scripts, adapted: no `src/lib.rs` VERSION const (single `lib.rs` instead) |
| **Dependabot** | cargo + github-actions, weekly | cargo only, weekly |
| **CODEOWNERS** | `* @enerBydev` | `* @enerBydev` (same owner) |
| **Branch Protection** | CI checks + 1 approval + no force push | Same |
| **PR Template** | Summary + Test plan | Same |
| **Issue Templates** | Bug + Feature + config.yml | Same |
| **CHANGELOG.md** | Keep a Changelog format | Same |
| **Conventional Commits** | Strict validation in commit-msg hook | Same |
| **3-File Version Sync** | VERSION + Cargo.toml + src/lib.rs | VERSION + Cargo.toml (2 files — Worker has no `pub const VERSION` since it's a `[lib]` crate) |
| **CodeRabbit Integration** | Recursive PR feedback | Same — add CodeRabbit to repo settings |
| **MD5 binary verification** | sync-binary task | N/A — no local binary |

### Key Differences

1. **Build target**: `wasm32-unknown-unknown` instead of `x86_64-unknown-linux-gnu`
2. **Deployment**: `wrangler deploy` instead of `cargo install` + systemd restart
3. **Testing**: `cargo test --target wasm32-unknown-unknown` OR `wrangler dev` + curl integration tests
4. **Size optimization**: `opt-level = "s"` + LTO mandatory (CF Workers have 10MB compressed limit)
5. **No `src/lib.rs` VERSION const**: The Worker is a `[lib]` crate — version comes from `env!("CARGO_PKG_VERSION")`
6. **2-file version sync**: VERSION + Cargo.toml (not 3 — no separate `src/lib.rs` const)
7. **No `cargo audit` job**: Tiny dependency surface (3 crates) — not worth the CI time
8. **No systemd/service tasks**: Serverless deployment

---

## Part III: Repository Structure

```
nexus-beacon-receiver/
├── .github/
│   ├── CODEOWNERS
│   ├── dependabot.yml
│   ├── BRANCH_PROTECTION.md
│   ├── PULL_REQUEST_TEMPLATE.md
│   ├── ISSUE_TEMPLATE/
│   │   ├── bug_report.md
│   │   ├── feature_request.md
│   │   └── config.yml
│   └── workflows/
│       ├── ci.yml              # Lint + Format + Build-WASM
│       └── auto-version.yml   # Auto-bump + tag + deploy on merge to main
│
├── scripts/
│   ├── hooks/
│   │   ├── pre-commit          # Secrets check + large files + cargo fmt --check
│   │   ├── commit-msg          # Conventional commits validation
│   │   ├── pre-push            # Version sync + cargo test + cargo clippy (main only)
│   │   └── post-commit         # Auto-version dry-run hint
│   ├── setup-hooks.sh          # Configure core.hooksPath
│   ├── auto-version.sh         # Analyze commits → determine bump
│   ├── bump-version.sh         # Apply version across files
│   └── increment-version.sh    # Calculate next version number
│
├── src/
│   └── lib.rs                  # Worker entry point (~200 lines)
│
├── docs/
│   ├── ARCHITECTURE.md         # (existing — 100 lines)
│   ├── PROBLEM-STATEMENT.md    # (existing — 122 lines)
│   ├── CLIENT-TYPES.md         # (existing — 170 lines)
│   ├── NEXUS-CHANGES.md        # (existing — 241 lines)
│   ├── WORKER-DESIGN.md        # (existing — 416 lines)
│   ├── IMPLEMENTATION-PLAN.md  # (existing — 275 lines)
│   └── MASTER-PLAN.md          # (this file)
│
├── Cargo.toml                  # Rust project: worker + serde + chrono
├── wrangler.toml               # CF Worker config: D1 binding, secrets
├── schema.sql                  # D1 schema: beacons + daily_global_stats
├── Taskfile.yaml               # Task runner (go-task)
├── CHANGELOG.md                # Keep a Changelog format
├── VERSION                     # Single line: "0.1.0"
├── .gitignore                  # target/, .env, .wrangler/
└── README.md                   # Public-facing documentation
```

### File Sizes (estimated)

| File | Purpose | Lines |
|------|---------|-------|
| `src/lib.rs` | Worker: routing, handlers, D1 queries, CORS | ~200 |
| `Cargo.toml` | Project definition + dependencies | ~25 |
| `wrangler.toml` | CF Worker config + D1 binding | ~15 |
| `schema.sql` | D1 database schema | ~30 |
| `Taskfile.yaml` | Build, test, deploy, version tasks | ~150 |
| `README.md` | Public docs: purpose, API, deployment | ~100 |
| `.github/workflows/ci.yml` | CI: lint + format + build | ~80 |
| `.github/workflows/auto-version.yml` | Auto-bump + deploy | ~120 |
| `scripts/hooks/*` | 4 git hooks | ~200 total |
| `scripts/*.sh` | Version management scripts | ~250 total |

---

## Part IV: Atomic Development Phases

### Phase 0: Repository Initialization

**Goal**: Create the GitHub repo, push initial structure, configure CI/CD.

**What this phase creates**:
- GitHub repo `enerBydev/nexus-beacon-receiver` (public)
- All infrastructure files (no application code yet)
- CI/CD pipeline operational on first push

**Steps**:

0.1. Create GitHub repo via `gh repo create`
0.2. Initialize local git repo inside `nexus-beacon-receiver/`
0.3. Create `VERSION` file: `0.1.0`
0.4. Create `Cargo.toml` — project definition with `worker`, `serde`, `chrono`
0.5. Create `.gitignore` — `target/`, `.env*`, `.wrangler/`, `*.wasm`
0.6. Create `CHANGELOG.md` — initial entry
0.7. Create `wrangler.toml` — placeholder (D1 binding with `database_id = "TODO"`)
0.8. Create `schema.sql` — D1 schema (beacons + daily_global_stats)
0.9. Create `scripts/hooks/pre-commit`
0.10. Create `scripts/hooks/commit-msg`
0.11. Create `scripts/hooks/pre-push`
0.12. Create `scripts/hooks/post-commit`
0.13. Create `scripts/setup-hooks.sh`
0.14. Create `scripts/auto-version.sh`
0.15. Create `scripts/bump-version.sh`
0.16. Create `scripts/increment-version.sh`
0.17. Create `Taskfile.yaml`
0.18. Create `.github/CODEOWNERS`
0.19. Create `.github/dependabot.yml`
0.20. Create `.github/BRANCH_PROTECTION.md`
0.21. Create `.github/PULL_REQUEST_TEMPLATE.md`
0.22. Create `.github/ISSUE_TEMPLATE/bug_report.md`
0.23. Create `.github/ISSUE_TEMPLATE/feature_request.md`
0.24. Create `.github/ISSUE_TEMPLATE/config.yml`
0.25. Create `.github/workflows/ci.yml`
0.26. Create `.github/workflows/auto-version.yml`
0.27. Create `README.md` — project overview, API spec, deployment guide
0.28. Create `src/lib.rs` — minimal stub: `#[event(fetch)]` returning "not implemented"
0.29. Run `git init` + first commit + push to remote
0.30. Configure CodeRabbit integration via GitHub settings
0.31. Configure branch protection via GitHub settings

**Verification**:
- `gh repo view enerBydev/nexus-beacon-receiver` shows repo
- CI workflow runs on push (even with stub — lint + format should pass)
- `task --list` shows all tasks
- `git config core.hooksPath` points to `scripts/hooks/`

**🏁 COMMIT/PUSH MILESTONE**: Phase 0 Complete

- Commit: `feat: initialize nexus-beacon-receiver repo with CI/CD infrastructure`
- Push to: `enerBydev/nexus-beacon-receiver` main branch
- CI should pass (format + lint on stub lib.rs)

---

### Phase 1: Core Types & Deserialization

**Goal**: Define all Rust types for the Worker — requests, responses, database rows.
These types are the **contract** that everything else depends on.

**What this phase creates**: Type definitions in `src/lib.rs` (top section)

**Steps**:

1.1. Define `BeaconPayload` struct (deserialized from POST /v1/beacon)
  - `instance_id: String`
  - `version: String`
  - `date: String`
  - `stats: BeaconStats`
  - Validation: `instance_id` non-empty ≤128 chars, `version` non-empty, `date` matches `YYYY-MM-DD`

1.2. Define `BeaconStats` struct (nested in payload)
  - `total_requests: u64`
  - `unique_fingerprints: u32`
  - `models_used: std::collections::HashMap<String, u64>`
  - `client_types: std::collections::HashMap<String, u32>`
  - `avg_message_count: f64`
  - `tool_use_ratio: f64`

1.3. Define `GlobalStatsResponse` struct (serialized for GET /v1/stats)
  - `generated_at: String`
  - `period_days: u32`
  - `global: GlobalSummary`
  - `models_used: HashMap<String, u64>`
  - `client_types: HashMap<String, u32>`
  - `versions: HashMap<String, u32>`
  - `daily: Vec<DailySummary>`

1.4. Define `GlobalSummary` struct
  - `total_instances: u32`
  - `total_requests: u64`
  - `total_unique_users: u32`
  - `avg_message_count: f64`
  - `tool_use_ratio: f64`

1.5. Define `DailySummary` struct
  - `date: String`
  - `total_instances: u32`
  - `total_requests: u64`
  - `total_unique_users: u32`

1.6. Define `SummaryResponse` struct (for GET /v1/stats/summary)
  - `total_instances: u32`
  - `total_requests: u64`
  - `total_unique_users: u32`
  - `period: String` (e.g., "7d")

1.7. Define `ErrorResponse` struct (for all error responses)
  - `error: String`
  - `detail: Option<String>`

1.8. Add `#[derive(Debug, Deserialize, Serialize)]` to all structs
1.9. Add `#[serde(rename_all = "snake_case")]` where appropriate
1.10. Write `#[cfg(test)]` unit tests for:
  - `BeaconPayload` deserialization from valid JSON
  - `BeaconPayload` deserialization with missing fields (should fail)
  - `BeaconStats` deserialization with empty models_used/client_types
  - `GlobalStatsResponse` serialization produces correct JSON
  - `SummaryResponse` serialization
  - `ErrorResponse` serialization

1.11. Run `cargo test` — all type tests pass
1.12. Run `cargo clippy -- -D warnings` — clean
1.13. Run `cargo fmt -- --check` — clean

**Verification**: All types compile, (de)serialize correctly, tests pass.

**🏁 COMMIT/PUSH MILESTONE**: Phase 1 Complete

- Commit: `feat: define core types for beacon payload, stats response, error response`
- Push to: feature branch → PR → CodeRabbit feedback loop → merge

---

### Phase 2: Validation & Auth Helpers

**Goal**: Implement validation functions and auth check — the gatekeepers that
all handlers depend on. TDD: write tests first, then implement.

**What this phase creates**: Helper functions in `src/lib.rs`

**Steps**:

2.1. **TDD RED**: Write failing tests for `validate_auth()`
  - Test: valid token → Ok(())
  - Test: missing Authorization header → Err(401)
  - Test: wrong token → Err(401)
  - Test: malformed header (no "Bearer" prefix) → Err(401)

2.2. **TDD GREEN**: Implement `validate_auth(req: &Request, env: &Env) -> Result<()>`
  - Extract `Authorization` header
  - Split "Bearer <token>"
  - Compare with `env.secret("BEACON_AUTH_TOKEN")`
  - Return `Err` with 401 status if invalid

2.3. **TDD RED**: Write failing tests for `validate_beacon_payload()`
  - Test: valid payload → Ok(())
  - Test: empty instance_id → Err(400)
  - Test: instance_id > 128 chars → Err(400)
  - Test: empty version → Err(400)
  - Test: date not YYYY-MM-DD → Err(400)
  - Test: negative total_requests → Err(400)
  - Test: negative unique_fingerprints → Err(400)

2.4. **TDD GREEN**: Implement `validate_beacon_payload(payload: &BeaconPayload) -> Result<()>`
  - Check `instance_id.is_empty()` and `.len() > 128`
  - Check `version.is_empty()`
  - Check `date` matches regex `^\d{4}-\d{2}-\d{2}$` (simple string check)
  - Check `total_requests` >= 0 (u64 is always >= 0, but check i64 cast)
  - Check `unique_fingerprints` >= 0

2.5. **TDD RED**: Write failing tests for `validate_date_format()`
  - Test: "2026-06-01" → Ok
  - Test: "2026-6-1" → Err
  - Test: "not-a-date" → Err
  - Test: "" → Err

2.6. **TDD GREEN**: Implement `validate_date_format(date: &str) -> Result<()>`
  - Check length == 10
  - Check dashes at positions 4 and 7
  - Check digits at other positions
  - (No need for full calendar validation — just format)

2.7. **TDD RED**: Write failing tests for `merge_json_maps()`
  - Test: merge `{"a": 1}` + `{"b": 2}` → `{"a": 1, "b": 2}`
  - Test: merge `{"a": 1}` + `{"a": 3}` → `{"a": 4}` (sum values)
  - Test: merge `{}` + `{"a": 1}` → `{"a": 1}`
  - Test: merge `{}` + `{}` → `{}`

2.8. **TDD GREEN**: Implement `merge_json_maps(existing: &str, incoming: &str) -> String`
  - Parse both JSON strings to `HashMap<String, u64>`
  - Sum values for overlapping keys
  - Serialize back to JSON string
  - This is critical for D1 upserts: `models_used` and `client_types` are JSON maps
  that need to be merged when a beacon replaces an old one for the same `(instance_id, date)`

2.9. **TDD RED**: Write failing test for `cors_headers()`
  - Test: returns Headers with correct Access-Control-Allow-Origin
  - Test: includes Access-Control-Allow-Methods
  - Test: includes Access-Control-Allow-Headers

2.10. **TDD GREEN**: Implement `cors_headers(origins: &str) -> Headers`
  - Parse `CORS_ORIGINS` from env var
  - Return Headers with CORS fields
  - Default: `*` if not configured

2.11. Run `cargo test` — all validation tests pass
2.12. Run `cargo clippy -- -D warnings` — clean

**Verification**: All helpers tested, edge cases covered, clippy clean.

**🏁 COMMIT/PUSH MILESTONE**: Phase 2 Complete

- Commit: `feat: implement auth validation, payload validation, JSON merge, CORS helpers`
- Push to: feature branch → PR → CodeRabbit feedback loop → merge

---

### Phase 3: POST /v1/beacon Handler

**Goal**: Implement the beacon receiving endpoint — the core write path.
This is the most critical handler: validate → store → acknowledge.

**What this phase creates**: `handle_beacon_post()` function + D1 write logic

**Steps**:

3.1. **TDD RED**: Write integration test skeleton for POST /v1/beacon
  - Test: valid beacon → 200 `{"status": "ok"}`
  - Test: missing auth → 401
  - Test: invalid payload → 400
  - Note: integration tests require `wrangler dev` running — these will be
  manual/CI integration tests, not `cargo test` unit tests

3.2. Implement `handle_beacon_post(req: Request, ctx: RouteContext<()>) -> Result<Response>`
  - Step 1: Call `validate_auth(&req, &ctx.env)?`
  - Step 2: Parse body as `BeaconPayload`
  - Step 3: Call `validate_beacon_payload(&payload)?`
  - Step 4: Get D1 database from `ctx.env.d1("DB")?`
  - Step 5: Upsert into `beacons` table:
    ```sql
    INSERT INTO beacons (instance_id, version, date, total_requests,
      unique_fingerprints, models_used, client_types, avg_message_count, tool_use_ratio)
    VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
    ON CONFLICT(instance_id, date) DO UPDATE SET
      version=excluded.version,
      total_requests=excluded.total_requests,
      unique_fingerprints=excluded.unique_fingerprints,
      models_used=excluded.models_used,
      client_types=excluded.client_types,
      avg_message_count=excluded.avg_message_count,
      tool_use_ratio=excluded.tool_use_ratio
    ```
  - Step 6: Upsert into `daily_global_stats` table:
    - On INSERT: use incoming values directly
    - On UPDATE: need to recalculate:
      1. Subtract OLD beacon values from global stats
      2. Add NEW beacon values to global stats
      3. This requires reading the OLD beacon first
    - Alternative (simpler): recalculate from all beacons for that day
      ```sql
      -- Delete old global stat for this date
      DELETE FROM daily_global_stats WHERE date = ?;
      -- Re-aggregate from all beacons
      INSERT INTO daily_global_stats (date, total_instances, total_requests, ...)
      SELECT date, COUNT(DISTINCT instance_id), SUM(total_requests), ...
      FROM beacons WHERE date = ? GROUP BY date;
      ```
    - **Decision**: Re-aggregation approach is simpler and more correct.
      With <1K beacons/day, the query is instant on D1.

  - Step 7: Return `200 {"status": "ok"}`

3.3. Write unit tests for the D1 upsert SQL logic (using mock/stub approach)
  - Test: first beacon for a date → creates new row
  - Test: duplicate (instance_id, date) → replaces old values
  - Test: re-aggregation: 2 beacons on same date → sums correctly

3.4. Implement error handling:
  - Auth failure → 401 `{"error": "invalid_auth_token"}`
  - Parse failure → 400 `{"error": "invalid_payload", "detail": "..."}`
  - Validation failure → 400 `{"error": "invalid_payload", "detail": "..."}`
  - D1 error → 500 `{"error": "internal_error"}`
  - Payload too large (>4KB) → 413 `{"error": "payload_too_large"}`

3.5. Add request size check (4KB limit):
  - Check `Content-Length` header before parsing
  - If > 4096 bytes → return 413

3.6. Run `cargo test` — all unit tests pass
3.7. Run `cargo clippy -- -D warnings` — clean
3.8. Run `cargo fmt -- --check` — clean

**Verification**: Handler compiles, unit tests pass, error paths covered.

**🏁 COMMIT/PUSH MILESTONE**: Phase 3 Complete

- Commit: `feat: implement POST /v1/beacon handler with D1 upsert and re-aggregation`
- Push to: feature branch → PR → CodeRabbit feedback loop → merge

---

### Phase 4: GET /v1/stats Handler

**Goal**: Implement the global stats endpoint — the core read path.
This is what consumers (enerby.dev, badges) will call.

**What this phase creates**: `handle_stats_get()` function + D1 read logic

**Steps**:

4.1. **TDD RED**: Write failing unit tests for stats aggregation logic
  - Test: single day → correct GlobalSummary
  - Test: 7 days → correct daily array (sorted newest-first)
  - Test: empty database → empty response with 0s
  - Test: `?days=1` → only today
  - Test: `?days=90` → up to 90 days

4.2. Implement `handle_stats_get(req: Request, ctx: RouteContext<()>) -> Result<Response>`
  - Step 1: Parse `?days=N` query parameter (default: 7, range: 1–90)
  - Step 2: Clamp `days` to [1, 90]
  - Step 3: Get D1 database
  - Step 4: Query `daily_global_stats`:
    ```sql
    SELECT * FROM daily_global_stats
    WHERE date >= date('now', ?||' days ago')
    ORDER BY date DESC
    ```
  - Step 5: Query raw beacons for model/client_type aggregation:
    ```sql
    SELECT models_used, client_types, version FROM beacons
    WHERE date >= date('now', ?||' days ago')
    ```
  - Step 6: Aggregate `models_used` across all beacons (merge JSON maps)
  - Step 7: Aggregate `client_types` across all beacons (merge JSON maps)
  - Step 8: Count distinct versions from beacons → `versions` map
  - Step 9: Build `GlobalStatsResponse` from aggregated data
  - Step 10: Return with CORS headers

4.3. Implement `aggregate_models_and_client_types()` helper
  - Takes Vec of (models_used_json, client_types_json) tuples
  - Returns merged (models_map, client_types_map)
  - Uses `merge_json_maps()` from Phase 2

4.4. Write unit tests for aggregation:
  - Test: merge `{"claude-sonnet-4-6": 100}` + `{"claude-opus-4-5": 50}` → correct
  - Test: overlapping keys sum correctly
  - Test: empty input → empty maps

4.5. Add CORS headers to response
4.6. Handle edge cases:
  - No data for requested period → return 0s, not 404
  - Invalid `?days=abc` → default to 7
  - `?days=0` → clamp to 1
  - `?days=999` → clamp to 90

4.7. Run `cargo test`
4.8. Run `cargo clippy -- -D warnings`
4.9. Run `cargo fmt -- --check`

**Verification**: Handler compiles, aggregation logic tested, edge cases covered.

**🏁 COMMIT/PUSH MILESTONE**: Phase 4 Complete

- Commit: `feat: implement GET /v1/stats handler with D1 query and JSON aggregation`
- Push to: feature branch → PR → CodeRabbit feedback loop → merge

---

### Phase 5: GET /v1/stats/summary Handler

**Goal**: Implement the lightweight summary endpoint for badges and widgets.

**What this phase creates**: `handle_summary_get()` function

**Steps**:

5.1. **TDD RED**: Write failing tests for summary response
  - Test: with data → correct SummaryResponse
  - Test: no data → zeros with "7d" period
  - Test: serialization produces `{"total_instances": N, ...}`

5.2. Implement `handle_summary_get(req: Request, ctx: RouteContext<()>) -> Result<Response>`
  - Step 1: Query last 7 days from `daily_global_stats`
  - Step 2: Sum `total_instances` (COUNT DISTINCT across all 7 days — need MAX, not SUM)
  - Step 3: Sum `total_requests`
  - Step 4: Sum `total_unique_users` (same — MAX per day, then SUM of MAXes)
  - Wait — `total_instances` in daily_global_stats is already COUNT(DISTINCT instance_id)
  per day. For 7-day summary, we want the MAX (peak) or SUM (cumulative unique).
  - **Decision**: SUM total_requests across 7 days, MAX total_unique_users across 7 days
    (users are not unique across days — a user counted Monday is the same user Tuesday,
    but our fingerprinting is daily, so SUM is acceptable approximation).
  - Actually: `total_unique_users` in `daily_global_stats` is SUM of unique fingerprints
  per instance per day. Across 7 days, SUM is reasonable as "total user-days".
  - **Final decision**: SUM all three metrics across 7 days. Label as "7d" period.
  - Step 5: Build `SummaryResponse`
  - Step 6: Return with CORS headers

5.3. Add CORS headers
5.4. Run `cargo test`
5.5. Run `cargo clippy -- -D warnings`

**Verification**: Summary endpoint returns correct lightweight response.

**🏁 COMMIT/PUSH MILESTONE**: Phase 5 Complete

- Commit: `feat: implement GET /v1/stats/summary lightweight endpoint`
- Push to: feature branch → PR → CodeRabbit feedback loop → merge

---

### Phase 6: Router & Event Fetch Wiring

**Goal**: Wire all 3 handlers into the Worker's router and entry point.

**What this phase modifies**: `src/lib.rs` — `#[event(fetch)]` + Router setup

**Steps**:

6.1. Implement `#[event(fetch)]` entry point:
  ```rust
  #[event(fetch)]
  async fn main(req: Request, env: Env, _ctx: Context) -> Result<Response> {
      let router = Router::new()
          .post("/v1/beacon", handle_beacon_post)
          .get("/v1/stats", handle_stats_get)
          .get("/v1/stats/summary", handle_summary_get)
          .run(req, env)
          .await
  }
  ```

6.2. Add OPTIONS handler for CORS preflight:
  ```rust
  .options("/v1/*path", handle_cors_preflight)
  ```

6.3. Implement `handle_cors_preflight()` — return 204 with CORS headers

6.4. Add 404 catch-all:
  ```rust
  .or_else(|req, _ctx| {
      // Return 404 for unmatched routes
      Response::error("not found", 404)
  })
  ```

6.5. Add request logging (via `worker::log!()`)
  - Log method + path for every request
  - Log response status

6.6. Run `cargo test`
6.7. Run `cargo clippy -- -D warnings`
6.8. Run `cargo fmt -- --check`

**Verification**: Router compiles, all 3 routes wired, CORS preflight works.

**🏁 COMMIT/PUSH MILESTONE**: Phase 6 Complete

- Commit: `feat: wire router with all 3 endpoints, CORS preflight, and 404 handler`
- Push to: feature branch → PR → CodeRabbit feedback loop → merge

---

### Phase 7: D1 Database Provisioning

**Goal**: Create the D1 database, run schema, verify connectivity.

**What this phase does**: Infrastructure setup — no code changes.

**Steps**:

7.1. Create D1 database:
  ```bash
  wrangler d1 create nexus-beacon-db
  ```
  - This outputs a `database_id` — copy it

7.2. Update `wrangler.toml` with the real `database_id` (replace "TODO")

7.3. Run schema against D1:
  ```bash
  wrangler d1 execute nexus-beacon-db --remote --file=schema.sql
  ```

7.4. Verify schema:
  ```bash
  wrangler d1 execute nexus-beacon-db --remote --command="SELECT name FROM sqlite_master WHERE type='table'"
  ```
  - Expected: `beacons`, `daily_global_stats`

7.5. Create D1 indexes:
  ```bash
  wrangler d1 execute nexus-beacon-db --remote --command="CREATE INDEX IF NOT EXISTS idx_beacons_instance ON beacons(instance_id)"
  wrangler d1 execute nexus-beacon-db --remote --command="CREATE INDEX IF NOT EXISTS idx_beacons_date ON beacons(date)"
  ```

7.6. Set secret:
  ```bash
  wrangler secret put BEACON_AUTH_TOKEN
  ```

7.7. Commit updated `wrangler.toml` (with real database_id)

**Verification**: D1 database exists, tables created, secret set.

**🏁 COMMIT/PUSH MILESTONE**: Phase 7 Complete

- Commit: `chore: configure D1 database binding and schema`
- This commit includes the `database_id` — it's not a secret, just an identifier

---

### Phase 8: Local Integration Testing

**Goal**: Verify the complete Worker works locally with `wrangler dev`.

**What this phase does**: Manual + scripted integration testing.

**Steps**:

8.1. Start local dev server:
  ```bash
  wrangler dev
  ```

8.2. Test POST /v1/beacon with valid payload:
  ```bash
  curl -X POST http://localhost:8787/v1/beacon \
    -H "Content-Type: application/json" \
    -H "Authorization: Bearer test-token" \
    -d '{
      "instance_id": "a3f7c28e0a4c",
      "version": "0.17.4",
      "date": "2026-06-02",
      "stats": {
        "total_requests": 1500,
        "unique_fingerprints": 3,
        "models_used": {"claude-sonnet-4-6": 1200, "claude-opus-4-5": 300},
        "client_types": {"claude_code": 2, "sdk": 1},
        "avg_message_count": 12.3,
        "tool_use_ratio": 0.78
      }
    }'
  ```
  - Expected: `200 {"status": "ok"}`

8.3. Test POST /v1/beacon without auth:
  ```bash
  curl -X POST http://localhost:8787/v1/beacon \
    -H "Content-Type: application/json" \
    -d '{"instance_id": "x", "version": "1", "date": "2026-06-02", "stats": {...}}'
  ```
  - Expected: `401 {"error": "invalid_auth_token"}`

8.4. Test POST /v1/beacon with invalid payload:
  ```bash
  curl -X POST http://localhost:8787/v1/beacon \
    -H "Authorization: Bearer test-token" \
    -H "Content-Type: application/json" \
    -d '{"instance_id": "", "version": "1", "date": "bad", "stats": {...}}'
  ```
  - Expected: `400 {"error": "invalid_payload", "detail": "..."}`

8.5. Test GET /v1/stats:
  ```bash
  curl http://localhost:8787/v1/stats?days=7
  ```
  - Expected: JSON with `global`, `models_used`, `client_types`, `daily` fields

8.6. Test GET /v1/stats/summary:
  ```bash
  curl http://localhost:8787/v1/stats/summary
  ```
  - Expected: `{"total_instances": 1, "total_requests": 1500, "total_unique_users": 3, "period": "7d"}`

8.7. Test duplicate beacon (same instance_id + date):
  - Send same payload again → 200 (upsert, not error)

8.8. Test CORS preflight:
  ```bash
  curl -X OPTIONS http://localhost:8787/v1/beacon \
    -H "Origin: https://enerby.dev" \
    -H "Access-Control-Request-Method: POST"
  ```
  - Expected: 204 with CORS headers

8.9. Test 404 for unknown routes:
  ```bash
  curl http://localhost:8787/unknown
  ```
  - Expected: 404

8.10. Create integration test script: `scripts/integration-test.sh`
  - Automates steps 8.2–8.9
  - Returns exit code 0 if all pass, 1 if any fail
  - Add to Taskfile: `task integration-test`

8.11. Fix any issues found during integration testing
8.12. Run `cargo test`, `cargo clippy`, `cargo fmt`

**Verification**: All endpoints work correctly locally.

**🏁 COMMIT/PUSH MILESTONE**: Phase 8 Complete

- Commit: `test: add integration test script for local wrangler dev`
- Push to: feature branch → PR → CodeRabbit feedback loop → merge

---

### Phase 9: Deployment to Cloudflare

**Goal**: Deploy the Worker to Cloudflare's edge network.

**What this phase does**: Production deployment.

**Steps**:

9.1. Build WASM locally to verify:
  ```bash
  wrangler deploy --dry-run
  ```
  - Check WASM binary size (should be <2MB compressed)

9.2. Deploy:
  ```bash
  wrangler deploy
  ```
  - This uploads the WASM binary and configures the Worker

9.3. Verify deployment URL:
  ```bash
  curl https://nexus-beacon-receiver.<subdomain>.workers.dev/health
  ```
  - Should return something (404 is fine — no /health route, but means Worker is live)

9.4. Test production endpoints:
  ```bash
  # Set the real BEACON_AUTH_TOKEN (from wrangler secret put in Phase 7)
  curl -X POST https://nexus-beacon-receiver.<subdomain>.workers.dev/v1/beacon \
    -H "Authorization: Bearer $REAL_TOKEN" \
    -H "Content-Type: application/json" \
    -d '{"instance_id":"test-prod","version":"0.1.0","date":"2026-06-02","stats":{"total_requests":1,"unique_fingerprints":1,"models_used":{},"client_types":{},"avg_message_count":1.0,"tool_use_ratio":0.0}}'
  ```

9.5. Verify D1 data in production:
  ```bash
  wrangler d1 execute nexus-beacon-db --remote --command="SELECT * FROM beacons LIMIT 5"
  ```

9.6. Test GET /v1/stats on production URL
9.7. Test GET /v1/stats/summary on production URL
9.8. Configure custom domain (optional): `beacon.enerby.dev`
  - Via Cloudflare dashboard → Workers → Custom Domains

9.9. Commit deployment documentation:
  - Update README.md with production URL
  - Add deployment section to ARCHITECTURE.md

**Verification**: Worker is live on CF edge, D1 writes work, API returns data.

**🏁 COMMIT/PUSH MILESTONE**: Phase 9 Complete

- Commit: `docs: document production deployment URL and configuration`
- The Worker is now LIVE and receiving beacons

---

### Phase 10: NEXUS-AI-Gateway Changes — Expand ClientType

**Goal**: Modify NEXUS to detect more AI tools. This is a NEXUS repo change.

**Refer to**: [NEXUS-CHANGES.md](./NEXUS-CHANGES.md) · [CLIENT-TYPES.md](./CLIENT-TYPES.md)

**What this phase modifies** (in NEXUS repo):
- `src/telemetry/fingerprint.rs` — expand enum, add detection rules, add tests

**Steps**:

10.1. Expand `ClientType` enum from 4 → 13 variants
10.2. Update `Display` impl with new labels
10.3. Add detection rules in `classify_client_type()` — priority-ordered
10.4. Add ~12 new `#[test]` functions
10.5. Run `cargo test` — all existing + new tests pass
10.6. Run `cargo clippy -- -D warnings` — clean
10.7. Run `cargo fmt` — clean

**Refer to NEXUS-CHANGES.md for detailed code changes.**

**🏁 COMMIT/PUSH MILESTONE**: Phase 10 Complete (NEXUS repo)

- Commit: `feat: expand ClientType detection — Cline, Aider, Continue, Codex, Cursor, Windsurf, Copilot`
- Push to: `feat/85-autonomous-git-sync-system` branch → PR#86
- Includes bug fixes from Phase 0 (gauge + log)

---

### Phase 11: NEXUS-AI-Gateway Changes — Wire Beacon

**Goal**: Make the existing `beacon.rs` code actually execute.

**Refer to**: [NEXUS-CHANGES.md](./NEXUS-CHANGES.md)

**What this phase modifies** (in NEXUS repo):
- `src/main.rs` — spawn daily beacon task (~30 lines)
- `src/telemetry/beacon.rs` — remove dead_code, add auth_token
- `src/config.rs` — add `beacon_auth_token` field
- `Cargo.toml` — add `hostname` crate

**Steps**:

11.1. Add `hostname` crate to `Cargo.toml`
11.2. Add `beacon_auth_token: Option<String>` to `Config` struct
11.3. Load `BEACON_AUTH_TOKEN` env var
11.4. Remove `#![allow(dead_code)]` from `beacon.rs`
11.5. Add `auth_token: String` to `BeaconConfig`
11.6. Update `send_beacon()` to include `Authorization: Bearer <token>` header
11.7. In `main.rs`, spawn beacon task after telemetry init
11.8. Run `cargo test`
11.9. Run `cargo clippy -- -D warnings`
11.10. Run `cargo fmt`

**Refer to NEXUS-CHANGES.md for detailed code changes.**

**🏁 COMMIT/PUSH MILESTONE**: Phase 11 Complete (NEXUS repo)

- Commit: `feat: wire telemetry beacon — daily POST to configured endpoint`
- Push to: `feat/85-autonomous-git-sync-system` branch → PR#86

---

### Phase 12: End-to-End Integration Test

**Goal**: Verify the complete pipeline: NEXUS → CF Worker → API response.

**What this phase does**: Verification only — no code changes.

**Steps**:

12.1. Deploy NEXUS locally with:
  ```
  TELEMETRY_ENABLED=true
  TELEMETRY_BEACON_URL=https://nexus-beacon-receiver.<subdomain>.workers.dev/v1/beacon
  BEACON_AUTH_TOKEN=<the-secret-set-in-wrangler>
  ```

12.2. Send requests to NEXUS with different User-Agents
12.3. Wait for 5-minute warm-up + first beacon
12.4. Check CF Worker logs: `wrangler tail`
12.5. Verify beacon was received and stored in D1
12.6. `curl https://nexus-beacon-receiver.<subdomain>.workers.dev/v1/stats`
12.7. Verify response contains data from step 12.2
12.8. Test from enerby.dev: `fetch("/v1/stats/summary")`

**Verification**: Full pipeline operational.

---

### Phase 13: Consumer Integration & Documentation

**Goal**: Finalize documentation, add consumer examples, close out the project.

**Steps**:

13.1. Update README.md with:
  - Production URL
  - Full API documentation (copy from WORKER-DESIGN.md)
  - Deployment guide
  - Development guide
  - Architecture overview

13.2. Update CHANGELOG.md with release notes
13.3. Ensure all docs in `docs/` are current and accurate
13.4. Add CodeRabbit configuration if custom rules needed
13.5. Final `task check` — all checks pass
13.6. Tag release: `v0.1.0`
13.7. Push tag → GitHub Release auto-created by CI

**🏁 FINAL MILESTONE**: Project v0.1.0 Released

- The complete pipeline is live: NEXUS → CF Worker → D1 → API
- Documentation is comprehensive and current
- CI/CD is operational and mirrors NEXUS patterns
- CodeRabbit review is clean

---

## Part V: Commit/Push Decision Matrix

| Phase | What | Repo | Commit? | Push? | CodeRabbit? |
|-------|------|------|---------|-------|-------------|
| 0 | Repo init + CI/CD infra | beacon-receiver | Yes | Yes → main | Yes (initial) |
| 1 | Core types | beacon-receiver | Yes | PR → merge | Yes |
| 2 | Validation + auth | beacon-receiver | Yes | PR → merge | Yes |
| 3 | POST /v1/beacon | beacon-receiver | Yes | PR → merge | Yes |
| 4 | GET /v1/stats | beacon-receiver | Yes | PR → merge | Yes |
| 5 | GET /v1/stats/summary | beacon-receiver | Yes | PR → merge | Yes |
| 6 | Router wiring | beacon-receiver | Yes | PR → merge | Yes |
| 7 | D1 provisioning | beacon-receiver | Yes | Direct (infra) | No |
| 8 | Integration testing | beacon-receiver | Yes | PR → merge | Yes |
| 9 | Deployment | beacon-receiver | Yes | Direct (deploy) | No |
| 10 | ClientType expansion | **NEXUS** | Yes | PR#86 | Yes |
| 11 | Wire beacon | **NEXUS** | Yes | PR#86 | Yes |
| 12 | E2E test | Both | No code | No push | No |
| 13 | Final docs + release | beacon-receiver | Yes | Tag → release | Yes |

**CodeRabbit rule**: After every PR push, wait for CodeRabbit feedback.
Address all actionable items. Re-push. Repeat until CodeRabbit returns no new feedback.
Only then is the PR ready for merge.

---

## Part VI: Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| workers-rs D1 bindings have bugs | Low | High | Test locally with `wrangler dev` before deploy |
| WASM binary exceeds 10MB compressed | Very Low | Medium | `opt-level = "s"` + LTO + only 3 dependencies |
| `wasm-pack` not installed | Low | Medium | `wrangler` handles WASM build internally — no `wasm-pack` needed |
| Beacon auth token leaked | Medium | Low | Rotate via `wrangler secret put`; payload has zero PII |
| D1 free tier exceeded | Very Low | Low | 5GB storage, 5M reads/day — years at projected scale |
| Cold start latency > 100ms | Low | Low | Rust WASM ~50ms; beacon is async fire-and-forget |
| `merge_json_maps` has integer overflow | Very Low | Medium | Use `u64` for counts; real data won't overflow |
| CORS blocks legitimate consumers | Low | Medium | Default `*` during dev; restrict to `*.enerby.dev` in prod |
| NEXUS and beacon-receiver version drift | Low | Low | Independent versioning — they communicate via API contract |
| `wrangler deploy` fails | Low | High | Test with `--dry-run` first; check CF dashboard for errors |

---

## Part VII: TDD Strategy (Per Manifesto)

Every phase follows the same TDD cycle:

```
1. RED    → Write test that describes the expected behavior
2. GREEN  → Write minimal code that makes the test pass
3. IMPROVE → Refactor while keeping tests green
4. VERIFY → cargo test + cargo clippy + cargo fmt
```

### Test Categories

| Category | Scope | Tool | Count (est.) |
|----------|-------|------|-------------|
| Unit tests | Type (de)serialization, validation logic | `cargo test` | ~25 |
| Unit tests | JSON merge, date validation, CORS | `cargo test` | ~10 |
| Integration tests | HTTP endpoints + D1 queries | `wrangler dev` + curl / integration-test.sh | ~8 |
| E2E test | NEXUS → CF Worker → API response | Manual + `wrangler tail` | 1 |

### Test File Structure

Since this is a single-file Worker (`src/lib.rs`), all unit tests live in:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    // Type tests
    #[test]
    fn deserialize_valid_beacon_payload() { ... }

    // Validation tests
    #[test]
    fn validate_auth_valid_token() { ... }

    // JSON merge tests
    #[test]
    fn merge_two_non_overlapping_maps() { ... }
    // ... etc
}
```

Integration tests live in `scripts/integration-test.sh` (curl-based).

---

## Appendix A: wrangler.toml (Full)

```toml
name = "nexus-beacon-receiver"
main = "src/lib.rs"
compatibility_date = "2026-05-09"
compatibility_flags = ["nodejs_compat"]

[[d1_databases]]
binding = "DB"
database_name = "nexus-beacon-db"
database_id = "TODO"  # Replaced after `wrangler d1 create`

[vars]
CORS_ORIGINS = "https://enerby.dev,https://www.enerby.dev"

# Secrets (set via `wrangler secret put`):
# - BEACON_AUTH_TOKEN — required for POST /v1/beacon
```

---

## Appendix B: Cargo.toml (Full)

```toml
[package]
name = "nexus-beacon-receiver"
version = "0.1.0"
edition = "2021"
authors = ["enerBydev <rjmemdoza.s@gmail.com>"]
description = "NEXUS global telemetry receiver — Cloudflare Worker (Rust/WASM)"
license = "MIT"
repository = "https://github.com/enerBydev/nexus-beacon-receiver"

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
```

---

## Appendix C: schema.sql (Full)

```sql
-- NEXUS Beacon Receiver — D1 Database Schema
-- Run once: wrangler d1 execute nexus-beacon-db --remote --file=schema.sql

CREATE TABLE IF NOT EXISTS beacons (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    instance_id TEXT NOT NULL,
    version TEXT NOT NULL,
    date TEXT NOT NULL,
    total_requests INTEGER NOT NULL,
    unique_fingerprints INTEGER NOT NULL DEFAULT 0,
    models_used TEXT NOT NULL DEFAULT '{}',
    client_types TEXT NOT NULL DEFAULT '{}',
    avg_message_count REAL NOT NULL DEFAULT 0,
    tool_use_ratio REAL NOT NULL DEFAULT 0,
    received_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(instance_id, date)
);

CREATE TABLE IF NOT EXISTS daily_global_stats (
    date TEXT PRIMARY KEY,
    total_instances INTEGER NOT NULL DEFAULT 0,
    total_requests INTEGER NOT NULL DEFAULT 0,
    total_unique_users INTEGER NOT NULL DEFAULT 0,
    models_used TEXT NOT NULL DEFAULT '{}',
    client_types TEXT NOT NULL DEFAULT '{}',
    avg_message_count REAL NOT NULL DEFAULT 0,
    tool_use_ratio REAL NOT NULL DEFAULT 0,
    versions TEXT NOT NULL DEFAULT '{}',
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_beacons_instance ON beacons(instance_id);
CREATE INDEX IF NOT EXISTS idx_beacons_date ON beacons(date);
```

---

## Appendix D: Taskfile.yaml (Full Design)

```yaml
version: '3'

vars:
  WORKER_NAME: nexus-beacon-receiver
  WRANGLER: wrangler

tasks:
  default:
    desc: Show available tasks
    cmds:
      - task --list

  # === BUILD ===
  build:
    desc: Build WASM binary (via wrangler)
    cmds:
      - "{{.WRANGLER}} deploy --dry-run"

  # === DEVELOPMENT ===
  dev:
    desc: Start local dev server (wrangler dev)
    cmds:
      - "{{.WRANGLER}} dev"

  # === TESTING ===
  test:
    desc: Run cargo tests
    cmds:
      - cargo test

  test-verbose:
    desc: Run tests with verbose output
    cmds:
      - cargo test -- --nocapture

  integration-test:
    desc: Run integration tests against local wrangler dev
    cmds:
      - bash scripts/integration-test.sh

  # === QUALITY ===
  fmt:
    desc: Format code
    cmds:
      - cargo fmt

  fmt-check:
    desc: Check code formatting
    cmds:
      - cargo fmt -- --check

  lint:
    desc: Run clippy linter
    cmds:
      - cargo clippy -- -D warnings

  lint-fix:
    desc: Run clippy with auto-fix
    cmds:
      - cargo clippy --fix --allow-dirty --allow-staged

  check:
    desc: Run all checks (fmt, lint, test)
    cmds:
      - task: fmt-check
      - task: lint
      - task: test

  # === DEPLOYMENT ===
  deploy:
    desc: Deploy to Cloudflare Workers
    cmds:
      - "{{.WRANGLER}} deploy"

  deploy-dry-run:
    desc: Preview deployment (no actual deploy)
    cmds:
      - "{{.WRANGLER}} deploy --dry-run"

  tail:
    desc: Follow Worker logs (wrangler tail)
    cmds:
      - "{{.WRANGLER}} tail"

  # === D1 DATABASE ===
  d1-create:
    desc: Create D1 database
    cmds:
      - "{{.WRANGLER}} d1 create nexus-beacon-db"

  d1-schema:
    desc: Run schema.sql against D1
    cmds:
      - "{{.WRANGLER}} d1 execute nexus-beacon-db --remote --file=schema.sql"

  d1-query:
    desc: Run ad-hoc D1 query
    cmds:
      - "{{.WRANGLER}} d1 execute nexus-beacon-db --remote --command='{{.CLI_ARGS}}'"

  d1-list-tables:
    desc: List D1 tables
    cmds:
      - "{{.WRANGLER}} d1 execute nexus-beacon-db --remote --command='SELECT name FROM sqlite_master WHERE type=\"table\"'"

  # === SECRETS ===
  secret-set:
    desc: Set BEACON_AUTH_TOKEN secret
    cmds:
      - "{{.WRANGLER}} secret put BEACON_AUTH_TOKEN"

  # === VERSION MANAGEMENT ===
  version:
    desc: Show current version
    cmds:
      - echo "VERSION file: $(cat VERSION)"
      - echo "Cargo.toml: $(grep '^version' Cargo.toml | sed 's/version = \"\(.*\)\"/\\1/')"
      - git describe --tags --always 2>/dev/null || echo "Git tag: none"

  version-check:
    desc: Validate version sync across VERSION + Cargo.toml
    cmds:
      - |
        V1=$(cat VERSION)
        V2=$(grep '^version' Cargo.toml | sed 's/version = "\(.*\)"/\1/')
        echo "VERSION file: $V1"
        echo "Cargo.toml: $V2"
        if [ "$V1" = "$V2" ]; then
          echo "✅ Both sources in sync"
        else
          echo "❌ VERSION MISMATCH"
          exit 1
        fi

  bump-patch:
    desc: Bump PATCH version
    cmds:
      - ./scripts/bump-version.sh $(./scripts/increment-version.sh patch)

  bump-minor:
    desc: Bump MINOR version
    cmds:
      - ./scripts/bump-version.sh $(./scripts/increment-version.sh minor)

  bump-major:
    desc: Bump MAJOR version
    cmds:
      - ./scripts/bump-version.sh $(./scripts/increment-version.sh major)

  auto-version:
    desc: Auto-detect version bump from commits and apply
    cmds:
      - bash scripts/auto-version.sh --apply

  auto-version-dry:
    desc: Preview auto-version bump without applying
    cmds:
      - bash scripts/auto-version.sh --dry-run

  release:
    desc: Create and push a release tag
    cmds:
      - |
        VERSION=$(cat VERSION)
        TAG="v$VERSION"
        if git rev-parse "$TAG" >/dev/null 2>&1; then
          echo "Tag $TAG already exists — bump version first"
          exit 1
        fi
        git tag -a "$TAG" -m "Release $VERSION"
        git push origin "$TAG"
        echo "✅ Release tag $TAG pushed"

  full-release:
    desc: One-command release (auto-bump + commit + tag + push + deploy)
    cmds:
      - task: auto-version
      - git push origin main --tags
      - task: deploy
      - echo "✅ Release pushed and deployed"

  # === SETUP ===
  setup-hooks:
    desc: Configure git to use portable hooks
    cmds:
      - bash scripts/setup-hooks.sh

  setup:
    desc: Full project setup (hooks + verify build)
    cmds:
      - task: setup-hooks
      - task: build
      - echo "✅ Project ready!"

  # === UTILITIES ===
  size:
    desc: Show WASM binary size
    cmds:
      - find target -name "*.wasm" -exec ls -lh {} \; 2>/dev/null || echo "No WASM built yet"

  clean:
    desc: Clean build artifacts
    cmds:
      - cargo clean
      - echo "✅ Cleaned"
```

---

## Appendix E: Git Hooks (Adapted from NEXUS)

### pre-commit

```sh
#!/bin/sh
# Pre-commit hook for code quality checks
echo "🔍 Running pre-commit checks..."

# Check for secrets
if git diff --cached --name-only | xargs grep -l "API_KEY\|SECRET\|PASSWORD\|PRIVATE_KEY\|AUTH_TOKEN" 2>/dev/null; then
    echo ""
    echo "⚠️ Warning: Potential secrets detected in staged files"
    echo " Please review before committing"
    echo ""
fi

# Check for large files (>1MB)
LARGE_FILES=$(git diff --cached --name-only | xargs -I {} sh -c 'if [ -f "{}" ]; then stat -c%s "{}" 2>/dev/null; fi' | awk '$1 > 1048576 {print}')
if [ -n "$LARGE_FILES" ]; then
    echo ""
    echo "⚠️ Warning: Large files detected (>1MB)"
    echo " WASM binaries should be in .gitignore"
    echo ""
fi

# Check Rust formatting
RUST_FILES=$(git diff --cached --name-only -- '*.rs')
if [ -n "$RUST_FILES" ]; then
    echo "🔧 Checking Rust code quality..."
    if ! cargo fmt --check 2>/dev/null; then
        echo "❌ Code formatting issues detected! Run: cargo fmt"
        exit 1
    fi
    echo " ✅ Format check passed"

    # Run clippy (warnings as errors)
    if ! cargo clippy -- -D warnings 2>/dev/null; then
        echo "❌ Clippy errors found! Run: cargo clippy --fix"
        exit 1
    fi
    echo " ✅ Clippy passed"
fi

echo "✅ Pre-commit checks passed"
```

### commit-msg

```sh
#!/bin/sh
# Commit-msg hook for conventional commits validation
COMMIT_MSG_FILE=$1
COMMIT_MSG=$(head -n 1 "$COMMIT_MSG_FILE")

if [ -z "$COMMIT_MSG" ]; then exit 0; fi
if echo "$COMMIT_MSG" | grep -qE "^Merge (branch|tag) "; then exit 0; fi

echo "🔍 Validating commit message format..."

if ! echo "$COMMIT_MSG" | grep -qE "^(feat|fix|chore|docs|refactor|test|ci|perf|style|build|revert)(\(.+\))?!?: .+"; then
    echo ""
    echo "❌ Commit message does not follow conventional commit format!"
    echo ""
    echo "Required: <type>(<scope>): <description>"
    echo "Types: feat, fix, chore, docs, refactor, test, ci, perf, style, build, revert"
    echo ""
    echo "Your message: $COMMIT_MSG"
    exit 1
fi

echo "✅ Valid conventional commit format"
```

### pre-push

```sh
#!/bin/sh
# Pre-push hook — full validation for main branch pushes
# Checks: version sync, cargo test, cargo clippy

remote="$1"
url="$2"

while read local_ref local_sha remote_ref remote_sha; do
    if [ "$remote_ref" = "refs/heads/main" ]; then
        echo "🔍 Validating before push to main..."

        # Version sync check (2 files: VERSION + Cargo.toml)
        V_FILE=$(cat VERSION 2>/dev/null)
        V_CARGO=$(grep '^version' Cargo.toml | sed 's/version = "\(.*\)"/\1/')

        if [ "$V_FILE" != "$V_CARGO" ]; then
            echo "❌ Version mismatch!"
            echo " VERSION: $V_FILE"
            echo " Cargo.toml: $V_CARGO"
            exit 1
        fi
        echo " ✅ Version sync: $V_FILE"

        # Run tests
        echo " Running cargo test..."
        if ! cargo test --quiet 2>/dev/null; then
            echo "❌ Tests failed! Push blocked."
            exit 1
        fi
        echo " ✅ Tests passed"

        # Run clippy
        echo " Running cargo clippy..."
        if ! cargo clippy -- -D warnings 2>/dev/null; then
            echo "❌ Clippy errors! Push blocked."
            exit 1
        fi
        echo " ✅ Clippy passed"

        echo "✅ Pre-push validation passed"
    fi
done
```

### post-commit

```sh
#!/bin/sh
# post-commit hook — suggest version bump
COMMIT_MSG=$(git log -1 --format=%s)
if echo "$COMMIT_MSG" | grep -qE "^chore: bump version to"; then exit 0; fi

if [ -x scripts/auto-version.sh ]; then
    scripts/auto-version.sh --dry-run 2>/dev/null
fi
```

---

## Appendix F: CI Workflows (Adapted from NEXUS)

### ci.yml

```yaml
name: CI/CD Pipeline

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

env:
  CARGO_TERM_COLOR: always
  FORCE_JAVASCRIPT_ACTIONS_TO_NODE24: true

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v5
      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable
        with:
          targets: wasm32-unknown-unknown
      - name: Cache cargo
        uses: actions/cache@v5
        with:
          path: |
            ~/.cargo/registry
            ~/.cargo/git
            target
          key: ${{ runner.os }}-cargo-test-${{ hashFiles('**/Cargo.lock') }}
      - name: Run tests
        run: cargo test

  lint:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v5
      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable
        with:
          components: clippy
      - name: Cache cargo
        uses: actions/cache@v5
        with:
          path: |
            ~/.cargo/registry
            ~/.cargo/git
            target
          key: ${{ runner.os }}-cargo-clippy-${{ hashFiles('**/Cargo.lock') }}
      - name: Run clippy
        run: cargo clippy -- -D warnings

  format:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v5
      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable
      - name: Check formatting
        run: cargo fmt -- --check

  build-wasm:
    runs-on: ubuntu-latest
    needs: [test, lint, format]
    steps:
      - uses: actions/checkout@v5
      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable
        with:
          targets: wasm32-unknown-unknown
      - name: Cache cargo
        uses: actions/cache@v5
        with:
          path: |
            ~/.cargo/registry
            ~/.cargo/git
            target
          key: ${{ runner.os }}-cargo-wasm-${{ hashFiles('**/Cargo.lock') }}
      - name: Build WASM
        run: cargo build --target wasm32-unknown-unknown --release
      - name: Check WASM size
        run: |
          SIZE=$(stat -c%s target/wasm32-unknown-unknown/release/*.wasm 2>/dev/null || echo 0)
          SIZE_MB=$(echo "scale=2; $SIZE / 1048576" | bc)
          echo "WASM binary size: ${SIZE_MB} MB"
          if [ "$SIZE" -gt 10485760 ]; then
            echo "::error::WASM binary exceeds 10MB limit (${SIZE_MB} MB)"
            exit 1
          fi

  release:
    needs: [test, lint, format, build-wasm]
    runs-on: ubuntu-latest
    if: github.ref == 'refs/heads/main'
    permissions:
      contents: write
    steps:
      - uses: actions/checkout@v5
        with:
          fetch-depth: 0
      - name: Get version
        id: get_version
        run: |
          VERSION=$(cat VERSION)
          echo "version=$VERSION" >> $GITHUB_OUTPUT
          echo "tag=v$VERSION" >> $GITHUB_OUTPUT
      - name: CI Summary
        run: |
          echo "## CI/CD Pipeline Results" >> $GITHUB_STEP_SUMMARY
          echo "| Check | Status |" >> $GITHUB_STEP_SUMMARY
          echo "|:------|:------:|" >> $GITHUB_STEP_SUMMARY
          echo "| Tests | ✅ |" >> $GITHUB_STEP_SUMMARY
          echo "| Lint | ✅ |" >> $GITHUB_STEP_SUMMARY
          echo "| Format | ✅ |" >> $GITHUB_STEP_SUMMARY
          echo "| WASM Build | ✅ |" >> $GITHUB_STEP_SUMMARY
          echo "" >> $GITHUB_STEP_SUMMARY
          echo "**Version**: ${{ steps.get_version.outputs.version }}" >> $GITHUB_STEP_SUMMARY
```

### auto-version.yml

```yaml
name: Auto Version & Deploy

on:
  workflow_run:
    workflows: ["CI/CD Pipeline"]
    types: [completed]
    branches: [main]

permissions:
  contents: write

jobs:
  auto-version:
    runs-on: ubuntu-latest
    if: ${{ github.event.workflow_run.conclusion == 'success' && github.event.workflow_run.event == 'push' }}
    steps:
      - uses: actions/checkout@v6
        with:
          fetch-depth: 0
          token: ${{ secrets.GITHUB_TOKEN }}

      - name: Configure git as repo owner
        run: |
          git config user.name "enerBydev"
          git config user.email "rjmemdoza.s@gmail.com"

      - name: Analyze commits for version bump
        id: analyze
        run: |
          CURRENT_VERSION=$(cat VERSION)
          CURRENT_TAG="v${CURRENT_VERSION}"

          if git rev-parse "$CURRENT_TAG" >/dev/null 2>&1; then
            BASE_TAG="$CURRENT_TAG"
          else
            BASE_TAG=$(git tag --sort=-version:refname | head -1)
          fi

          if [ -z "$BASE_TAG" ]; then
            echo "bump=none" >> $GITHUB_OUTPUT
            exit 0
          fi

          COMMITS=$(git log "${BASE_TAG}..HEAD" --oneline 2>/dev/null)
          TOTAL=$(echo "$COMMITS" | grep -c '.' || true)

          if [ "$TOTAL" -eq 0 ]; then
            echo "bump=none" >> $GITHUB_OUTPUT
            exit 0
          fi

          FEAT=$(echo "$COMMITS" | grep -cE '^[a-f0-9]+ feat' || true)
          FIX=$(echo "$COMMITS" | grep -cE '^[a-f0-9]+ fix' || true)
          REFACTOR=$(echo "$COMMITS" | grep -cE '^[a-f0-9]+ refactor' || true)
          PERF=$(echo "$COMMITS" | grep -cE '^[a-f0-9]+ perf' || true)
          BREAKING=$(echo "$COMMITS" | grep -cE '!:' || true)

          if [ "$BREAKING" -gt 0 ] || [ "$FEAT" -gt 0 ]; then
            BUMP="minor"
          elif [ "$FIX" -gt 0 ] || [ "$REFACTOR" -gt 0 ] || [ "$PERF" -gt 0 ]; then
            BUMP="patch"
          else
            BUMP="none"
          fi

          echo "bump=$BUMP" >> $GITHUB_OUTPUT
          echo "current=$CURRENT_VERSION" >> $GITHUB_OUTPUT

      - name: Calculate new version
        id: version
        if: steps.analyze.outputs.bump != 'none'
        run: |
          BUMP=${{ steps.analyze.outputs.bump }}
          CURRENT=${{ steps.analyze.outputs.current }}
          MAJOR=$(echo "$CURRENT" | cut -d. -f1)
          MINOR=$(echo "$CURRENT" | cut -d. -f2)
          PATCH=$(echo "$CURRENT" | cut -d. -f3)

          case $BUMP in
            minor) NEW="$MAJOR.$((MINOR + 1)).0" ;;
            patch) NEW="$MAJOR.$MINOR.$((PATCH + 1))" ;;
          esac

          echo "new=$NEW" >> $GITHUB_OUTPUT
          echo "tag=v$NEW" >> $GITHUB_OUTPUT

      - name: Apply version bump
        if: steps.version.outputs.new != ''
        run: |
          NEW=${{ steps.version.outputs.new }}
          echo "$NEW" > VERSION
          sed -i "s/^version = \".*\"/version = \"$NEW\"/" Cargo.toml

          if [ -f CHANGELOG.md ]; then
            TODAY=$(date +%Y-%m-%d)
            sed -i "s/## \[Unreleased\]/## [Unreleased]\n\n### Added\n\n### Changed\n\n### Fixed\n\n---\n\n## [$NEW] - $TODAY/" CHANGELOG.md
          fi

          git add VERSION Cargo.toml CHANGELOG.md
          git commit -m "chore: bump version to $NEW [skip ci]"
          git tag -a "v$NEW" -m "Release v$NEW"
          git push origin main --tags

      - name: Generate release notes
        id: notes
        if: steps.version.outputs.new != ''
        run: |
          NEW=${{ steps.version.outputs.new }}
          CURRENT=${{ steps.analyze.outputs.current }}

          {
            FEAT_RAW=$(git log "v${CURRENT}..HEAD~1" --format="%s" | grep -E "^feat" || true)
            if [ -n "$FEAT_RAW" ]; then
              echo "### Added"
              echo "$FEAT_RAW" | while IFS= read -r line; do
                CLEAN=$(echo "$line" | sed 's/^feat\([^)]*\)\?: //')
                echo "- ${CLEAN^}"
              done
              echo ""
            fi

            FIX_RAW=$(git log "v${CURRENT}..HEAD~1" --format="%s" | grep -E "^fix" || true)
            if [ -n "$FIX_RAW" ]; then
              echo "### Fixed"
              echo "$FIX_RAW" | while IFS= read -r line; do
                CLEAN=$(echo "$line" | sed 's/^fix\([^)]*\)\?: //')
                echo "- ${CLEAN^}"
              done
            fi
          } > /tmp/release_notes.md
          echo "body_path=/tmp/release_notes.md" >> $GITHUB_OUTPUT

      - name: Create GitHub Release
        if: steps.version.outputs.new != ''
        uses: softprops/action-gh-release@v3
        with:
          tag_name: ${{ steps.version.outputs.tag }}
          name: "${{ steps.version.outputs.tag }}"
          body_path: ${{ steps.notes.outputs.body_path }}
          draft: false
          prerelease: false
```

---

## Appendix G: Version Management Scripts

### bump-version.sh (adapted — 2-file sync)

Same as NEXUS but without `src/lib.rs` VERSION const update.
Updates only: VERSION file + Cargo.toml.

### increment-version.sh

Identical to NEXUS — no changes needed.

### auto-version.sh (adapted — 2-file sync)

Same as NEXUS but:
- Version sync checks only 2 files (VERSION + Cargo.toml)
- No `src/lib.rs` check
- No binary install step (Worker deploys via `wrangler`)

---

## Appendix H: .gitignore

```gitignore
# Build artifacts
/target/
*.wasm
*.rs.bk

# Wrangler
.wrangler/
.dev.vars

# Environment
.env
.env.*
*.env.local

# OS
.DS_Store
Thumbs.db

# IDE
.idea/
.vscode/
*.swp
*.swo

# Logs
*.log

# Claude Code / AI tools
.claude/
.omc/
CLAUDE.md
.gstack/
AGENTS.md
```
