#![cfg(not(feature = "enabled"))]

#[telemetry::trace]
fn instrumented(value: u32) -> u32 {
    value + 1
}

#[test]
fn the_attribute_and_runtime_are_inert() {
    assert_eq!(instrumented(1), 2);

    telemetry::context! {
        "ignored" => 1,
    };

    let guard = telemetry::SpanGuard::enter("ignored");
    guard.set_attribute("ignored", 1i64.into());

    let error = std::io::Error::other("ignored");
    telemetry::record_error("ignored", &error);
    telemetry::record_error_detail("ignored", &error);
    telemetry::record_error_message("ignored", "ignored");

    assert!(!telemetry::is_available());
    telemetry::enable();
    assert!(!telemetry::is_enabled());
    telemetry::disable();
    assert_eq!(telemetry::flush().traces, 0);
    assert_eq!(telemetry::pending_traces(), 0);
    assert_eq!(telemetry::dropped_traces(), 0);
}
