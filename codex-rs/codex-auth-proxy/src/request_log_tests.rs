use pretty_assertions::assert_eq;
use sqlx::Row;
use std::num::NonZeroU64;
use tokio::runtime::Runtime;

use super::*;

#[derive(Debug, PartialEq, Eq)]
struct LoggedRequest {
    id: String,
    started_at: String,
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
    request_body: Option<String>,
    response_body: Option<String>,
    error: Option<String>,
}

#[test]
fn request_logger_writes_request_and_response_body() {
    with_logger(|runtime, logger| {
        runtime.block_on(async {
            logger
                .insert_start(RequestLogStart {
                    id: "req-1",
                    started_at: "1000.001",
                    client_ip: Some("127.0.0.1"),
                    method: "POST",
                    path: "/v1/responses",
                    query: Some("timeout=120"),
                    model: Some("gpt-5.5"),
                    request_body: br#"{"model":"gpt-5.5","input":"hello"}"#,
                })
                .await
                .expect("insert request log");
            logger
                .complete(
                    "req-1",
                    RequestLogCompletion {
                        completed_at: "1000.123",
                        upstream_status: Some(200),
                        latency_ms: 122,
                        response_body: br#"event: response.completed
data: {"response":{"usage":{"input_tokens":120,"output_tokens":30,"total_tokens":150,"input_tokens_details":{"cached_tokens":80},"output_tokens_details":{"reasoning_tokens":12}}}}

"#,
                        error: None,
                    },
                )
                .await
                .expect("complete request log");

            assert_eq!(
                fetch_request(logger, "req-1").await,
                LoggedRequest {
                    id: "req-1".to_string(),
                    started_at: "1000.001".to_string(),
                    completed_at: Some("1000.123".to_string()),
                    client_ip: Some("127.0.0.1".to_string()),
                    method: "POST".to_string(),
                    path: "/v1/responses".to_string(),
                    query: Some("timeout=120".to_string()),
                    model: Some("gpt-5.5".to_string()),
                    upstream_status: Some(200),
                    latency_ms: Some(122),
                    request_bytes: Some(35),
                    response_bytes: Some(207),
                    request_body_truncated: false,
                    response_body_truncated: false,
                    input_tokens: Some(120),
                    output_tokens: Some(30),
                    total_tokens: Some(150),
                    cached_input_tokens: Some(80),
                    reasoning_output_tokens: Some(12),
                    request_body: Some(r#"{"model":"gpt-5.5","input":"hello"}"#.to_string()),
                    response_body: Some(
                        "event: response.completed\ndata: {\"response\":{\"usage\":{\"input_tokens\":120,\"output_tokens\":30,\"total_tokens\":150,\"input_tokens_details\":{\"cached_tokens\":80},\"output_tokens_details\":{\"reasoning_tokens\":12}}}}\n\n".to_string()
                    ),
                    error: None,
                }
            );
        });
    });
}

#[test]
fn request_logger_records_upstream_errors() {
    with_logger(|runtime, logger| {
        runtime.block_on(async {
            logger
                .insert_start(RequestLogStart {
                    id: "req-error",
                    started_at: "2000.001",
                    client_ip: None,
                    method: "POST",
                    path: "/v1/responses",
                    query: None,
                    model: None,
                    request_body: b"{}",
                })
                .await
                .expect("insert request log");
            logger
                .complete(
                    "req-error",
                    RequestLogCompletion {
                        completed_at: "2000.002",
                        upstream_status: None,
                        latency_ms: 1,
                        response_body: b"",
                        error: Some("upstream unavailable"),
                    },
                )
                .await
                .expect("complete request log");

            assert_eq!(
                fetch_request(logger, "req-error").await.error,
                Some("upstream unavailable".to_string())
            );
        });
    });
}

#[test]
fn request_logger_lists_recent_rows_and_reads_detail() {
    with_logger(|runtime, logger| {
        runtime.block_on(async {
            logger
                .insert_start(RequestLogStart {
                    id: "req-older",
                    started_at: "1000.001",
                    client_ip: Some("127.0.0.1"),
                    method: "POST",
                    path: "/v1/responses",
                    query: None,
                    model: Some("gpt-5.5"),
                    request_body: b"{}",
                })
                .await
                .expect("insert older row");
            logger
                .insert_start(RequestLogStart {
                    id: "req-newer",
                    started_at: "2000.001",
                    client_ip: Some("127.0.0.2"),
                    method: "GET",
                    path: "/v1/models",
                    query: Some("client_version=1"),
                    model: None,
                    request_body: b"",
                })
                .await
                .expect("insert newer row");
            logger
                .complete(
                    "req-older",
                    RequestLogCompletion {
                        completed_at: "1000.100",
                        upstream_status: Some(200),
                        latency_ms: 99,
                        response_body: b"event: done\n\n",
                        error: None,
                    },
                )
                .await
                .expect("complete older row");

            assert_eq!(
                logger
                    .list_recent_matching(RequestLogListQuery {
                        limit: 1,
                        ..RequestLogListQuery::default()
                    })
                    .await
                    .expect("recent rows"),
                vec![RequestLogSummary {
                    id: "req-newer".to_string(),
                    started_at: "2000.001".to_string(),
                    completed_at: None,
                    client_ip: Some("127.0.0.2".to_string()),
                    method: "GET".to_string(),
                    path: "/v1/models".to_string(),
                    query: Some("client_version=1".to_string()),
                    model: None,
                    upstream_status: None,
                    latency_ms: None,
                    request_bytes: Some(0),
                    response_bytes: None,
                    request_body_truncated: false,
                    response_body_truncated: false,
                    input_tokens: None,
                    output_tokens: None,
                    total_tokens: None,
                    cached_input_tokens: None,
                    reasoning_output_tokens: None,
                    error: None,
                }]
            );

            assert_eq!(
                logger.get_detail("req-older").await.expect("detail row"),
                Some(RequestLogDetail {
                    id: "req-older".to_string(),
                    started_at: "1000.001".to_string(),
                    completed_at: Some("1000.100".to_string()),
                    client_ip: Some("127.0.0.1".to_string()),
                    method: "POST".to_string(),
                    path: "/v1/responses".to_string(),
                    query: None,
                    model: Some("gpt-5.5".to_string()),
                    upstream_status: Some(200),
                    latency_ms: Some(99),
                    request_bytes: Some(2),
                    response_bytes: Some(13),
                    request_body_truncated: false,
                    response_body_truncated: false,
                    input_tokens: None,
                    output_tokens: None,
                    total_tokens: None,
                    cached_input_tokens: None,
                    reasoning_output_tokens: None,
                    request_body: Some("{}".to_string()),
                    response_body: Some("event: done\n\n".to_string()),
                    error: None,
                })
            );
        });
    });
}

#[test]
fn request_logger_schema_does_not_persist_authorization_headers() {
    with_logger(|runtime, logger| {
        runtime.block_on(async {
            let rows = sqlx::query("PRAGMA table_info(proxy_requests)")
                .fetch_all(logger.pool())
                .await
                .expect("schema rows");
            let column_names = rows
                .iter()
                .map(|row| row.get::<String, _>("name"))
                .collect::<Vec<_>>();
            assert!(
                !column_names
                    .iter()
                    .any(|name| name.to_ascii_lowercase().contains("authorization")),
                "authorization data must not have a storage column: {column_names:?}"
            );
        });
    });
}

#[test]
fn request_logger_migrates_existing_db_for_token_columns() {
    let runtime = Runtime::new().expect("runtime");
    let temp_dir = tempfile::tempdir().expect("temp dir");
    let db_path = temp_dir.path().join("proxy.sqlite");

    runtime.block_on(async {
        let options = sqlx::sqlite::SqliteConnectOptions::new()
            .filename(&db_path)
            .create_if_missing(true);
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .expect("open pre-migration db");
        sqlx::query(
            r#"
CREATE TABLE proxy_requests (
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
        .execute(&pool)
        .await
        .expect("create old schema");
        pool.close().await;

        let logger = RequestLogger::open(&db_path).await.expect("migrate db");
        let rows = sqlx::query("PRAGMA table_info(proxy_requests)")
            .fetch_all(logger.pool())
            .await
            .expect("schema rows");
        let column_names = rows
            .iter()
            .map(|row| row.get::<String, _>("name"))
            .collect::<Vec<_>>();

        assert!(column_names.contains(&"input_tokens".to_string()));
        assert!(column_names.contains(&"output_tokens".to_string()));
        assert!(column_names.contains(&"total_tokens".to_string()));
        assert!(column_names.contains(&"cached_input_tokens".to_string()));
        assert!(column_names.contains(&"reasoning_output_tokens".to_string()));
        assert!(column_names.contains(&"request_body_truncated".to_string()));
        assert!(column_names.contains(&"response_body_truncated".to_string()));
    });
}

#[test]
fn get_detail_backfills_token_usage_from_existing_response_body() {
    with_logger(|runtime, logger| {
        runtime.block_on(async {
            let response_body = r#"event: response.completed
data: {"response":{"usage":{"input_tokens":20,"output_tokens":5,"total_tokens":25,"input_tokens_details":{"cached_tokens":7},"output_tokens_details":{"reasoning_tokens":3}}}}

"#;
            sqlx::query(
                r#"
INSERT INTO proxy_requests (
  id,
  started_at,
  method,
  path,
  response_body
)
VALUES (?, ?, ?, ?, ?)
"#,
            )
            .bind("req-old")
            .bind("3000.001")
            .bind("POST")
            .bind("/v1/responses")
            .bind(response_body)
            .execute(logger.pool())
            .await
            .expect("insert old row");

            let detail = logger
                .get_detail("req-old")
                .await
                .expect("detail")
                .expect("row");
            assert_eq!(
                (
                    detail.input_tokens,
                    detail.output_tokens,
                    detail.total_tokens,
                    detail.cached_input_tokens,
                    detail.reasoning_output_tokens,
                ),
                (Some(20), Some(5), Some(25), Some(7), Some(3))
            );

            let row = sqlx::query(
                r#"
SELECT
  input_tokens,
  output_tokens,
  total_tokens,
  cached_input_tokens,
  reasoning_output_tokens
FROM proxy_requests
WHERE id = ?
"#,
            )
            .bind("req-old")
            .fetch_one(logger.pool())
            .await
            .expect("backfilled row");
            assert_eq!(
                (
                    row.get::<Option<i64>, _>("input_tokens"),
                    row.get::<Option<i64>, _>("output_tokens"),
                    row.get::<Option<i64>, _>("total_tokens"),
                    row.get::<Option<i64>, _>("cached_input_tokens"),
                    row.get::<Option<i64>, _>("reasoning_output_tokens"),
                ),
                (Some(20), Some(5), Some(25), Some(7), Some(3))
            );
        });
    });
}

#[test]
fn request_logger_truncates_stored_bodies_only() {
    let runtime = Runtime::new().expect("runtime");
    let temp_dir = tempfile::tempdir().expect("temp dir");
    let db_path = temp_dir.path().join("proxy.sqlite");

    runtime.block_on(async {
        let logger = RequestLogger::open_with_options(
            &db_path,
            RequestLogOptions {
                retention: None,
                body_limit: Some(RequestLogBodyLimit::new(
                    NonZeroU64::new(5).expect("non-zero"),
                )),
            },
        )
        .await
        .expect("request logger");
        logger
            .insert_start(RequestLogStart {
                id: "req-truncated",
                started_at: "4000.001",
                client_ip: None,
                method: "POST",
                path: "/v1/responses",
                query: None,
                model: Some("gpt-5.5"),
                request_body: b"abcdefghi",
            })
            .await
            .expect("insert request log");
        logger
            .complete(
                "req-truncated",
                RequestLogCompletion {
                    completed_at: "4000.002",
                    upstream_status: Some(200),
                    latency_ms: 1,
                    response_body: b"0123456789",
                    error: None,
                },
            )
            .await
            .expect("complete request log");

        let row = fetch_request(&logger, "req-truncated").await;
        assert_eq!(
            (
                row.request_bytes,
                row.request_body,
                row.request_body_truncated,
                row.response_bytes,
                row.response_body,
                row.response_body_truncated,
            ),
            (
                Some(9),
                Some("abcde".to_string()),
                true,
                Some(10),
                Some("01234".to_string()),
                true,
            )
        );
    });
}

#[test]
fn request_logger_filters_recent_rows() {
    with_logger(|runtime, logger| {
        runtime.block_on(async {
            insert_completed_with(
                logger,
                "req-error",
                "5000.001",
                Some(500),
                1,
                b"request needle",
                b"",
            )
            .await;
            insert_completed_with(
                logger,
                "req-slow",
                "5000.002",
                Some(200),
                30_000,
                b"{}",
                b"response needle",
            )
            .await;
            insert_completed_with(
                logger,
                "req-tokens",
                "5000.003",
                Some(200),
                1,
                b"{}",
                br#"event: response.completed
data: {"response":{"usage":{"input_tokens":100000,"output_tokens":1,"total_tokens":120001}}}

"#,
            )
            .await;
            insert_completed_with(
                logger,
                "req-truncated",
                "5000.004",
                Some(200),
                1,
                b"{}",
                b"",
            )
            .await;
            sqlx::query("UPDATE proxy_requests SET request_body_truncated = 1 WHERE id = ?")
                .bind("req-truncated")
                .execute(logger.pool())
                .await
                .expect("mark truncated");

            assert_eq!(
                ids_for_query(logger, RequestLogFilter::Errors, /*search*/ None,).await,
                vec!["req-error".to_string()]
            );
            assert_eq!(
                ids_for_query(logger, RequestLogFilter::Slow, /*search*/ None).await,
                vec!["req-slow".to_string()]
            );
            assert_eq!(
                ids_for_query(logger, RequestLogFilter::HighTokens, /*search*/ None).await,
                vec!["req-tokens".to_string()]
            );
            assert_eq!(
                ids_for_query(logger, RequestLogFilter::Truncated, /*search*/ None).await,
                vec!["req-truncated".to_string()]
            );
            assert_eq!(
                ids_for_query(
                    logger,
                    RequestLogFilter::All,
                    Some("request needle".to_string()),
                )
                .await,
                vec!["req-error".to_string()]
            );
            assert_eq!(
                ids_for_query(
                    logger,
                    RequestLogFilter::All,
                    Some("response needle".to_string()),
                )
                .await,
                vec!["req-slow".to_string()]
            );
        });
    });
}

#[test]
fn request_logger_reads_flow_around_selected_row() {
    with_logger(|runtime, logger| {
        runtime.block_on(async {
            insert_completed_with_meta(
                logger,
                "req-before",
                "940.000",
                Some("192.168.0.10"),
                "/v1/responses",
                Some("same user request"),
            )
            .await;
            insert_completed_with_meta(
                logger,
                "req-selected",
                "1000.000",
                Some("192.168.0.10"),
                "/v1/responses",
                Some("same user request"),
            )
            .await;
            insert_completed_with_meta(
                logger,
                "req-after",
                "1060.000",
                Some("192.168.0.10"),
                "/v1/responses",
                Some("same user request"),
            )
            .await;
            insert_completed_with_meta(
                logger,
                "req-different-user",
                "1001.000",
                Some("192.168.0.10"),
                "/v1/responses",
                Some("different user request"),
            )
            .await;
            insert_completed_with_meta(
                logger,
                "req-other-client",
                "1001.000",
                Some("192.168.0.11"),
                "/v1/responses",
                Some("same user request"),
            )
            .await;
            insert_completed_with_meta(
                logger,
                "req-models",
                "1002.000",
                Some("192.168.0.10"),
                "/v1/models",
                Some("same user request"),
            )
            .await;
            insert_completed_with_meta(
                logger,
                "req-far",
                "2000.000",
                Some("192.168.0.10"),
                "/v1/responses",
                Some("same user request"),
            )
            .await;

            let rows = logger.flow_around("req-selected").await.expect("flow rows");
            assert_eq!(
                rows.into_iter().map(|row| row.id).collect::<Vec<_>>(),
                vec![
                    "req-before".to_string(),
                    "req-selected".to_string(),
                    "req-after".to_string(),
                ]
            );
            assert_eq!(
                logger.flow_around("missing").await.expect("missing flow"),
                Vec::<RequestLogSummary>::new()
            );
        });
    });
}

#[test]
fn user_asked_key_reads_latest_user_input_text() {
    assert_eq!(
        user_asked_key_from_request_body(Some(
            r#"{"input":[{"role":"user","content":[{"type":"input_text","text":"first"}]},{"role":"assistant","content":[{"type":"output_text","text":"answer"}]},{"role":"user","content":[{"type":"input_text","text":"second\n\nquestion"}]}]}"#
        )),
        Some("second question".to_string())
    );
    assert_eq!(
        user_asked_key_from_request_body(Some(r#"{"input":"simple question"}"#)),
        Some("simple question".to_string())
    );
    assert_eq!(
        user_asked_key_from_request_body(Some(
            r#"{"input":[{"role":"assistant","content":[{"type":"output_text","text":"answer"}]}]}"#
        )),
        None
    );
}

#[test]
fn open_with_retention_prunes_existing_completed_rows() {
    let runtime = Runtime::new().expect("runtime");
    let temp_dir = tempfile::tempdir().expect("temp dir");
    let db_path = temp_dir.path().join("proxy.sqlite");

    runtime.block_on(async {
        let logger = RequestLogger::open(&db_path).await.expect("open db");
        insert_completed(&logger, "req-old", "1000.001").await;
        insert_completed(&logger, "req-middle", "2000.001").await;
        insert_completed(&logger, "req-new", "3000.001").await;
        insert_started(&logger, "req-active", "0001.001").await;
        logger.pool().close().await;

        let logger = RequestLogger::open_with_options(
            &db_path,
            RequestLogOptions {
                retention: Some(RequestLogRetention::new(
                    NonZeroU64::new(2).expect("non-zero"),
                )),
                body_limit: None,
            },
        )
        .await
        .expect("open with retention");

        assert_eq!(
            fetch_ids_by_started_at(&logger).await,
            vec![
                "req-active".to_string(),
                "req-middle".to_string(),
                "req-new".to_string()
            ]
        );
    });
}

#[test]
fn complete_prunes_to_retention_limit() {
    let runtime = Runtime::new().expect("runtime");
    let temp_dir = tempfile::tempdir().expect("temp dir");
    let db_path = temp_dir.path().join("proxy.sqlite");

    runtime.block_on(async {
        let logger = RequestLogger::open_with_options(
            &db_path,
            RequestLogOptions {
                retention: Some(RequestLogRetention::new(
                    NonZeroU64::new(2).expect("non-zero"),
                )),
                body_limit: None,
            },
        )
        .await
        .expect("open with retention");

        insert_completed(&logger, "req-old", "1000.001").await;
        insert_completed(&logger, "req-middle", "2000.001").await;
        insert_completed(&logger, "req-new", "3000.001").await;

        assert_eq!(
            fetch_ids_by_started_at(&logger).await,
            vec!["req-middle".to_string(), "req-new".to_string()]
        );
    });
}

#[test]
fn model_from_body_reads_top_level_model() {
    assert_eq!(
        model_from_body(br#"{"model":"gpt-5.5","input":"hello"}"#),
        Some("gpt-5.5".to_string())
    );
    assert_eq!(model_from_body(br#"{"input":"hello"}"#), None);
    assert_eq!(model_from_body(b"not json"), None);
}

async fn insert_started(logger: &RequestLogger, id: &str, started_at: &str) {
    logger
        .insert_start(RequestLogStart {
            id,
            started_at,
            client_ip: None,
            method: "POST",
            path: "/v1/responses",
            query: None,
            model: Some("gpt-5.5"),
            request_body: b"{}",
        })
        .await
        .expect("insert started row");
}

async fn insert_completed(logger: &RequestLogger, id: &str, started_at: &str) {
    insert_started(logger, id, started_at).await;
    logger
        .complete(
            id,
            RequestLogCompletion {
                completed_at: "9999.001",
                upstream_status: Some(200),
                latency_ms: 1,
                response_body: b"event: done\n\n",
                error: None,
            },
        )
        .await
        .expect("complete row");
}

async fn insert_completed_with(
    logger: &RequestLogger,
    id: &str,
    started_at: &str,
    upstream_status: Option<u16>,
    latency_ms: u128,
    request_body: &[u8],
    response_body: &[u8],
) {
    logger
        .insert_start(RequestLogStart {
            id,
            started_at,
            client_ip: None,
            method: "POST",
            path: "/v1/responses",
            query: None,
            model: Some("gpt-5.5"),
            request_body,
        })
        .await
        .expect("insert row");
    logger
        .complete(
            id,
            RequestLogCompletion {
                completed_at: "9999.001",
                upstream_status,
                latency_ms,
                response_body,
                error: None,
            },
        )
        .await
        .expect("complete row");
}

async fn insert_completed_with_meta(
    logger: &RequestLogger,
    id: &str,
    started_at: &str,
    client_ip: Option<&str>,
    path: &str,
    user_asked: Option<&str>,
) {
    let request_body = user_asked
        .map(request_body_with_user)
        .unwrap_or_else(|| "{}".to_string());
    logger
        .insert_start(RequestLogStart {
            id,
            started_at,
            client_ip,
            method: "POST",
            path,
            query: None,
            model: Some("gpt-5.5"),
            request_body: request_body.as_bytes(),
        })
        .await
        .expect("insert row");
    logger
        .complete(
            id,
            RequestLogCompletion {
                completed_at: "9999.001",
                upstream_status: Some(200),
                latency_ms: 1,
                response_body: b"event: done\n\n",
                error: None,
            },
        )
        .await
        .expect("complete row");
}

fn request_body_with_user(text: &str) -> String {
    serde_json::json!({
        "model": "gpt-5.5",
        "input": [
            {
                "role": "user",
                "content": [
                    {
                        "type": "input_text",
                        "text": text
                    }
                ]
            }
        ]
    })
    .to_string()
}

async fn fetch_ids_by_started_at(logger: &RequestLogger) -> Vec<String> {
    sqlx::query_scalar("SELECT id FROM proxy_requests ORDER BY started_at, id")
        .fetch_all(logger.pool())
        .await
        .expect("fetch ids")
}

async fn ids_for_query(
    logger: &RequestLogger,
    filter: RequestLogFilter,
    search: Option<String>,
) -> Vec<String> {
    logger
        .list_recent_matching(RequestLogListQuery {
            limit: 10,
            filter,
            search,
        })
        .await
        .expect("filtered rows")
        .into_iter()
        .map(|row| row.id)
        .collect()
}

fn with_logger(test: impl FnOnce(&Runtime, &RequestLogger)) {
    let runtime = Runtime::new().expect("runtime");
    let temp_dir = tempfile::tempdir().expect("temp dir");
    let db_path = temp_dir.path().join("proxy.sqlite");
    let logger = runtime
        .block_on(RequestLogger::open(&db_path))
        .expect("request logger");

    test(&runtime, &logger);
}

async fn fetch_request(logger: &RequestLogger, id: &str) -> LoggedRequest {
    let row = sqlx::query(
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
    .fetch_one(logger.pool())
    .await
    .expect("logged request row");

    LoggedRequest {
        id: row.get("id"),
        started_at: row.get("started_at"),
        completed_at: row.get("completed_at"),
        client_ip: row.get("client_ip"),
        method: row.get("method"),
        path: row.get("path"),
        query: row.get("query"),
        model: row.get("model"),
        upstream_status: row.get("upstream_status"),
        latency_ms: row.get("latency_ms"),
        request_bytes: row.get("request_bytes"),
        response_bytes: row.get("response_bytes"),
        request_body_truncated: row.get("request_body_truncated"),
        response_body_truncated: row.get("response_body_truncated"),
        input_tokens: row.get("input_tokens"),
        output_tokens: row.get("output_tokens"),
        total_tokens: row.get("total_tokens"),
        cached_input_tokens: row.get("cached_input_tokens"),
        reasoning_output_tokens: row.get("reasoning_output_tokens"),
        request_body: row.get("request_body"),
        response_body: row.get("response_body"),
        error: row.get("error"),
    }
}
