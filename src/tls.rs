//! Self-signed TLS cert management for the local agent.
//!
//! Why we need this at all: the Lekha AI website is HTTPS, and browsers
//! refuse to call `http://` URLs from an HTTPS page (mixed-content block).
//! So this agent must serve HTTPS. Real CAs can't issue certs for
//! `localhost`, so we mint our own.
//!
//! Phase 4 just generates and uses the cert. Phase 8 (the MSI installer)
//! will additionally add it to the Windows trust store so browsers stop
//! warning about it.

use rcgen::{CertifiedKey, generate_simple_self_signed};
use std::path::PathBuf;

const CERT_FILE: &str = "cert.pem";
const KEY_FILE: &str = "key.pem";

/// Per-user data directory for the agent.
/// Windows: `%LOCALAPPDATA%\LekhaAI\TallyConnector\`
pub fn data_dir() -> PathBuf {
    let base = std::env::var("LOCALAPPDATA")
        .or_else(|_| std::env::var("APPDATA"))
        .unwrap_or_else(|_| ".".to_string());
    PathBuf::from(base).join("LekhaAI").join("TallyConnector")
}

/// Paths to the cert and key files on disk.
#[derive(Debug)]
pub struct CertPaths {
    pub cert: PathBuf,
    pub key: PathBuf,
}

/// Load the cert+key from disk; generate them on first run.
pub fn load_or_generate() -> Result<CertPaths, Box<dyn std::error::Error>> {
    let dir = data_dir();
    std::fs::create_dir_all(&dir)?;

    let cert_path = dir.join(CERT_FILE);
    let key_path = dir.join(KEY_FILE);

    if cert_path.exists() && key_path.exists() {
        println!("[OK]   reusing TLS cert from {}", dir.display());
    } else {
        println!("[OK]   generating self-signed TLS cert in {}", dir.display());
        // SANs (Subject Alternative Names): the names this cert is valid for.
        // We include both "localhost" and the loopback IP so callers using
        // either form get a valid cert match.
        let CertifiedKey { cert, key_pair } = generate_simple_self_signed(vec![
            "localhost".to_string(),
            "127.0.0.1".to_string(),
        ])?;
        std::fs::write(&cert_path, cert.pem())?;
        std::fs::write(&key_path, key_pair.serialize_pem())?;
    }

    Ok(CertPaths {
        cert: cert_path,
        key: key_path,
    })
}
