use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

const MARKER: &str = "[boottrace]";
const RUST_PID: u32 = 1;

fn lanes() -> &'static Mutex<HashMap<&'static str, u32>> {
    static LANES: OnceLock<Mutex<HashMap<&'static str, u32>>> = OnceLock::new();
    LANES.get_or_init(|| Mutex::new(HashMap::new()))
}

fn lane(name: &'static str) -> u32 {
    let mut map = lanes()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some(existing) = map.get(name) {
        return *existing;
    }
    let next = map.len() as u32 + 1;
    map.insert(name, next);
    next
}

fn micros(time: SystemTime) -> u128 {
    time.duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_micros())
}

fn line(name: &'static str, detail: Option<&str>, ts: u128, dur: u128, tid: u32) -> String {
    let args = match detail {
        Some(detail) => {
            let escaped = serde_json::Value::String(detail.to_owned()).to_string();
            format!(",\"args\":{{\"detail\":{escaped}}}")
        }
        None => String::new(),
    };
    format!(
        "{{\"name\":\"{name}\",\"cat\":\"boot\",\"ph\":\"X\",\"ts\":{ts},\"dur\":{dur},\"pid\":{RUST_PID},\"tid\":{tid}{args}}}"
    )
}

fn emit(name: &'static str, detail: Option<&str>, started: u128, dur: u128) {
    log::info!("{MARKER} {}", line(name, detail, started, dur, lane(name)));
}

pub struct BootSpan {
    name: &'static str,
    detail: Option<String>,
    started_instant: Instant,
    started_wall: SystemTime,
    ended: bool,
}

impl BootSpan {
    pub fn end(mut self) {
        self.finish();
    }

    fn finish(&mut self) {
        if self.ended {
            return;
        }
        self.ended = true;
        emit(
            self.name,
            self.detail.as_deref(),
            micros(self.started_wall),
            self.started_instant.elapsed().as_micros(),
        );
    }
}

impl Drop for BootSpan {
    fn drop(&mut self) {
        self.finish();
    }
}

fn make_span(
    name: &'static str,
    detail: Option<String>,
    started_instant: Instant,
    started_wall: SystemTime,
) -> BootSpan {
    BootSpan {
        name,
        detail,
        started_instant,
        started_wall,
        ended: false,
    }
}

pub fn span(name: &'static str) -> BootSpan {
    make_span(name, None, Instant::now(), SystemTime::now())
}

pub fn span_detailed(name: &'static str, detail: impl Into<String>) -> BootSpan {
    make_span(name, Some(detail.into()), Instant::now(), SystemTime::now())
}

pub fn record(name: &'static str, started_instant: Instant, started_wall: SystemTime) {
    emit(
        name,
        None,
        micros(started_wall),
        started_instant.elapsed().as_micros(),
    );
}

pub fn span_from(
    name: &'static str,
    started_instant: Instant,
    started_wall: SystemTime,
) -> BootSpan {
    make_span(name, None, started_instant, started_wall)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_is_chrome_trace_json() {
        let text = line("rust.setup", None, 100, 50, 3);
        let value: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(value["name"], "rust.setup");
        assert_eq!(value["cat"], "boot");
        assert_eq!(value["ph"], "X");
        assert_eq!(value["ts"], 100);
        assert_eq!(value["dur"], 50);
        assert_eq!(value["pid"], 1);
        assert_eq!(value["tid"], 3);
        assert!(value.get("args").is_none());
    }

    #[test]
    fn detail_is_json_escaped() {
        let text = line("git.open_repository", Some(r#"C:\a\"b"#), 1, 2, 4);
        let value: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(value["args"]["detail"], r#"C:\a\"b"#);
    }

    #[test]
    fn lanes_are_stable_and_distinct() {
        assert_eq!(lane("boot_trace_test_a"), lane("boot_trace_test_a"));
        assert_ne!(lane("boot_trace_test_a"), lane("boot_trace_test_b"));
    }
}
