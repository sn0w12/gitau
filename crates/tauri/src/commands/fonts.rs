use std::collections::BTreeSet;
use std::sync::OnceLock;

use crate::commands::CommandResult;

static SYSTEM_FONT_FAMILIES: OnceLock<Vec<String>> = OnceLock::new();

/// Families installed on this machine, used as the choices for the font
/// settings. Enumeration touches every font file, so it is cached for the
/// life of the process.
#[tauri::command]
pub async fn fonts_list() -> CommandResult<Vec<String>> {
    if let Some(families) = SYSTEM_FONT_FAMILIES.get() {
        return Ok(families.clone());
    }

    let families = tauri::async_runtime::spawn_blocking(installed_font_families)
        .await
        .unwrap_or_else(|error| {
            log::warn!("system font enumeration failed: {error}");
            Vec::new()
        });

    Ok(SYSTEM_FONT_FAMILIES.get_or_init(|| families).clone())
}

fn installed_font_families() -> Vec<String> {
    let mut database = fontdb::Database::new();
    database.load_system_fonts();

    let mut families = BTreeSet::new();
    for face in database.faces() {
        // fontdb stores every localized family name; prefer the English
        // (ASCII) one, which is what CSS family lookups match. Vertical
        // variants carry an `@` prefix.
        let name = face
            .families
            .iter()
            .find(|(name, _)| name.is_ascii())
            .or_else(|| face.families.first())
            .map(|(name, _)| name.trim().trim_start_matches('@'));
        if let Some(name) = name.filter(|name| !name.is_empty()) {
            families.insert(name.to_owned());
        }
    }

    families.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::installed_font_families;

    #[test]
    fn enumerates_sorted_unique_families() {
        let families = installed_font_families();
        assert!(families.iter().all(|family| !family.trim().is_empty()));
        assert!(families.windows(2).all(|pair| pair[0] < pair[1]));
    }
}
