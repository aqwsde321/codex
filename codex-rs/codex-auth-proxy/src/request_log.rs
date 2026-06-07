use std::cmp::Ordering;
use std::collections::BTreeSet;
use std::collections::VecDeque;
use std::num::NonZeroU64;
use std::path::Path;
use std::time::Duration;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use anyhow::Context;
use anyhow::Result;
use serde::Serialize;
use serde_json::Value;
use sqlx::QueryBuilder;
use sqlx::Row;
use sqlx::Sqlite;
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
    body_limit: Option<RequestLogBodyLimit>,
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct RequestLogOptions {
    pub(crate) retention: Option<RequestLogRetention>,
    pub(crate) body_limit: Option<RequestLogBodyLimit>,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RequestLogBodyLimit {
    max_bytes: NonZeroU64,
}

impl RequestLogBodyLimit {
    pub(crate) fn new(max_bytes: NonZeroU64) -> Self {
        Self { max_bytes }
    }
}

struct StoredBody {
    text: String,
    truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RequestLogListQuery {
    pub(crate) limit: i64,
    pub(crate) filter: RequestLogFilter,
    pub(crate) search: Option<String>,
}

impl Default for RequestLogListQuery {
    fn default() -> Self {
        Self {
            limit: 200,
            filter: RequestLogFilter::All,
            search: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RequestLogFilter {
    All,
    Errors,
    Slow,
    HighTokens,
    Truncated,
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
    pub(crate) request_body_truncated: bool,
    pub(crate) response_body_truncated: bool,
    pub(crate) input_tokens: Option<i64>,
    pub(crate) output_tokens: Option<i64>,
    pub(crate) total_tokens: Option<i64>,
    pub(crate) cached_input_tokens: Option<i64>,
    pub(crate) reasoning_output_tokens: Option<i64>,
    pub(crate) error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct RequestLogFlow {
    pub(crate) basis: RequestLogFlowBasis,
    pub(crate) rows: Vec<RequestLogSummary>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RequestLogFlowBasis {
    Unavailable,
    ToolCallChain,
    UserAsked,
    Nearby,
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
    pub(crate) request_body_truncated: bool,
    pub(crate) response_body_truncated: bool,
    pub(crate) input_tokens: Option<i64>,
    pub(crate) output_tokens: Option<i64>,
    pub(crate) total_tokens: Option<i64>,
    pub(crate) cached_input_tokens: Option<i64>,
    pub(crate) reasoning_output_tokens: Option<i64>,
    pub(crate) request_body: Option<String>,
    pub(crate) response_body: Option<String>,
    pub(crate) error: Option<String>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
struct RequestLogFlowSeed {
    started_at: String,
    client_ip: Option<String>,
    path: String,
    request_body: Option<String>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
struct RequestLogFlowCandidate {
    id: String,
    started_at: String,
    started_at_seconds: f64,
    completed_at: Option<String>,
    client_ip: Option<String>,
    method: String,
    path: String,
    query: Option<String>,
    model: Option<String>,
    upstream_status: Option<i64>,
    latency_ms: Option<i64>,
    request_bytes: Option<i64>,
    response_bytes: Option<i64>,
    request_body_truncated: bool,
    response_body_truncated: bool,
    input_tokens: Option<i64>,
    output_tokens: Option<i64>,
    total_tokens: Option<i64>,
    cached_input_tokens: Option<i64>,
    reasoning_output_tokens: Option<i64>,
    error: Option<String>,
    request_body: Option<String>,
    response_body: Option<String>,
    distance: f64,
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

const FLOW_WINDOW_SECONDS: f64 = 600.0;
const FLOW_CANDIDATE_LIMIT: i64 = 200;
const FLOW_ROW_LIMIT: i64 = 40;

impl RequestLogger {
    pub(crate) async fn open(path: &Path) -> Result<Self> {
        Self::open_with_options(path, RequestLogOptions::default()).await
    }

    pub(crate) async fn open_with_options(path: &Path, options: RequestLogOptions) -> Result<Self> {
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating log DB parent {}", parent.display()))?;
        }

        let connect_options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .synchronous(SqliteSynchronous::Normal)
            .busy_timeout(Duration::from_secs(5));
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(connect_options)
            .await
            .with_context(|| format!("opening log DB {}", path.display()))?;
        create_schema(&pool).await?;
        let logger = Self {
            pool,
            retention: options.retention,
            body_limit: options.body_limit,
        };
        logger.prune_to_retention().await?;
        Ok(logger)
    }

    pub(crate) async fn insert_start(&self, request: RequestLogStart<'_>) -> Result<()> {
        let stored_request_body = stored_body(request.request_body, self.body_limit);
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
  request_body,
  request_body_truncated
)
VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
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
        .bind(stored_request_body.text)
        .bind(stored_request_body.truncated)
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
        let stored_response_body = stored_body(completion.response_body, self.body_limit);
        sqlx::query(
            r#"
UPDATE proxy_requests
SET
  completed_at = ?,
  upstream_status = ?,
  latency_ms = ?,
  response_bytes = ?,
  response_body = ?,
  response_body_truncated = ?,
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
        .bind(stored_response_body.text)
        .bind(stored_response_body.truncated)
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

    pub(crate) async fn list_recent_matching(
        &self,
        query: RequestLogListQuery,
    ) -> Result<Vec<RequestLogSummary>> {
        let limit = query.limit.clamp(1, 500);
        let mut builder = QueryBuilder::<Sqlite>::new(
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
  request_body_truncated,
  response_body_truncated,
  input_tokens,
  output_tokens,
  total_tokens,
  cached_input_tokens,
  reasoning_output_tokens,
  error
FROM proxy_requests
"#,
        );
        let mut has_where = false;
        match query.filter {
            RequestLogFilter::All => {}
            RequestLogFilter::Errors => {
                push_where(&mut builder, &mut has_where);
                builder.push("(upstream_status >= 400 OR error IS NOT NULL)");
            }
            RequestLogFilter::Slow => {
                push_where(&mut builder, &mut has_where);
                builder.push("latency_ms >= 30000");
            }
            RequestLogFilter::HighTokens => {
                push_where(&mut builder, &mut has_where);
                builder.push(
                    r#"
(
  input_tokens >= 100000
  OR output_tokens >= 8000
  OR total_tokens >= 120000
  OR reasoning_output_tokens >= 8000
)
"#,
                );
            }
            RequestLogFilter::Truncated => {
                push_where(&mut builder, &mut has_where);
                builder.push("(request_body_truncated OR response_body_truncated)");
            }
        }
        if let Some(search) = query.search.as_deref().map(str::trim)
            && !search.is_empty()
        {
            let pattern = format!("%{search}%");
            push_where(&mut builder, &mut has_where);
            builder.push(
                r#"
(
  id LIKE
"#,
            );
            builder.push_bind(pattern.clone());
            builder.push(" OR client_ip LIKE ");
            builder.push_bind(pattern.clone());
            builder.push(" OR method LIKE ");
            builder.push_bind(pattern.clone());
            builder.push(" OR path LIKE ");
            builder.push_bind(pattern.clone());
            builder.push(" OR query LIKE ");
            builder.push_bind(pattern.clone());
            builder.push(" OR model LIKE ");
            builder.push_bind(pattern.clone());
            builder.push(" OR error LIKE ");
            builder.push_bind(pattern.clone());
            builder.push(" OR request_body LIKE ");
            builder.push_bind(pattern.clone());
            builder.push(" OR response_body LIKE ");
            builder.push_bind(pattern);
            builder.push(")");
        }
        builder.push(
            r#"
ORDER BY started_at DESC
LIMIT
"#,
        );
        builder.push_bind(limit);
        builder
            .build_query_as()
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
  request_body_truncated,
  response_body_truncated,
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

        self.backfill_detail_token_usage(&mut detail).await?;

        Ok(Some(detail))
    }

    pub(crate) async fn flow_around(&self, id: &str) -> Result<RequestLogFlow> {
        let Some(selected) = sqlx::query_as::<_, RequestLogFlowSeed>(
            r#"
SELECT
  started_at,
  client_ip,
  path,
  request_body
FROM proxy_requests
WHERE id = ?
"#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .context("reading selected proxy request flow row")?
        else {
            return Ok(RequestLogFlow {
                basis: RequestLogFlowBasis::Unavailable,
                rows: Vec::new(),
            });
        };

        if selected.path != "/v1/responses" {
            return Ok(RequestLogFlow {
                basis: RequestLogFlowBasis::Unavailable,
                rows: Vec::new(),
            });
        }

        let selected_user_asked =
            user_asked_key_from_request_body(selected.request_body.as_deref());
        let mut candidates = sqlx::query_as::<_, RequestLogFlowCandidate>(
            r#"
SELECT
  r.id,
  r.started_at,
  CAST(r.started_at AS REAL) AS started_at_seconds,
  r.completed_at,
  r.client_ip,
  r.method,
  r.path,
  r.query,
  r.model,
  r.upstream_status,
  r.latency_ms,
  r.request_bytes,
  r.response_bytes,
  r.request_body_truncated,
  r.response_body_truncated,
  r.input_tokens,
  r.output_tokens,
  r.total_tokens,
  r.cached_input_tokens,
  r.reasoning_output_tokens,
  r.error,
  r.request_body,
  r.response_body,
  ABS(CAST(r.started_at AS REAL) - CAST(? AS REAL)) AS distance
FROM proxy_requests AS r
WHERE r.path = '/v1/responses'
  AND ABS(CAST(r.started_at AS REAL) - CAST(? AS REAL)) <= ?
  AND (
    (? IS NULL AND r.client_ip IS NULL)
    OR r.client_ip = ?
  )
ORDER BY
  distance,
  CAST(r.started_at AS REAL),
  r.started_at,
  r.id
LIMIT ?
"#,
        )
        .bind(&selected.started_at)
        .bind(&selected.started_at)
        .bind(FLOW_WINDOW_SECONDS)
        .bind(selected.client_ip.as_deref())
        .bind(selected.client_ip.as_deref())
        .bind(FLOW_CANDIDATE_LIMIT)
        .fetch_all(&self.pool)
        .await
        .context("reading proxy request flow rows")?;

        let mut basis = RequestLogFlowBasis::Nearby;
        if let Some(user_asked) = selected_user_asked.as_deref() {
            candidates.retain(|candidate| {
                user_asked_key_from_request_body(candidate.request_body.as_deref()).as_deref()
                    == Some(user_asked)
            });
            basis = RequestLogFlowBasis::UserAsked;
        }

        if let Some(chain) = call_id_flow_candidates(&candidates, id) {
            candidates = chain;
            basis = RequestLogFlowBasis::ToolCallChain;
        }

        candidates.sort_by(|left, right| {
            left.distance
                .partial_cmp(&right.distance)
                .unwrap_or(Ordering::Equal)
                .then_with(|| {
                    left.started_at_seconds
                        .partial_cmp(&right.started_at_seconds)
                        .unwrap_or(Ordering::Equal)
                })
                .then_with(|| left.started_at.cmp(&right.started_at))
                .then_with(|| left.id.cmp(&right.id))
        });
        candidates.truncate(FLOW_ROW_LIMIT as usize);
        candidates.sort_by(|left, right| {
            left.started_at_seconds
                .partial_cmp(&right.started_at_seconds)
                .unwrap_or(Ordering::Equal)
                .then_with(|| left.started_at.cmp(&right.started_at))
                .then_with(|| left.id.cmp(&right.id))
        });

        let rows = candidates
            .into_iter()
            .map(|candidate| RequestLogSummary {
                id: candidate.id,
                started_at: candidate.started_at,
                completed_at: candidate.completed_at,
                client_ip: candidate.client_ip,
                method: candidate.method,
                path: candidate.path,
                query: candidate.query,
                model: candidate.model,
                upstream_status: candidate.upstream_status,
                latency_ms: candidate.latency_ms,
                request_bytes: candidate.request_bytes,
                response_bytes: candidate.response_bytes,
                request_body_truncated: candidate.request_body_truncated,
                response_body_truncated: candidate.response_body_truncated,
                input_tokens: candidate.input_tokens,
                output_tokens: candidate.output_tokens,
                total_tokens: candidate.total_tokens,
                cached_input_tokens: candidate.cached_input_tokens,
                reasoning_output_tokens: candidate.reasoning_output_tokens,
                error: candidate.error,
            })
            .collect();

        Ok(RequestLogFlow { basis, rows })
    }

    #[cfg(test)]
    pub(crate) fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    async fn backfill_detail_token_usage(&self, detail: &mut RequestLogDetail) -> Result<()> {
        if detail.has_no_token_usage()
            && let Some(response_body) = detail.response_body.as_deref()
        {
            let usage = token_usage_from_response_body(response_body.as_bytes());
            if !usage.is_empty() {
                self.update_token_usage(&detail.id, usage).await?;
                detail.apply_token_usage(usage);
            }
        }
        Ok(())
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

fn stored_body(body: &[u8], limit: Option<RequestLogBodyLimit>) -> StoredBody {
    let Some(limit) = limit else {
        return StoredBody {
            text: String::from_utf8_lossy(body).into_owned(),
            truncated: false,
        };
    };
    let max_bytes = usize::try_from(limit.max_bytes.get()).unwrap_or(usize::MAX);
    let truncated = body.len() > max_bytes;
    let stored = if truncated { &body[..max_bytes] } else { body };
    StoredBody {
        text: String::from_utf8_lossy(stored).into_owned(),
        truncated,
    }
}

fn push_where(builder: &mut QueryBuilder<Sqlite>, has_where: &mut bool) {
    if *has_where {
        builder.push(" AND ");
    } else {
        builder.push(" WHERE ");
        *has_where = true;
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
  request_body_truncated INTEGER NOT NULL DEFAULT 0,
  response_body_truncated INTEGER NOT NULL DEFAULT 0,
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
    ensure_column(pool, "request_body_truncated", "INTEGER NOT NULL DEFAULT 0").await?;
    ensure_column(
        pool,
        "response_body_truncated",
        "INTEGER NOT NULL DEFAULT 0",
    )
    .await?;
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

fn user_asked_key_from_request_body(body: Option<&str>) -> Option<String> {
    let request = serde_json::from_str::<Value>(body?).ok()?;
    match request.get("input")? {
        Value::String(text) => normalize_user_asked_text(text),
        Value::Array(items) => items.iter().rev().find_map(|item| {
            if item.get("role").and_then(Value::as_str) != Some("user") {
                return None;
            }
            input_item_text(item)
        }),
        _ => None,
    }
}

fn input_item_text(item: &Value) -> Option<String> {
    match item {
        Value::String(text) => normalize_user_asked_text(text),
        Value::Object(map) => [
            map.get("content").and_then(content_text),
            map.get("output").and_then(text_value),
            map.get("input").and_then(text_value),
            map.get("arguments").and_then(text_value),
            map.get("text").and_then(text_value),
            map.get("summary").and_then(text_value),
        ]
        .into_iter()
        .flatten()
        .next(),
        _ => None,
    }
}

fn content_text(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => normalize_user_asked_text(text),
        Value::Array(parts) => {
            let text = parts
                .iter()
                .filter_map(content_part_text)
                .collect::<Vec<_>>()
                .join("\n");
            normalize_user_asked_text(&text)
        }
        Value::Object(_) => content_part_text(value),
        _ => None,
    }
}

fn content_part_text(part: &Value) -> Option<String> {
    match part {
        Value::String(text) => normalize_user_asked_text(text),
        Value::Object(map) => [
            map.get("text").and_then(text_value),
            map.get("input_text").and_then(text_value),
            map.get("output_text").and_then(text_value),
            map.get("content").and_then(content_text),
            map.get("summary").and_then(text_value),
        ]
        .into_iter()
        .flatten()
        .next(),
        _ => None,
    }
}

fn text_value(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => normalize_user_asked_text(text),
        Value::Array(_) | Value::Object(_) => content_text(value),
        _ => None,
    }
}

fn normalize_user_asked_text(text: &str) -> Option<String> {
    let normalized = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.is_empty() {
        None
    } else {
        Some(normalized)
    }
}

struct FlowCallIds {
    request_tool_outputs: BTreeSet<String>,
    response_tool_calls: BTreeSet<String>,
}

fn call_id_flow_candidates(
    candidates: &[RequestLogFlowCandidate],
    selected_id: &str,
) -> Option<Vec<RequestLogFlowCandidate>> {
    let selected_index = candidates
        .iter()
        .position(|candidate| candidate.id == selected_id)?;
    let call_ids = candidates
        .iter()
        .map(|candidate| FlowCallIds {
            request_tool_outputs: request_tool_output_call_ids(candidate.request_body.as_deref()),
            response_tool_calls: response_tool_call_ids(candidate.response_body.as_deref()),
        })
        .collect::<Vec<_>>();
    let mut adjacency = vec![Vec::new(); candidates.len()];
    for earlier in 0..candidates.len() {
        for later in 0..candidates.len() {
            if earlier == later
                || candidates[earlier].started_at_seconds > candidates[later].started_at_seconds
            {
                continue;
            }
            if call_ids[earlier]
                .response_tool_calls
                .is_disjoint(&call_ids[later].request_tool_outputs)
            {
                continue;
            }
            adjacency[earlier].push(later);
            adjacency[later].push(earlier);
        }
    }

    let mut seen = BTreeSet::new();
    let mut queue = VecDeque::from([selected_index]);
    while let Some(index) = queue.pop_front() {
        if !seen.insert(index) {
            continue;
        }
        queue.extend(adjacency[index].iter().copied());
    }
    if seen.len() <= 1 {
        return None;
    }

    Some(
        seen.into_iter()
            .map(|index| candidates[index].clone())
            .collect(),
    )
}

fn request_tool_output_call_ids(body: Option<&str>) -> BTreeSet<String> {
    let mut call_ids = BTreeSet::new();
    let Some(Value::Array(items)) = body
        .and_then(|body| serde_json::from_str::<Value>(body).ok())
        .and_then(|request| request.get("input").cloned())
    else {
        return call_ids;
    };

    for item in items {
        let Value::Object(map) = item else {
            continue;
        };
        let kind = map.get("type").and_then(Value::as_str).unwrap_or_default();
        if !(kind.contains("output") || kind.contains("call_result")) {
            continue;
        }
        if let Some(call_id) = map.get("call_id").and_then(Value::as_str)
            && !call_id.is_empty()
        {
            call_ids.insert(call_id.to_string());
        }
    }
    call_ids
}

fn response_tool_call_ids(body: Option<&str>) -> BTreeSet<String> {
    let mut call_ids = BTreeSet::new();
    let Some(body) = body else {
        return call_ids;
    };

    let mut data_lines = Vec::new();
    for line in body.lines().map(|line| line.trim_end_matches('\r')) {
        if line.is_empty() {
            collect_response_tool_call_ids_from_sse_data(&data_lines, &mut call_ids);
            data_lines.clear();
            continue;
        }
        if let Some(data) = line.strip_prefix("data:") {
            data_lines.push(data.trim_start());
        }
    }
    collect_response_tool_call_ids_from_sse_data(&data_lines, &mut call_ids);

    call_ids
}

fn collect_response_tool_call_ids_from_sse_data(
    data_lines: &[&str],
    call_ids: &mut BTreeSet<String>,
) {
    if data_lines.is_empty() {
        return;
    }
    let data = data_lines.join("\n");
    let data = data.trim();
    if data.is_empty() || data == "[DONE]" {
        return;
    }
    if let Ok(value) = serde_json::from_str::<Value>(data) {
        collect_response_tool_call_ids(&value, call_ids);
    }
}

fn collect_response_tool_call_ids(value: &Value, call_ids: &mut BTreeSet<String>) {
    match value {
        Value::Array(items) => {
            for item in items {
                collect_response_tool_call_ids(item, call_ids);
            }
        }
        Value::Object(map) => {
            let kind = map.get("type").and_then(Value::as_str).unwrap_or_default();
            if (kind.contains("function_call") || kind.contains("tool_call"))
                && !kind.contains("output")
                && let Some(call_id) = map.get("call_id").and_then(Value::as_str)
                && !call_id.is_empty()
            {
                call_ids.insert(call_id.to_string());
            }
            for item in map.values() {
                collect_response_tool_call_ids(item, call_ids);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
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
