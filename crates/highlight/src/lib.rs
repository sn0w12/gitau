mod section;
mod snippet;
mod style;
mod syntax;
mod themes;

pub use section::SectionHighlighter;
pub use snippet::{highlight_snippet, SnippetHighlight};
pub use style::{LineKind, LineRef, Style};
pub use themes::{set_theme_pair, theme_options, DEFAULT_DARK_THEME, DEFAULT_LIGHT_THEME};

/// Files above this size render without highlighting rather than stalling
/// section delivery.
pub const MAX_HIGHLIGHT_BYTES: usize = 512 * 1024;

pub(crate) const NO_NEWLINE_MARKER: &str = "\\ No newline at end of file";

/// Loads the syntax set and themes so the first user-facing diff never pays
/// lazy-init cost. Safe to call multiple times; runs once.
pub fn warm_up() {
    syntax::warm_up();
    let _ = themes::themes();
}
