use serde::{Deserialize, Serialize};

use crate::domain::errors::DomainError;

/// A validated media URL. Construction rejects anything that isn't http(s).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaUrl(String);

impl MediaUrl {
    pub fn parse(raw: impl Into<String>) -> Result<Self, DomainError> {
        let raw = raw.into();
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(DomainError::InvalidUrl("empty".into()));
        }
        if !(trimmed.starts_with("http://") || trimmed.starts_with("https://")) {
            return Err(DomainError::InvalidUrl(trimmed.to_string()));
        }
        Ok(Self(trimmed.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for MediaUrl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_http_and_https() {
        assert!(MediaUrl::parse("http://a.com/v.mp4").is_ok());
        assert!(MediaUrl::parse("https://a.com/v.mp4").is_ok());
    }

    #[test]
    fn trims_whitespace() {
        let u = MediaUrl::parse("  https://a.com/v.mp4  ").unwrap();
        assert_eq!(u.as_str(), "https://a.com/v.mp4");
    }

    #[test]
    fn rejects_non_http() {
        assert!(MediaUrl::parse("ftp://a.com").is_err());
        assert!(MediaUrl::parse("not a url").is_err());
        assert!(MediaUrl::parse("").is_err());
    }
}
