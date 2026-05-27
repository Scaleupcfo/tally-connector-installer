//! Pairing-token authentication for the local agent.
//!
//! Why this exists even though we have CORS:
//!   - A browser extension running on lekha.ai *could* call the agent
//!     from inside the page's context, bypassing CORS.
//!   - An XSS on lekha.ai (or a compromised third-party script) could
//!     do the same.
//!   - In multi-user shared-laptop scenarios, "is the caller my user?"
//!     needs to be answered by something stronger than "they reached
//!     localhost."
//!
//! The token is generated once on first run, stored in the per-user data
//! directory, and shipped to Lekha AI via a one-time copy-paste pairing
//! flow. After that, every request to a protected endpoint must include
//! `Authorization: Bearer <token>` or the agent returns 401.

use std::path::PathBuf;
use uuid::Uuid;

const TOKEN_FILE: &str = "token.txt";

/// Where the token file lives on disk.
/// Reuses the same per-user data dir as the TLS cert.
fn token_path() -> PathBuf {
    crate::tls::data_dir().join(TOKEN_FILE)
}

/// Load the existing pairing token from disk; create one on first run.
pub fn load_or_generate() -> Result<String, std::io::Error> {
    let path = token_path();
    if path.exists() {
        let token = std::fs::read_to_string(&path)?;
        let trimmed = token.trim().to_string();
        if !trimmed.is_empty() {
            println!("[OK]   reusing pairing token from {}", path.display());
            return Ok(trimmed);
        }
        // File exists but empty -> fall through and regenerate.
    }

    std::fs::create_dir_all(path.parent().expect("token path has parent"))?;
    let token = Uuid::new_v4().to_string();
    std::fs::write(&path, &token)?;
    println!("[OK]   generated new pairing token at {}", path.display());
    Ok(token)
}

/// Constant-time byte comparison.
///
/// Why not just `a == b`? Naive string comparison short-circuits on the
/// first differing byte. An attacker can use the time difference to learn
/// the token byte-by-byte (a "timing attack"). This loops over every byte
/// regardless, so timing leaks nothing about how close the guess was.
pub fn ct_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff: u8 = 0;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}
