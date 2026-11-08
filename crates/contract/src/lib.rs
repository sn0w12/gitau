//! Generates the frontend command contract from the Rust serde surface.
//!
//! Parses workspace source with `syn`, walks the types reachable from every
//! `#[tauri::command]`, and emits the protocol types plus one typed wire
//! client method per command.

pub mod emit;
pub mod model;
pub mod scan;

use std::path::Path;

pub const PROTOCOL_TS_PATH: &str = "../../src/lib/backend/protocol.ts";
pub const COMMANDS_TS_PATH: &str = "../../src/lib/backend/transport/commands.generated.ts";

pub fn typescript_contracts() -> Result<(String, String), String> {
    let workspace = scan::load(&scan::workspace_root())?;
    emit::emit(&workspace)
}

pub fn write_contracts() -> Result<(), String> {
    let (protocol, commands) = typescript_contracts()?;
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let protocol_path = manifest.join(PROTOCOL_TS_PATH);
    let commands_path = manifest.join(COMMANDS_TS_PATH);
    std::fs::write(&protocol_path, protocol)
        .map_err(|error| format!("write {}: {error}", protocol_path.display()))?;
    std::fs::write(&commands_path, commands)
        .map_err(|error| format!("write {}: {error}", commands_path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn generated_commands_match_the_invoke_handler() {
        let workspace =
            super::scan::load(&super::scan::workspace_root()).expect("workspace must parse");
        let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let lib = std::fs::read_to_string(manifest.join("../../crates/tauri/src/lib.rs"))
            .expect("tauri lib.rs must exist");
        let marker = "generate_handler![";
        let start = lib.find(marker).expect("invoke handler list must exist") + marker.len();
        let rest = &lib[start..];
        let end = rest.find("])").expect("invoke handler list must close");
        let registered = rest[..end]
            .split(',')
            .map(str::trim)
            .filter(|entry| entry.starts_with("commands::"))
            .filter_map(|entry| entry.rsplit("::").next())
            .map(ToOwned::to_owned)
            .collect::<std::collections::BTreeSet<_>>();
        let generated = workspace
            .commands
            .iter()
            .map(|command| command.name.clone())
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(
            registered, generated,
            "the generated command list must match generate_handler!"
        );
    }

    #[test]
    fn every_registered_command_is_reachable_from_the_client() {
        let workspace =
            super::scan::load(&super::scan::workspace_root()).expect("workspace must parse");
        let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let client =
            std::fs::read_to_string(manifest.join("../../src/lib/backend/transport/client.ts"))
                .expect("client.ts must exist");
        for command in &workspace.commands {
            assert!(
                client.contains(&format!("commands.{}(", command.name)),
                "command `{}` is registered but unreachable from client.ts",
                command.name
            );
        }
    }

    #[test]
    fn generated_contracts_are_fresh() {
        let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let (protocol, commands) =
            super::typescript_contracts().expect("contract generation must succeed");
        for (path, expected) in [
            (super::PROTOCOL_TS_PATH, protocol),
            (super::COMMANDS_TS_PATH, commands),
        ] {
            let path = manifest.join(path);
            let committed = std::fs::read_to_string(&path).unwrap_or_else(|error| {
                panic!(
                    "generated contract missing at {} (run cargo run -p contract --bin export_contract): {error}",
                    path.display()
                )
            });
            // autocrlf rewrites the checked-out file to CRLF, which is a
            // platform artifact rather than contract drift.
            assert_eq!(committed.replace("\r\n", "\n"), expected);
        }
    }
}
