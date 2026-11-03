//! Sample corpus mirrored from github-linguist `samples/` (MIT), used by
//! the tuning test to keep the classifier honest about real files. One
//! line per sample, tab-separated: `file name<TAB>content`.

#[cfg(test)]
pub(crate) const SAMPLES: &str = include_str!("../../tests/linguist_samples.tsv");

/// Parses the corpus into `(file name, content)` pairs. Content is stored
/// flattened, so only a record's first line survives; continuation lines
/// carry a leading tab and are dropped so they don't parse as empty-named
/// records. `#` comments are skipped.
#[cfg(test)]
pub(crate) fn parse_samples() -> Vec<(String, String)> {
    SAMPLES
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#') && !line.starts_with('\t'))
        .filter_map(|line| line.split_once('\t'))
        .filter(|(name, _)| !name.is_empty())
        .map(|(name, text)| (name.to_owned(), text.to_owned()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn corpus_is_populated_and_wellformed() {
        let samples = parse_samples();
        assert!(samples.len() >= 40, "corpus shrunk: {}", samples.len());
        for (name, text) in &samples {
            assert!(
                !name.contains('\t') && !text.contains('\t'),
                "{name} malformed"
            );
        }
    }
}
