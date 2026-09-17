//! Regenerates the frontend settings contract at
//! `src/lib/settings/settings.generated.ts` from the builtin schema.
//!
//! Usage: `cargo run -p gitau --bin export_settings`

fn main() {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let path = std::path::Path::new(manifest_dir).join(gitau_lib::settings::GENERATED_TS_PATH);
    std::fs::write(&path, gitau_lib::settings::typescript_contract())
        .unwrap_or_else(|error| panic!("failed to write {}: {error}", path.display()));
    println!("wrote {}", path.display());
}
