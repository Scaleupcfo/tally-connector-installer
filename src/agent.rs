//! The HTTPS agent — axum + TLS + CORS + auth + Tally endpoints.
//!
//! Lifted out of `main.rs` so Phase 7's `main()` can give the main thread
//! over to the Windows event loop while this server runs in a worker thread.
//!
//! Public surface is just `spawn(token)` — fire-and-forget. The HTTPS server
//! lives until the process exits.

use std::sync::Arc;

use axum::{
    Json, Router,
    body::Body,
    extract::{Query, State},
    http::{Method, Request, StatusCode, header::AUTHORIZATION, header::CONTENT_TYPE},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::get,
};
use axum_server::tls_rustls::RustlsConfig;
use serde::Deserialize;
use serde_json::{Value, json};
use tower_http::cors::CorsLayer;

use crate::{auth, tally, tls};

const AGENT_BIND_ADDR: &str = "127.0.0.1:9100";

const ALLOWED_ORIGINS: &[&str] = &[
    "https://lekha.ai",
    "https://www.lekha.ai",
    "http://localhost:3000",
    "http://localhost:5173",
    "http://127.0.0.1:3000",
];

#[derive(Clone)]
struct AppState {
    pairing_token: Arc<String>,
}

// ---------- Query types ------------------------------------------------------

#[derive(Deserialize)]
struct CompanyQuery {
    company: String,
}

#[derive(Deserialize)]
struct VoucherQuery {
    company: String,
    from: String,
    to: String,
}

// ---------- Handlers ---------------------------------------------------------

async fn health() -> Json<Value> {
    Json(json!({
        "ok": true,
        "service": "lekha_tally_installer",
        "version": env!("CARGO_PKG_VERSION"),
    }))
}

async fn companies() -> Response {
    let res = tokio::task::spawn_blocking(tally::list_companies).await;
    finish(res.map(|inner| inner.map(|v| json!({ "ok": true, "companies": v }))))
}

async fn ledgers(Query(q): Query<CompanyQuery>) -> Response {
    let res = tokio::task::spawn_blocking(move || tally::list_ledgers(&q.company)).await;
    finish(res.map(|inner| inner.map(|v| json!({ "ok": true, "ledgers": v }))))
}

async fn vouchers(Query(q): Query<VoucherQuery>) -> Response {
    let res = tokio::task::spawn_blocking(move || {
        tally::list_vouchers(&q.company, &q.from, &q.to)
    })
    .await;
    finish(res.map(|inner| inner.map(|v| json!({ "ok": true, "vouchers": v }))))
}

fn finish(
    res: Result<Result<Value, tally::TallyError>, tokio::task::JoinError>,
) -> Response {
    match res {
        Ok(Ok(payload)) => Json(payload).into_response(),
        Ok(Err(err)) => {
            let status = match err {
                tally::TallyError::PortClosed => StatusCode::SERVICE_UNAVAILABLE,
                tally::TallyError::HttpFailed(_) => StatusCode::BAD_GATEWAY,
                tally::TallyError::BadXml(_) => StatusCode::BAD_GATEWAY,
                tally::TallyError::BadRequest(_) => StatusCode::BAD_REQUEST,
            };
            (
                status,
                Json(json!({ "ok": false, "error": err.to_string() })),
            )
                .into_response()
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
pub fn spawn(pairing_token: String) {
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
            rt.block_on(run(pairing_token));
        })
        .expect("spawn agent thread");
}

async fn run(pairing_token: String) {
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
    };

    let origins: Vec<_> = ALLOWED_ORIGINS
        .iter()
        .map(|s| s.parse().expect("static origin is valid"))
        .collect();
    let cors = CorsLayer::new()
        .allow_origin(origins)
        .allow_methods([Method::GET])
        .allow_headers([AUTHORIZATION, CONTENT_TYPE]);

    let protected = Router::new()
        .route("/companies", get(companies))
        .route("/ledgers", get(ledgers))
        .route("/vouchers", get(vouchers))
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
