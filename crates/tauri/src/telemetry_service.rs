use std::sync::Arc;
use std::time::Duration;

use tokio::sync::mpsc;
use tokio::sync::mpsc::UnboundedReceiver;

const DEFAULT_ENDPOINT: &str = "http://127.0.0.1:18080/v1/batches";
const FLUSH_INTERVAL: Duration = Duration::from_secs(15);

pub(crate) const ENABLED_SETTING_KEY: &str = "telemetryEnabled";

pub fn initialize(app_version: &str) {
    if !telemetry::is_available() {
        return;
    }

    let endpoint =
        std::env::var("GITAU_TELEMETRY_URL").unwrap_or_else(|_| DEFAULT_ENDPOINT.to_owned());
    let (sender, receiver) = mpsc::unbounded_channel();
    let sink = telemetry::CallbackSink::new(move |batch| {
        let _ = sender.send(batch.to_json());
    });
    let config = telemetry::Config::new(telemetry::EnvironmentContext::detect(app_version))
        .enabled(false)
        .sample_permille(if cfg!(debug_assertions) { 1000 } else { 250 })
        .buffer_capacity(128);

    if let Err(error) = telemetry::init(config, vec![Arc::new(sink)]) {
        log::warn!("telemetry could not start: {error}");
        return;
    }

    tauri::async_runtime::spawn(deliver(receiver, endpoint));
    tauri::async_runtime::spawn(async move {
        let mut ticker = tokio::time::interval(FLUSH_INTERVAL);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            ticker.tick().await;
            flush();
        }
    });
}

async fn deliver(mut receiver: UnboundedReceiver<String>, endpoint: String) {
    let client = reqwest::Client::new();
    while let Some(body) = receiver.recv().await {
        match client
            .post(&endpoint)
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .body(body)
            .send()
            .await
        {
            Ok(response) if !response.status().is_success() => {
                log::warn!("telemetry upload rejected: {}", response.status());
            }
            Ok(_) => {}
            Err(error) => log::warn!("telemetry upload failed: {error}"),
        }
    }
}

pub(crate) fn set_enabled(enabled: bool) {
    telemetry::set_enabled(enabled);
}

pub fn flush() {
    let report = telemetry::flush();
    if report.traces > 0 || report.errors > 0 {
        log::debug!(
            "telemetry flushed {} traces, {} errors, {} dropped",
            report.traces,
            report.errors,
            report.dropped_traces
        );
    }
}
