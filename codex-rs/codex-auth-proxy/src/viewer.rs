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
            let limit = limit_for_url(&url).unwrap_or(200);
            let rows = runtime.block_on(logger.list_recent(limit))?;
            respond_json(req, &rows)?;
        }
        path => {
            let Some(id) = path.strip_prefix("/api/requests/") else {
                respond_text(req, StatusCode(404), "Not found", "text/plain");
                return Ok(());
            };
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

fn limit_for_url(url: &str) -> Option<i64> {
    let query = url.split_once('?')?.1;
    query.split('&').find_map(|pair| {
        let (name, value) = pair.split_once('=')?;
        if name == "limit" {
            value.parse().ok()
        } else {
            None
        }
    })
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
    fn limit_for_url_reads_limit_query_param() {
        assert_eq!(limit_for_url("/api/requests"), None);
        assert_eq!(limit_for_url("/api/requests?limit=50"), Some(50));
        assert_eq!(limit_for_url("/api/requests?x=1&limit=25"), Some(25));
    }
}
