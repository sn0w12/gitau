const COMMANDS: &[&str] = &[
    "enable",
    "disable",
    "is_enabled",
    "set",
    "get_state",
    "get_geometry",
    "bootstrap",
];

fn main() {
    // A `cfg` alias for the platforms whose overlay is drawn with the Freedesktop
    // icon-theme look, so those parts of the canvas only exist where the Linux
    // backend uses them (see `src/paint.rs`). The list has to match the one gating
    // `platform::linux` in `src/platform/mod.rs`.
    println!("cargo:rustc-check-cfg=cfg(freedesktop)");
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if matches!(
        target_os.as_str(),
        "linux" | "dragonfly" | "freebsd" | "openbsd" | "netbsd"
    ) {
        println!("cargo:rustc-cfg=freedesktop");
    }

    tauri_plugin::Builder::new(COMMANDS).build();
}
