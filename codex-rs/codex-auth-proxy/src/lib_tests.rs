use super::*;
use clap::Parser;
use pretty_assertions::assert_eq;
use std::num::NonZeroU64;
use std::path::Path;

#[test]
fn route_for_allows_only_responses_and_models() {
    assert_eq!(
        route_for(&Method::Post, "/v1/responses"),
        Some(RouteMatch {
            route: Route::Responses,
            query: None
        })
    );
    assert_eq!(
        route_for(&Method::Get, "/v1/models?client_version=1.2.3"),
        Some(RouteMatch {
            route: Route::Models,
            query: Some("client_version=1.2.3".to_string())
        })
    );
    assert!(route_for(&Method::Get, "/v1/responses").is_none());
    assert!(route_for(&Method::Get, "/health").is_none());
    assert_eq!(
        route_for(&Method::Post, "/v1/responses?x=1"),
        Some(RouteMatch {
            route: Route::Responses,
            query: Some("x=1".to_string())
        })
    );
}

#[test]
fn path_for_url_strips_query() {
    assert_eq!(path_for_url("/health"), "/health");
    assert_eq!(path_for_url("/health?verbose=true"), "/health");
    assert_eq!(
        path_for_url("/v1/models?client_version=1.2.3"),
        "/v1/models"
    );
}

#[test]
fn upstream_url_for_appends_incoming_query() {
    let config = ForwardConfig {
        responses_url: Url::parse("https://example.com/v1/responses?api-version=2025-04-01")
            .expect("url"),
        models_url: Url::parse("https://example.com/v1/models").expect("url"),
        proxy_auth: ProxyAuth::None,
        request_logger: None,
    };

    assert_eq!(
        upstream_url_for(
            &config,
            &RouteMatch {
                route: Route::Models,
                query: Some("client_version=1.2.3".to_string()),
            }
        )
        .as_str(),
        "https://example.com/v1/models?client_version=1.2.3"
    );
    assert_eq!(
        upstream_url_for(
            &config,
            &RouteMatch {
                route: Route::Responses,
                query: Some("timeout=120".to_string()),
            }
        )
        .as_str(),
        "https://example.com/v1/responses?api-version=2025-04-01&timeout=120"
    );
}

#[test]
fn non_loopback_requires_proxy_auth_unless_explicitly_allowed() {
    let listen = "0.0.0.0:8787".parse().expect("socket addr");
    assert!(require_safe_auth_configuration(listen, &ProxyAuth::None, false).is_err());
    assert!(require_safe_auth_configuration(listen, &ProxyAuth::None, true).is_ok());
    assert!(
        require_safe_auth_configuration(listen, &ProxyAuth::Bearer("token".into()), false).is_ok()
    );
}

#[test]
fn loopback_can_run_without_proxy_auth() {
    let listen = "127.0.0.1:8787".parse().expect("socket addr");
    assert_eq!(
        require_safe_auth_configuration(listen, &ProxyAuth::None, false).is_ok(),
        true
    );
}

#[test]
fn args_parse_log_retain_rows() {
    let args = Args::try_parse_from([
        "codex-auth-proxy",
        "--log-db",
        "proxy.sqlite",
        "--log-retain-rows",
        "1000",
    ])
    .expect("parse args");

    assert_eq!(
        args.log_retain_rows,
        Some(LogRetainRowsArg::Rows(
            NonZeroU64::new(1000).expect("non-zero")
        ))
    );
}

#[test]
fn args_parse_unlimited_log_retain_rows() {
    let args = Args::try_parse_from([
        "codex-auth-proxy",
        "--log-db",
        "proxy.sqlite",
        "--log-retain-rows",
        "unlimited",
    ])
    .expect("parse args");

    assert_eq!(args.log_retain_rows, Some(LogRetainRowsArg::Unlimited));
}

#[test]
fn args_reject_zero_log_retain_rows() {
    assert!(
        Args::try_parse_from([
            "codex-auth-proxy",
            "--log-db",
            "proxy.sqlite",
            "--log-retain-rows",
            "0",
        ])
        .is_err()
    );
}

#[test]
fn args_require_log_db_for_log_retain_rows() {
    assert!(Args::try_parse_from(["codex-auth-proxy", "--log-retain-rows", "1000"]).is_err());
}

#[test]
fn args_parse_log_max_body_bytes() {
    let args = Args::try_parse_from([
        "codex-auth-proxy",
        "--log-db",
        "proxy.sqlite",
        "--log-max-body-bytes",
        "1048576",
    ])
    .expect("parse args");

    assert_eq!(
        args.log_max_body_bytes,
        Some(LogMaxBodyBytesArg::Bytes(
            NonZeroU64::new(1_048_576).expect("non-zero")
        ))
    );
}

#[test]
fn args_parse_unlimited_log_max_body_bytes() {
    let args = Args::try_parse_from([
        "codex-auth-proxy",
        "--log-db",
        "proxy.sqlite",
        "--log-max-body-bytes",
        "unlimited",
    ])
    .expect("parse args");

    assert_eq!(args.log_max_body_bytes, Some(LogMaxBodyBytesArg::Unlimited));
}

#[test]
fn args_reject_zero_log_max_body_bytes() {
    assert!(
        Args::try_parse_from([
            "codex-auth-proxy",
            "--log-db",
            "proxy.sqlite",
            "--log-max-body-bytes",
            "0",
        ])
        .is_err()
    );
}

#[test]
fn args_require_log_db_for_log_max_body_bytes() {
    assert!(Args::try_parse_from(["codex-auth-proxy", "--log-max-body-bytes", "1024"]).is_err());
}

#[test]
fn request_log_retention_defaults_to_1000_rows() {
    assert_eq!(
        request_log_retention(None),
        Some(RequestLogRetention::new(
            NonZeroU64::new(1000).expect("non-zero")
        ))
    );
}

#[test]
fn request_log_retention_allows_unlimited() {
    assert_eq!(
        request_log_retention(Some(LogRetainRowsArg::Unlimited)),
        None
    );
}

#[test]
fn request_log_body_limit_defaults_to_1mb() {
    assert_eq!(
        request_log_body_limit(None),
        Some(RequestLogBodyLimit::new(
            NonZeroU64::new(1_048_576).expect("non-zero")
        ))
    );
}

#[test]
fn request_log_body_limit_allows_unlimited() {
    assert_eq!(
        request_log_body_limit(Some(LogMaxBodyBytesArg::Unlimited)),
        None
    );
}

#[test]
fn startup_log_labels_show_log_defaults_when_db_enabled() {
    let db = Path::new("proxy.sqlite");

    assert_eq!(
        log_retention_label(Some(db), /*arg*/ None),
        "1000 completed rows"
    );
    assert_eq!(
        log_body_limit_label(Some(db), /*arg*/ None),
        "1048576 bytes per request/response body"
    );
}

#[test]
fn startup_log_labels_show_disabled_when_db_is_disabled() {
    assert_eq!(
        log_retention_label(/*log_db*/ None, /*arg*/ None),
        "disabled"
    );
    assert_eq!(
        log_body_limit_label(/*log_db*/ None, /*arg*/ None),
        "disabled"
    );
}

#[test]
fn startup_log_labels_show_unlimited_values() {
    let db = Path::new("proxy.sqlite");

    assert_eq!(
        log_retention_label(Some(db), Some(LogRetainRowsArg::Unlimited)),
        "unlimited"
    );
    assert_eq!(
        log_body_limit_label(Some(db), Some(LogMaxBodyBytesArg::Unlimited)),
        "unlimited"
    );
}

#[test]
fn proxy_url_formats_ipv4_and_ipv6_addresses() {
    assert_eq!(
        proxy_url("127.0.0.1:8787".parse().expect("addr"), "/v1"),
        "http://127.0.0.1:8787/v1"
    );
    assert_eq!(
        proxy_url("[::1]:8787".parse().expect("addr"), "/health"),
        "http://[::1]:8787/health"
    );
}
