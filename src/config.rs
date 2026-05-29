//! Runtime configuration loaded from `config.toml` in the data directory.
//!
//! On first run, writes a default config file. On subsequent runs, reads it.
//! Users (or a future settings UI) can edit the file to change the Tally port.

use std::path::PathBuf;

const CONFIG_FILE: &str = "config.toml";
const DEFAULT_TALLY_PORT: u16 = 9000;

pub struct Config {
    pub tally_port: u16,
}

fn config_path() -> PathBuf {
    crate::tls::data_dir().join(CONFIG_FILE)
}

fn default_config_contents() -> String {
    format!(
        "# Lekha AI Tally Connector — configuration\n\
         #\n\
         # Tally Prime's XML/HTTP gateway port (default: 9000).\n\
         # Change this if your Tally is configured to use a different port.\n\
         tally_port = {DEFAULT_TALLY_PORT}\n"
    )
}

/// Load config from disk, or create the default file on first run.
pub fn load_or_create() -> Config {
    let path = config_path();

    if path.exists() {
        if let Ok(contents) = std::fs::read_to_string(&path) {
            let port = parse_tally_port(&contents).unwrap_or(DEFAULT_TALLY_PORT);
            println!("[OK]   config loaded from {}", path.display());
            return Config { tally_port: port };
        }
    }

    // Write default config (best-effort — don't fail startup over this)
    let _ = std::fs::write(&path, default_config_contents());
    println!("[OK]   wrote default config to {}", path.display());
    Config {
        tally_port: DEFAULT_TALLY_PORT,
    }
}

/// Minimal TOML parser — just extracts `tally_port = <number>`.
fn parse_tally_port(contents: &str) -> Option<u16> {
    for line in contents.lines() {
        let line = line.trim();
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        if let Some(rest) = line.strip_prefix("tally_port") {
            let rest = rest.trim_start();
            if let Some(value) = rest.strip_prefix('=') {
                if let Ok(port) = value.trim().parse::<u16>() {
                    return Some(port);
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_default_port() {
        assert_eq!(parse_tally_port("tally_port = 9000"), Some(9000));
    }

    #[test]
    fn parses_custom_port() {
        assert_eq!(parse_tally_port("tally_port = 9001"), Some(9001));
    }

    #[test]
    fn ignores_comments() {
        let input = "# tally_port = 1234\ntally_port = 9002\n";
        assert_eq!(parse_tally_port(input), Some(9002));
    }

    #[test]
    fn handles_extra_whitespace() {
        assert_eq!(parse_tally_port("  tally_port  =  9003  "), Some(9003));
    }

    #[test]
    fn returns_none_for_missing_key() {
        assert_eq!(parse_tally_port("other_key = 123"), None);
    }

    #[test]
    fn returns_none_for_invalid_number() {
        assert_eq!(parse_tally_port("tally_port = abc"), None);
    }
}
