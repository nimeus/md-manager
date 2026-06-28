//! Input validation shared by all surfaces.

use crate::error::{Error, Result};

/// Validate an org/project slug: 1–63 chars of `[a-z0-9-]`, not starting/ending with `-`.
pub fn validate_slug(slug: &str) -> Result<()> {
    let n = slug.len();
    if !(1..=63).contains(&n) {
        return Err(Error::invalid("slug must be 1–63 characters"));
    }
    if slug.starts_with('-') || slug.ends_with('-') {
        return Err(Error::invalid("slug must not start or end with '-'"));
    }
    if !slug
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        return Err(Error::invalid(
            "slug may contain only lowercase letters, digits, and '-'",
        ));
    }
    Ok(())
}

/// Validate a document path: `seg(/seg)*`, each seg `[a-z0-9._-]+`, total ≤ 512 chars.
pub fn validate_path(path: &str) -> Result<()> {
    if path.is_empty() || path.len() > 512 {
        return Err(Error::invalid("path must be 1–512 characters"));
    }
    if path.starts_with('/') || path.ends_with('/') || path.contains("//") {
        return Err(Error::invalid(
            "path must not start/end with '/' or contain '//'",
        ));
    }
    for seg in path.split('/') {
        if seg.is_empty() || seg == "." || seg == ".." {
            return Err(Error::invalid(
                "path segments must be non-empty and not '.'/'..'",
            ));
        }
        if !seg
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '_' | '-'))
        {
            return Err(Error::invalid("path segments may contain only [a-z0-9._-]"));
        }
    }
    Ok(())
}

/// Validate a human title: non-empty after trimming, ≤ 512 chars.
pub fn validate_title(title: &str) -> Result<()> {
    let t = title.trim();
    if t.is_empty() || t.len() > 512 {
        return Err(Error::invalid("title must be 1–512 characters"));
    }
    Ok(())
}

/// Reject document bodies larger than `max_bytes`.
pub fn validate_content_size(content: &str, max_bytes: i64) -> Result<()> {
    if content.len() as i64 > max_bytes {
        return Err(Error::invalid(format!(
            "document exceeds maximum size of {max_bytes} bytes"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugs() {
        assert!(validate_slug("acme").is_ok());
        assert!(validate_slug("acme-corp-1").is_ok());
        assert!(validate_slug("-bad").is_err());
        assert!(validate_slug("Bad").is_err());
        assert!(validate_slug("").is_err());
    }

    #[test]
    fn paths() {
        assert!(validate_path("guides/setup").is_ok());
        assert!(validate_path("readme.md").is_ok());
        assert!(validate_path("/leading").is_err());
        assert!(validate_path("a//b").is_err());
        assert!(validate_path("../escape").is_err());
        assert!(validate_path("Caps").is_err());
    }

    #[test]
    fn size() {
        assert!(validate_content_size("hello", 10).is_ok());
        assert!(validate_content_size("hello world!!", 10).is_err());
    }
}
