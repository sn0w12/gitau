use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::sync::Mutex;

use crate::model::Batch;

/// Receives batches from [`crate::flush`].
///
/// Called on the flushing thread, so it must not block. Enqueue the batch for
/// a worker thread if delivery can be slow.
pub trait Sink: Send + Sync + 'static {
    fn accept(&self, batch: &Batch);
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FlushReport {
    pub traces: usize,
    pub errors: usize,
    pub dropped_traces: u64,
    pub sink_failures: usize,
}

pub struct CallbackSink<F> {
    callback: F,
}

impl<F> CallbackSink<F>
where
    F: Fn(&Batch) + Send + Sync + 'static,
{
    pub fn new(callback: F) -> Self {
        Self { callback }
    }
}

impl<F> Sink for CallbackSink<F>
where
    F: Fn(&Batch) + Send + Sync + 'static,
{
    fn accept(&self, batch: &Batch) {
        (self.callback)(batch);
    }
}

/// Appends one JSON batch per line. Meant for local debugging; the file stays
/// on the user's machine.
pub struct FileSink {
    file: Mutex<File>,
}

impl FileSink {
    pub fn new(path: impl AsRef<Path>) -> std::io::Result<Self> {
        let file = OpenOptions::new().create(true).append(true).open(path)?;
        Ok(Self {
            file: Mutex::new(file),
        })
    }
}

impl Sink for FileSink {
    fn accept(&self, batch: &Batch) {
        if let Ok(mut file) = self.file.lock() {
            let _ = writeln!(file, "{}", batch.to_json());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::EnvironmentContext;
    use crate::model::SCHEMA_VERSION;

    fn empty_batch() -> Batch {
        Batch {
            schema_version: SCHEMA_VERSION,
            created_at_unix_ms: 0,
            context: EnvironmentContext::detect("test"),
            traces: Vec::new(),
            errors: Vec::new(),
            dropped_traces: 0,
        }
    }

    #[test]
    fn callback_sink_sees_batches() {
        let seen = std::sync::Arc::new(Mutex::new(0usize));
        let counter = seen.clone();
        let sink = CallbackSink::new(move |_| {
            *counter.lock().unwrap() += 1;
        });
        sink.accept(&empty_batch());
        assert_eq!(*seen.lock().unwrap(), 1);
    }

    #[test]
    fn file_sink_appends_json_lines() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("telemetry.jsonl");
        let sink = FileSink::new(&path).unwrap();
        sink.accept(&empty_batch());
        sink.accept(&empty_batch());

        let contents = std::fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = contents.lines().collect();
        assert_eq!(lines.len(), 2);
        for line in lines {
            let value: serde_json::Value = serde_json::from_str(line).unwrap();
            assert_eq!(
                value["schemaVersion"].as_u64(),
                Some(u64::from(SCHEMA_VERSION))
            );
        }
    }
}
