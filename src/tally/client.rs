//! HTTP transport to Tally's XML/HTTP gateway on `localhost:9000`.
//!
//! Every submodule that talks to Tally (companies, vouchers, ledgers) goes
//! through this one `post_xml()` function. That gives us a single place to
//! add retries, timeouts, request logging, etc. later.

use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

use super::TallyError;
use super::sanitize::sanitize_xml;

pub const TALLY_HOST: &str = "localhost";
pub const TALLY_PORT: u16 = 9000;
const TIMEOUT_SECS: u64 = 900; // 15 min — voucher pulls on big companies are slow

/// POST an XML envelope to Tally, sanitize the response, return the body.
pub fn post_xml(envelope: &str) -> Result<String, TallyError> {
    if !port_is_open() {
        return Err(TallyError::PortClosed);
    }
    let url = format!("http://{TALLY_HOST}:{TALLY_PORT}");
    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(TIMEOUT_SECS))
        .build();
    let raw = agent
        .post(&url)
        .set("Content-Type", "text/xml")
        .send_string(envelope)
        .map_err(|e| TallyError::HttpFailed(e.to_string()))?
        .into_string()
        .map_err(|e| TallyError::HttpFailed(e.to_string()))?;
    Ok(sanitize_xml(&raw))
}

/// TCP-level check: is anything listening on `localhost:9000`?
/// We do this before each request so we can return a friendly "is Tally
/// running?" error instead of ureq's generic "connection refused".
pub fn port_is_open() -> bool {
    let addr = format!("{TALLY_HOST}:{TALLY_PORT}");
    let addrs = match addr.to_socket_addrs() {
        Ok(it) => it,
        Err(_) => return false,
    };
    for sa in addrs {
        if TcpStream::connect_timeout(&sa, Duration::from_secs(5)).is_ok() {
            return true;
        }
    }
    false
}
