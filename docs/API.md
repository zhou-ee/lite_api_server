# API Draft

## Public API

### `GET /healthz`

Returns process health.

### `GET /v1/models`

Returns configured model aliases and route keys as OpenAI-style model objects.

### `POST /v1/chat/completions`

OpenAI-compatible chat completion endpoint.

Headers:

```http
Authorization: Bearer <client_api_key>
```

Body example:

```json
{
  "model": "fast",
  "messages": [
    {"role": "user", "content": "hello"}
  ]
}
```

## Admin API

Admin endpoints require:

```http
Authorization: Bearer <admin_token>
```

### `GET /admin/config`

Returns full runtime config.

### `PUT /admin/config`

Replaces runtime config and saves it to disk.

### `GET /admin/providers`

Lists provider pool.

### `POST /admin/providers`

Upserts a provider.

### `PATCH /admin/providers/:id`

Upserts provider by path id.

### `DELETE /admin/providers/:id`

Deletes a provider.

### `GET /admin/routes`

Lists model routing rules.

### `PUT /admin/routes`

Replaces routing rules.

### `GET /admin/logs?limit=100`

Returns recent request logs.

### `GET /admin/stats/today`

Returns daily aggregate telemetry.
