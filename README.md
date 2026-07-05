# lite_api_server

Lightweight Rust data-plane server for a self-hosted LLM API gateway.

This service is designed to run on a small VPS while the local UI/control plane lives in a separate repository: `zhou-ee/lite_api_local`.

## Current progress

Implemented server-side capabilities:

- OpenAI-compatible `POST /v1/chat/completions`
- OpenAI-style `GET /v1/models`
- streaming passthrough for OpenAI-compatible responses
- provider pool config
- model alias mapping
- route strategies:
  - `priority_fallback`
  - `weighted`
  - `round_robin`
  - `weighted_random`
  - `lowest_latency`
  - `cheapest`
- provider-level model pricing with global model pricing fallback
- client authentication and optional daily usage caps
- SQLite request/token/cost telemetry
- provider/model/client usage aggregation
- config diagnostics
- route preview
- Admin API for config, providers, routes, aliases, logs and stats

## Runtime split

```text
Local control panel: zhou-ee/lite_api_local
        ↓ admin API
VPS data plane: zhou-ee/lite_api_server
        ↓ provider routing
OpenAI-compatible upstream providers
```

The local UI can be closed after configuration. Proxying, routing and logging happen in this server process.

## Quick start

```bash
cp config.example.yaml config.yaml
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

## Smoke test flow

After starting the server:

1. Run health check.
2. Run `/v1/models` and confirm aliases/routes appear.
3. Run Admin diagnostics and fix reported errors.
4. Run route preview for an alias such as `fast`.
5. Send one chat completion request.
6. Check logs and daily stats.
7. Close the local UI and send another request to confirm server-side logging still works.

See `docs/SMOKE_TEST.md` and `docs/ROUTING.md` for more detail.

## Routing notes

- `priority_fallback` keeps deterministic priority order.
- `weighted` sorts by configured provider weight.
- `round_robin` rotates provider order using an in-process cursor.
- `weighted_random` samples a provider order from provider weights per request.
- `lowest_latency` uses today's successful request latency from SQLite telemetry.
- `cheapest` prefers provider-level model pricing, then global model pricing.

## Pricing notes

Pricing can be declared globally by model, and can also be declared on a provider for a specific model. Provider-level pricing wins over global pricing. This allows the same model to be routed differently when providers have different cost structures.

## Development checklist

Before considering a change usable:

```bash
cargo check
cargo test
```

Manual checks:

- diagnostics has no errors
- route preview matches expected provider order
- healthcheck works for at least one provider
- a non-streaming request records token/cost usage when upstream usage fields are present
- a streaming request returns chunks and records request metadata

## Current limitations

- upstream adapters beyond OpenAI-compatible are not complete yet
- streaming token accounting is not complete yet
- round-robin cursor is in-memory and resets on process restart
- weighted-random uses in-process request-derived randomness, not persistent distribution accounting
- secrets are stored in config for the MVP; production should add safer secret storage
