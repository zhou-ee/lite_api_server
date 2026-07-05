# Routing

`lite_api_server` supports config-driven model routing. A client asks for a model name, the gateway resolves aliases, then chooses an ordered provider list.

## Flow

```text
requested model
  -> alias resolution
  -> upstream model
  -> route lookup
  -> provider ordering
  -> fallback loop
```

## Strategies

### `priority_fallback`

Sorts enabled providers by ascending `priority`. The first provider is tried first. Retryable upstream errors can fall back to the next provider.

### `weighted`

Sorts enabled providers by descending `weight`. This is a lightweight deterministic strategy for now. Future work can turn it into probabilistic weighted balancing.

### `lowest_latency`

Sorts enabled providers by today's average successful request latency from SQLite telemetry. Providers with no latency samples are placed after providers with known latency. Priority breaks ties.

### `cheapest`

Uses model-level pricing for cost-aware routing. Provider-level pricing is not implemented yet, so priority currently breaks ties when providers share the same model price.

## Route preview

Use the Admin API route preview endpoint to verify the provider order without making a real model request.

```text
GET /admin/routing/preview?model=<model-or-alias>
```

The response includes:

- requested model
- resolved upstream model
- provider order
- latency snapshot used by the planner

## Diagnostics

Use `/admin/diagnostics` after editing routes, aliases, providers or pricing. It reports missing providers, invalid routes, unknown models and pricing gaps.
