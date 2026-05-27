//! Date helpers for the Tally <-> agent boundary.
//!
//! Tally speaks `YYYYMMDD` (no separators).
//! The agent speaks `YYYY-MM-DD` (ISO 8601) at its HTTP boundary.
//! These two functions are the entire conversion layer.

/// `20240501` -> `Some("2024-05-01")`. Invalid input -> `None`.
pub fn from_yyyymmdd(s: &str) -> Option<String> {
    let s = s.trim();
    if s.len() < 8 || !s[..8].chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    Some(format!("{}-{}-{}", &s[0..4], &s[4..6], &s[6..8]))
}

/// `2024-05-01` -> `Some("20240501")`. Invalid input -> `None`.
/// Used when we have to inject a date into a Tally XML request.
pub fn to_yyyymmdd(iso: &str) -> Option<String> {
    let s = iso.trim();
    let parts: Vec<&str> = s.split('-').collect();
    if parts.len() != 3 {
        return None;
    }
    let (y, m, d) = (parts[0], parts[1], parts[2]);
    if y.len() != 4 || m.len() != 2 || d.len() != 2 {
        return None;
    }
    if !y.chars().all(|c| c.is_ascii_digit())
        || !m.chars().all(|c| c.is_ascii_digit())
        || !d.chars().all(|c| c.is_ascii_digit())
    {
        return None;
    }
    Some(format!("{y}{m}{d}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_yyyymmdd_basic() {
        assert_eq!(from_yyyymmdd("20240501").as_deref(), Some("2024-05-01"));
        assert_eq!(from_yyyymmdd("19990101").as_deref(), Some("1999-01-01"));
    }

    #[test]
    fn from_yyyymmdd_rejects_bad() {
        assert_eq!(from_yyyymmdd(""), None);
        assert_eq!(from_yyyymmdd("not-a-date"), None);
        assert_eq!(from_yyyymmdd("2024"), None);
    }

    #[test]
    fn to_yyyymmdd_basic() {
        assert_eq!(to_yyyymmdd("2024-05-01").as_deref(), Some("20240501"));
        assert_eq!(to_yyyymmdd("1999-01-01").as_deref(), Some("19990101"));
    }

    #[test]
    fn to_yyyymmdd_rejects_bad() {
        assert_eq!(to_yyyymmdd(""), None);
        assert_eq!(to_yyyymmdd("2024/05/01"), None);
        assert_eq!(to_yyyymmdd("24-05-01"), None);
        assert_eq!(to_yyyymmdd("2024-5-1"), None);
    }
}
