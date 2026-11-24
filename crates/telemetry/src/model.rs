use std::collections::BTreeMap;
#[cfg(feature = "enabled")]
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::config::EnvironmentContext;
use crate::error::ErrorRecord;
use crate::sanitize;

pub const SCHEMA_VERSION: u32 = 1;

/// A value attached to a span by the developer. Text is sanitized and
/// truncated at construction time.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum AttrValue {
    Bool(bool),
    Int(i64),
    Uint(u64),
    Float(f64),
    Text(String),
}

impl From<bool> for AttrValue {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}

impl From<i64> for AttrValue {
    fn from(value: i64) -> Self {
        Self::Int(value)
    }
}

impl From<u64> for AttrValue {
    fn from(value: u64) -> Self {
        Self::Uint(value)
    }
}

impl From<i32> for AttrValue {
    fn from(value: i32) -> Self {
        Self::Int(i64::from(value))
    }
}

impl From<u32> for AttrValue {
    fn from(value: u32) -> Self {
        Self::Uint(u64::from(value))
    }
}

impl From<u16> for AttrValue {
    fn from(value: u16) -> Self {
        Self::Uint(u64::from(value))
    }
}

impl From<u8> for AttrValue {
    fn from(value: u8) -> Self {
        Self::Uint(u64::from(value))
    }
}

impl From<i16> for AttrValue {
    fn from(value: i16) -> Self {
        Self::Int(i64::from(value))
    }
}

impl From<i8> for AttrValue {
    fn from(value: i8) -> Self {
        Self::Int(i64::from(value))
    }
}

impl From<usize> for AttrValue {
    fn from(value: usize) -> Self {
        Self::Uint(value as u64)
    }
}

impl From<isize> for AttrValue {
    fn from(value: isize) -> Self {
        Self::Int(value as i64)
    }
}

impl From<f64> for AttrValue {
    fn from(value: f64) -> Self {
        if value.is_finite() {
            Self::Float(value)
        } else {
            Self::Text("<non-finite>".into())
        }
    }
}

impl From<f32> for AttrValue {
    fn from(value: f32) -> Self {
        Self::from(f64::from(value))
    }
}

impl From<&str> for AttrValue {
    fn from(value: &str) -> Self {
        Self::Text(sanitize::text(value))
    }
}

impl From<String> for AttrValue {
    fn from(value: String) -> Self {
        Self::Text(sanitize::text(&value))
    }
}

impl From<&String> for AttrValue {
    fn from(value: &String) -> Self {
        Self::Text(sanitize::text(value))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Span {
    pub name: String,
    pub start_us: u64,
    pub duration_us: u64,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub attributes: BTreeMap<String, AttrValue>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorRecord>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<Span>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Trace {
    pub start_unix_ms: u64,
    pub duration_us: u64,
    pub dropped_spans: u64,
    pub root: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Batch {
    pub schema_version: u32,
    pub created_at_unix_ms: u64,
    pub context: EnvironmentContext,
    pub traces: Vec<Trace>,
    pub errors: Vec<ErrorRecord>,
    pub dropped_traces: u64,
}

#[cfg(feature = "enabled")]
pub(crate) fn unix_ms_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitizes_text_attributes() {
        let value = AttrValue::from("see https://example.com/secret");
        assert_eq!(value, AttrValue::Text("see <url>".into()));
    }

    #[test]
    fn clamps_non_finite_floats() {
        assert_eq!(
            AttrValue::from(f64::NAN),
            AttrValue::Text("<non-finite>".into())
        );
    }
}
