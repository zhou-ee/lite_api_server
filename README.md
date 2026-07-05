# lite_api_server

Lightweight Rust data-plane server for a self-hosted LLM API gateway.

This service is intended to run on a small VPS, such as 1H1G, while the local UI/control plane lives in a separate repository.

## Goals

- OpenAI-compatible `/v1/chat/completions` endpoint
- provider pool management
- model alias and routing rules
- priority fallback routing
- request/token telemetry in SQLite
- admin API for local control panel

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
cargo run -- --config config.yaml
```

Health check:

```bash
curl http://127.0.0.1:8080/healthz
```

Admin API requires:

```http
Authorization: Bearer change-me-admin-token
```

## Status

Initial framework only. Next steps:

- streaming support
- multi-provider fallback loop
- request auth/key pool
- cost calculator
- config hot-reload hardening
