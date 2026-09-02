# Agent Interface: How LLM Agents Interact with Their Node

**Wayfinder Research Ticket #3 — Part of #1 (AI Bank: Decentralised credit network for LLM agents)**  
**Branch:** `research/agent-interface` | **Date:** 2026-09-02 | **Author:** Muse Spark (research subagent)  
**Status:** Research complete — decision-ready

---

## TL;DR for Decision-Maker

| Option | What it is | Verdict for AI Bank MVP |
|---|---|---|
| **A: Local HTTP / REST+JSON API** (axum, `127.0.0.1:PORT`) | Node runs a tiny HTTP server on localhost. Agent calls it via tool-use → HTTP client (OpenAI `tools`, Claude `tool_use`, LangChain, etc.). JSON in/out, OpenAPI schema. | **Recommended for MVP.** Universal (every LLM agent framework can do HTTP), localhost-only so no auth server needed, trivial to debug with `curl`, leaves MCP add-on path clean. |
| **B: MCP server (rmcp)** over stdio or Streamable HTTP | Node exposes `tools/list` + `tools/call` over Model Context Protocol (JSON-RPC 2.0). Agent sees the node as a native MCP tool provider (Claude Desktop, Cursor, Copilot, etc. discover tools automatically). | **Recommended as Phase-2 / co-exist.** MCP stdio transport is near-zero extra code once the HTTP service exists; Streamable HTTP transport reuses the same axum server. Do NOT ship MCP-only — many agents don't speak MCP yet. |
| **C: CLI (`ai-bank balance` via subprocess)** | Agent shells out: `exec("ai-bank transfer --to ...")`, parses stdout. | **Reject as primary.** Works only for agents with shell access (Claude Code, OpenCode); fails for sandboxed/API-only agents. Keep CLI for humans, not as agent contract. |
| **D: WebSocket (persistent, bidirectional)** | Long-lived `ws://127.0.0.1:PORT/ws` with JSON-RPC or custom frames. Server can push events. | **Reject for MVP.** Needed only for real-time streaming/steering or multi-device sync. HTTP + polling covers MVP ledger queries; add WS/SSE later if live notifications matter. |

**Bottom line:** Ship MVP as **(A) localhost HTTP API on `127.0.0.1`** with a clean handler layer, then add **(B) MCP stdio + Streamable HTTP** as a thin adapter that reuses those same handlers. CLI stays for humans. WebSocket is deferred. Binding to `127.0.0.1` satisfies "runs on user's machine, no cloud bills" and avoids auth entirely for MVP.

---

## 1. What Can LLM Agents Actually Call?

LLM-based agents do not "browse" an API on their own. The agent framework mediates: the model declares *intent to call a tool*, the framework executes it, feeds the result back, and the model continues reasoning. Three tool-calling dialects dominate; they share one mental model (name + description + JSON Schema) and differ only in envelope.

### 1.1 The shared mental model

- Each callable operation is described by **name**, **description** (when to use it), and a **JSON Schema** for its parameters.
- Description quality (50–200 chars per field) is the single biggest accuracy lever — it is the only documentation the model sees.
- The model may emit 0..N tool calls per turn; the framework should assume parallel calls.

### 1.2 OpenAI function calling / tools

Current wire format (since June 2024) wraps each function with a `type` discriminator; legacy `functions` / `function_call` is deprecated but still accepted.

```json
{
  "type": "function",
  "function": {
    "name": "ai_bank_transfer",
    "description": "Transfer credits from your account to another PeerId. Use after user confirms amount.",
    "parameters": {
      "type": "object",
      "properties": {
        "to":   { "type": "string", "description": "Recipient PeerId (12D3Koo... or bafz...)" },
        "amount": { "type": "integer", "description": "Credits to send (1..1_000_000)" }
      },
      "required": ["to", "amount"],
      "additionalProperties": false
    },
    "strict": true
  }
}
```

- Request field: `tools: [...]`, control via `tool_choice: "auto" | "required" | {"type":"function","name":"..."}`.
- Response field: `output[].type == "function_call"` with `call_id`, `name`, `arguments` (JSON string).
- Strict mode (`strict: true`) enables token-level constrained decoding — requires `additionalProperties: false` and every property in `required`. Without it, the model is fine-tuned but not constrained. Supported on `gpt-4o*` family; graceful fallback on others.

[Source: OpenAI Docs — Function Calling](https://developers.openai.com/api/docs/guides/function-calling) (`tools` param, `strict`, `tool_choice`); [OpenAI — Introducing Structured Outputs](https://openai.com/index/introducing-structured-outputs-in-the-api) (constrained decoding, `additionalProperties: false`); [Jsonic — JSON Schema for Function Calling](https://jsonic.io/guides/json-schema-function-calling) (envelope comparison, `type: function` wrapper note).

### 1.3 Anthropic Claude tool use

Flatter envelope: each tool is `{ name, description, input_schema }` — no `type` wrapper; schema lives under `input_schema`. Claude accepts the broadest JSON Schema dialect and allows parallel `tool_use` blocks by default (disable with `disable_parallel_tool_use`).

```python
tools=[{
  "name": "ai_bank_transfer",
  "description": "Transfer credits from your account to another PeerId. Use after user confirms amount.",
  "input_schema": {"type":"object","properties":{"to":{"type":"string"},"amount":{"type":"integer"}},"required":["to","amount"]}
}]
# Response: content blocks with type "tool_use" { id, name, input }
# Result fed back as type "tool_result" { tool_use_id, content, is_error }
```

Advanced features (Nov 2025): `tool_search` (dynamic discovery without loading every schema), `programmatic_tool_calling` (server executes tool loop without streaming each call), `input_examples` (few-shot per tool). Require beta header `advanced-tool-use-2025-11-20`.

[Source: Anthropic — Tool use overview](https://platform.claude.com/docs/en/agents-and-tools/tool-use/overview) (tool definition, `input_schema`, parallel calls); [Anthropic Skills — Tool Use Concepts](https://github.com/anthropics/skills/blob/main/skills/claude-api/shared/tool-use-concepts.md) (Tool Runner vs manual loop, `tool_choice` values); [Anthropic — Advanced Tool Use engineering post](https://www.anthropic.com/engineering/advanced-tool-use) (beta header, Tool Search / PTC).

### 1.4 MCP tools (Model Context Protocol)

MCP standardizes discovery + invocation at the protocol layer so any MCP-aware client (Claude Desktop, Cursor, VS Code, Copilot, etc.) can discover tools without custom wiring:

- Discovery: client → `tools/list` (`{ "method":"tools/list", "params":{"cursor":...}}`) → server returns `{ tools:[{name,title,description,inputSchema,outputSchema}] }` with pagination/caching (2026-07-28 adds `ttlMs`, `cacheScope`, `resultType`).
- Invocation: client → `tools/call` (`{ "method":"tools/call", "params":{"name":"ai_bank_transfer","arguments":{"to":"...","amount":42}}}`) → server returns `{ content:[{type:"text",text:"..."}], structuredContent:{...}, isError:false }` or JSON-RPC error (`-32602` for unknown tool — NOT `isError:true`).
- Evolution: `outputSchema` (2026-06-18) enables typed `structuredContent`; `x-mcp-header` annotations (2026-07-28) let top-level string/int/bool params be mirrored into `Mcp-Param-*` HTTP headers so intermediaries can route without parsing the body.

[Source: MCP Spec — Tools 2026-07-28](https://modelcontextprotocol.io/specification/2026-07-28/server/tools) (`tools/list`, `tools/call`, `inputSchema`/`outputSchema`, `x-mcp-header`, `tasks/*`); [MCP Spec — Transports 2026-07-28](https://modelcontextprotocol.io/specification/2026-07-28/basic/transports) (header standardization `Mcp-Method`/`Mcp-Name`); [typescript-sdk #1510](https://github.com/modelcontextprotocol/typescript-sdk/issues/1510) (unknown tool MUST be JSON-RPC error, not `isError:true` — fixed in #1389).

### 1.5 What this implies for AI Bank

- **Every agent framework already knows how to do HTTP tool calling.** LangChain, CrewAI, Vercel AI SDK, Claude Agent SDK, OpenAI Agents SDK all have a "define a tool = give a function that does an HTTP fetch" pattern. So exposing a REST+JSON localhost API is the *lowest common denominator* — no agent needs to speak MCP to use it.
- **MCP is a distribution advantage, not a replacement.** MCP-aware hosts auto-discover `tools/list` and render tool UIs. But an MCP-only node would exclude plain HTTP agents and requires Claude/Cursor/etc. to have the server configured (via `claude mcp add` or `.mcp.json`). The right shape is HTTP first, MCP as an adapter on the same handlers.
- **Agents do NOT shell out to CLIs unless the harness allows it.** Browser/edge/sandboxed agents never get a shell. CLI is viable only for "agent on your machine has a terminal" (Claude Code / OpenCode style) — exactly the local-node case, but still narrower than HTTP tool use.

---

## 2. Interface Comparison: HTTP vs MCP vs CLI vs WebSocket

### 2.1 At a glance

| Dimension | **Local HTTP API** (REST+JSON) | **MCP server** (JSON-RPC 2.0) | **CLI** (subprocess) | **WebSocket** |
|---|---|---|---|---|
| **Wire** | HTTP POST/GET, JSON bodies, status codes | JSON-RPC 2.0 over stdio **or** Streamable HTTP (POST single endpoint, optional SSE stream) | Stdin/stdout text, exit codes | Persistent TCP + WS frames, bidirectional |
| **Who initiates** | Agent (client) → Node (server) request/response; server cannot push (poll instead) | Same: client request → server response. Server-initiated `sampling`/`elicitation` exists but client polls for `notifications/tools/list_changed` | Agent forks `ai-bank ...`, reads stdout | Either side at any time (full duplex) |
| **Discovery** | OpenAPI / manual tool definitions in agent config | Automatic: `tools/list` paginated, `listChanged` notifications via `subscriptions/listen` | `--help` text; brittle | Manual |
| **Latency / overhead** | ~1–5 ms localhost, stateless, trivial to cache | stdio: <1 ms (no TCP); Streamable HTTP: same as HTTP but with JSON-RPC framing + optional SSE chunk overhead | Fork+exec ~5–20 ms, serialization via text parsing | Handshake once, then <1 ms frames; but needs reconnection logic |
| **Auth on localhost** | None needed (bind `127.0.0.1` — only local processes can reach it) — see §4 | stdio: inherits process permissions (most secure). Streamable HTTP localhost: same as HTTP | Inherits process user | Same as HTTP but harder to gate |
| **Complexity** | Minimal: routes + extractors, no protocol state machine | Low–medium: handler trait + transport trait; spec versioning (`2025-03-26` vs `2026-07-28`) matters | Minimal to implement, high to *consume* (text parsing, fragility) | High: ping/pong, backpressure, reconnection, message ordering |
| **Real-time / push** | No (use SSE or WS later). For MVP ledger queries, request/response is enough. | Built into Streamable HTTP SSE stream — server can push notifications/resources before result | No | Yes — native |
| **Ecosystem reach** | **Universal** — every agent can do HTTP | **Growing fast** — Claude Desktop/Cursor/VS Code/Copilot native; OpenAI not native yet (needs connector) | **Narrow** — only agents with shell access | Narrow — custom client needed |
| **Debuggability** | `curl`, `httpie`, browser, OpenAPI UI | `mcp-inspector`, `rmcp` examples; `tools/list` → `tools/call` visible in spec | Manual shell | `websocat`, but stateful |
| **Spec churn risk** | HTTP semantics stable since 1999 | MCP revs every ~3–6 months (2024-11-05 HTTP+SSE **deprecated** → 2025-03-26 Streamable HTTP → 2026-07-28 headers/tasks). Official `rmcp` tracks it fast, but you inherit upgrades. | Stable (POSIX) | WS stable; app protocol you invent is not |

Sources: [MCP Spec — Transports 2026-07-28](https://modelcontextprotocol.io/specification/2026-07-28/basic/transports) (stdio newline-delimited, Streamable HTTP single POST, custom transports SHOULD reuse stdio framing, `2024-11-05` HTTP+SSE deprecated); [MCP Transports explainer — stdio vs SSE vs Streamable HTTP](https://www.gingerlabs.ai/blog/mcp-transport-comparison) (decision guide, deprecation notice, stateless Streamable HTTP); [Ably — WebSockets vs HTTP for AI 2026](https://ably.com/blog/websockets-vs-http-for-ai-streaming-and-agents) (HTTP email vs WS phone-call analogy, steering/multi-device needs); [StackOne — Two-Loop Architecture](https://www.stackone.com/blog/ai-agent-cli-mcp-hybrid-architecture/) (inner CLI for local + outer MCP for remote, LLM routes naturally); [MindStudio — CLI vs MCP vs API](https://www.mindstudio.ai/blog/cli-vs-mcp-vs-api-ai-agents) (execution models).

### 2.2 Deeper trade-offs

**Why HTTP wins on generality:** Tool calling (OpenAI/Anthropic) is transport-agnostic — your tool function can be `async fn transfer(to:String, amount:u64) -> String { reqwest::get(format!("http://127.0.0.1:{port}/transfer...")).await?... }`. The model describes the operation; the framework turns it into an HTTP request. Zero agent-side protocol to adopt. By contrast, MCP asks the agent host to have an MCP client and to have configured *your* server entry (command or URL) — frictionless once configured, but an extra onboarding step.

**Why MCP wins on UX/distribution:** Claude/Cursor/VS Code show MCP tools natively, handle pagination/caching, and let users audit/approve tool invocations with human-in-the-loop UI the spec *SHOULD*s (clear visual indicator, confirmation prompt). You get that for free by speaking MCP; with pure HTTP the agent framework must rebuild it.

**Why CLI loses for agents:** Output is unstructured text (or ad-hoc `--json` flag), parse errors on format changes, no typed error codes (exit 0/1), security footgun if agent interpolates user input into a shell string. Text-oriented output also burns tokens when fed back to the LLM. Quantified anecdote: naive MCP 44k tokens vs CLI 1.3k for a simple task is *not* CLI being cheaper — it is MCP being mis-configured (no lazy loading/filtering); with `tool_search` the gap collapses. Do not pick CLI for cost.

**Why WebSocket loses for MVP:** WebSocket shines for *bidirectional streaming* (token streaming, live steering, multi-device session continuity, human-in-the-loop approvals inline) and for *server push* (block arrives, notify all agents). AI Bank MVP transactions are request/response (query balance, submit transfer, list peers). Polling or short-poll + optional SSE covers it. Adding WS means managing upgrades, heartbeats, reconnect storms, backpressure, and auth on a long-lived socket — all orthogonal to ledger/P2P work that already has a networking decision pending (ticket #4).

### 2.3 Common failure modes

| Anti-pattern | Why it hurts | Alternative |
|---|---|---|
| HTTP+SSE legacy transport (`GET /sse` + `POST /messages`) | Deprecated since 2025-03-26 (`[Warn] SHOULD NOT adopt`), removed from `rmcp` entirely. Clients that still expect it need a proxy shim. | Use Streamable HTTP (`POST /mcp` → `application/json` or `text/event-stream`) or stdio. |
| Returning MCP unknown-tool as `isError:true` result | Clients handle JSON-RPC errors and result-level errors on different paths; retry semantics differ; agent loops re-try a missing tool forever. | Throw `McpError` / JSON-RPC `-32602` before the `try` block ([fix #1389](https://github.com/modelcontextprotocol/typescript-sdk/issues/1510)). |
| Tool bloat (exposing 20 tools up front) | Burns context window (Perplexity case: 72% of window before first user message), degrades tool-choice accuracy. | Lean schemas + lazy loading (`tool_search_tool_regex`, `defer_loading`), or expose small `mcp_toolset` per server. |
| Checking `Host` header to decide "is localhost" / "needs auth" | Spoofable — remote caller sends `Host: 127.0.0.1` and bypasses auth ([GHSA-2gpf-2492-q9jh](https://github.com/MervinPraison/PraisonAI/security/advisories/GHSA-2gpf-2492-q9jh)). | Check the *listener* address at startup (bind `127.0.0.1` vs `0.0.0.0`), or the socket's `peer_addr` vs `local_addr`. Never inspect `Host`. |

---

## 3. Rust Options for Each Approach

All crates below are async, Tokio-based, and compatible with a single Tokio runtime — so the node can run one `#[tokio::main]` with axum + rmcp + libp2p side-by-side.

### 3.1 HTTP API — `axum` (recommended)

| Crate | Role | Current version | Notes |
|---|---|---|---|
| **`axum`** | HTTP routing, extractors, `Json<T>` | **0.8.9** (MSRV 1.80) | Tokio team, `tower`/`tower-http` middleware, no routing macros, `/{id}` path syntax in 0.8, `forbid(unsafe_code)`. |
| `tokio` | Async runtime + `TcpListener::bind("127.0.0.1:0")` ephemeral port | 1.x | Required by axum/rmcp/libp2p alike. |
| `serde` + `serde_json` | JSON (de)serialization | 1.x | Define `Input`/`Output` structs, derive `Deserialize`/`Serialize`. |
| `schemars` | Derive JSON Schema from structs (for OpenAPI + MCP `inputSchema` generation) | 1.x | Shared with `rmcp`'s schema path. |
| `utoipa` | OpenAPI 3.x from `#[openapi]` / `ToSchema` | 5.x | Auto-generates Swagger UI; pairs with `utoipa-axum`. |
| `tower-http` | CORS, tracing, compression, timeouts, `ServeDir` | 0.6.x | No custom middleware system — axum inherits Tower services. |

Minimal localhost-only server (10 lines, compiles on axum 0.8):

```rust
use axum::{routing::get, Router, Json};
use serde_json::{json, Value};

#[tokio::main]
async fn main() {
    let app = Router::new()
        .route("/health", get(|| async { Json(json!({"status":"ok"})) }))
        .route("/v1/balance/{peer_id}", get(get_balance))
        .route("/v1/transfer", axum::routing::post(post_transfer));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await.unwrap();
    // ALWAYS 127.0.0.1 for MVP — see §4
    axum::serve(listener, app).await.unwrap();
}
async fn get_balance(axum::extract::Path(peer_id): axum::extract::Path<String>) -> Json<Value> {
    Json(json!({"peerId": peer_id, "credits": 42}))
}
async fn post_transfer(Json(body): Json<Value>) -> (axum::http::StatusCode, Json<Value>) {
    (axum::http::StatusCode::CREATED, Json(json!({"txId":"..."})))
}
```

Testing without a real listener: `axum::serve` is a Tower service; call with `tower::ServiceExt::oneshot` or `axum-test` crate.

[Source: `tokio-rs/axum` README](https://github.com/tokio-rs/axum) (tower middleware, `unsafe_code` forbid, MSRV 1.80); [crates.io — `axum` 0.8.9](https://crates.io/crates/axum); [crates.io — `rmcp` 3.2.0](https://crates.io/crates/rmcp) (lists `axum 0.8` as dev dep, i.e. compatible).

**Alternatives considered**

- `actix-web` 4 + `actix-rt` — mature, fastest raw throughput in benchmarks, but separate actor runtime, custom middleware, macro-heavy routing. Not Tower-compatible; awkward to share Tokio runtime with `rmcp`/`libp2p`. Prefer axum.
- `warp` — elegant filter composition, but fewer examples, less active, no Tower integration. axum has superseded it for most new projects.
- `jsonrpsee` — if you want pure JSON-RPC over HTTP+WS (`POST /` with `{"jsonrpc":"2.0","method":"...","params":...}`) without MCP's tool discovery layer. Valid for agent-to-node calls, but you'd reinvent `tools/list`/typed discovery. Keep REST for agents that expect REST; use `rmcp` if you want JSON-RPC discovery.

### 3.2 MCP — `rmcp` (official Rust SDK)

| Crate | Role | Current version | Feature flags needed |
|---|---|---|---|
| **`rmcp`** | Official MCP SDK, `ServerHandler`/`ClientHandler`, `#[tool]`, JSON-RPC framing | **3.2.0** (Aug 31 2026), tracks spec **2026-07-28**, compatible back to **2025-11-25** | `server`, `client`, `macros`, `schemars`; transports: `transport-io` (stdio server), `transport-streamable-http-server` (HTTP), `transport-streamable-http-client*` |
| `rmcp-macros` | `#[tool_router]`, `#[tool]`, `#[prompt]` proc macros — generates `inputSchema` from `JsonSchema` derived structs | 3.2.0 (re-exported by `rmcp`) | via `macros` |
| `rmcp-actix-web` / `rmcp-openapi` | Community adapters: serve `rmcp` over `actix_web`, or expose OpenAPI endpoints as MCP tools | 0.x | Only if you already use actix |

Declarative tool (schemars derives schema automatically):

```rust
use rmcp::{ServerHandler, tool, tool_router, model::*};
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Debug, Deserialize, JsonSchema)]
struct TransferArgs { to: String, amount: u64 }

#[tool_router]
impl MyBank {
    #[tool(description = "Transfer credits to a peer")]
    async fn transfer(&self, params: rmcp::handler::server::wrapper::Parameters<TransferArgs>)
        -> Result<CallToolResult, rmcp::ErrorData> {
        let TransferArgs { to, amount } = params.0;
        // reuse the same service fn that POST /v1/transfer calls
        let tx_id = self.service.transfer(&to, amount).await.map_err(|e| rmcp::ErrorData::internal_error(e.to_string(), None))?;
        Ok(CallToolResult::success(vec![Content::text(format!("{{\"txId\":\"{tx_id}\"}}"))]))
    }
}
```

Transports (feature-gated — enable only what you use):

| Transport | Server side | Client side | When to use |
|---|---|---|---|
| **stdio** | `feature = "transport-io"`, `transport::stdio()` → `(stdin(), stdout())` | `TokioChildProcess` — spawn server as child | **Default for localhost MCP** — Claude Desktop, Cursor, `claude mcp add --transport stdio -- ai-bank mcp` |
| **Streamable HTTP** | `feature = "transport-streamable-http-server"`, `StreamableHttpService` mounted in an axum Router | `StreamableHttpClientTransport::from_uri("http://127.0.0.1:3000/mcp")` | Remote/HTTP MCP (`claude mcp add --transport http ai-bank http://127.0.0.1:3000/mcp`). Shares the same axum listener; responses may be `application/json` or `text/event-stream`. |

`rmcp` **intentionally does not ship** the deprecated `2024-11-05` two-endpoint HTTP+SSE transport. If you must talk to a legacy server, front it with a proxy; new code uses Streamable HTTP.

Legacy alternatives: `rust-mcp-sdk` (crate `rust-mcp-sdk`, community SDK ~2.0) and `rust-mcp-stack/rust-mcp-sdk` — functional but not official; `rmcp` supersedes them now that it is the official SDK under `modelcontextprotocol/rust-sdk`. Prefer `rmcp`.

[Source: `modelcontextprotocol/rust-sdk` README & crates/rmcp](https://github.com/modelcontextprotocol/rust-sdk) (crates `rmcp` + `rmcp-macros`, tokio runtime, spec 2026-07-28 + backward compat 2025-11-25, transports table, `Legacy HTTP+SSE intentionally not provided`); [crates.io — `rmcp` 3.2.0](https://crates.io/crates/rmcp) (feature flags, MSRV 1.88, deps `tokio`/`schemars`/`hyper`); [crates.io banners — `rmcp` vs `rust-mcp-sdk`](https://crates.io/search?q=rmcp) (official vs community fork distinction).

### 3.3 CLI — `clap` + `tokio::process`

| Crate | Role | Version |
|---|---|---|
| `clap` | `#[derive(Parser)]` arg parsing, `--json` flag, shell completions | 4.x |
| `tokio` / `tokio::process::Command` | Async subprocess for agent `exec` (if you ever test agent CLI invocation) | 1.x |

CLI is orthogonal — keep it for humans (`ai-bank balance`, `ai-bank transfer --to ... --amount 10`, `ai-bank peers list`, `ai-bank export-key`). Flag `--json` for machine-friendly output, but do not make `--json` the agent contract; HTTP is better for typed contracts.

### 3.4 WebSocket — `axum::extract::ws` / `tokio-tungstenite`

| Crate | Role |
|---|---|
| `axum` (feature `ws`) | `WebSocketUpgrade` extractor + `axum::extract::ws::WebSocket` | streams frames via `tokio-tungstenite` underneath, no extra dep needed if you stay on axum |
| `tokio-tungstenite` | Raw `WsStream` if you need custom heartbeat/backoff | 0.26.x |
| `jsonrpsee` | JSON-RPC over WS — if you want `method` routing atop WS (`jsonrpsee::server::ServerBuilder`, `jsonrpsee::core::RpcModule`) | 0.24.x |

Axum WS sketch (defer for now, but this is how it nests with REST):

```rust
// GET /ws upgrades; same Router as REST routes
Router::new()
    .route("/health", get(health))
    .route("/ws", get(ws_handler))

async fn ws_handler(ws: WebSocketUpgrade, State(s): State<AppState>) -> impl IntoResponse {
    ws.on_upgrade(|mut socket| async move {
        while let Some(Ok(msg)) = socket.recv().await {
            if let axum::extract::ws::Message::Text(t) = msg {
                // parse JSON-RPC, call same service, send back
                socket.send(axum::extract::ws::Message::Text(format!("{{\"result\":...}}"))).await.ok();
            }
        }
    })
}
```

**Recommendation:** Do not ship WS in MVP. If later you need push (new block/tx notification, reputation update), first try SSE over the existing axum server (`text/event-stream`); it covers "server → agent events" without WS complexity and Streamable HTTP already uses SSE under the hood.

---

## 4. What "Runs on User's Machine, No Cloud Bills" Implies

This single constraint determines architecture more than any other.

### 4.1 Identity, ownership, cost

- **No central CA, no PKI server, no hosted registry.** Keys generated locally with OS CSPRNG (`libp2p_identity::Keypair::generate_ed25519()` → file on disk, see `node-identity.md`). Onboarding is `cargo run` → identity exists; no registration call.
- **Verification is local.** `PublicKey::verify(msg, sig)` and `PeerId::is_public_key(&key)` need only bytes — no OCSP/CRL lookup.
- **No privileged introducer.** Bootstrap peers are just `Multiaddr`s with embedded PeerId; any node can be a bootstrap. Reputation keys off `PeerId`, not alias (petname layer local-only; see `node-identity.md §1.3`).

### 4.2 Networking / localhost-only for the agent interface

For MVP the agent↔node channel is **loopback-only** — it is not the inter-node P2P channel (ticket #4) and must not be exposed to the network.

**Implication 1 — bind to `127.0.0.1` (or `::1`), not `0.0.0.0`.** Only processes on the same machine can reach the control API. This is the same model used by Prisma Studio, pgAdmin-local, LM Studio default (`127.0.0.1:1234`), and Paperclip "Local Trusted Mode" (`127.0.0.1:3100`).

```rust
// MVP — hardcoded localhost, port from config or ephemeral
let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await?;
// or ephemeral for tests/parallel nodes:
let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?; // OS picks port
let addr = listener.local_addr()?; // remember addr.port() for agent config
```

Verification that a server is localhost-only:
```bash
curl http://localhost:3000/health          # works (loopback)
curl http://192.168.1.100:3000/health      # Connection refused — not listening on LAN iface
```

**Implication 2 — no auth server needed for MVP.** If you can reach the server, you are trusted — because only local processes can reach it and local processes inherit the OS user. This removes session management, tokens, login pages — the UI/API is immediately accessible. See Paperclip Local Trusted Mode pattern cited.

When you outgrow localhost (teammate needs access, or node runs on a server), switch to authenticated mode then: bind `0.0.0.0` + require Bearer token / OAuth. The security decision is "which iface you bound", not a `Host` header check.

**Implication 3 — never gate auth by inspecting the `Host` header.** `Host` is client-supplied and spoofable (`curl -H "Host: 127.0.0.1" https://public.example.com/`). The violated invariant is that "localhost binding" is a server-owned startup/socket property, not a per-request header. Check `listener.local_addr()` at startup, or gate by `peer_addr` being loopback, not by `request.headers["host"]`. [Source: GHSA-2gpf-2492-q9jh discussion](https://github.com/MervinPraison/PraisonAI/security/advisories/GHSA-2gpf-2492-q9jh) (Host-header localhost bypass) + QwenPaw #3582 (localhost bypass via `client_host in ("127.0.0.1","::1")` check failed behind proxy).

**Implication 4 — fail-closed if you ever add network bind without auth.** If `--host 0.0.0.0` is passed and no API key / auth provider is configured, refuse to start. The Hermes Dashboard pattern: "non-loopback bind without an auth provider → refuse to start" is the right fail-closed semantics.

**Implication 5 — CORS is localhost-only by default.** If the dashboard/UI talks to the API via `fetch`, restrict allowed origins to `http://localhost:*`, `http://127.0.0.1:*`. Don't add `*`.

### 4.3 Failure modes without a cloud relay

- Two nodes behind symmetric NATs may not be able to dial each other without UPnP, hole-punching (libp2p DCUtR), or an opt-in community relay. The agent-interface choice does not solve this — but using loopback for agent↔node and libp2p `PeerId` for inter-node keeps both decisions orthogonal (identity layer leaves relay add-on open).
- History retention, snapshots (postfinance-labs paper patterns: head/tail materialization, journaling) remain local; there is no hosted DB to back up. Document the blast radius.

---

## 5. Recommendation for AI Bank MVP

### Decision: Ship local HTTP API on `127.0.0.1` now; add MCP stdio + Streamable HTTP as a thin adapter next.

**Why this order:**

1. **HTTP is universal.** Every LLM agent harness can do `fetch("http://127.0.0.1:PORT/...")`. OpenAI `tools`, Claude `tool_use`, LangChain, Vercel AI SDK, custom JS/Python agent loops — all of them can wrap HTTP calls. MCP clients are growing (Claude Desktop, Cursor, VS Code/Copilot via `claude mcp add`) but still split (OpenAI not native; legacy SSE servers already sunsetting). Shipping HTTP first guarantees no agent is excluded; MCP is then an additive UX win.
2. **HTTP and MCP share the same business logic.** Define a single service layer (`struct BankService { ledger, registry, ... }` with methods `balance(peer_id)->Credits`, `transfer(to, amount)->TxId`, `peers()->Vec<PeerInfo>`, etc.). HTTP handlers `Json`-serialize their inputs/outputs and call `service.*`. MCP `#[tool]` methods do the same — they parse `Parameters<T>` (via the same `JsonSchema`/`Deserialize`) and call `service.*`. No duplication, no divergence.
3. **Localhost binding avoids auth for MVP.** Binding to `127.0.0.1` is a security-and-simplicity win; you defer tokens, OAuth, expiry, revocation — all of which violate "no cloud bills / no hosted auth server". Implement the bind check once at startup; add auth-gated `0.0.0.0` mode later only if teammates need remote access.
4. **Adding MCP later is near-zero cost if you plan for it now.** The trick is to generate JSON Schema from the same structs you already define for the HTTP API (`#[derive(JsonSchema, Deserialize)]` via `schemars`). With `rmcp`'s `#[tool]` macro that schema becomes `inputSchema` automatically. The transport trait means you can support **both stdio and Streamable HTTP simultaneously** with no code change in tools — just serve two transports.
5. **No cloud bills stays intact.** All three localhost patterns (HTTP, stdio MCP, future WS/SSE) are local-only. The only "remote MCP" case — Streamable HTTP over LAN — is an opt-in config change (`--host 0.0.0.0 --auth bearer`) that the operator explicitly chooses, not the default.

### 5.1 Concrete structure (Rust)

```
src/
  service/        # pure logic — no HTTP, no MCP — tested in isolation
    balance.rs
    transfer.rs
    peers.rs
  api/            # HTTP layer — axum Routes → service calls
    mod.rs        # fn router(state: AppState) -> Router
    routes/
      health.rs   # GET  /health
      balance.rs  # GET  /v1/balance/{peerId}
      transfer.rs # POST /v1/transfer
      peers.rs    # GET  /v1/peers
  mcp/            # MCP layer — rmcp ServerHandler → same service calls
    mod.rs        # struct BankMcp { service: Arc<BankService> }
    tools/
      transfer.rs # #[tool] fn transfer(Parameters<TransferArgs>)
      balance.rs  # #[tool] fn get_balance(...)
  bin/
    ai-bank.rs    # clap CLI for humans; also `ai-bank serve` (HTTP) and `ai-bank mcp` (stdio)
Cargo.toml        # deps: axum 0.8, tokio full, serde/serde_json, schemars, rmcp { server, macros, transport-io, transport-streamable-http-server }, clap, libp2p-identity
```

Key `Cargo.toml` excerpt (MSRVs pinned so `rmcp` 3.2 requires Rust 1.88+ — bump `rust-version`):

```toml
[dependencies]
axum = { version = "0.8", features = ["json", "http1", "http2"] }
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
schemars = { version = "1", features = ["derive"] }
rmcp = { version = "3.2", features = ["server", "macros", "transport-io", "transport-streamable-http-server", "schemars"] }
clap = { version = "4", features = ["derive"] }
tower-http = { version = "0.6", features = ["trace", "cors"] }

[dev-dependencies]
axum-test = "0.4"
```

### 5.2 MVP API sketch (illustrative — not a spec)

All endpoints on `http://127.0.0.1:<PORT>`; `PORT` from `~/.ai-bank/config.toml` or `AI_BANK_PORT` env or ephemeral `0` with printed address.

| Method | Path | Input (JSON) | Output | Notes |
|---|---|---|---|---|
| `GET` | `/health` | — | `{status:"ok", peerId:"12D3...", version:"0.1.0"}` | liveness; also proves bind works |
| `GET` | `/v1/balance/{peerId}` | — | `{peerId, credits, nonce}` | ledger read |
| `POST` | `/v1/transfer` | `{to, amount}` | `201 {txId, status}` | signs with local `identity.key`, submits to ledger replication layer (#4) |
| `GET` | `/v1/peers` | — | `[{peerId, alias, addrs, lastSeen}]` | petname table + peerstore |
| `GET` | `/v1/transactions?peerId=&limit=&cursor=` | query | `[{txId, from, to, amount, ts}]` | paginated |
| `GET` | `/openapi.json` | — | OpenAPI 3.x | `utoipa` generated; also drives MCP `inputSchema` via shared types |

MCP tools (same operations, advertised via `tools/list`):

- `get_balance` / `transfer_credits` / `list_peers` / `list_transactions` — thin wrappers that `Schemars`-derive their `inputSchema` from the same structs.

`--help` CLI for humans (not for agents): `ai-bank balance`, `ai-bank transfer --to <peerId> --amount 10`, `ai-bank peers add <peerId> --alias alice`.

### 5.3 Phases

**Phase MVP (this ticket closes here):**

1. `axum` HTTP on `127.0.0.1`, pure `AppState { service: Arc<BankService> }`, JSON, no auth, ephemeral port discoverable via `~/.ai-bank/port` file + `health` response.
2. `clap` CLI piggybacking on the same service layer (`ai-bank serve` starts HTTP; direct CLI commands talk to the service in-process for offline use).
3. Structured errors `{"code":"INSUFFICIENT_FUNDS","message":"..."}` with correct HTTP status; `Port` + `PeerId` validated at extractor level.

**Phase MCP (immediately after, no refactor needed):**

4. Add `rmcp` with `transport-io` (stdio) — `ai-bank mcp` subcommand: `let service = MyBank::new(service.clone()); service.serve((tokio::io::stdin(), tokio::io::stdout())).await?;`. Agents configure `claude mcp add --transport stdio ai-bank -- ai-bank mcp`.
5. Add `transport-streamable-http-server` — mount `StreamableHttpService::new(service)` alongside the axum REST router on the same listener (`Router::new().route("/mcp", …)`). Agents can then use `claude mcp add --transport http ai-bank http://127.0.0.1:3000/mcp`.
6. Publish schemas via `schemars` → MCP `inputSchema`/`outputSchema` reused for HTTP OpenAPI.

**Phase push/live (deferred):**

7. If ledger/gossip needs push, add SSE endpoint `GET /v1/events` or WebSocket `GET /ws` (axum `ws` feature) — only if polling proves insufficient. Streamable HTTP's SSE already provides server→client streaming for MCP consumers.

### 5.4 Open questions to resolve with other tickets

- **#4 (Communication protocol):** Inter-node P2P (libp2p Noise/gossipsub vs HTTP/gRPC) should not reuse the localhost HTTP port; keep agent↔node loopback distinct from node↔node swarm. The `service` layer should not know which transport delivered a transfer — P2P handler and localhost handlers both call it.
- **#2 (Node identity):** `GET /health` returns `peerId` derived from `libp2p-identity::Keypair`; `POST /v1/transfer` signs the transaction envelope with the same key (domain-separated `b"/ai-bank/1/transfer:" || cbor(tx)`). Agents can pass `to` as either `PeerId` string or local petname (`alias` resolved server-side via `peers.json`).
- **Safety docs (#5/#6/#7):** Document localhost-only security model explicitly — "if you can reach the API, you are trusted; binding `0.0.0.0` requires auth (fail-closed)" — so safety evaluation (#7) can assert blast radius.

---

## Appendix: Primary Sources

- MCP Spec — **Transports 2026-07-28** — stdio newline-delimited, Streamable HTTP single POST + SSE, custom transports SHOULD reuse stdio framing, `2024-11-05` HTTP+SSE deprecated vs Streamable HTTP introduction `2025-03-26`. [Link](https://modelcontextprotocol.io/specification/2026-07-28/basic/transports)
- MCP Spec — **Tools 2026-07-28** — `tools/list` pagination/caching, `tools/call`, `inputSchema`/`outputSchema`, `x-mcp-header` routing headers, `notifications/tools/list_changed`, error handling (unknown tool → JSON-RPC `-32602`). [Link](https://modelcontextprotocol.io/specification/2026-07-28/server/tools)
- MCP Spec — **Transports history 2025-03-26** — introduced Streamable HTTP, deprecated HTTP+SSE. [Link](https://modelcontextprotocol.io/specification/2025-03-26/basic/transports) / ported warning in 2026-07-28
- `modelcontextprotocol/rust-sdk` (official `rmcp`) — crates `rmcp` + `rmcp-macros`, supported specs 2026-07-28 / 2025-11-25, transports `transport-io` / `transport-streamable-http-*`, legacy HTTP+SSE intentionally not provided. [Link](https://github.com/modelcontextprotocol/rust-sdk) / [crates.io — `rmcp` 3.2.0](https://crates.io/crates/rmcp)
- `rmcp` Feature flags & transport table — `server`/`client`/`macros`/`schemars`, `transport-io` (stdio server), `transport-streamable-http-server`, stdio `(stdin, stdout)` pair. [Link docs.rs rmcp](https://docs.rs/rmcp/latest/rmcp/)
- OpenAI Docs — **Function Calling** — `tools:[{type:"function", function:{name,description,parameters,strict}}]`, `tool_choice`, `function_call` response shape, strict mode `additionalProperties:false`. [Link](https://developers.openai.com/api/docs/guides/function-calling)
- OpenAI — **Introducing Structured Outputs** — constrained decoding, `strict:true` + `additionalProperties:false` guarantee. [Link](https://openai.com/index/introducing-structured-outputs-in-the-api)
- Anthropic Docs — **Tool Use Overview** — `{name,description,input_schema}`, parallel `tool_use`, `tool_result`, `disable_parallel_tool_use`. [Link](https://platform.claude.com/docs/en/agents-and-tools/tool-use/overview)
- Anthropic Skills — **Tool Use Concepts** — Tool Runner vs manual loop, `tool_choice` values, SDK helpers `anthropic.lib.tools.mcp`. [Link](https://github.com/anthropics/skills/blob/main/skills/claude-api/shared/tool-use-concepts.md)
- Anthropic Engineering — **Advanced Tool Use** (2025-11-24) — `tool_search`, `programmatic_tool_calling`, beta `advanced-tool-use-2025-11-20`. [Link](https://www.anthropic.com/engineering/advanced-tool-use)
- `tokio-rs/axum` — HTTP routing, `tower` middleware, `forbid(unsafe_code)`, MSRV 1.80, `/{id}` path syntax (0.8). [Link](https://github.com/tokio-rs/axum) / [crates.io — `axum` 0.8.9](https://crates.io/crates/axum)
- Axum patterns — router/handlers/extractors/state/error-handling (axum 0.8 tutorial, routing/macros-free). [Link](https://www.rustfinity.com/blog/axum-rust-tutorial) / [Link](https://www.hyaking.com/rust-rest-api-axum-complete-guide/)
- **MCP Transport comparison** — stdio vs SSE vs Streamable HTTP table, deprecation guidance, Streamable HTTP as enterprise standard. [Link](https://www.gingerlabs.ai/blog/mcp-transport-comparison) / [Link](https://rollbrains.com/mcp/mcp-transports-compared/)
- **StackOne — Two-Loop Architecture** — inner CLI for local + outer MCP for remote, LLM routes naturally, token bloat mitigations. [Link](https://www.stackone.com/blog/ai-agent-cli-mcp-hybrid-architecture/)
- **MindStudio — CLI vs MCP vs API** — execution models (subprocess vs HTTP fetch vs MCP JSON-RPC), CLI universal for local but no auth. [Link](https://www.mindstudio.ai/blog/cli-vs-mcp-vs-api-ai-agents)
- **Ably — WebSockets vs HTTP for AI 2026** — WS for conversational/steerable/multi-device, HTTP for stateless/cacheable. [Link](https://ably.com/blog/websockets-vs-http-for-ai-streaming-and-agents)
- **Localhost-only pattern** — Paperclip Local Trusted Mode (`127.0.0.1:3100`, "no login, bind localhost, fail-closed on 0.0.0.0"). [Link](https://www.stanza.dev/courses/paperclip-setup/deployment-modes/paperclip-setup-local-trusted)
- **Hermes Dashboard** — fail-closed on non-loopback bind without auth provider, `127.0.0.1:9119` default. [Link](https://hermes-agent.nousresearch.com/docs/user-guide/features/web-dashboard)
- **GHSA-2gpf-2492-q9jh** — localhost binding MUST be server-owned socket property, not `Host` header (spoofable). [Link advisory](https://github.com/MervinPraison/PraisonAI/security/advisories/GHSA-2gpf-2492-q9jh)
- **QwenPaw #3582** — localhost auth bypass via `Host`/`client_host` check breaks behind proxy. [Link](https://github.com/agentscope-ai/QwenPaw/issues/3582)
- **typescript-sdk #1510** — unknown tool MUST be JSON-RPC error (`-32602`), not `isError:true` result. [Link](https://github.com/modelcontextprotocol/typescript-sdk/issues/1510)
- `jsonic.io — JSON Schema for Function Calling` — envelope diffs OpenAI `parameters` vs Anthropic `input_schema`, `strict` flag requirements. [Link](https://jsonic.io/guides/json-schema-function-calling)

---

*Next step: Decision-maker reviews §5 and opens an ADR (`docs/adr/0003-agent-interface.md`) locking in localhost HTTP (axum) for MVP + rmcp stdio/Streamable HTTP as Phase-2 adapter sharing the same service layer. Ticket #3 can then be closed with a pointer to this file.*
