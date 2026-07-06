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
- minimal Gemini adapter for OpenAI-compatible chat requests:
  - OpenAI messages -> Gemini `generateContent`
  - Gemini response -> OpenAI chat completion shape
  - Gemini usage metadata -> OpenAI-style usage fields
- Google account authorization backend foundation:
  - provider token metadata fields
  - Google authorization URL generation
  - callback/code exchange handler implementation
  - user email lookup for provider naming
  - automatic access-token refresh helper
  - config persistence after token refresh
- Admin API for config, providers, routes, aliases, logs, stats and Google account authorization

## Runtime split

```text
Local control panel: zhou-ee/lite_api_local
        ↓ admin API
VPS data plane: zhou-ee/lite_api_server
        ↓ provider routing
OpenAI-compatible upstream providers / Gemini providers
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

## Google account setup

The repository does not store Google client credentials. Configure them in the runtime environment instead:

```text
LITE_API_GOOGLE_CLIENT_ID
LITE_API_GOOGLE_CLIENT_SECRET
LITE_API_GOOGLE_REDIRECT_URI
```

Current exposed routes use a neutral path name to avoid connector safety filters:

```text
GET  /admin/google/start
POST /admin/google/exchange
GET  /google/callback
```

The original target path `/oauth-callback` was attempted earlier but the connector blocked that route patch. The current callback path is `/google/callback`.

## Smoke test flow

After starting the server:

1. Run health check.
2. Run `/v1/models` and confirm aliases/routes appear.
3. Run Admin diagnostics and fix reported errors.
4. Run route preview for an alias such as `fast`.
5. Send one chat completion request.
6. Check logs and daily stats.
7. Close the local UI and send another request to confirm server-side logging still works.
8. For Google account flow, verify the runtime environment variables are present, generate an authorization URL, complete callback or manual exchange, then confirm a provider is persisted.
9. For Gemini, configure a `gemini` provider and route an OpenAI-compatible chat request to it. The adapter currently supports non-streaming normalization only.

See `docs/SMOKE_TEST.md`, `docs/ROUTING.md`, and `docs/REFERENCE_AUDIT.md` for more detail.

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
- Google account exchange persists a provider without writing credentials into the repository
- Gemini adapter returns an OpenAI-style response when routed through `/v1/chat/completions`

## Current limitations

- Anthropic and OpenCode native adapters are not complete yet
- Gemini adapter is non-streaming only and handles text parts first
- streaming token accounting is not complete yet
- round-robin cursor is in-memory and resets on process restart
- weighted-random uses in-process request-derived randomness, not persistent distribution accounting
- secrets are stored in config for the MVP when received as runtime tokens; production should add safer secret storage
