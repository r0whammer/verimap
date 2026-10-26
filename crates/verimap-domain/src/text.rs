//! Validation for untrusted text. Labels are displayed as text, never
//! interpreted, but are still bounded and free of control characters.

use crate::DomainError;

/// Accepts a slug identifier: 1 to `max_bytes` bytes of `a-z`, `0-9`, and `-`,
/// not starting or ending with `-`.
pub fn validate_slug(field: &'static str, s: &str, max_bytes: usize) -> Result<(), DomainError> {
    let invalid = |reason| Err(DomainError::InvalidText { field, reason });
    if s.is_empty() {
        return invalid("must not be empty");
    }
    if s.len() > max_bytes {
        return invalid("too long");
    }
    if !s
        .bytes()
        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
    {
        return invalid("only a-z, 0-9 and '-' are allowed");
    }
    if s.starts_with('-') || s.ends_with('-') {
        return invalid("must not start or end with '-'");
    }
    Ok(())
}

/// Accepts a display label: 1 to `max_bytes` bytes of UTF-8 without control
/// characters, and not only whitespace.
pub fn validate_label(field: &'static str, s: &str, max_bytes: usize) -> Result<(), DomainError> {
    let invalid = |reason| Err(DomainError::InvalidText { field, reason });
    if s.trim().is_empty() {
        return invalid("must not be empty");
    }
    if s.len() > max_bytes {
        return invalid("too long");
    }
    if s.chars().any(char::is_control) {
        return invalid("must not contain control characters");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugs() {
        assert!(validate_slug("id", "demo-site-1", 64).is_ok());
        assert!(validate_slug("id", "a", 1).is_ok());
        for bad in ["", "-a", "a-", "Upper", "under_score", "spa ce", "é"] {
            assert!(validate_slug("id", bad, 64).is_err(), "{bad:?} accepted");
        }
        assert!(validate_slug("id", "ab", 1).is_err());
    }

    #[test]
    fn labels() {
        assert!(validate_label("name", "Synthetic Site α", 128).is_ok());
        assert!(validate_label("name", "<script>alert(1)</script>", 128).is_ok());
        for bad in ["", "   ", "line\nbreak", "nul\0", "bell\u{7}"] {
            assert!(
                validate_label("name", bad, 128).is_err(),
                "{bad:?} accepted"
            );
        }
        // Length is measured in bytes: "α" is two.
        assert!(validate_label("name", "αα", 3).is_err());
        assert!(validate_label("name", "αα", 4).is_ok());
    }
}
