# NEXUS Beacon Receiver — Problem Statement

## The Problem

NEXUS-AI-Gateway is an open-source API proxy. Anyone can deploy it. The owner currently
has **zero visibility** into:

1. **How many people use it** — Is it 5 people or 5,000?
2. **Where they are** — Which countries, which networks?
3. **What tools they use** — Claude Code? Aider? Cline? Cursor? curl?
4. **What models they request** — Sonnet? Opus? Haiku?
5. **How actively they use it** — 10 requests/day? 10,000?
6. **What version they run** — Are people on the latest release?

Without this data, every decision is a guess:
- Which model to optimize for?
- Which AI tool integration to prioritize?
- Whether the project is growing or stagnant?
- Is the latest release adopted or ignored?

## Why Existing Solutions Don't Work

| Approach | Problem |
|----------|---------|
| GitHub stars | Vanity metric — doesn't reflect actual usage |
| Download counts | People download but may never run it |
| Prometheus metrics | Only visible locally — each instance is an island |
| /analytics endpoint | Only shows THIS instance's data |
| User surveys | Response bias, low coverage |
| Phone-home with plaintext | Security risk — open source means attackers see the code |

## The Core Tension

We need **global usage data** from an **open-source project** where the code is public
and the database is accessible. Any fingerprinting scheme must survive:

- **Full source code inspection** — attacker reads every line
- **Database access** — attacker sees every row in the SQLite
- **Network sniffing** — attacker captures the beacon payload
- **Replay attacks** — attacker re-sends captured beacons

## The Solution: HMAC + Aggregated Beacon

### Security Model (3 layers)

**Layer 1: Local fingerprinting (already implemented in NEXUS)**

```
Client IP 10.0.1.5 → extract /24 prefix "10.0.1" → HMAC(secret, "ip:10.0.1") → "915fc2..."
API key sk-ant-a3-... → take first 8 chars "sk-ant-a3" → HMAC(secret, "key:sk-ant-a3") → "c69d5c..."
```

Properties:
- Irreversible: HMAC is one-way, even with source code
- Not verifiable: attacker can't test if a specific IP is in the DB (no secret)
- Instance-specific: each NEXUS generates its own secret, so fingerprints differ
  across instances (no cross-correlation)
- Secret file chmod 0600: only the process user can read it
- Secret zeroed on Drop: best-effort memory wipe

**Layer 2: Beacon payload (what NEXUS sends daily)**

```json
{
  "instance_id": "a3f7c2...8e0a4c",    // HMAC(secret, hostname) — 32 hex chars
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

What is NOT sent (zero PII):
- No individual fingerprints (only count of unique users)
- No IP addresses (not even hashed — just the count)
- No API keys (not even hashed)
- No User-Agent strings (only the category label)
- No request content, prompts, or responses
- No paths, hostnames, or system info (instance_id is HMAC'd)

**Layer 3: CF Worker storage (what the receiver stores)**

The D1 database stores only what's in the beacon payload — aggregated counts.
Even if D1 is compromised, there is nothing to reverse because the raw PII never
left the NEXUS instance.

### Threat Model

| Threat | Mitigation |
|--------|-----------|
| Attacker reads NEXUS source code | HMAC with instance-specific secret — can't verify hashes without it |
| Attacker reads NEXUS SQLite DB | Only HMAC hex strings stored — irreversible |
| Attacker captures beacon in transit | HTTPS only (CF Workers enforces); payload has zero PII |
| Attacker replays beacon | Auth token + date deduplication in D1 |
| Attacker floods beacon endpoint | Rate limiting per instance_id + auth token |
| Attacker spins up fake NEXUS instances | instance_id is HMAC'd — can't forge without secret |
| Two NEXUS instances have same fingerprint | Different secrets → different HMACs → no cross-correlation |
| Attacker deletes the secret file | All fingerprints change (effectively resets analytics) — this is a feature |

### What the Owner Gets vs What is Impossible

| Question | Answerable? | How |
|----------|-------------|-----|
| How many active instances worldwide? | Yes | COUNT(DISTINCT instance_id) in D1 |
| How many total requests this week? | Yes | SUM(total_requests) in D1 |
| Which AI tools are most popular? | Yes | Aggregate client_types JSON across instances |
| Which models are most requested? | Yes | Aggregate models_used JSON across instances |
| What version are people running? | Yes | GROUP BY version in D1 |
| Is usage growing over time? | Yes | Time series from daily_global_stats |
| Who specifically is using it? | **No** | instance_id is HMAC'd — irreversible |
| What are their IP addresses? | **No** | IPs never leave the NEXUS instance |
| What are their API keys? | **No** | Keys never leave the NEXUS instance |
| Which country are they in? | **Partial** | Only if correlated with CF's geo data (not stored by us) |

This is the right balance: the owner gets business intelligence without
ever possessing personally identifiable information.
