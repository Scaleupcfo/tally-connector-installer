//! Lekha Tally Installer — local agent for Tally Prime.
//!
//! Phase 7: runs as a Windows system tray app.
//!   • Main thread = tray icon + Win32 event loop (see `tray`).
//!   • Worker thread = tokio runtime + axum HTTPS server (see `agent`).
//!
//! Why the split: tray icons require a thread with a Win32 message pump,
//! and tokio's `#[tokio::main]` takes over the main thread for itself.
//! So the agent moves to a worker thread and main becomes synchronous.

// Release builds: run as a Windows GUI app, no console window.
// Debug builds: stay as a console app so `cargo run` shows our println output.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod agent;
mod auth;
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

    println!("[OK]   Lekha Tally agent starting...");
    println!();
    println!("       PAIRING TOKEN: {pairing_token}");
    println!("       (Paste this once into the Lekha AI website to pair this PC.)");
    println!("       (Also accessible from tray icon -> Show pairing token.)");
    println!();
    println!("       Endpoints:");
    println!("         GET /health                                    [public]");
    println!("         GET /companies                                 [auth]");
    println!("         GET /ledgers?company=<name>                    [auth]");
    println!("         GET /vouchers?company=<name>&from=<iso>&to=<iso>  [auth]");
    println!();

    // Spawn the HTTPS server in a worker thread.
    agent::spawn(pairing_token);

    // Take over the main thread with the tray icon + event loop.
    // Never returns — Quit menu item calls std::process::exit(0).
    tray::run_event_loop();
}
