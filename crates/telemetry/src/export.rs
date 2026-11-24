use std::collections::BTreeMap;

use serde::Serialize;

use crate::error::ErrorRecord;
use crate::model::{AttrValue, Batch, Span};

impl Batch {
    /// Compact JSON for upload or storage.
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| fallback_json())
    }

    pub fn to_json_pretty(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|_| fallback_json())
    }

    /// Chrome Trace Event JSON, which Perfetto and `chrome://tracing` render as
    /// a nested waterfall. Every span is a complete event; nesting follows the
    /// time ranges.
    pub fn to_chrome_trace_json(&self) -> String {
        let mut events = Vec::new();
        for trace in &self.traces {
            let base = trace.start_unix_ms.saturating_mul(1000);
            collect_events(&trace.root, base, &mut events);
        }
        for error in &self.errors {
            events.push(ChromeEvent {
                name: format!("error: {}", error.code),
                cat: "error",
                ph: "i",
                ts: error.at_unix_ms.unwrap_or(0).saturating_mul(1000),
                dur: 0,
                pid: 1,
                tid: 1,
                args: Some(ChromeArgs {
                    attributes: BTreeMap::new(),
                    error: Some(error.clone()),
                }),
            });
        }
        let document = ChromeTrace {
            display_time_unit: "ms",
            trace_events: events,
        };
        serde_json::to_string(&document).unwrap_or_else(|_| fallback_json())
    }
}

fn fallback_json() -> String {
    r#"{"error":"serialization failed"}"#.into()
}

#[derive(Serialize)]
struct ChromeTrace<'a> {
    #[serde(rename = "displayTimeUnit")]
    display_time_unit: &'a str,
    #[serde(rename = "traceEvents")]
    trace_events: Vec<ChromeEvent>,
}

#[derive(Serialize)]
struct ChromeEvent {
    name: String,
    cat: &'static str,
    ph: &'static str,
    ts: u64,
    dur: u64,
    pid: u32,
    tid: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    args: Option<ChromeArgs>,
}

#[derive(Serialize)]
struct ChromeArgs {
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    attributes: BTreeMap<String, AttrValue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<ErrorRecord>,
}

fn collect_events(span: &Span, base_us: u64, events: &mut Vec<ChromeEvent>) {
    let args = if span.attributes.is_empty() && span.error.is_none() {
        None
    } else {
        Some(ChromeArgs {
            attributes: span.attributes.clone(),
            error: span.error.clone(),
        })
    };

    events.push(ChromeEvent {
        name: span.name.clone(),
        cat: "span",
        ph: "X",
        ts: base_us.saturating_add(span.start_us),
        dur: span.duration_us,
        pid: 1,
        tid: 1,
        args,
    });

    for child in &span.children {
        collect_events(child, base_us, events);
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::config::EnvironmentContext;
    use crate::model::{Trace, SCHEMA_VERSION};

    fn sample_batch() -> Batch {
        let child = Span {
            name: "fetch".into(),
            start_us: 10,
            duration_us: 20,
            attributes: BTreeMap::new(),
            error: None,
            children: Vec::new(),
        };
        let root = Span {
            name: "load".into(),
            start_us: 0,
            duration_us: 100,
            attributes: BTreeMap::new(),
            error: None,
            children: vec![child],
        };
        Batch {
            schema_version: SCHEMA_VERSION,
            created_at_unix_ms: 1,
            context: EnvironmentContext::detect("test"),
            traces: vec![Trace {
                start_unix_ms: 1_000,
                duration_us: 100,
                dropped_spans: 0,
                root,
            }],
            errors: Vec::new(),
            dropped_traces: 0,
        }
    }

    #[test]
    fn serializes_a_batch() {
        let batch = sample_batch();
        let value: serde_json::Value = serde_json::from_str(&batch.to_json()).unwrap();
        assert_eq!(
            value["schemaVersion"].as_u64(),
            Some(u64::from(SCHEMA_VERSION))
        );
        assert_eq!(value["traces"][0]["root"]["children"][0]["name"], "fetch");
    }

    #[test]
    fn exports_standalone_errors_as_instant_events() {
        let mut batch = sample_batch();
        batch.errors.push(ErrorRecord {
            code: "fetch_failed".into(),
            kind: "test::Error".into(),
            message: None,
            sources: Vec::new(),
            at_unix_ms: Some(2_000),
        });

        let value: serde_json::Value = serde_json::from_str(&batch.to_chrome_trace_json()).unwrap();
        let events = value["traceEvents"].as_array().unwrap();
        let error = events.iter().find(|event| event["ph"] == "i").unwrap();
        assert_eq!(error["name"], "error: fetch_failed");
        assert_eq!(error["ts"].as_u64(), Some(2_000_000));
    }

    #[test]
    fn exports_a_chrome_trace() {
        let batch = sample_batch();
        let value: serde_json::Value = serde_json::from_str(&batch.to_chrome_trace_json()).unwrap();
        let events = value["traceEvents"].as_array().unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0]["name"], "load");
        assert_eq!(events[0]["ph"], "X");
        assert_eq!(events[0]["ts"].as_u64(), Some(1_000_000));
        assert_eq!(events[1]["ts"].as_u64(), Some(1_000_010));
    }
}
