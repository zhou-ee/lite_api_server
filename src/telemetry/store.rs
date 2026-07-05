use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::{sqlite::SqlitePoolOptions, Row, SqlitePool};

#[derive(Clone)]
pub struct TelemetryStore {
    pool: SqlitePool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestLog {
    pub id: String,
    pub ts: i64,
    pub client_name: Option<String>,
    pub provider_id: String,
    pub requested_model: String,
    pub upstream_model: String,
    pub status_code: i64,
    pub error_type: Option<String>,
    pub latency_ms: i64,
    pub input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    pub total_tokens: Option<i64>,
    pub estimated_cost_usd: Option<f64>,
}

impl TelemetryStore {
    pub async fn connect(path: &str) -> anyhow::Result<Self> {
        let url = format!("sqlite://{}?mode=rwc", path);
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect(&url)
            .await?;
        Ok(Self { pool })
    }

    pub async fn migrate(&self) -> anyhow::Result<()> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS request_logs (
              id TEXT PRIMARY KEY,
              ts INTEGER NOT NULL,
              client_name TEXT,
              provider_id TEXT NOT NULL,
              requested_model TEXT NOT NULL,
              upstream_model TEXT NOT NULL,
              status_code INTEGER NOT NULL,
              error_type TEXT,
              latency_ms INTEGER NOT NULL,
              input_tokens INTEGER,
              output_tokens INTEGER,
              total_tokens INTEGER,
              estimated_cost_usd REAL
            );
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_request_logs_ts ON request_logs(ts);")
            .execute(&self.pool)
            .await?;
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_request_logs_provider ON request_logs(provider_id);")
            .execute(&self.pool)
            .await?;
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_request_logs_model ON request_logs(requested_model, upstream_model);")
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    pub async fn record_request(&self, log: RequestLog) -> anyhow::Result<()> {
        sqlx::query(
            r#"
            INSERT INTO request_logs (
              id, ts, client_name, provider_id, requested_model, upstream_model,
              status_code, error_type, latency_ms, input_tokens, output_tokens,
              total_tokens, estimated_cost_usd
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(log.id)
        .bind(log.ts)
        .bind(log.client_name)
        .bind(log.provider_id)
        .bind(log.requested_model)
        .bind(log.upstream_model)
        .bind(log.status_code)
        .bind(log.error_type)
        .bind(log.latency_ms)
        .bind(log.input_tokens)
        .bind(log.output_tokens)
        .bind(log.total_tokens)
        .bind(log.estimated_cost_usd)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn recent_logs(&self, limit: i64) -> anyhow::Result<Vec<RequestLog>> {
        let rows = sqlx::query(
            r#"
            SELECT id, ts, client_name, provider_id, requested_model, upstream_model,
                   status_code, error_type, latency_ms, input_tokens, output_tokens,
                   total_tokens, estimated_cost_usd
            FROM request_logs
            ORDER BY ts DESC
            LIMIT ?
            "#,
        )
        .bind(limit.clamp(1, 1000))
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| RequestLog {
                id: r.get("id"),
                ts: r.get("ts"),
                client_name: r.get("client_name"),
                provider_id: r.get("provider_id"),
                requested_model: r.get("requested_model"),
                upstream_model: r.get("upstream_model"),
                status_code: r.get("status_code"),
                error_type: r.get("error_type"),
                latency_ms: r.get("latency_ms"),
                input_tokens: r.get("input_tokens"),
                output_tokens: r.get("output_tokens"),
                total_tokens: r.get("total_tokens"),
                estimated_cost_usd: r.get("estimated_cost_usd"),
            })
            .collect())
    }

    pub async fn today_stats(&self) -> anyhow::Result<Value> {
        let start_of_day = start_of_today_utc();

        let row = sqlx::query(
            r#"
            SELECT
              COUNT(*) as request_count,
              COALESCE(SUM(total_tokens), 0) as total_tokens,
              COALESCE(SUM(input_tokens), 0) as input_tokens,
              COALESCE(SUM(output_tokens), 0) as output_tokens,
              COALESCE(AVG(latency_ms), 0) as avg_latency_ms,
              COALESCE(SUM(CASE WHEN status_code >= 200 AND status_code < 300 THEN 1 ELSE 0 END), 0) as success_count,
              COALESCE(SUM(CASE WHEN status_code < 200 OR status_code >= 300 THEN 1 ELSE 0 END), 0) as error_count
            FROM request_logs
            WHERE ts >= ?
            "#,
        )
        .bind(start_of_day)
        .fetch_one(&self.pool)
        .await?;

        Ok(json!({
            "request_count": row.get::<i64, _>("request_count"),
            "success_count": row.get::<i64, _>("success_count"),
            "error_count": row.get::<i64, _>("error_count"),
            "total_tokens": row.get::<i64, _>("total_tokens"),
            "input_tokens": row.get::<i64, _>("input_tokens"),
            "output_tokens": row.get::<i64, _>("output_tokens"),
            "avg_latency_ms": row.get::<f64, _>("avg_latency_ms")
        }))
    }

    pub async fn provider_stats(&self) -> anyhow::Result<Value> {
        let start_of_day = start_of_today_utc();
        let rows = sqlx::query(
            r#"
            SELECT
              provider_id,
              COUNT(*) as request_count,
              COALESCE(SUM(total_tokens), 0) as total_tokens,
              COALESCE(AVG(latency_ms), 0) as avg_latency_ms,
              COALESCE(SUM(CASE WHEN status_code >= 200 AND status_code < 300 THEN 1 ELSE 0 END), 0) as success_count,
              COALESCE(SUM(CASE WHEN status_code < 200 OR status_code >= 300 THEN 1 ELSE 0 END), 0) as error_count
            FROM request_logs
            WHERE ts >= ?
            GROUP BY provider_id
            ORDER BY request_count DESC
            "#,
        )
        .bind(start_of_day)
        .fetch_all(&self.pool)
        .await?;

        Ok(json!({
            "data": rows.into_iter().map(|r| json!({
                "provider_id": r.get::<String, _>("provider_id"),
                "request_count": r.get::<i64, _>("request_count"),
                "success_count": r.get::<i64, _>("success_count"),
                "error_count": r.get::<i64, _>("error_count"),
                "total_tokens": r.get::<i64, _>("total_tokens"),
                "avg_latency_ms": r.get::<f64, _>("avg_latency_ms")
            })).collect::<Vec<_>>()
        }))
    }

    pub async fn model_stats(&self) -> anyhow::Result<Value> {
        let start_of_day = start_of_today_utc();
        let rows = sqlx::query(
            r#"
            SELECT
              requested_model,
              upstream_model,
              COUNT(*) as request_count,
              COALESCE(SUM(total_tokens), 0) as total_tokens,
              COALESCE(AVG(latency_ms), 0) as avg_latency_ms,
              COALESCE(SUM(CASE WHEN status_code >= 200 AND status_code < 300 THEN 1 ELSE 0 END), 0) as success_count,
              COALESCE(SUM(CASE WHEN status_code < 200 OR status_code >= 300 THEN 1 ELSE 0 END), 0) as error_count
            FROM request_logs
            WHERE ts >= ?
            GROUP BY requested_model, upstream_model
            ORDER BY request_count DESC
            "#,
        )
        .bind(start_of_day)
        .fetch_all(&self.pool)
        .await?;

        Ok(json!({
            "data": rows.into_iter().map(|r| json!({
                "requested_model": r.get::<String, _>("requested_model"),
                "upstream_model": r.get::<String, _>("upstream_model"),
                "request_count": r.get::<i64, _>("request_count"),
                "success_count": r.get::<i64, _>("success_count"),
                "error_count": r.get::<i64, _>("error_count"),
                "total_tokens": r.get::<i64, _>("total_tokens"),
                "avg_latency_ms": r.get::<f64, _>("avg_latency_ms")
            })).collect::<Vec<_>>()
        }))
    }
}

fn start_of_today_utc() -> i64 {
    chrono::Utc::now()
        .date_naive()
        .and_hms_opt(0, 0, 0)
        .unwrap()
        .and_utc()
        .timestamp()
}
