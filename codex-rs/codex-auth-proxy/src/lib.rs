use std::fs;
use std::fs::File;
use std::io::Write;
use std::net::IpAddr;
use std::net::SocketAddr;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use clap::Parser;
use clap::ValueEnum;
use codex_config::types::AuthCredentialsStoreMode;
use codex_login::AuthManager;
use codex_model_provider::auth_provider_from_auth;
use codex_model_provider_info::CHATGPT_CODEX_BASE_URL;
use reqwest::Url;
use reqwest::blocking::Client;
use reqwest::header::AUTHORIZATION;
use reqwest::header::HOST;
use reqwest::header::HeaderMap;
use reqwest::header::HeaderValue;
use serde::Serialize;
use tiny_http::Header;
use tiny_http::Method;
use tiny_http::Request;
use tiny_http::Response;
use tiny_http::Server;
use tiny_http::StatusCode;
use tokio::runtime::Runtime;

const DEFAULT_LISTEN_ADDR: &str = "127.0.0.1:0";

/// CLI arguments for the Codex auth backed Responses proxy.
#[derive(Debug, Clone, Parser)]
#[command(
    name = "codex-auth-proxy",
    about = "Codex auth backed Responses API proxy"
)]
pub struct Args {
    /// Address to listen on. Use 0.0.0.0:PORT only with proxy authentication.
    #[arg(long, default_value = DEFAULT_LISTEN_ADDR)]
    pub listen: SocketAddr,

    /// Path to a JSON file to write startup info.
    #[arg(long, value_name = "FILE")]
    pub server_info: Option<PathBuf>,

    /// Enable HTTP shutdown endpoint at GET /shutdown.
    #[arg(long)]
    pub http_shutdown: bool,

    /// Absolute upstream URL for POST /v1/responses.
    #[arg(long, default_value_t = default_responses_url())]
    pub upstream_responses_url: String,

    /// Absolute upstream URL for GET /v1/models.
    #[arg(long, default_value_t = default_models_url())]
    pub upstream_models_url: String,

    /// Environment variable containing the bearer token external clients must send.
    #[arg(long, value_name = "ENV_VAR")]
    pub proxy_token_env: Option<String>,

    /// Allow requests without proxy authentication. Refused for non-loopback listen addresses
    /// unless this flag is explicit.
    #[arg(long)]
    pub allow_unauthenticated: bool,

    /// Codex home directory. Defaults to CODEX_HOME or ~/.codex.
    #[arg(long, value_name = "DIR")]
    pub codex_home: Option<PathBuf>,

    /// Where Codex login credentials are stored.
    #[arg(long, value_enum, default_value_t = AuthStoreArg::File)]
    pub auth_store: AuthStoreArg,

    /// ChatGPT backend base URL used while loading/refreshing Codex auth.
    #[arg(long)]
    pub chatgpt_base_url: Option<String>,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum AuthStoreArg {
    File,
    Keyring,
    Auto,
}

impl std::fmt::Display for AuthStoreArg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::File => f.write_str("file"),
            Self::Keyring => f.write_str("keyring"),
            Self::Auto => f.write_str("auto"),
        }
    }
}

impl From<AuthStoreArg> for AuthCredentialsStoreMode {
    fn from(value: AuthStoreArg) -> Self {
        match value {
            AuthStoreArg::File => Self::File,
            AuthStoreArg::Keyring => Self::Keyring,
            AuthStoreArg::Auto => Self::Auto,
        }
    }
}

#[derive(Serialize)]
struct ServerInfo {
    port: u16,
    pid: u32,
    upstream_responses_url: String,
    upstream_models_url: String,
}

#[derive(Debug, Clone)]
struct ForwardConfig {
    responses_url: Url,
    models_url: Url,
    proxy_auth: ProxyAuth,
}

#[derive(Debug, Clone)]
enum ProxyAuth {
    None,
    Bearer(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RouteMatch {
    route: Route,
    query: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Route {
    Responses,
    Models,
}

pub fn run_main(args: Args) -> Result<()> {
    let proxy_auth = proxy_auth_from_args(&args)?;
    require_safe_auth_configuration(args.listen, &proxy_auth, args.allow_unauthenticated)?;

    let codex_home = match args.codex_home {
        Some(path) => path,
        None => codex_utils_home_dir::find_codex_home()
            .context("resolving CODEX_HOME")?
            .into(),
    };

    let runtime = Arc::new(
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .context("building Tokio runtime")?,
    );
    let auth_manager = runtime.block_on(AuthManager::shared(
        codex_home,
        /*enable_codex_api_key_env*/ false,
        args.auth_store.into(),
        args.chatgpt_base_url.clone(),
    ));

    let config = Arc::new(ForwardConfig {
        responses_url: Url::parse(&args.upstream_responses_url)
            .context("parsing --upstream-responses-url")?,
        models_url: Url::parse(&args.upstream_models_url)
            .context("parsing --upstream-models-url")?,
        proxy_auth,
    });

    let server = Server::http(args.listen).map_err(|err| anyhow!("creating HTTP server: {err}"))?;
    let bound_addr = server
        .server_addr()
        .to_ip()
        .ok_or_else(|| anyhow!("server did not expose a TCP listen address"))?;
    if let Some(path) = args.server_info.as_ref() {
        write_server_info(
            path,
            bound_addr.port(),
            &config.responses_url,
            &config.models_url,
        )?;
    }

    let client = Arc::new(
        Client::builder()
            // Disable reqwest's 30s default so long-lived response streams keep flowing.
            .timeout(None::<Duration>)
            .build()
            .context("building reqwest client")?,
    );

    eprintln!("codex-auth-proxy listening on {bound_addr}");

    let http_shutdown = args.http_shutdown;
    for request in server.incoming_requests() {
        let client = client.clone();
        let config = config.clone();
        let runtime = runtime.clone();
        let auth_manager = auth_manager.clone();
        std::thread::spawn(move || {
            if http_shutdown && request.method() == &Method::Get && request.url() == "/shutdown" {
                let _ = request.respond(Response::new_empty(StatusCode(200)));
                std::process::exit(0);
            }

            if let Err(err) = forward_request(&client, &runtime, &auth_manager, &config, request) {
                eprintln!("forwarding error: {err:#}");
            }
        });
    }

    Err(anyhow!("server stopped unexpectedly"))
}

fn default_responses_url() -> String {
    format!("{}/responses", CHATGPT_CODEX_BASE_URL.trim_end_matches('/'))
}

fn default_models_url() -> String {
    format!("{}/models", CHATGPT_CODEX_BASE_URL.trim_end_matches('/'))
}

fn write_server_info(
    path: &Path,
    port: u16,
    upstream_responses_url: &Url,
    upstream_models_url: &Url,
) -> Result<()> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)?;
    }

    let info = ServerInfo {
        port,
        pid: std::process::id(),
        upstream_responses_url: upstream_responses_url.to_string(),
        upstream_models_url: upstream_models_url.to_string(),
    };
    let mut data = serde_json::to_string(&info)?;
    data.push('\n');
    let mut file = File::create(path)?;
    file.write_all(data.as_bytes())?;
    Ok(())
}

fn proxy_auth_from_args(args: &Args) -> Result<ProxyAuth> {
    let Some(env_var) = args.proxy_token_env.as_ref() else {
        return Ok(ProxyAuth::None);
    };
    let token = std::env::var(env_var)
        .with_context(|| format!("reading proxy token from ${env_var}"))?
        .trim()
        .to_string();
    if token.is_empty() {
        return Err(anyhow!("proxy token from ${env_var} is empty"));
    }
    Ok(ProxyAuth::Bearer(token))
}

fn require_safe_auth_configuration(
    listen: SocketAddr,
    proxy_auth: &ProxyAuth,
    allow_unauthenticated: bool,
) -> Result<()> {
    if matches!(proxy_auth, ProxyAuth::Bearer(_))
        || allow_unauthenticated
        || is_loopback(listen.ip())
    {
        return Ok(());
    }

    Err(anyhow!(
        "refusing unauthenticated non-loopback listener {listen}; pass --proxy-token-env or --allow-unauthenticated"
    ))
}

fn is_loopback(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => ip.is_loopback(),
        IpAddr::V6(ip) => ip.is_loopback(),
    }
}

fn forward_request(
    client: &Client,
    runtime: &Runtime,
    auth_manager: &AuthManager,
    config: &ForwardConfig,
    mut req: Request,
) -> Result<()> {
    if !authorize_proxy_request(config.proxy_auth.clone(), &req) {
        let response = Response::from_string("Unauthorized").with_status_code(StatusCode(401));
        let _ = req.respond(response);
        return Ok(());
    }

    let method = req.method().clone();
    let url_path = req.url().to_string();
    if method == Method::Get && path_for_url(&url_path) == "/health" {
        respond_health(req);
        return Ok(());
    }

    let route_match = route_for(&method, &url_path);
    let Some(route_match) = route_match else {
        let response = Response::new_empty(StatusCode(403));
        let _ = req.respond(response);
        return Ok(());
    };

    let mut body = Vec::new();
    req.as_reader().read_to_end(&mut body)?;

    let mut upstream_response = send_upstream(
        client,
        runtime,
        auth_manager,
        config,
        &route_match,
        &req,
        body.clone(),
    )?;

    if upstream_response.status() == reqwest::StatusCode::UNAUTHORIZED {
        runtime
            .block_on(auth_manager.refresh_token())
            .context("refreshing Codex auth after upstream 401")?;
        upstream_response = send_upstream(
            client,
            runtime,
            auth_manager,
            config,
            &route_match,
            &req,
            body,
        )?;
    }

    respond_with_upstream(req, upstream_response);
    Ok(())
}

fn authorize_proxy_request(proxy_auth: ProxyAuth, req: &Request) -> bool {
    let ProxyAuth::Bearer(expected_token) = proxy_auth else {
        return true;
    };

    req.headers().iter().any(|header| {
        header.field.equiv("Authorization")
            && header.value.as_str() == format!("Bearer {expected_token}")
    })
}

fn route_for(method: &Method, url: &str) -> Option<RouteMatch> {
    let path = path_for_url(url);
    let query = query_for_url(url);

    let route = match (method, path) {
        (Method::Post, "/v1/responses") => Route::Responses,
        (Method::Get, "/v1/models") => Route::Models,
        _ => return None,
    };

    Some(RouteMatch { route, query })
}

fn path_for_url(url: &str) -> &str {
    url.split_once('?').map_or(url, |(path, _query)| path)
}

fn query_for_url(url: &str) -> Option<String> {
    url.split_once('?').map(|(_path, query)| query.to_string())
}

fn send_upstream(
    client: &Client,
    runtime: &Runtime,
    auth_manager: &AuthManager,
    config: &ForwardConfig,
    route_match: &RouteMatch,
    req: &Request,
    body: Vec<u8>,
) -> Result<reqwest::blocking::Response> {
    let upstream_url = upstream_url_for(config, route_match);

    let headers = build_upstream_headers(runtime, auth_manager, &upstream_url, req)?;
    let builder = match route_match.route {
        Route::Models => client.get(upstream_url),
        Route::Responses => client.post(upstream_url).body(body),
    };

    builder
        .headers(headers)
        .send()
        .context("forwarding request to upstream")
}

fn upstream_url_for(config: &ForwardConfig, route_match: &RouteMatch) -> Url {
    let mut upstream_url = match route_match.route {
        Route::Responses => config.responses_url.clone(),
        Route::Models => config.models_url.clone(),
    };
    let Some(incoming_query) = route_match.query.as_deref() else {
        return upstream_url;
    };
    if incoming_query.is_empty() {
        return upstream_url;
    }

    let next_query = match upstream_url.query() {
        Some(existing_query) if !existing_query.is_empty() => {
            format!("{existing_query}&{incoming_query}")
        }
        _ => incoming_query.to_string(),
    };
    upstream_url.set_query(Some(&next_query));
    upstream_url
}

fn build_upstream_headers(
    runtime: &Runtime,
    auth_manager: &AuthManager,
    upstream_url: &Url,
    req: &Request,
) -> Result<HeaderMap> {
    let auth_headers = current_codex_auth_headers(runtime, auth_manager)?;
    let host_header = host_header_for_url(upstream_url)?;

    let mut headers = HeaderMap::new();
    for header in req.headers() {
        let name = header.field.as_str().as_str();
        if name.eq_ignore_ascii_case("authorization") || name.eq_ignore_ascii_case("host") {
            continue;
        }

        let Ok(header_name) = reqwest::header::HeaderName::from_bytes(name.as_bytes()) else {
            continue;
        };
        if let Ok(value) = HeaderValue::from_bytes(header.value.as_bytes()) {
            headers.append(header_name, value);
        }
    }

    for (name, value) in auth_headers {
        if let Some(name) = name {
            let mut value = value;
            if name == AUTHORIZATION {
                value.set_sensitive(true);
            }
            headers.insert(name, value);
        }
    }
    headers.insert(HOST, host_header);
    Ok(headers)
}

fn current_codex_auth_headers(runtime: &Runtime, auth_manager: &AuthManager) -> Result<HeaderMap> {
    let auth = runtime
        .block_on(auth_manager.auth())
        .ok_or_else(|| anyhow!("Codex auth not found; run `codex login` on the proxy host"))?;
    if auth.is_api_key_auth() {
        return Err(anyhow!(
            "Codex auth proxy requires ChatGPT/Codex login auth, but current auth is API key auth"
        ));
    }

    let auth_provider = auth_provider_from_auth(&auth);
    let mut headers = HeaderMap::new();
    auth_provider.add_auth_headers(&mut headers);
    if !headers.contains_key(AUTHORIZATION) {
        return Err(anyhow!(
            "Codex auth did not produce an Authorization header"
        ));
    }
    Ok(headers)
}

fn host_header_for_url(url: &Url) -> Result<HeaderValue> {
    let host = match (url.host_str(), url.port()) {
        (Some(host), Some(port)) => format!("{host}:{port}"),
        (Some(host), None) => host.to_string(),
        _ => return Err(anyhow!("upstream URL must include a host")),
    };
    HeaderValue::from_str(&host).context("constructing Host header")
}

fn respond_with_upstream(req: Request, upstream_response: reqwest::blocking::Response) {
    let status = upstream_response.status();
    let mut response_headers = Vec::new();
    for (name, value) in upstream_response.headers() {
        if matches!(
            name.as_str(),
            "content-length" | "transfer-encoding" | "connection" | "trailer" | "upgrade"
        ) {
            continue;
        }

        if let Ok(header) = Header::from_bytes(name.as_str().as_bytes(), value.as_bytes()) {
            response_headers.push(header);
        }
    }

    let content_length = upstream_response.content_length().and_then(|len| {
        if len <= usize::MAX as u64 {
            Some(len as usize)
        } else {
            None
        }
    });

    let response = Response::new(
        StatusCode(status.as_u16()),
        response_headers,
        Box::new(upstream_response),
        content_length,
        None,
    );

    let _ = req.respond(response);
}

fn respond_health(req: Request) {
    let mut response =
        Response::from_string("{\"status\":\"ok\"}\n").with_status_code(StatusCode(200));
    if let Ok(header) = Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]) {
        response = response.with_header(header);
    }
    let _ = req.respond(response);
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

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
            require_safe_auth_configuration(listen, &ProxyAuth::Bearer("token".into()), false)
                .is_ok()
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
}
