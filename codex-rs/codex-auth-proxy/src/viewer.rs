use std::net::IpAddr;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use tiny_http::Header;
use tiny_http::Method;
use tiny_http::Request;
use tiny_http::Response;
use tiny_http::Server;
use tiny_http::StatusCode;
use tokio::runtime::Runtime;

use crate::request_log::RequestLogFilter;
use crate::request_log::RequestLogListQuery;
use crate::request_log::RequestLogger;
use crate::viewer_html;

const DEFAULT_VIEWER_LISTEN_ADDR: &str = "127.0.0.1:8788";

#[derive(Debug, Clone, clap::Args)]
pub struct ViewerArgs {
    /// SQLite request log database to inspect.
    #[arg(long, value_name = "FILE")]
    pub db: PathBuf,

    /// Address to listen on. The viewer only accepts loopback addresses.
    #[arg(long, default_value = DEFAULT_VIEWER_LISTEN_ADDR)]
    pub listen: SocketAddr,
}

pub(crate) fn run_viewer(args: ViewerArgs) -> Result<()> {
    require_loopback_listener(args.listen)?;

    let runtime = Arc::new(
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .context("building Tokio runtime")?,
    );
    let logger = runtime
        .block_on(RequestLogger::open(&args.db))
        .context("opening request log DB")?;

    let server =
        Server::http(args.listen).map_err(|err| anyhow!("creating viewer HTTP server: {err}"))?;
    let bound_addr = server
        .server_addr()
        .to_ip()
        .ok_or_else(|| anyhow!("viewer did not expose a TCP listen address"))?;
    eprintln!("codex-auth-proxy viewer listening on http://{bound_addr}");

    for request in server.incoming_requests() {
        let runtime = runtime.clone();
        let logger = logger.clone();
        std::thread::spawn(move || {
            if let Err(err) = handle_viewer_request(&runtime, &logger, request) {
                eprintln!("viewer request error: {err:#}");
            }
        });
    }

    Err(anyhow!("viewer stopped unexpectedly"))
}

fn require_loopback_listener(listen: SocketAddr) -> Result<()> {
    let is_loopback = match listen.ip() {
        IpAddr::V4(ip) => ip.is_loopback(),
        IpAddr::V6(ip) => ip.is_loopback(),
    };
    if is_loopback {
        return Ok(());
    }

    Err(anyhow!(
        "refusing non-loopback viewer listener {listen}; the SQLite log can contain sensitive data"
    ))
}

fn handle_viewer_request(runtime: &Runtime, logger: &RequestLogger, req: Request) -> Result<()> {
    if req.method() != &Method::Get {
        respond_text(req, StatusCode(405), "Method not allowed", "text/plain");
        return Ok(());
    }

    let url = req.url().to_string();
    let path = path_for_url(&url);
    match path {
        "/" => {
            let html = viewer_html::html();
            respond_text(req, StatusCode(200), &html, "text/html; charset=utf-8");
        }
        "/api/requests" => {
            let query = list_query_for_url(&url);
            let rows = runtime.block_on(logger.list_recent_matching(query))?;
            respond_json(req, &rows)?;
        }
        path => {
            let Some(id) = path.strip_prefix("/api/requests/") else {
                respond_text(req, StatusCode(404), "Not found", "text/plain");
                return Ok(());
            };
            if let Some(id) = id.strip_suffix("/flow") {
                let rows = runtime.block_on(logger.flow_around(id))?;
                respond_json(req, &rows)?;
                return Ok(());
            }
            match runtime.block_on(logger.get_detail(id))? {
                Some(detail) => respond_json(req, &detail)?,
                None => respond_text(req, StatusCode(404), "Not found", "text/plain"),
            }
        }
    }

    Ok(())
}

fn respond_json(req: Request, value: &impl serde::Serialize) -> Result<()> {
    let data = serde_json::to_string(value).context("serializing viewer JSON response")?;
    respond_text(req, StatusCode(200), &data, "application/json");
    Ok(())
}

fn respond_text(req: Request, status: StatusCode, body: &str, content_type: &str) {
    let mut response = Response::from_string(body.to_string()).with_status_code(status);
    if let Ok(header) = Header::from_bytes(&b"Content-Type"[..], content_type.as_bytes()) {
        response = response.with_header(header);
    }
    let _ = req.respond(response);
}

fn path_for_url(url: &str) -> &str {
    url.split_once('?').map_or(url, |(path, _query)| path)
}

fn list_query_for_url(url: &str) -> RequestLogListQuery {
    let mut query = RequestLogListQuery::default();
    for (name, value) in query_params_for_url(url) {
        match name.as_str() {
            "limit" => {
                if let Ok(limit) = value.parse() {
                    query.limit = limit;
                }
            }
            "filter" => query.filter = request_filter_from_str(&value),
            "q" | "search" if !value.trim().is_empty() => {
                query.search = Some(value);
            }
            _ => {}
        }
    }
    query
}

fn request_filter_from_str(value: &str) -> RequestLogFilter {
    match value {
        "errors" => RequestLogFilter::Errors,
        "slow" => RequestLogFilter::Slow,
        "tokens" => RequestLogFilter::HighTokens,
        "truncated" => RequestLogFilter::Truncated,
        _ => RequestLogFilter::All,
    }
}

fn query_params_for_url(url: &str) -> Vec<(String, String)> {
    let Some(query) = url.split_once('?').map(|(_path, query)| query) else {
        return Vec::new();
    };
    query
        .split('&')
        .filter_map(|pair| {
            let (name, value) = pair.split_once('=')?;
            Some((decode_query_component(name), decode_query_component(value)))
        })
        .collect()
}

fn decode_query_component(value: &str) -> String {
    let mut bytes = Vec::with_capacity(value.len());
    let mut chars = value.as_bytes().iter().copied();
    while let Some(byte) = chars.next() {
        if byte == b'+' {
            bytes.push(b' ');
        } else if byte == b'%' {
            let first = chars.next();
            let second = chars.next();
            if let (Some(first), Some(second)) = (first, second)
                && let Some(decoded) = hex_pair(first, second)
            {
                bytes.push(decoded);
                continue;
            }
            bytes.push(byte);
            if let Some(first) = first {
                bytes.push(first);
            }
            if let Some(second) = second {
                bytes.push(second);
            }
        } else {
            bytes.push(byte);
        }
    }
    String::from_utf8_lossy(&bytes).into_owned()
}

fn hex_pair(first: u8, second: u8) -> Option<u8> {
    let high = hex_value(first)?;
    let low = hex_value(second)?;
    Some((high << 4) | low)
}

fn hex_value(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn viewer_rejects_non_loopback_listeners() {
        assert!(require_loopback_listener("127.0.0.1:8788".parse().expect("addr")).is_ok());
        assert!(require_loopback_listener("[::1]:8788".parse().expect("addr")).is_ok());
        assert!(require_loopback_listener("0.0.0.0:8788".parse().expect("addr")).is_err());
    }

    #[test]
    fn list_query_for_url_reads_query_params() {
        assert_eq!(
            list_query_for_url("/api/requests"),
            RequestLogListQuery::default()
        );
        assert_eq!(
            list_query_for_url("/api/requests?limit=50&filter=errors&q=hello+world"),
            RequestLogListQuery {
                limit: 50,
                filter: RequestLogFilter::Errors,
                search: Some("hello world".to_string()),
            }
        );
        assert_eq!(
            list_query_for_url("/api/requests?x=1&limit=25&filter=tokens&q=foo%2Fbar"),
            RequestLogListQuery {
                limit: 25,
                filter: RequestLogFilter::HighTokens,
                search: Some("foo/bar".to_string()),
            }
        );
    }
}
