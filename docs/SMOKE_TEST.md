# Server Smoke Test

Use this checklist after deployment or after changing routing/provider config.

## 1. Start server

```bash
cp config.example.yaml config.yaml
cargo run -- --config config.yaml
```

## 2. Health check

```bash
curl http://127.0.0.1:8080/healthz
```

Expected: JSON with `ok: true`.

## 3. Models

```bash
curl http://127.0.0.1:8080/v1/models
```

Expected: aliases and route keys are returned as model objects.

## 4. Diagnostics

```bash
curl http://127.0.0.1:8080/admin/diagnostics \
  -H "Authorization: Bearer <admin-token>"
```

Expected: `ok: true` or actionable diagnostic items.

## 5. Route preview

```bash
curl "http://127.0.0.1:8080/admin/routing/preview?model=fast" \
  -H "Authorization: Bearer <admin-token>"
```

Expected: provider order for the requested model or alias.

## 6. Chat completion

```bash
curl http://127.0.0.1:8080/v1/chat/completions \
  -H "Authorization: Bearer <client-token>" \
  -H "Content-Type: application/json" \
  -d '{"model":"fast","messages":[{"role":"user","content":"hello"}]}'
```

Expected: upstream response plus `x-lite-api-request-id` and `x-lite-api-provider` headers.

## 7. Logs and stats

```bash
curl http://127.0.0.1:8080/admin/logs \
  -H "Authorization: Bearer <admin-token>"

curl http://127.0.0.1:8080/admin/stats/today \
  -H "Authorization: Bearer <admin-token>"
```

Expected: request log entries and daily usage counters.

## Notes

- Keep local UI closed during one request to confirm server-side logging works independently.
- Use `/admin/diagnostics` after every manual config edit or config import.
- Use route preview before sending real traffic after changing route strategy.
