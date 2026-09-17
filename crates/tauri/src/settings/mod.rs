//! Rust-owned application settings: schema, persistence, validation, and
//! the generated TypeScript contract consumed by the frontend.
//!
//! The schema is the single source of truth. Values are stored and served
//! as a generic validated map; the frontend receives a generated interface
//! whose shape is derived from the same schema.

pub mod model;
pub mod schema;
pub mod store;

pub use model::{
    SelectOption, SettingDefinition, SettingKind, SettingType, SettingValue, SettingsSchema,
    SettingsSection, SettingsSnapshot, SettingsTab, SettingsValues, ValuesSnapshot,
    SETTINGS_VERSION,
};
pub use store::{SettingsError, SettingsStore};

/// Path of the checked-in generated TypeScript contract, relative to this
/// crate's manifest dir.
pub const GENERATED_TS_PATH: &str = "../../src/lib/settings/settings.generated.ts";

/// Emits the exact contents of `src/lib/settings/settings.generated.ts`.
/// Driven entirely by the builtin schema so the frontend contract cannot
/// drift from the Rust source of truth.
pub fn typescript_contract() -> String {
    let schema = schema::builtin();

    let mut properties = Vec::new();
    let mut shortcut_keys = Vec::new();
    for definition in schema.definitions() {
        // Hidden settings are part of the typed contract too. They are
        // persisted and hook-accessible; only the settings UI hides them.
        properties.push(format!(
            "    {}: {};",
            definition.key,
            ts_value_type(&definition.setting_type)
        ));
        if matches!(definition.setting_type, SettingType::Shortcut) {
            shortcut_keys.push(definition.key.to_owned());
        }
    }

    let mut output = String::new();
    output.push_str("/**\n");
    output.push_str(" * GENERATED FILE - do not edit by hand.\n");
    output.push_str(" *\n");
    output.push_str(" * Source of truth: crates/tauri/src/settings/schema.rs\n");
    output.push_str(" * Regenerate with: cargo run -p gitau --bin export_settings\n");
    output.push_str(
        " * Freshness is verified by cargo test (settings::tests::generated_typescript_is_fresh).\n",
    );
    output.push_str(" */\n\n");

    output.push_str("export interface SettingsValues {\n");
    for property in &properties {
        output.push_str(property);
        output.push('\n');
    }
    output.push_str("}\n\n");
    output.push_str("export type SettingKey = keyof SettingsValues;\n");
    output.push_str("export type ThemeMode = SettingsValues[\"theme\"];\n\n");

    output.push_str("export const SHORTCUT_SETTING_KEYS = [\n");
    for key in &shortcut_keys {
        output.push_str(&format!("    \"{key}\",\n"));
    }
    output.push_str("] as const;\n\n");
    output.push_str("export type ShortcutSettingKey = typeof SHORTCUT_SETTING_KEYS[number];\n");

    output
}

fn ts_value_type(setting_type: &SettingType) -> String {
    match setting_type {
        SettingType::Boolean => "boolean".to_owned(),
        SettingType::Number { .. } => "number".to_owned(),
        SettingType::String | SettingType::Shortcut | SettingType::MultilineString => {
            "string".to_owned()
        }
        SettingType::StringList => "string[]".to_owned(),
        SettingType::Select { options } => options
            .iter()
            .map(|option| format!("\"{}\"", escape_ts(&option.value)))
            .collect::<Vec<_>>()
            .join(" | "),
    }
}

fn escape_ts(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::store::SettingsStore;
    use std::sync::Arc;

    #[test]
    fn schema_keys_are_unique_and_defaults_valid() {
        schema::builtin()
            .validate_invariants()
            .expect("builtin schema must satisfy its own invariants");
    }

    #[test]
    fn hidden_settings_are_retained_but_flagged() {
        let builtin = schema::builtin();
        let pinned = builtin
            .find("pinnedRepos")
            .expect("pinnedRepos must exist in schema");
        assert!(pinned.hidden);

        let visible = builtin
            .definitions()
            .filter(|definition| !definition.hidden)
            .count();
        assert!(visible > 0);
    }

    #[test]
    fn merged_values_cover_exactly_the_schema_keys() {
        let snapshot = store_for_tests().snapshot();
        let mut schema_keys: Vec<_> = schema::builtin()
            .definitions()
            .map(|definition| definition.key.to_owned())
            .collect();
        schema_keys.sort();
        let value_keys: Vec<_> = snapshot.values.keys().cloned().collect();

        assert_eq!(schema_keys, value_keys, "values must mirror the schema");
    }

    #[test]
    fn defaults_are_served_when_nothing_is_stored() {
        let snapshot = store_for_tests().snapshot();

        assert_eq!(
            snapshot.values.get("closeRepoWithLastTab"),
            Some(&SettingValue::Bool(true))
        );
        assert_eq!(
            snapshot.values.get("theme"),
            Some(&SettingValue::Str("system".to_owned()))
        );
        assert_eq!(
            snapshot.values.get("pinnedRepos"),
            Some(&SettingValue::StringList(Vec::new()))
        );
        assert!(snapshot.saved_keys.is_empty());
    }

    #[test]
    fn validation_rejects_bad_values_and_clamps_numbers() {
        let builtin = schema::builtin();

        let theme = builtin.find("theme").unwrap();
        assert!(theme.validate(SettingValue::Str("dark".to_owned())).is_ok());
        assert!(theme
            .validate(SettingValue::Str("sepia".to_owned()))
            .is_err());

        let size = builtin.find("historyPageSize").unwrap();
        let clamped = size
            .validate(SettingValue::Number(1000.0))
            .expect("in-range after clamp");
        assert_eq!(clamped.as_f64(), Some(200.0));
        assert!(size.validate(SettingValue::Str("50".to_owned())).is_err());

        let new_tab = builtin.find("newTabShortcut").unwrap();
        assert!(new_tab
            .validate(SettingValue::Str("Mod+K".to_owned()))
            .is_ok());
        assert!(new_tab.validate(SettingValue::Str(String::new())).is_ok());
        assert!(new_tab
            .validate(SettingValue::Str("Bogus+T".to_owned()))
            .is_err());
        assert!(new_tab
            .validate(SettingValue::Str("Mod+".to_owned()))
            .is_err());

        let pinned = builtin.find("pinnedRepos").unwrap();
        assert!(pinned
            .validate(SettingValue::StringList(vec!["a".to_owned()]))
            .is_ok());
        assert!(pinned.validate(SettingValue::Bool(true)).is_err());
    }

    #[test]
    fn multiline_string_setting_accepts_multiline_text() {
        let builtin = schema::builtin();
        let global = builtin.find("globalGitignore").unwrap();
        assert!(!global.hidden);
        assert!(matches!(global.setting_type, SettingType::MultilineString));
        assert_eq!(global.default_value, SettingValue::Str(String::new()));
        assert!(global
            .validate(SettingValue::Str("*.log\n!important.log\n".to_owned()))
            .is_ok());
        assert!(global.validate(SettingValue::Bool(true)).is_err());
    }

    #[test]
    fn store_roundtrips_and_preserves_unknown_keys() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("settings.json");

        let store = SettingsStore::new(schema::builtin());
        store.initialize(path.clone());

        store
            .set_and_snapshot("theme", SettingValue::Str("dark".to_owned()))
            .unwrap();
        store
            .set_and_snapshot(
                "pinnedRepos",
                SettingValue::StringList(vec!["/repo".to_owned()]),
            )
            .unwrap();
        drop(store);

        // Hand-inject an unknown key as a newer version would leave behind.
        let raw = std::fs::read_to_string(&path).unwrap();
        let mut document: serde_json::Value = serde_json::from_str(&raw).unwrap();
        document["values"]["futureSetting"] = serde_json::json!(42);
        std::fs::write(&path, serde_json::to_string(&document).unwrap()).unwrap();

        let reloaded = SettingsStore::new(schema::builtin());
        reloaded.initialize(path.clone());
        let snapshot = reloaded.snapshot();

        assert_eq!(
            snapshot.values.get("theme"),
            Some(&SettingValue::Str("dark".to_owned()))
        );
        assert_eq!(
            snapshot.values.get("pinnedRepos"),
            Some(&SettingValue::StringList(vec!["/repo".to_owned()]))
        );
        assert!(snapshot.saved_keys.contains(&"theme".to_owned()));
        assert!(!snapshot
            .saved_keys
            .contains(&"defaultBranchName".to_owned()));

        // Saving again preserves the unknown key.
        reloaded
            .set_and_snapshot("historyPageSize", SettingValue::Number(80.0))
            .unwrap();
        let raw = std::fs::read_to_string(&path).unwrap();
        let document: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(document["values"]["futureSetting"], 42);
        assert_eq!(document["version"], SETTINGS_VERSION);
    }

    #[test]
    fn corrupt_file_is_backed_up_and_reset() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("settings.json");
        std::fs::write(&path, "{ not json !!!").unwrap();

        let store = SettingsStore::new(schema::builtin());
        store.initialize(path.clone());

        assert_eq!(
            store.snapshot().values.get("theme"),
            Some(&SettingValue::Str("system".to_owned()))
        );
        assert!(directory.path().join("settings.json.bak").exists());
    }

    #[test]
    fn missing_file_starts_from_defaults() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("nested/settings.json");

        let store = SettingsStore::new(schema::builtin());
        store.initialize(path.clone());

        assert_eq!(
            store.snapshot().values.get("theme"),
            Some(&SettingValue::Str("system".to_owned()))
        );
        assert!(store.snapshot().saved_keys.is_empty());
    }

    #[test]
    fn concurrent_sets_do_not_lose_updates() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("settings.json");

        let store = Arc::new(SettingsStore::new(schema::builtin()));
        store.initialize(path.clone());

        let handles: Vec<_> = (0..8)
            .map(|index| {
                let store = Arc::clone(&store);
                std::thread::spawn(move || {
                    store
                        .set_and_snapshot(
                            "historyPageSize",
                            SettingValue::Number(10.0 + f64::from(index)),
                        )
                        .unwrap();
                })
            })
            .collect();
        for handle in handles {
            handle.join().unwrap();
        }

        let raw = std::fs::read_to_string(&path).unwrap();
        let document: serde_json::Value = serde_json::from_str(&raw).unwrap();
        let saved = document["values"]["historyPageSize"].as_f64().unwrap();
        assert!((10.0..=17.0).contains(&saved));
    }

    fn store_for_tests() -> SettingsStore {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("settings.json");
        let store = SettingsStore::new(schema::builtin());
        store.initialize(path);
        store
    }

    /// CI drift check: the checked-in frontend contract must match the
    /// schema exactly. Regenerate with
    /// `cargo run -p gitau --bin export_settings`.
    #[test]
    fn generated_typescript_is_fresh() {
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let path = std::path::Path::new(manifest_dir).join(GENERATED_TS_PATH);
        let committed = std::fs::read_to_string(&path).unwrap_or_else(|error| {
            panic!(
                "generated contract missing at {} (run cargo run -p gitau --bin export_settings): {error}",
                path.display()
            )
        });
        assert_eq!(committed, typescript_contract());
    }
}
