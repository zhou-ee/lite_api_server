# lite_api_server

Lightweight Rust data-plane server for a self-hosted LLM API gateway.

This service is intended to run on a small VPS, such as 1H1G, while the local UI/control plane lives in a separate repository: `zhou-ee/lite_api_local`.

## Implemented MVP

- OpenAI-compatible `POST /v1/chat/completions`
- OpenAI-style `GET /v1/models`
- provider pool config
- model alias mapping
- `priority_fallback` routing
- client API-key auth
- SQLite request/token telemetry
- Admin API for config, providers, routes, logs and daily stats

## Runtime split

```text
Local control panel: zhou-ee/lite_api_local
        ↓ admin API
VPS data plane: zhou-ee/lite_api_server
        ↓ provider routing
OpenAI / Claude / Gemini / OpenCode / New API / custom endpoints
```

## Quick start

```bash
cp config.example.yaml config.yaml
# edit provider api_key and admin_token
cargo run -- --config config.yaml
```

Health check:

```bash
curl http://127.0.0.1:8080/healthz
```

List models:

```bash
curl http://127.0.0.1:8080/v1/models
```

Chat completion:

```bash
curl http://127.0.0.1:8080/v1/chat/completions \
  -H "Authorization: Bearer local-test-key" \
  -H "Content-Type: application/json" \
  -d '{"model":"fast","messages":[{"role":"user","content":"hello"}]}'
```

Admin API:

```bash
curl http://127.0.0.1:8080/admin/stats/today \
  -H "Authorization: Bearer change-me-admin-token"
```

## Config

See `config.example.yaml`.

Important fields:

- `server.admin_token`: protects Admin API
- `clients.default.api_key`: client API key used by Claude Code/Codex/OpenCode/etc.
- `providers`: upstream provider pool
- `aliases`: user-facing model aliases
- `routes`: upstream routing rules

## Current limitations

- upstream support is currently OpenAI-compatible only
- streaming is proxied as a full buffered response for now
- cost calculation is not implemented yet
- secrets are stored in config file for MVP; production should add encrypted secret storage
