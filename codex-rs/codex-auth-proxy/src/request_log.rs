use std::num::NonZeroU64;
use std::path::Path;
use std::time::Duration;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use anyhow::Context;
use anyhow::Result;
use serde::Serialize;
use serde_json::Value;
use sqlx::Row;
use sqlx::SqlitePool;
use sqlx::sqlite::SqliteConnectOptions;
use sqlx::sqlite::SqliteJournalMode;
use sqlx::sqlite::SqlitePoolOptions;
use sqlx::sqlite::SqliteSynchronous;
use tiny_http::Method;
use tiny_http::Request;
use tokio::runtime::Runtime;

use crate::token_usage::TokenUsage;
use crate::token_usage::token_usage_from_response_body;

#[derive(Debug, Clone)]
pub(crate) struct RequestLogger {
    pool: SqlitePool,
    retention: Option<RequestLogRetention>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RequestLogRetention {
    retain_rows: NonZeroU64,
}

impl RequestLogRetention {
    pub(crate) fn new(retain_rows: NonZeroU64) -> Self {
        Self { retain_rows }
    }
}

pub(crate) struct RequestLogStart<'a> {
    pub(crate) id: &'a str,
    pub(crate) started_at: &'a str,
    pub(crate) client_ip: Option<&'a str>,
    pub(crate) method: &'a str,
    pub(crate) path: &'a str,
    pub(crate) query: Option<&'a str>,
    pub(crate) model: Option<&'a str>,
    pub(crate) request_body: &'a [u8],
}

pub(crate) struct RequestLogCompletion<'a> {
    pub(crate) completed_at: &'a str,
    pub(crate) upstream_status: Option<u16>,
    pub(crate) latency_ms: u128,
    pub(crate) response_body: &'a [u8],
    pub(crate) error: Option<&'a str>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, sqlx::FromRow)]
pub(crate) struct RequestLogSummary {
    pub(crate) id: String,
    pub(crate) started_at: String,
    pub(crate) completed_at: Option<String>,
    pub(crate) client_ip: Option<String>,
    pub(crate) method: String,
    pub(crate) path: String,
    pub(crate) query: Option<String>,
    pub(crate) model: Option<String>,
    pub(crate) upstream_status: Option<i64>,
    pub(crate) latency_ms: Option<i64>,
    pub(crate) request_bytes: Option<i64>,
    pub(crate) response_bytes: Option<i64>,
    pub(crate) input_tokens: Option<i64>,
    pub(crate) output_tokens: Option<i64>,
    pub(crate) total_tokens: Option<i64>,
    pub(crate) cached_input_tokens: Option<i64>,
    pub(crate) reasoning_output_tokens: Option<i64>,
    pub(crate) error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, sqlx::FromRow)]
pub(crate) struct RequestLogDetail {
    pub(crate) id: String,
    pub(crate) started_at: String,
    pub(crate) completed_at: Option<String>,
    pub(crate) client_ip: Option<String>,
    pub(crate) method: String,
    pub(crate) path: String,
    pub(crate) query: Option<String>,
    pub(crate) model: Option<String>,
    pub(crate) upstream_status: Option<i64>,
    pub(crate) latency_ms: Option<i64>,
    pub(crate) request_bytes: Option<i64>,
    pub(crate) response_bytes: Option<i64>,
    pub(crate) input_tokens: Option<i64>,
    pub(crate) output_tokens: Option<i64>,
    pub(crate) total_tokens: Option<i64>,
    pub(crate) cached_input_tokens: Option<i64>,
    pub(crate) reasoning_output_tokens: Option<i64>,
    pub(crate) request_body: Option<String>,
    pub(crate) response_body: Option<String>,
    pub(crate) error: Option<String>,
}

impl RequestLogDetail {
    fn has_no_token_usage(&self) -> bool {
        self.input_tokens.is_none()
            && self.output_tokens.is_none()
            && self.total_tokens.is_none()
            && self.cached_input_tokens.is_none()
            && self.reasoning_output_tokens.is_none()
    }

    fn apply_token_usage(&mut self, usage: TokenUsage) {
        self.input_tokens = usage.input_tokens;
        self.output_tokens = usage.output_tokens;
        self.total_tokens = usage.total_tokens;
        self.cached_input_tokens = usage.cached_input_tokens;
        self.reasoning_output_tokens = usage.reasoning_output_tokens;
    }
}

impl RequestLogger {
    pub(crate) async fn open(path: &Path) -> Result<Self> {
        Self::open_internal(path, /*retention*/ None).await
    }

    pub(crate) async fn open_with_retention(
        path: &Path,
        retention: RequestLogRetention,
    ) -> Result<Self> {
        Self::open_internal(path, Some(retention)).await
    }

    async fn open_internal(path: &Path, retention: Option<RequestLogRetention>) -> Result<Self> {
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating log DB parent {}", parent.display()))?;
        }

        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .synchronous(SqliteSynchronous::Normal)
            .busy_timeout(Duration::from_secs(5));
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(options)
            .await
            .with_context(|| format!("opening log DB {}", path.display()))?;
        create_schema(&pool).await?;
        let logger = Self { pool, retention };
        logger.prune_to_retention().await?;
        Ok(logger)
    }

    pub(crate) async fn insert_start(&self, request: RequestLogStart<'_>) -> Result<()> {
        sqlx::query(
            r#"
INSERT INTO proxy_requests (
  id,
  started_at,
  client_ip,
  method,
  path,
  query,
  model,
  request_bytes,
  request_body
)
VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
"#,
        )
        .bind(request.id)
        .bind(request.started_at)
        .bind(request.client_ip)
        .bind(request.method)
        .bind(request.path)
        .bind(request.query)
        .bind(request.model)
        .bind(request.request_body.len() as i64)
        .bind(String::from_utf8_lossy(request.request_body).into_owned())
        .execute(&self.pool)
        .await
        .context("inserting proxy request log row")?;
        Ok(())
    }

    pub(crate) async fn complete(
        &self,
        id: &str,
        completion: RequestLogCompletion<'_>,
    ) -> Result<()> {
        let usage = token_usage_from_response_body(completion.response_body);
        sqlx::query(
            r#"
UPDATE proxy_requests
SET
  completed_at = ?,
  upstream_status = ?,
  latency_ms = ?,
  response_bytes = ?,
  response_body = ?,
  input_tokens = ?,
  output_tokens = ?,
  total_tokens = ?,
  cached_input_tokens = ?,
  reasoning_output_tokens = ?,
  error = ?
WHERE id = ?
"#,
        )
        .bind(completion.completed_at)
        .bind(completion.upstream_status.map(i64::from))
        .bind(completion.latency_ms as i64)
        .bind(completion.response_body.len() as i64)
        .bind(String::from_utf8_lossy(completion.response_body).into_owned())
        .bind(usage.input_tokens)
        .bind(usage.output_tokens)
        .bind(usage.total_tokens)
        .bind(usage.cached_input_tokens)
        .bind(usage.reasoning_output_tokens)
        .bind(completion.error)
        .bind(id)
        .execute(&self.pool)
        .await
        .context("updating proxy request log row")?;
        self.prune_to_retention().await?;
        Ok(())
    }

    pub(crate) async fn list_recent(&self, limit: i64) -> Result<Vec<RequestLogSummary>> {
        let limit = limit.clamp(1, 500);
        sqlx::query_as(
            r#"
SELECT
  id,
  started_at,
  completed_at,
  client_ip,
  method,
  path,
  query,
  model,
  upstream_status,
  latency_ms,
  request_bytes,
  response_bytes,
  input_tokens,
  output_tokens,
  total_tokens,
  cached_input_tokens,
  reasoning_output_tokens,
  error
FROM proxy_requests
ORDER BY started_at DESC
LIMIT ?
"#,
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .context("listing proxy request log rows")
    }

    pub(crate) async fn get_detail(&self, id: &str) -> Result<Option<RequestLogDetail>> {
        let Some(mut detail) = sqlx::query_as::<_, RequestLogDetail>(
            r#"
SELECT
  id,
  started_at,
  completed_at,
  client_ip,
  method,
  path,
  query,
  model,
  upstream_status,
  latency_ms,
  request_bytes,
  response_bytes,
  input_tokens,
  output_tokens,
  total_tokens,
  cached_input_tokens,
  reasoning_output_tokens,
  request_body,
  response_body,
  error
FROM proxy_requests
WHERE id = ?
"#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .context("reading proxy request log row")?
        else {
            return Ok(None);
        };

        if detail.has_no_token_usage()
            && let Some(response_body) = detail.response_body.as_deref()
        {
            let usage = token_usage_from_response_body(response_body.as_bytes());
            if !usage.is_empty() {
                self.update_token_usage(id, usage).await?;
                detail.apply_token_usage(usage);
            }
        }

        Ok(Some(detail))
    }

    #[cfg(test)]
    pub(crate) fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    async fn update_token_usage(&self, id: &str, usage: TokenUsage) -> Result<()> {
        sqlx::query(
            r#"
UPDATE proxy_requests
SET
  input_tokens = ?,
  output_tokens = ?,
  total_tokens = ?,
  cached_input_tokens = ?,
  reasoning_output_tokens = ?
WHERE id = ?
"#,
        )
        .bind(usage.input_tokens)
        .bind(usage.output_tokens)
        .bind(usage.total_tokens)
        .bind(usage.cached_input_tokens)
        .bind(usage.reasoning_output_tokens)
        .bind(id)
        .execute(&self.pool)
        .await
        .context("updating proxy request token usage")?;
        Ok(())
    }

    async fn prune_to_retention(&self) -> Result<()> {
        let Some(retention) = self.retention else {
            return Ok(());
        };
        let retain_rows = i64::try_from(retention.retain_rows.get()).unwrap_or(i64::MAX);
        sqlx::query(
            r#"
DELETE FROM proxy_requests
WHERE id IN (
  SELECT id
  FROM (
    SELECT id
    FROM proxy_requests
    WHERE completed_at IS NOT NULL
    ORDER BY started_at DESC, id DESC
    LIMIT -1 OFFSET ?
  )
)
"#,
        )
        .bind(retain_rows)
        .execute(&self.pool)
        .await
        .context("pruning proxy request log rows")?;
        Ok(())
    }
}

async fn create_schema(pool: &SqlitePool) -> Result<()> {
    sqlx::query(
        r#"
CREATE TABLE IF NOT EXISTS proxy_requests (
  id TEXT PRIMARY KEY,
  started_at TEXT NOT NULL,
  completed_at TEXT,
  client_ip TEXT,
  method TEXT NOT NULL,
  path TEXT NOT NULL,
  query TEXT,
  model TEXT,
  upstream_status INTEGER,
  latency_ms INTEGER,
  request_bytes INTEGER,
  response_bytes INTEGER,
  request_body TEXT,
  response_body TEXT,
  error TEXT
)
"#,
    )
    .execute(pool)
    .await
    .context("creating proxy request log schema")?;
    ensure_column(pool, "input_tokens", "INTEGER").await?;
    ensure_column(pool, "output_tokens", "INTEGER").await?;
    ensure_column(pool, "total_tokens", "INTEGER").await?;
    ensure_column(pool, "cached_input_tokens", "INTEGER").await?;
    ensure_column(pool, "reasoning_output_tokens", "INTEGER").await?;
    Ok(())
}

async fn ensure_column(pool: &SqlitePool, name: &str, definition: &str) -> Result<()> {
    let columns = sqlx::query("PRAGMA table_info(proxy_requests)")
        .fetch_all(pool)
        .await
        .context("reading proxy request log schema columns")?;
    let exists = columns
        .iter()
        .any(|row| row.get::<String, _>("name") == name);
    if exists {
        return Ok(());
    }

    let sql = format!("ALTER TABLE proxy_requests ADD COLUMN {name} {definition}");
    sqlx::query(sqlx::AssertSqlSafe(sql))
        .execute(pool)
        .await
        .with_context(|| format!("adding proxy request log column {name}"))?;
    Ok(())
}

pub(crate) fn timestamp_now() -> String {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    format!("{}.{:03}", duration.as_secs(), duration.subsec_millis())
}

pub(crate) fn model_from_body(body: &[u8]) -> Option<String> {
    let value = serde_json::from_slice::<Value>(body).ok()?;
    value
        .get("model")
        .and_then(Value::as_str)
        .map(ToString::to_string)
}

pub(crate) fn log_request_start(
    runtime: &Runtime,
    logger: Option<&RequestLogger>,
    request: RequestLogStart<'_>,
) -> bool {
    let Some(logger) = logger else {
        return false;
    };
    if let Err(err) = runtime.block_on(logger.insert_start(request)) {
        eprintln!("request log insert failed: {err:#}");
        return false;
    }
    true
}

pub(crate) fn log_request_complete(
    runtime: &Runtime,
    logger: Option<&RequestLogger>,
    row_started: bool,
    request_id: &str,
    completion: RequestLogCompletion<'_>,
) {
    if !row_started {
        return;
    }
    let Some(logger) = logger else {
        return;
    };
    if let Err(err) = runtime.block_on(logger.complete(request_id, completion)) {
        eprintln!("request log update failed id={request_id}: {err:#}");
    }
}

pub(crate) fn log_access_received(
    request_id: &str,
    client_ip: Option<&str>,
    method: &Method,
    path: &str,
    query: Option<&str>,
    request_bytes: Option<u64>,
) {
    eprintln!(
        "request received id={} client_ip={} method={} path={} query={} request_bytes={}",
        request_id,
        client_ip.unwrap_or("-"),
        method,
        path,
        query.unwrap_or("-"),
        request_bytes
            .map(|bytes| bytes.to_string())
            .unwrap_or_else(|| "-".to_string())
    );
}

pub(crate) fn log_access_complete(
    request_id: &str,
    status: Option<u16>,
    response_bytes: usize,
    latency: Duration,
    error: Option<&str>,
) {
    eprintln!(
        "request completed id={} status={} response_bytes={} latency_ms={} error={}",
        request_id,
        status
            .map(|status| status.to_string())
            .unwrap_or_else(|| "-".to_string()),
        response_bytes,
        latency.as_millis(),
        error
            .map(sanitize_log_value)
            .unwrap_or_else(|| "-".to_string())
    );
}

pub(crate) fn request_content_length(req: &Request) -> Option<u64> {
    req.headers().iter().find_map(|header| {
        if header.field.equiv("Content-Length") {
            header.value.as_str().parse().ok()
        } else {
            None
        }
    })
}

fn sanitize_log_value(value: &str) -> String {
    value.replace(['\r', '\n'], " ")
}

#[cfg(test)]
#[path = "request_log_tests.rs"]
mod tests;
