//! The HTTPS agent — axum + TLS + CORS + auth + Tally passthrough.
//!
//! Public surface is just `spawn(token)` — fire-and-forget. The HTTPS server
//! lives until the process exits.
//!
//! Single endpoint: `POST /tally` forwards raw XML to Tally on localhost:9000
//! and returns the (sanitized) XML response. The proxy never inspects, parses,
//! or modifies the XML content.

use std::sync::Arc;
use std::time::Duration;

use axum::{
    Json, Router,
    body::{Body, Bytes},
    extract::State,
    http::{Method, Request, StatusCode, header::AUTHORIZATION, header::CONTENT_TYPE},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use axum::extract::DefaultBodyLimit;
use axum_server::tls_rustls::RustlsConfig;
use serde_json::json;
use tower_http::cors::{AllowOrigin, CorsLayer};

use crate::{auth, tally, tls};

const AGENT_BIND_ADDR: &str = "127.0.0.1:9100";
const MAX_REQUEST_BODY: usize = 10 * 1024 * 1024; // 10 MB

const ALLOWED_ORIGINS: &[&str] = &[
    "https://lekha.ai",
    "https://www.lekha.ai",
    "https://lekhaai.app",
    "https://www.lekhaai.app",
    "http://localhost:3000",
    "http://localhost:5173",
    "http://127.0.0.1:3000",
];

#[derive(Clone)]
struct AppState {
    pairing_token: Arc<String>,
    tally_port: u16,
}

// ---------- Handlers ---------------------------------------------------------

async fn health() -> Json<serde_json::Value> {
    Json(json!({
        "ok": true,
        "service": "lekha_tally_proxy",
        "version": env!("CARGO_PKG_VERSION"),
    }))
}

async fn tally_passthrough(
    State(state): State<AppState>,
    uri: axum::http::Uri,
    headers: axum::http::HeaderMap,
    body: Bytes,
) -> Response {
    let content_type = headers
        .get(CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if !content_type.starts_with("text/xml") && !content_type.starts_with("application/xml") {
        return (
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            Json(json!({
                "ok": false,
                "error": "Content-Type must be text/xml or application/xml",
            })),
        )
            .into_response();
    }

    if body.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "ok": false,
                "error": "request body must not be empty",
            })),
        )
            .into_response();
    }

    let xml_body = match std::str::from_utf8(&body) {
        Ok(s) => s.to_string(),
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "ok": false,
                    "error": "request body must be valid UTF-8 XML",
                })),
            )
                .into_response();
        }
    };

    // Optional ?port=NNNN override; falls back to config default.
    let port = parse_port_param(&uri).unwrap_or(state.tally_port);
    let res = tokio::task::spawn_blocking(move || tally::forward_xml(&xml_body, port)).await;

    match res {
        Ok(Ok(xml_response)) => Response::builder()
            .status(StatusCode::OK)
            .header(CONTENT_TYPE, "text/xml; charset=utf-8")
            .body(Body::from(xml_response))
            .unwrap(),
        Ok(Err(err)) => {
            let status = match &err {
                tally::TallyError::PortClosed(_) => StatusCode::SERVICE_UNAVAILABLE,
                tally::TallyError::HttpFailed(_) => StatusCode::BAD_GATEWAY,
            };
            (status, Json(json!({ "ok": false, "error": err.to_string() }))).into_response()
        }
        Err(join_err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "ok": false,
                "error": format!("internal error: {join_err}"),
            })),
        )
            .into_response(),
    }
}

/// Extract `port` from query string: `/tally?port=9001` -> Some(9001)
fn parse_port_param(uri: &axum::http::Uri) -> Option<u16> {
    uri.query()?
        .split('&')
        .find_map(|pair| {
            let (key, value) = pair.split_once('=')?;
            if key == "port" { value.parse().ok() } else { None }
        })
}

// ---------- Auth middleware --------------------------------------------------

async fn require_auth(
    State(state): State<AppState>,
    req: Request<Body>,
    next: Next,
) -> Response {
    let ok = req
        .headers()
        .get(AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .map(|t| auth::ct_eq(t.as_bytes(), state.pairing_token.as_bytes()))
        .unwrap_or(false);

    if !ok {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({
                "ok": false,
                "error": "missing or invalid Authorization: Bearer <token> header",
            })),
        )
            .into_response();
    }
    next.run(req).await
}

// ---------- Spawn ------------------------------------------------------------

/// Start the HTTPS agent on a worker thread. Returns immediately.
/// The thread owns its own tokio runtime; the server runs until the process exits.
pub fn spawn(pairing_token: String, tally_port: u16) {
    std::thread::Builder::new()
        .name("lekha-agent".into())
        .spawn(move || {
            let rt = match tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
            {
                Ok(rt) => rt,
                Err(e) => {
                    eprintln!("[FATAL] could not build tokio runtime: {e}");
                    std::process::exit(1);
                }
            };
            rt.block_on(run(pairing_token, tally_port));
        })
        .expect("spawn agent thread");
}

async fn run(pairing_token: String, tally_port: u16) {
    let cert_paths = match tls::load_or_generate() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("[FATAL] cert setup: {e}");
            std::process::exit(1);
        }
    };
    let tls_config = match RustlsConfig::from_pem_file(&cert_paths.cert, &cert_paths.key).await {
        Ok(c) => c,
        Err(e) => {
            eprintln!("[FATAL] load TLS config: {e}");
            std::process::exit(1);
        }
    };

    let state = AppState {
        pairing_token: Arc::new(pairing_token),
        tally_port,
    };

    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::predicate(|origin, _parts| {
            let Ok(s) = origin.to_str() else { return false };
            ALLOWED_ORIGINS.iter().any(|&allowed| allowed == s)
        }))
        .allow_methods([Method::GET, Method::POST])
        .allow_headers([AUTHORIZATION, CONTENT_TYPE])
        .max_age(Duration::from_secs(3600));

    let protected = Router::new()
        .route("/tally", post(tally_passthrough))
        .layer(DefaultBodyLimit::max(MAX_REQUEST_BODY))
        .layer(middleware::from_fn_with_state(state.clone(), require_auth));

    let public = Router::new().route("/health", get(health));

    let app = Router::new()
        .merge(public)
        .merge(protected)
        .with_state(state)
        .layer(cors);

    let addr: std::net::SocketAddr = AGENT_BIND_ADDR.parse().expect("valid addr");
    println!("[OK]   HTTPS agent listening on https://{AGENT_BIND_ADDR}");

    if let Err(e) = axum_server::bind_rustls(addr, tls_config)
        .serve(app.into_make_service())
        .await
    {
        eprintln!("[FATAL] server stopped: {e}");
        std::process::exit(1);
    }
}
