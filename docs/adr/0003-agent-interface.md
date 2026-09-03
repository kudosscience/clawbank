# ADR 0003: Agent interface is localhost HTTP (axum) with MCP adapter as Phase 2

LLM agents interact with their local AI Bank node via a localhost-only HTTP API on `127.0.0.1` (axum + serde/schemars). The HTTP service is the contract; an MCP adapter (`rmcp` stdio + Streamable HTTP) is added as Phase 2 reusing the same service layer. CLI remains for humans; WebSocket is deferred — see `research/agent-interface` decision record.

## Status

Accepted — implements wayfinder ticket [#3 Agent interface: how LLM agents interact with their node](https://github.com/kudosscience/ai-bank/issues/3) → `research/agent-interface` (`docs/research/agent-interface.md:1`).

## Context

Agents are LLM-based tool-using agents (GPT/Claude). The node runs on the user's machine with no cloud bills, so the agent→node channel must be loopback-only without an auth server. The choice must cover OpenAI `tools` / Claude `tool_use` dialect universality, plus growing MCP distribution, without excluding non-MCP agents.

## Considered Options

- **Local HTTP REST+JSON on 127.0.0.1 (chosen for MVP)** — `axum 0.8` + `tokio`, `Json<T>` handlers, OpenAPI via `utoipa`/`schemars`. Universal: every agent framework can do `fetch("http://127.0.0.1:PORT/v1/…")`. `curl`-debuggable, stateless. Binding to `127.0.0.1` (not `0.0.0.0`) satisfies trust boundary — never check `Host` header (GHSA-2gpf-2492-q9jh).
- **MCP server via `rmcp 3.2` (chosen as Phase-2 adapter)** — `#[tool]` / `#[tool_router]` with `schemars`-derived `inputSchema`/`outputSchema`, transports `transport-io` (stdio) and `transport-streamable-http-server` (shares axum listener). Discovery via `tools/list` + `tools/call` JSON-RPC 2.0. Stdio `<1 ms`, Streamable HTTP `POST /mcp` with optional SSE. Not MVP-only: excludes plain HTTP agents and requires host config.
- **CLI subprocess (rejected as primary)** — `exec("ai-bank …")` text parsing, only for shell-capable harnesses; keep `clap 4` CLI for humans (`ai-bank balance|transfer|peers` with `--json`).
- **WebSocket (deferred)** — `axum::extract::ws` / `tokio-tungstenite`; needed only for bidirectional streaming; polling/SSE over existing axum covers MVP (ledger queries/request-response).

All share a single `#[tokio::main]` runtime; service handlers are shared so HTTP and MCP call the same functions.

## Consequences

- Agents define tools as HTTP calls; descriptions drive model accuracy.
- MCP `tools/list`/`tools/call` use same `schemars` schemas; unknown tool returns JSON-RPC `-32602`, not `isError:true`; avoid tool bloat via `tool_search`/lazy loading.
- Legacy MCP HTTP+SSE (`2024-11-05`) not shipped — use Streamable HTTP.
- Localhost-only: `TcpListener::bind("127.0.0.1:0")` (ephemeral for tests), remote not reachable; future LAN exposure requires explicit opt-in.
- Deferred: WebSocket/SSE push if live notifications needed later.
