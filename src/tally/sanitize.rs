//! Sanitize Tally's malformed XML before handing it to a parser.
//!
//! Tally's XML/HTTP gateway occasionally emits two kinds of garbage that
//! make standard parsers refuse the document:
//!
//! 1. Raw bytes outside the XML 1.0 valid-character set (e.g. NUL).
//! 2. Numeric character references like `&#1;` or `&#x0;` that point
//!    to invalid codepoints.
//!
//! We strip both. Lifted directly from `sanitize_xml()` in
//! tally-integration/fetch_tally_data.py.

/// XML 1.0 spec: which Unicode codepoints are allowed in a document.
fn valid_xml_char(c: u32) -> bool {
    matches!(c, 0x09 | 0x0A | 0x0D)
        || (0x20..=0xD7FF).contains(&c)
        || (0xE000..=0xFFFD).contains(&c)
        || (0x10000..=0x10FFFF).contains(&c)
}

/// Strip raw invalid chars AND numeric references to invalid chars.
pub fn sanitize_xml(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();

    while let Some(c) = chars.next() {
        // 1) Filter raw chars outside the XML 1.0 valid range.
        if !valid_xml_char(c as u32) {
            continue;
        }

        // 2) For numeric character refs (&#N; or &#xN;), look up the codepoint
        //    and drop the whole reference if it would resolve to an invalid char.
        if c == '&' && chars.peek() == Some(&'#') {
            chars.next(); // consume '#'
            let mut num_str = String::new();
            let mut closed = false;
            for nc in chars.by_ref() {
                if nc == ';' {
                    closed = true;
                    break;
                }
                num_str.push(nc);
            }
            if !closed {
                continue; // malformed ref — drop it
            }
            let (radix, digits) = if let Some(rest) = num_str.strip_prefix(['x', 'X']) {
                (16, rest)
            } else {
                (10, num_str.as_str())
            };
            if let Ok(cp) = u32::from_str_radix(digits, radix) {
                if valid_xml_char(cp) {
                    // Keep the original reference exactly as it was.
                    out.push('&');
                    out.push('#');
                    if radix == 16 {
                        out.push('x');
                    }
                    out.push_str(digits);
                    out.push(';');
                }
                // else: drop the entire reference.
            }
            // un-parseable number -> drop.
            continue;
        }

        out.push(c);
    }

    out
}

/// XML-escape a string for safe interpolation into an element body or attribute.
pub fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_raw_nul_bytes() {
        assert_eq!(sanitize_xml("a\0b"), "ab");
    }

    #[test]
    fn keeps_valid_chars() {
        assert_eq!(sanitize_xml("hello world\n\t"), "hello world\n\t");
    }

    #[test]
    fn drops_numeric_ref_to_invalid_codepoint() {
        // &#1; is a control char NOT in the XML 1.0 valid set.
        assert_eq!(sanitize_xml("a&#1;b"), "ab");
    }

    #[test]
    fn keeps_numeric_ref_to_valid_codepoint() {
        // &#65; is 'A' — keep as-is.
        assert_eq!(sanitize_xml("&#65;"), "&#65;");
    }

    #[test]
    fn keeps_hex_ref_to_valid_codepoint() {
        // &#x41; is 'A' — keep as-is.
        assert_eq!(sanitize_xml("&#x41;"), "&#x41;");
    }

    #[test]
    fn xml_escape_basics() {
        assert_eq!(xml_escape("A & B"), "A &amp; B");
        assert_eq!(xml_escape("<x>"), "&lt;x&gt;");
        assert_eq!(xml_escape(r#"He said "hi""#), "He said &quot;hi&quot;");
    }
}
