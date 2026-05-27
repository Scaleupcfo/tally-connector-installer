//! Tally Prime client — talks to Tally's XML/HTTP gateway on localhost:9000.
//!
//! Module layout (each submodule is `src/tally/<name>.rs`):
//!   client     — HTTP transport (POST envelope, return body)
//!   sanitize   — strip Tally's malformed control chars from responses
//!   dates      — YYYYMMDD <-> YYYY-MM-DD conversion
//!   companies  — list loaded companies with their books period
//!   ledgers    — list master ledgers for one company
//!   vouchers   — list vouchers for one company in a date range

pub mod client;
pub mod companies;
pub mod dates;
pub mod ledgers;
pub mod sanitize;
pub mod vouchers;

// Re-export the public API so main.rs can say `tally::list_vouchers(...)`
// instead of `tally::vouchers::list_vouchers(...)`.
pub use companies::{Company, list_companies};
pub use ledgers::{Amount, Ledger, list_ledgers};
pub use vouchers::{LedgerEntry, Voucher, list_vouchers};

/// All the ways talking to Tally can fail.
/// Shared across submodules.
#[derive(Debug)]
pub enum TallyError {
    /// Nothing listening on port 9000 — Tally probably isn't running.
    PortClosed,
    /// HTTP-level failure (couldn't POST or read the response body).
    HttpFailed(String),
    /// Tally returned XML the parser couldn't make sense of.
    BadXml(String),
    /// The agent's caller asked for something invalid (e.g. bad date format).
    BadRequest(String),
}

impl std::fmt::Display for TallyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PortClosed => write!(
                f,
                "Tally is not reachable on port {}. Is Tally Prime running?",
                client::TALLY_PORT,
            ),
            Self::HttpFailed(s) => write!(f, "HTTP call to Tally failed: {s}"),
            Self::BadXml(s) => write!(f, "Tally returned malformed XML: {s}"),
            Self::BadRequest(s) => write!(f, "{s}"),
        }
    }
}
impl std::error::Error for TallyError {}
