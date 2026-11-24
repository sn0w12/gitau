#[cfg(feature = "enabled")]
use std::error::Error;

use serde::{Deserialize, Serialize};

#[cfg(feature = "enabled")]
use crate::sanitize;

/// An error reduced to data that is safe to upload. `message` and `sources`
/// are only present when the call site opted into sanitized detail.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorRecord {
    pub code: String,
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sources: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at_unix_ms: Option<u64>,
}

#[cfg(feature = "enabled")]
impl ErrorRecord {
    pub(crate) fn bare<E: Error>(code: &str, _error: &E) -> Self {
        Self {
            code: sanitize::key_or_default(code),
            kind: sanitize::type_name(std::any::type_name::<E>()),
            message: None,
            sources: Vec::new(),
            at_unix_ms: None,
        }
    }

    pub(crate) fn detailed<E: Error>(code: &str, error: &E) -> Self {
        let mut record = Self::bare(code, error);
        record.message = Some(sanitize::text(&error.to_string()));

        let mut source = error.source();
        while let Some(cause) = source {
            if record.sources.len() >= 3 {
                break;
            }
            record.sources.push(sanitize::text(&cause.to_string()));
            source = cause.source();
        }

        record
    }

    pub(crate) fn from_message(code: &str, message: &str) -> Self {
        Self {
            code: sanitize::key_or_default(code),
            kind: "message".into(),
            message: Some(sanitize::text(message)),
            sources: Vec::new(),
            at_unix_ms: None,
        }
    }
}

#[cfg(all(test, feature = "enabled"))]
mod tests {
    use std::fmt;

    use super::*;

    #[derive(Debug)]
    struct Inner;

    impl fmt::Display for Inner {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("inner failed at C:\\Users\\bob\\repo")
        }
    }

    impl Error for Inner {}

    #[derive(Debug)]
    struct Outer(Inner);

    impl fmt::Display for Outer {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("outer failed at https://example.com/x")
        }
    }

    impl Error for Outer {
        fn source(&self) -> Option<&(dyn Error + 'static)> {
            Some(&self.0)
        }
    }

    #[test]
    fn bare_errors_carry_no_message() {
        let record = ErrorRecord::bare("diff_failed", &Outer(Inner));
        assert_eq!(record.code, "diff_failed");
        assert!(record.kind.contains("Outer"));
        assert_eq!(record.message, None);
        assert!(record.sources.is_empty());
    }

    #[test]
    fn detailed_errors_are_sanitized() {
        let record = ErrorRecord::detailed("diff_failed", &Outer(Inner));
        assert_eq!(record.message.as_deref(), Some("outer failed at <url>"));
        assert_eq!(record.sources, vec!["inner failed at <path>"]);
    }

    #[test]
    fn normalized_codes_survive() {
        let record = ErrorRecord::from_message("Files Changed", "plain message");
        assert_eq!(record.code, "files_changed");
        assert_eq!(record.message.as_deref(), Some("plain message"));
    }
}
