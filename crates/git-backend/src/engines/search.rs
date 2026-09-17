use fuzzy_matcher::FuzzyMatcher;
use fuzzy_matcher::skim::SkimMatcherV2;

/// Parsed history-search query backed by the `fuzzy-matcher` library.
///
/// Whitespace splits the input into terms; a commit matches when every term
/// matches at least one of its fields. A term matches a field when it
/// appears as a case-insensitive substring, or fuzzy-matches inside a single
/// word (runs of alphanumeric characters). Single-word fuzzy matching keeps
/// typo tolerance while whole-message subsequence matching is rejected: it
/// lets a term like "cookie" match through letters scattered across random
/// words of a long message.
pub(crate) struct CommitSearch {
    matcher: SkimMatcherV2,
    terms: Vec<String>,
}

/// Builds a search from raw filter text; empty/whitespace filters nothing.
pub(crate) fn commit_search(raw: Option<&str>) -> CommitSearch {
    let terms = raw
        .map(|s| {
            s.split_whitespace()
                .map(str::to_lowercase)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    CommitSearch {
        matcher: SkimMatcherV2::default(),
        terms,
    }
}

impl CommitSearch {
    /// True when the search filters nothing and every commit passes.
    pub fn is_empty(&self) -> bool {
        self.terms.is_empty()
    }

    /// True when every term matches at least one haystack, either as a
    /// substring of the whole field or fuzzy inside a single word. Callers
    /// pass lowercased commit fields so matching is case-insensitive
    /// regardless of the matcher's defaults.
    pub fn matches(&self, haystacks: &[&str]) -> bool {
        self.terms
            .iter()
            .all(|term| haystacks.iter().any(|hay| self.term_matches(term, hay)))
    }

    /// True for one term against one lowercased field.
    fn term_matches(&self, term: &str, hay: &str) -> bool {
        if hay.contains(term) {
            return true;
        }
        words_with_char_offsets(hay)
            .iter()
            .any(|(_, word)| self.matcher.fuzzy_match(word, term).is_some())
    }

    /// Char-offset ranges for highlighting, merged per field. Callers pass
    /// lowercased `summary` / `author`; skim reports char indices, so
    /// word-relative hits map back onto field offsets directly.
    pub fn ranges_for(
        &self,
        summary: &str,
        author: &str,
    ) -> Vec<crate::domain::commits::SearchMatchRange> {
        use crate::domain::commits::{SearchMatchField, SearchMatchRange};
        let mut out = Vec::new();
        for (field, hay) in [
            (SearchMatchField::Summary, summary),
            (SearchMatchField::Author, author),
        ] {
            let mut hits: Vec<usize> = Vec::new();
            let words = words_with_char_offsets(hay);
            for term in &self.terms {
                if term.is_empty() {
                    continue;
                }
                let mut found_substring = false;
                let mut from = 0;
                while let Some(rel) = hay[from..].find(term) {
                    found_substring = true;
                    let start_char = hay[..from + rel].chars().count();
                    let term_chars = term.chars().count();
                    hits.extend(start_char..start_char + term_chars);
                    from += rel + term.len();
                }
                if !found_substring {
                    for (word_start, word) in &words {
                        if let Some((_, indices)) = self.matcher.fuzzy_indices(word, term) {
                            hits.extend(indices.into_iter().map(|i| word_start + i));
                        }
                    }
                }
            }
            if hits.is_empty() {
                continue;
            }
            hits.sort_unstable();
            hits.dedup();
            let mut start = hits[0];
            let mut prev = start;
            for &idx in &hits[1..] {
                if idx == prev + 1 {
                    prev = idx;
                    continue;
                }
                out.push(SearchMatchRange {
                    field,
                    start: start as u32,
                    length: (prev - start + 1) as u32,
                });
                start = idx;
                prev = idx;
            }
            out.push(SearchMatchRange {
                field,
                start: start as u32,
                length: (prev - start + 1) as u32,
            });
        }
        out
    }
}

/// Words as `(char-offset, text)` pairs, split on non-alphanumeric
/// characters. Offsets are char counts so skim's char indices map back onto
/// the field.
fn words_with_char_offsets(hay: &str) -> Vec<(usize, &str)> {
    let mut out = Vec::new();
    let mut word_start: Option<(usize, usize)> = None;
    for (char_idx, (byte_idx, c)) in hay.char_indices().enumerate() {
        if c.is_alphanumeric() {
            if word_start.is_none() {
                word_start = Some((char_idx, byte_idx));
            }
        } else if let Some((start_char, start_byte)) = word_start.take() {
            out.push((start_char, &hay[start_byte..byte_idx]));
        }
    }
    if let Some((start_char, start_byte)) = word_start {
        out.push((start_char, &hay[start_byte..]));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn search(raw: &str) -> CommitSearch {
        commit_search(Some(raw))
    }

    #[test]
    fn substring_matches() {
        let s = search("cookie");
        assert!(s.matches(&["add cookie consent banner"]));
    }

    #[test]
    fn scattered_letters_across_words_do_not_match() {
        let s = search("cookie");
        assert!(!s.matches(&["merge branch 'dev' of https://github.com/sn0w12/akari into dev"]));
    }

    #[test]
    fn typo_inside_one_word_still_matches() {
        let s = search("securty");
        assert!(s.matches(&["refactor security layer 6"]));
    }

    #[test]
    fn cross_word_abbreviation_does_not_match() {
        let s = search("rl6");
        assert!(!s.matches(&["refactor security layer 6"]));
    }

    #[test]
    fn every_term_must_match_some_field() {
        let s = search("refactor 6");
        assert!(s.matches(&["refactor security layer 6"]));
        let missing = search("refactor 7");
        assert!(!missing.matches(&["refactor security layer 6"]));
    }

    #[test]
    fn substring_highlight_is_contiguous() {
        use crate::domain::commits::SearchMatchField;
        let s = search("cookie");
        let ranges = s.ranges_for("add cookie consent banner", "ada");
        assert_eq!(ranges.len(), 1);
        assert_eq!(ranges[0].field, SearchMatchField::Summary);
        assert_eq!(ranges[0].start, 4);
        assert_eq!(ranges[0].length, 6);
    }

    #[test]
    fn scattered_term_yields_no_ranges() {
        let s = search("cookie");
        let ranges = s.ranges_for(
            "merge branch 'dev' of https://github.com/sn0w12/akari into dev",
            "ada",
        );
        assert!(ranges.is_empty());
    }

    #[test]
    fn typo_highlight_stays_inside_the_word() {
        use crate::domain::commits::SearchMatchField;
        let s = search("securty");
        let ranges = s.ranges_for("refactor security layer 6", "ada");
        assert!(!ranges.is_empty());
        for range in &ranges {
            assert_eq!(range.field, SearchMatchField::Summary);
            // "security" spans char offsets 9..17.
            assert!(range.start >= 9);
            assert!(range.start + range.length <= 17);
        }
    }
}
