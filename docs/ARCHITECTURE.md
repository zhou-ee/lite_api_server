# lite_api_server Architecture

This repository is the always-on data plane.

## Responsibilities

- expose OpenAI-compatible endpoints
- authenticate local clients
- resolve model aliases
- select provider routes
- forward requests to upstream providers
- record request/token telemetry into SQLite
- expose admin APIs for `lite_api_local`

## Runtime flow

```text
Client tools
  Claude Code / Codex / OpenCode / Antigravity / Gemini CLI
        ↓
/v1/chat/completions
        ↓
Auth → Alias → RoutePlan → ProviderPool → Upstream
        ↓
SQLite request_logs
```

## Why server stores logs

The local control panel can be closed at any time. Therefore request logging and token statistics must happen inside this server process, not in the local UI.

## MVP constraints

- only OpenAI-compatible upstreams are implemented first
- non-streaming proxy is implemented first
- fallback retries only happen for request errors, 429 and 5xx
- full prompt/response body logging is disabled by default
