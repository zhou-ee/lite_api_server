# Reference Project Feature Map

This project intentionally borrows concepts from larger gateway/control-plane projects, but keeps the runtime small enough for a 1H1G VPS.

## LiteLLM comparison

LiteLLM positions itself as an AI Gateway that provides a unified OpenAI-format interface for many providers, plus virtual keys, spend tracking, load balancing and dashboard features.

Mapped into `lite_api_server`:

- OpenAI-compatible `/v1/chat/completions`
- config-driven providers
- model aliases
- route strategies: `priority_fallback`, `weighted`, `cheapest` base ordering
- client API keys
- token and estimated cost telemetry
- provider/model aggregate stats

Not yet implemented:

- full provider adapter matrix
- true per-key budget limits
- advanced guardrails
- team/user management

## New API / One API comparison

New API and One API are useful references for channel/provider management, quota accounting, logs and private deployment.

Mapped into `lite_api_server`:

- provider pool CRUD through Admin API
- request logs in SQLite
- daily stats
- systemd deployment example

Not yet implemented:

- multi-user quota system
- payment/billing layer
- web management panel on the server

## CC Switch comparison

CC Switch is a local all-in-one manager for coding agents and CLI clients.

Mapped into the split system:

- `lite_api_local` is the local control plane
- importers live locally rather than on the VPS
- the server only receives normalized providers/routes/aliases

## Current design rule

Server stores runtime config and telemetry. Local UI can be closed without affecting API serving or logging.
