//! Tally Prime client — forwards XML to Tally's HTTP gateway on localhost.
//!
//! Module layout:
//!   client   — HTTP transport (POST envelope, return sanitized body)
//!   sanitize — strip Tally's malformed control chars from responses

pub mod client;
pub mod sanitize;

pub use client::forward_xml;

#[derive(Debug)]
pub enum TallyError {
    /// Nothing listening on the configured port — Tally probably isn't running.
    PortClosed(u16),
    /// HTTP-level failure (couldn't POST or read the response body).
    HttpFailed(String),
}

impl std::fmt::Display for TallyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PortClosed(port) => write!(
                f,
                "Tally is not reachable on port {port}. Is Tally Prime running?",
            ),
            Self::HttpFailed(s) => write!(f, "HTTP call to Tally failed: {s}"),
        }
    }
}
impl std::error::Error for TallyError {}
