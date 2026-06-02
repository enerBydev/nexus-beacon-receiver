# NEXUS Beacon Receiver — Client Type Detection Specification

## Current State (NEXUS v0.17.4)

The `ClientType` enum in `src/telemetry/fingerprint.rs` has only 4 variants:

```rust
pub enum ClientType {
    ClaudeCode,      // User-Agent contains "claude-code" or "ClaudeCode"
    AnthropicSDK,    // User-Agent contains "anthropic"
    CustomScript,    // curl, python-requests, axios, go-resty, etc.
    Unknown,         // No User-Agent or unrecognized
}
```

This is insufficient. The AI coding tool landscape has exploded — Cline, Aider,
Continue, Cursor, Windsurf, Codex, and others are all making API requests through
proxies like NEXUS. We need to detect and classify them.

## Proposed `ClientType` Enum

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ClientType {
    // --- AI Coding Tools (primary classification) ---
    ClaudeCode,       // Claude Code CLI by Anthropic
    Cline,            // Cline (formerly Claude Dev) — VS Code extension
    Aider,            // Aider — AI pair programming CLI
    Continue,         // Continue — AI code assistant extension
    Codex,            // OpenAI Codex CLI
    Cursor,           // Cursor IDE (inferred from patterns)
    Windsurf,         // Windsurf (formerly Codeium) IDE
    Copilot,          // GitHub Copilot Chat/Agent

    // --- SDKs ---
    AnthropicSDK,     // anthropic-python, anthropic-typescript
    OpenAISDK,        // openai-python, openai-node

    // --- Generic ---
    CustomScript,     // curl, python-requests, axios, etc.
    AnotherProxy,     // Detected by header patterns (future)
    Unknown,          // No User-Agent or unrecognized
}
```

## Detection Rules — Primary Signal: User-Agent Header

Each tool's User-Agent pattern was verified by searching its source code on GitHub.

### Tier 1: Verified (found in source code)

| Tool | User-Agent Pattern | Header Extras | Source |
|------|--------------------|---------------|--------|
| **Claude Code** | `claude-code/X.Y.Z` or `ClaudeCode/X.Y.Z` | `anthropic-beta` header present | Anthropic internal |
| **Cline** | `Cline/X.Y.Z` | `x-client-type`, `x-platform`, `x-client-version` headers | [EnvUtils.ts](https://github.com/cline/cline/blob/main/apps/vscode/src/services/EnvUtils.ts) |
| **Aider** | `Aider/X.Y.Z` (via anthropic SDK, so also `anthropic-python/...`) | — | [scrape.py](https://github.com/Aider-AI/aider/blob/main/aider/scrape.py) |
| **Continue** | `Continue/X.Y.Z` | — | [ClawRouter.ts](https://github.com/continuedev/continue/blob/main/core/llm/llms/ClawRouter.ts) |
| **Codex** | `codex/...` (Rust reqwest client) | `originator` header | [default_client.rs](https://github.com/openai/codex/blob/main/codex-rs/login/src/auth/default_client.rs) |
| **Anthropic SDK Python** | `anthropic-python/X.Y.Z` | — | pip package |
| **Anthropic SDK JS** | `anthropic-typescript/X.Y.Z` | — | npm package |

### Tier 2: Inferred (closed source, patterns estimated)

| Tool | Expected User-Agent Pattern | Detection Method | Confidence |
|------|-----------------------------|------------------|------------|
| **Cursor** | `cursor/X.Y.Z` or embedded `anthropic-sdk` | UA contains "cursor" OR multiple rapid model switches without Anthropic headers | Medium |
| **Windsurf** | `windsurf/X.Y.Z` or `Codeium/...` | UA contains "windsurf" or "codeium" | Medium |
| **Copilot** | `CopilotChat/X.Y.Z` or `GitHub-Copilot/...` | UA contains "copilot" or "github-copilot" | Medium |
| **TRAE** | `trae/...` or `bytedance-...` | UA contains "trae" | Low — unverified |
| **OpenAI SDK Python** | `openai-python/X.Y.Z` | UA contains "openai-python" | High |

### Tier 3: Script/Tool Detection (existing, preserved)

| Pattern | Classification | Existing? |
|---------|---------------|-----------|
| `curl/...` | CustomScript | Yes |
| `python-requests/...` | CustomScript | Yes |
| `axios/...` | CustomScript | Yes |
| `go-resty/...` | CustomScript | Yes |
| `node-fetch/...` | CustomScript | Yes |
| `ruby/...` | CustomScript | Yes |
| `java/...` | CustomScript | Yes |
| `okhttp/...` | CustomScript | Yes |
| `wget/...` | CustomScript | Yes |
| `httpie/...` | CustomScript | Yes |

## Detection Algorithm (priority order)

```
1. Exact match on User-Agent (Tier 1 — highest confidence)
   ├── contains "claude-code" or "claudecode" → ClaudeCode
   ├── contains "cline/"                     → Cline
   ├── contains "aider/"                     → Aider
   ├── contains "continue/"                  → Continue
   ├── contains "codex/"                     → Codex
   ├── contains "cursor/"                    → Cursor
   ├── contains "windsurf/" or "codeium/"    → Windsurf
   ├── contains "copilot"                    → Copilot
   ├── contains "trae/"                      → TRAE (tentative)
   ├── contains "anthropic-python/"          → AnthropicSDK
   ├── contains "anthropic-typescript/"      → AnthropicSDK
   ├── contains "anthropic" (generic)        → AnthropicSDK
   └── contains "openai-python/"             → OpenAISDK

2. Secondary signal (if no UA match)
   ├── anthropic-beta header present         → ClaudeCode
   └── originator header present             → Codex

3. Script signatures (existing logic, preserved)
   └── curl/, python-requests/, axios/, etc. → CustomScript

4. Fallback
   └── Unknown
```

## Special Case: Aider via Anthropic SDK

Aider uses the `anthropic` Python SDK under the hood, so its User-Agent typically
looks like `anthropic-python/0.52.0 Aider/0.86.0`. The detection must check for
`aider` BEFORE `anthropic` in the priority order, otherwise Aider gets classified
as AnthropicSDK when it's actually a distinct tool.

## Display Labels (for Prometheus + /analytics + beacon)

| ClientType | `to_string()` label | Prometheus label |
|------------|---------------------|------------------|
| ClaudeCode | `claude_code` | `claude_code` |
| Cline | `cline` | `cline` |
| Aider | `aider` | `aider` |
| Continue | `continue` | `continue` |
| Codex | `codex` | `codex` |
| Cursor | `cursor` | `cursor` |
| Windsurf | `windsurf` | `windsurf` |
| Copilot | `copilot` | `copilot` |
| AnthropicSDK | `sdk` | `sdk` |
| OpenAISDK | `openai_sdk` | `openai_sdk` |
| CustomScript | `script` | `script` |
| AnotherProxy | `proxy` | `proxy` |
| Unknown | `unknown` | `unknown` |

## Files Modified

| File | Change |
|------|--------|
| `src/telemetry/fingerprint.rs` | Expand `ClientType` enum, update `classify_client_type()`, add new test cases |
| `src/telemetry/metrics.rs` | No change needed — uses `client_type.to_string()` dynamically |
| `src/telemetry/store.rs` | No change needed — stores `client_type` as string |
| `src/telemetry/beacon.rs` | No change needed — payload uses JSON serialization |

## Test Cases Required

```rust
// Tier 1 (verified)
assert_eq!(classify(&[("user-agent", "Cline/3.5.0")]), ClientType::Cline);
assert_eq!(classify(&[("user-agent", "Aider/0.86.0")]), ClientType::Aider);
assert_eq!(classify(&[("user-agent", "Continue/0.8.0")]), ClientType::Continue);
assert_eq!(classify(&[("user-agent", "codex/0.1.0")]), ClientType::Codex);

// Aider via SDK — Aider must take priority over AnthropicSDK
assert_eq!(classify(&[("user-agent", "anthropic-python/0.52.0 Aider/0.86.0")]), ClientType::Aider);

// Secondary signals
assert_eq!(classify(&[("user-agent", "SomeApp/1.0"), ("anthropic-beta", "...")]), ClientType::ClaudeCode);
assert_eq!(classify(&[("user-agent", "SomeApp/1.0"), ("originator", "codex")]), ClientType::Codex);

// Closed source (inferred)
assert_eq!(classify(&[("user-agent", "cursor/0.45.0")]), ClientType::Cursor);
assert_eq!(classify(&[("user-agent", "Windsurf/1.5.0")]), ClientType::Windsurf);
```
