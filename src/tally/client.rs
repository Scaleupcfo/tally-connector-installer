//! HTTP transport to Tally's XML/HTTP gateway on localhost.
//!
//! The single `forward_xml()` function is the entire public API of the tally
//! module. It forwards raw XML to Tally, sanitizes the response, and returns it.

use std::io::Read;
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

use super::TallyError;
use super::sanitize::sanitize_xml;

const TALLY_HOST: &str = "localhost";
const TIMEOUT_SECS: u64 = 900; // 15 min — large export pulls are slow

/// Forward raw XML to Tally's HTTP gateway, sanitize the response, return it.
pub fn forward_xml(xml_body: &str, port: u16) -> Result<String, TallyError> {
    if !port_is_open(port) {
        return Err(TallyError::PortClosed(port));
    }
    let url = format!("http://{TALLY_HOST}:{port}");
    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(TIMEOUT_SECS))
        .build();
    // Read via into_reader() rather than into_string(): the latter caps bodies
    // at 10 MB in ureq 2.x and errors past it, which 502s large voucher pulls
    // (a month of one company can exceed 20 MB). into_reader() is unbounded.
    let mut reader = agent
        .post(&url)
        .set("Content-Type", "text/xml")
        .send_string(xml_body)
        .map_err(|e| TallyError::HttpFailed(e.to_string()))?
        .into_reader();
    let mut bytes = Vec::new();
    reader
        .read_to_end(&mut bytes)
        .map_err(|e| TallyError::HttpFailed(e.to_string()))?;
    let raw = String::from_utf8_lossy(&bytes).into_owned();
    Ok(sanitize_xml(&raw))
}

fn port_is_open(port: u16) -> bool {
    let addr = format!("{TALLY_HOST}:{port}");
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
