mod lang_colors;
mod linguist_samples;
mod stats;

pub use lang_colors::{classify_language, color_for_language, resolve, Resolution, LANGUAGES};
pub use stats::{language_stats, LanguageError, LanguageShare, LanguageStats, Result};
