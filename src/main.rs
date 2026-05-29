//! Lekha AI Tally Connector — local CORS proxy for Tally Prime.
//!
//! Runs as a Windows system tray app.
//!   - Main thread = tray icon + Win32 event loop (see `tray`).
//!   - Worker thread = tokio runtime + axum HTTPS server (see `agent`).
//!
//! The proxy forwards raw XML between the Lekha AI web app and Tally's
//! XML/HTTP gateway on localhost. It never inspects the XML content.

// Release builds: run as a Windows GUI app, no console window.
// Debug builds: stay as a console app so `cargo run` shows our println output.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod agent;
mod auth;
mod config;
mod tally;
mod tls;
mod tray;

fn main() {
    // rustls 0.23 requires explicit crypto-provider install before any TLS.
    let _ = rustls::crypto::ring::default_provider().install_default();

    // Pairing token — load from disk or create on first run.
    let pairing_token = match auth::load_or_generate() {
        Ok(t) => t,
        Err(e) => {
            eprintln!("[FATAL] pairing token setup: {e}");
            std::process::exit(1);
        }
    };

    // Configuration — load from config.toml or create default.
    let config = config::load_or_create();

    println!("[OK]   Lekha AI Tally Connector starting...");
    println!();
    println!("       PAIRING TOKEN: {pairing_token}");
    println!("       (Paste this once into the Lekha AI website to pair this PC.)");
    println!("       (Also accessible from tray icon -> Show pairing token.)");
    println!();
    println!("       Tally port: {}", config.tally_port);
    println!();
    println!("       Endpoints:");
    println!("         GET  /health                    [public]");
    println!("         POST /tally  (XML passthrough)  [auth]");
    println!();

    // Spawn the HTTPS server in a worker thread.
    agent::spawn(pairing_token, config.tally_port);

    // Take over the main thread with the tray icon + event loop.
    // Never returns — Quit menu item calls std::process::exit(0).
    tray::run_event_loop();
}
