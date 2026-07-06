# Reference Project Audit

This audit compares `lite_api_server` against the planned reference projects and the split architecture target.

## Source plan

The planned architecture is server Data Plane plus local Control Plane:

- server keeps the lightweight Rust gateway, routing and telemetry
- local UI handles config editing, import/export, log viewing and analysis
- do not embed LiteLLM, New API, One API or CC Switch directly
- extract only their useful design patterns

## Reference capability map

| Reference | Capability to learn from | Current status |
|---|---|---|
| LiteLLM | OpenAI-compatible gateway, provider abstraction, fallback, load balancing, cost tracking | Partially implemented: OpenAI-compatible chat, provider pool, fallback, route strategies, provider-level pricing, token/cost telemetry |
| New API | channel management, model aggregation, usage analytics, cost accounting | Partially implemented: providers, model routes, stats, cost estimates; missing users/groups/billing-grade management |
| One API | lightweight API distribution, key redistribution, simple deployment | Partially implemented: small Rust binary target, client auth, SQLite; missing mature key pools and quota dashboard |
| CC Switch | client config import/switching for Claude Code, Codex, OpenCode, Gemini CLI | Partially implemented in local repo via JSON importers; exact filesystem read/write requires a desktop shell |
| Claude Code Router | coding-agent route experience and local gateway control | Partially implemented: route preview, aliases, strategies; missing dedicated client config rewrite flow |
| Antigravity Manager | Tauri + React UI shell and account switching UX | Partially implemented in local repo styling; Tauri shell still blocked by connector safety checks |
| axum/tokio/reqwest | lightweight async Rust HTTP server and upstream client | Implemented |
| SQLx/tracing/metrics-rs | SQLite telemetry, structured logs, future metrics exporter | SQLx and tracing implemented; metrics exporter not yet implemented |

## Current server capabilities verified by code inspection

- `POST /v1/chat/completions` exists and routes through the model router.
- `GET /v1/models` returns aliases and route keys.
- Provider pool, aliases, routes and Admin API exist.
- Routing supports `priority_fallback`, `weighted`, `round_robin`, `weighted_random`, `lowest_latency`, and `cheapest`.
- Cheapest routing uses provider-level price when available, then global model price.
- SQLite telemetry records request metadata, usage, estimated cost and route strategy.
- OAuth-backed providers can store token metadata and refresh before upstream calls.
- Minimal Gemini adapter now supports non-streaming OpenAI-compatible chat request normalization.

## Important gaps

### Protocol adapters

Implemented:

- OpenAI-compatible chat completions
- minimal Gemini text chat normalization

Missing:

- `/v1/responses`
- Anthropic `/v1/messages`
- Gemini streaming
- OpenCode native adapter
- embeddings/images/audio endpoints

### Provider pool maturity

Implemented:

- add/update/list/delete providers
- health check for OpenAI-compatible providers
- provider-level pricing

Missing:

- health check for Gemini and other provider kinds
- key pool rotation
- automatic failure scoring or cool-down
- provider-level circuit breaker

### Usage and quota

Implemented:

- SQLite request logs
- provider/model stats
- per-client daily caps
- cost estimate from usage fields

Missing:

- persistent per-client monthly budgets
- grouped users / teams
- billing-grade ledger
- streaming token accounting

### Local control plane

Implemented in local repo:

- dashboard
- provider editor
- route editor
- alias editor
- diagnostics
- route preview
- import preview

Missing:

- Tauri desktop shell
- exact local config file read/write
- polished Google account UI in ProviderEditor
- route strategy display in request log table

## Functional risk notes

1. The newest Gemini adapter has not been build-tested in CI from this environment. Run `cargo check` immediately after pulling.
2. Gemini adapter currently supports text-only non-streaming output.
3. Google OAuth client credentials must be supplied at runtime and must not be committed.
4. The local Vite app cannot safely read arbitrary local config files; exact import/export needs Tauri.
5. Existing connector safety checks may block large OAuth/UI patches, so use smaller commits.

## Recommended next implementation order

1. Run build checks:

```bash
cargo check
cargo test
npm install
npm run build
```

2. Fix any compiler errors from Gemini adapter integration.
3. Add Gemini provider healthcheck.
4. Add route strategy column to local request logs.
5. Add ProviderEditor Google account UI locally or through very small commits.
6. Add Anthropic adapter.
7. Add Tauri shell and exact filesystem import/export.
8. Add lightweight `/metrics` endpoint only after the core gateway is stable.
