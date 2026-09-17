//! Small shared disk helpers: atomic file writes and filesystem-safe
//! cache-key components.

/// Writes bytes via temp file + persist so a crash can never leave a
/// truncated file behind.
pub(crate) fn write_atomic(path: &std::path::Path, bytes: &[u8]) -> std::io::Result<()> {
    let directory = path.parent().unwrap_or_else(|| std::path::Path::new("."));
    let temp = tempfile::Builder::new()
        .prefix(".gitau-tmp")
        .tempfile_in(directory)?;
    std::io::Write::write_all(&mut temp.as_file(), bytes)?;
    temp.persist(path)?;
    Ok(())
}

/// Filesystem-safe single path component. Anything outside
/// `[a-zA-Z0-9._-]` is replaced with `-`; degenerate results (`.`/`..`
/// or empty after sanitizing) become `x{fnv1a}` to stay unique-ish.
pub(crate) fn sanitize_component(input: &str) -> String {
    let cleaned: String = input
        .chars()
        .take(64)
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') {
                c
            } else {
                '-'
            }
        })
        .collect();

    if cleaned.is_empty() || cleaned == "." || cleaned == ".." {
        return format!("x{:08x}", fnv1a(input));
    }
    cleaned
}

fn fnv1a(input: &str) -> u32 {
    let mut hash: u32 = 0x811c_9dc5;
    for byte in input.as_bytes() {
        hash ^= u32::from(*byte);
        hash = hash.wrapping_mul(0x0100_0193);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitizes_path_components() {
        assert_eq!(sanitize_component("github.com"), "github.com");
        assert_eq!(sanitize_component("Octo Corp"), "Octo-Corp");
        let evil = sanitize_component("../../evil");
        assert!(!evil.contains('/'));
        assert_ne!(evil, "..");
        assert!(!sanitize_component("..").contains(".."));
        assert!(sanitize_component("").starts_with('x'));
    }

    #[test]
    fn atomic_writes_survive_and_replace() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested").join("file.json");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();

        write_atomic(&path, b"first").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"first");

        write_atomic(&path, b"second").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"second");
    }
}
