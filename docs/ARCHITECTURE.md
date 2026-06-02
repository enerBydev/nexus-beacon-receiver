# NEXUS Beacon Receiver — Architecture Overview

## What This Is

An **edge microservice** running on Cloudflare Workers (Rust/WASM) that receives
daily telemetry beacons from NEXUS-AI-Gateway instances worldwide, stores them in
D1 (Cloudflare's SQLite), and exposes a public API for consuming aggregated global
usage statistics.

## Why It Exists

NEXUS-AI-Gateway is open-source. Multiple people deploy it independently. The owner
needs visibility into **how many people use it, from where, with what tools, and which
models** — but the code is public, so the security model must resist full source code
inspection + database access.

The beacon receiver is the **central aggregation point** that transforms per-instance
local data into a global picture.

## Architecture Diagram

```
  NEXUS Instance A          NEXUS Instance B          NEXUS Instance N
  (User in Tokyo)           (User in Berlin)          (User in São Paulo)
       |                          |                          |
       |  Daily beacon POST       |  Daily beacon POST       |  Daily beacon POST
       |  (aggregated stats,      |  (aggregated stats,      |  (aggregated stats,
       |   zero PII)              |   zero PII)              |   zero PII)
       v                          v                          v
  ┌──────────────────────────────────────────────────────────────────┐
  │              CLOUDFLARE WORKERS (Edge Microservice)              │
  │                                                                  │
  │  POST /v1/beacon ──► Validate ──► Store in D1 ──► Ack 200      │
  │                                                                  │
  │  GET /v1/stats ────► Query D1 ──► Aggregate ──► JSON response   │
  │                                                                  │
  │  ┌─────────────────────────────────────────────────────────┐     │
  │  │  D1 Database (SQLite at the edge)                       │     │
  │  │  ┌──────────────┐    ┌────────────────────────────┐     │     │
  │  │  │  beacons      │    │  daily_global_stats        │     │     │
  │  │  │  (raw posts)  │───►│  (pre-aggregated per day)  │     │     │
  │  │  └──────────────┘    └────────────────────────────┘     │     │
  │  └─────────────────────────────────────────────────────────┘     │
  └───────────────────────────┬──────────────────────────────────────┘
                              │
              GET /v1/stats   │   CORS: *.enerby.dev
                              v
  ┌──────────────────────────────────────────────────────────────────┐
  │                     CONSUMERS                                     │
  │                                                                  │
  │  • enerby.dev portfolio ── "X active instances globally"         │
  │  • GitHub README badge ── "NEXUS: Y requests processed"         │
  │  • Business decisions ── model popularity, feature adoption      │
  │  • Social proof ── "Used in 30+ countries"                      │
  └──────────────────────────────────────────────────────────────────┘
```

## Key Design Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| **Language** | Rust (WASM via workers-rs) | Same language as NEXUS, type safety, zero-cost abstractions |
| **Platform** | Cloudflare Workers | Free tier covers our scale, global edge, D1 built-in |
| **Storage** | D1 (SQLite) | SQL queries for aggregation, free tier generous, edge-local |
| **Auth** | Shared secret (BEACON_AUTH_TOKEN) | Simple, effective, prevents spam |
| **Code visibility** | Public repo | Transparency builds trust; secrets in wrangler secret |
| **Deployment** | In-tree development, separate repo for production | .gitignore during dev; own repo when mature |

## Components Summary

| Component | Location | Role |
|-----------|----------|------|
| Beacon sender | `src/telemetry/beacon.rs` (NEXUS) | Daily POST with aggregated stats |
| Beacon spawner | `src/main.rs` (NEXUS) | tokio::spawn periodic task |
| Client type detection | `src/telemetry/fingerprint.rs` (NEXUS) | Classify AI tool from User-Agent |
| Beacon receiver | `nexus-beacon-receiver/src/lib.rs` | CF Worker: validate, store, serve |
| D1 schema | `nexus-beacon-receiver/schema.sql` | beacons + daily_global_stats tables |
| Stats API | `nexus-beacon-receiver/src/lib.rs` | GET /v1/stats endpoint |

## Technology Stack

| Layer | Technology | Version |
|-------|-----------|---------|
| Runtime | Cloudflare Workers | — |
| Language | Rust → WASM | 2021 edition |
| SDK | workers-rs | 0.8.3 |
| Database | D1 (SQLite) | — |
| Deployment | wrangler CLI | 4.56.0+ |
| Serialization | serde + serde_json | Latest |
| Timestamps | chrono | Latest |

## Free Tier Limits (sufficient for projected scale)

| Resource | Free Tier | Estimated Usage |
|----------|-----------|-----------------|
| Worker requests | 100K/day | <1K/day (1 beacon/instance/day) |
| D1 reads | 5M/day | <100/day (API queries) |
| D1 writes | 100K/day | <1K/day (beacon inserts) |
| D1 storage | 5 GB | <10 MB (years of data) |
| Worker CPU time | 10ms/invocation | <2ms per beacon |
