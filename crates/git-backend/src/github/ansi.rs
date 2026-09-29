//! SGR colour parsing for CI logs. Build tools colour their output with ANSI
//! escapes, so a log rendered verbatim shows raw `[32m` markers and loses
//! every distinction the runner made.

use std::collections::HashMap;

use crate::api::highlight::SnippetStyle;

/// One colour a log line can ask for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AnsiColor {
    /// 0-255 from the xterm palette.
    Indexed(u8),
    /// 24-bit colour.
    Rgb(u8, u8, u8),
}

/// The text attributes a line can carry at any point.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct AnsiStyle {
    pub color: Option<AnsiColor>,
    pub bold: bool,
    pub dim: bool,
    pub italic: bool,
    pub underline: bool,
}

impl AnsiStyle {
    fn is_plain(&self) -> bool {
        *self == AnsiStyle::default()
    }
}

/// The eight standard colours, tuned per theme so both stay readable: the
/// light values are darkened and the dark values lightened, since the same
/// hue at one brightness disappears against one of the backgrounds.
const STANDARD: [(AnsiColor, &str, &str); 8] = [
    (AnsiColor::Indexed(0), "#24292f", "#6e7681"),
    (AnsiColor::Indexed(1), "#cf222e", "#ff7b72"),
    (AnsiColor::Indexed(2), "#116329", "#3fb950"),
    (AnsiColor::Indexed(3), "#4d2d00", "#d29922"),
    (AnsiColor::Indexed(4), "#0969da", "#58a6ff"),
    (AnsiColor::Indexed(5), "#8250df", "#bc8cff"),
    (AnsiColor::Indexed(6), "#1b7c83", "#39c5cf"),
    (AnsiColor::Indexed(7), "#6e7781", "#b1bac4"),
];

/// Resolves one colour to a hex string for a theme.
fn hex_for(color: AnsiColor, dark: bool) -> String {
    if let AnsiColor::Rgb(r, g, b) = color {
        return format!("#{r:02x}{g:02x}{b:02x}");
    }
    let AnsiColor::Indexed(index) = color else {
        unreachable!("rgb colours are handled above")
    };
    if let Some((_, light, dark_hex)) = STANDARD.get(index as usize) {
        let hex = if dark { *dark_hex } else { *light };
        return hex.to_string();
    }
    let (r, g, b) = if index < 16 {
        // Bright variants reuse the standard hue at the opposite end of the
        // range, which is how terminals draw them.
        let base = STANDARD[(index % 8) as usize];
        let hex = if dark { base.1 } else { base.2 };
        parse_hex(hex)
    } else {
        xterm_cube(index)
    };
    format!("#{r:02x}{g:02x}{b:02x}")
}

fn parse_hex(hex: &str) -> (u8, u8, u8) {
    let value = u32::from_str_radix(hex.trim_start_matches('#'), 16).unwrap_or(0);
    (
        ((value >> 16) & 0xff) as u8,
        ((value >> 8) & 0xff) as u8,
        (value & 0xff) as u8,
    )
}

/// The 6x6x6 colour cube plus the greyscale ramp used by xterm indices 16
/// through 255.
fn xterm_cube(index: u8) -> (u8, u8, u8) {
    const LEVELS: [u8; 6] = [0, 95, 135, 175, 215, 255];
    if index >= 232 {
        let grey = 8 + (index as u16 - 232) * 10;
        let level = grey.min(255) as u8;
        return (level, level, level);
    }
    let offset = index - 16;
    (
        LEVELS[(offset / 36) as usize],
        LEVELS[((offset % 36) / 6) as usize],
        LEVELS[(offset % 6) as usize],
    )
}

fn style_entry(style: AnsiStyle) -> Option<SnippetStyle> {
    if style.is_plain() {
        return None;
    }
    let (light, dark) = match style.color {
        Some(color) => (hex_for(color, false), hex_for(color, true)),
        None => {
            // A bold or dim run with no colour still needs a visible value,
            // so it inherits the foreground and relies on weight alone.
            ("#1f2328".to_string(), "#e6edf3".to_string())
        }
    };
    Some(SnippetStyle {
        light,
        dark,
        bold: style.bold,
        italic: style.italic,
        underline: style.underline,
    })
}

/// The styled run currently open on a line: where it began in the output and
/// the style it carries.
#[derive(Default)]
struct Run {
    start: usize,
    style_id: Option<u32>,
}

/// One parsed log line: the escape-free text, and the span triples the
/// frontend already renders for code fences, as `[start, len, style_id]`.
pub struct ParsedLine {
    pub text: String,
    pub spans: Vec<u32>,
}

/// Accumulates the style table across every line of a job log, so the span
/// ids in each line index one shared list.
#[derive(Default)]
pub struct AnsiTable {
    styles: Vec<SnippetStyle>,
    ids: HashMap<AnsiStyle, u32>,
}

impl AnsiTable {
    pub fn styles(&self) -> &[SnippetStyle] {
        &self.styles
    }

    /// Interns a style, returning its 1-based id as the span format wants.
    fn id_for(&mut self, style: AnsiStyle) -> Option<u32> {
        let entry = style_entry(style)?;
        if let Some(id) = self.ids.get(&style) {
            return Some(*id);
        }
        self.styles.push(entry);
        let id = self.styles.len() as u32;
        self.ids.insert(style, id);
        Some(id)
    }
}

/// GitHub's own workflow commands. They structure the log for the web UI and
/// mean nothing to a reader, so they are dropped rather than shown.
const WORKFLOW_COMMAND: [&str; 7] = [
    "##[group]",
    "##[endgroup]",
    "##[command]",
    "##[error]",
    "##[warning]",
    "##[notice]",
    "##[debug]",
];

/// Removes the workflow command markers from a line, keeping the group title
/// and the command's own text, which follow the marker on the same line.
pub fn strip_workflow_commands(line: &str) -> String {
    WORKFLOW_COMMAND
        .iter()
        .fold(line.to_string(), |out, marker| out.replace(marker, ""))
}

/// The prefix the runner puts on the shell command it ran, which is how a
/// reader tells a command the runner executed from a command a tool printed.
const COMMAND_PREFIX: &str = "[command]";

/// The style a `[command]` line is shown in. GitHub dims these to separate
/// them from a tool's own output, and blue reads the same way here while
/// staying distinct from the cyan and green a build tool uses.
fn command_style() -> AnsiStyle {
    AnsiStyle {
        color: Some(AnsiColor::Indexed(4)),
        ..AnsiStyle::default()
    }
}

/// Strips the `[command]` prefix from a line, returning the command it marks.
///
/// The prefix is only a marker, so showing it adds noise to every line the
/// runner echoes, and the line is styled separately by
/// [`parse_command_line`] instead.
pub fn strip_command_prefix(line: &str) -> Option<&str> {
    line.strip_prefix(COMMAND_PREFIX)
}

/// Parses a line the runner marked as a command, dropping the prefix and
/// styling what remains so it reads as the runner's own line.
pub fn parse_command_line(line: &str, table: &mut AnsiTable) -> Option<ParsedLine> {
    let command = strip_command_prefix(line)?;
    // The style id is interned once, so every command line shares one entry
    // however many the log has.
    let id = table.id_for(command_style())?;
    let text = command.to_string();
    let spans = if text.is_empty() {
        Vec::new()
    } else {
        vec![
            0,
            text.chars().map(char::len_utf16).sum::<usize>() as u32,
            id,
        ]
    };
    Some(ParsedLine { text, spans })
}

/// Splits a log line into runs of constant style, dropping the escapes. A
/// styled run becomes one span triple, which is the format `HighlightedLine`
/// already consumes for code fences and diffs.
///
/// `log_is_coloured` is true when the log carries escape bytes somewhere. A
/// log that does is one where the producer emitted ANSI, so a bare `[32m`
/// left behind by a lossy hop is still an escape worth honouring. A log with
/// no escapes anywhere keeps its brackets as text, because in a log that never
/// colourised anything those brackets are the content.
pub fn parse_line(line: &str, table: &mut AnsiTable, log_is_coloured: bool) -> ParsedLine {
    let mut style = AnsiStyle::default();
    let mut text = String::with_capacity(line.len());
    let mut spans: Vec<u32> = Vec::new();
    // Offsets are counted in UTF-16 units, not bytes, because the span
    // format is consumed by string slicing in the frontend where a non-ASCII
    // character like a check mark is one unit and not three bytes.
    let mut units: usize = 0;
    let mut run = Run::default();

    fn close_run(spans: &mut Vec<u32>, run: &mut Run, units: usize) {
        if let Some(id) = run.style_id {
            if units > run.start {
                spans.extend_from_slice(&[run.start as u32, (units - run.start) as u32, id]);
            }
        }
        run.style_id = None;
    }

    /// Reads the parameters and final byte of a CSI sequence, given its
    /// already-consumed introducer.
    fn read_csi(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) -> (String, Option<char>) {
        let mut params = String::new();
        let mut final_byte = None;
        for next in chars.by_ref() {
            if next.is_ascii_alphabetic() {
                final_byte = Some(next);
                break;
            }
            if !next.is_ascii_digit() && next != ';' {
                final_byte = None;
                break;
            }
            params.push(next);
        }
        (params, final_byte)
    }

    let mut chars = line.chars().peekable();
    while let Some(ch) = chars.next() {
        let bare = log_is_coloured && ch == '[';
        if ch != '\u{1b}' && !bare {
            text.push(ch);
            units += ch.len_utf16();
            continue;
        }
        // A CSI sequence is `\x1b[`, or a bare `[` in a log known to be
        // coloured. A bare `[` only starts one when a parameter follows
        // immediately, which is what separates a colour from GitHub's own
        // `##[group]` workflow commands.
        if ch == '\u{1b}' {
            if chars.peek() != Some(&'[') {
                continue;
            }
            chars.next();
        } else if !matches!(chars.peek(), Some('0'..='9') | Some(';')) {
            text.push('[');
            units += 1;
            continue;
        }
        let (params, final_byte) = read_csi(&mut chars);
        if final_byte != Some('m') {
            // Not styling, so the introducer and its parameters are dropped
            // rather than shown as text.
            continue;
        }
        close_run(&mut spans, &mut run, units);
        apply_sgr(&params, &mut style);
        if !style.is_plain() {
            run.start = units;
            run.style_id = table.id_for(style);
        }
    }
    close_run(&mut spans, &mut run, units);

    ParsedLine { text, spans }
}

/// Applies one SGR parameter list to the running style.
fn apply_sgr(params: &str, style: &mut AnsiStyle) {
    let codes: Vec<u8> = params
        .split(';')
        .filter_map(|part| {
            if part.is_empty() {
                return Some(0);
            }
            part.parse::<u16>()
                .ok()
                .filter(|code| *code <= u8::MAX as u16)
                .map(|code| code as u8)
        })
        .collect();
    let mut index = 0;
    while index < codes.len() {
        let code = codes[index];
        // 38 and 48 carry a colour selector in the following parameters.
        if code == 38 || code == 48 {
            if let Some(kind) = codes.get(index + 1) {
                if *kind == 5 {
                    if let Some(value) = codes.get(index + 2) {
                        if code == 38 {
                            style.color = Some(AnsiColor::Indexed(*value));
                        }
                        index += 2;
                    }
                } else if *kind == 2 {
                    let r = codes.get(index + 2).copied().unwrap_or(0);
                    let g = codes.get(index + 3).copied().unwrap_or(0);
                    let b = codes.get(index + 4).copied().unwrap_or(0);
                    if code == 38 {
                        style.color = Some(AnsiColor::Rgb(r, g, b));
                    }
                    index += 4;
                }
            }
            index += 1;
            continue;
        }
        match code {
            0 => *style = AnsiStyle::default(),
            1 => style.bold = true,
            2 => style.dim = true,
            3 => style.italic = true,
            4 => style.underline = true,
            22 => {
                style.bold = false;
                style.dim = false;
            }
            23 => style.italic = false,
            24 => style.underline = false,
            39 => style.color = None,
            30..=37 => style.color = Some(AnsiColor::Indexed(code - 30)),
            90..=97 => style.color = Some(AnsiColor::Indexed(code - 90 + 8)),
            // Background colours are dropped: the log already sits on a
            // background, and painting over it loses the theme's surface.
            _ => {}
        }
        index += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(log: &str) -> (Vec<ParsedLine>, Vec<SnippetStyle>) {
        let mut table = AnsiTable::default();
        let coloured = log.contains('\u{1b}');
        let lines = log
            .lines()
            .map(|line| parse_line(line, &mut table, coloured))
            .collect();
        (lines, table.styles)
    }

    #[test]
    fn colour_run_becomes_a_span_and_escapes_are_dropped() {
        let (lines, styles) = parse("\u{1b}[32mPASS\u{1b}[0m failed");
        assert_eq!(lines[0].text, "PASS failed");
        assert_eq!(lines[0].spans, vec![0, 4, 1]);
        assert_eq!(styles.len(), 1);
        assert_eq!(styles[0].dark, "#3fb950");
    }

    #[test]
    fn a_line_with_no_escapes_has_no_spans() {
        let (lines, styles) = parse("plain output");
        assert_eq!(lines[0].text, "plain output");
        assert!(lines[0].spans.is_empty());
        assert!(styles.is_empty());
    }

    #[test]
    fn reset_returns_to_the_unstyled_table_entry() {
        let (lines, _) = parse("\u{1b}[1mbold\u{1b}[0m plain");
        assert_eq!(lines[0].text, "bold plain");
        assert_eq!(lines[0].spans, vec![0, 4, 1]);
    }

    #[test]
    fn out_of_range_sgr_parameters_are_dropped_instead_of_resetting() {
        let (lines, _) = parse("\u{1b}[1mbold\u{1b}[99999m still bold");
        assert_eq!(lines[0].text, "bold still bold");
        assert_eq!(lines[0].spans, vec![0, 4, 1, 4, 11, 1]);
    }

    #[test]
    fn truecolor_is_kept_verbatim() {
        let (lines, styles) = parse("\u{1b}[38;2;255;128;0morange");
        assert_eq!(lines[0].text, "orange");
        assert_eq!(styles[0].light, "#ff8000");
        assert_eq!(styles[0].dark, "#ff8000");
    }

    #[test]
    fn indexed_colours_use_the_cube() {
        let (lines, styles) = parse("\u{1b}[38;5;196mred\u{1b}[0m");
        assert_eq!(lines[0].text, "red");
        assert_eq!(styles[0].dark, "#ff0000");
    }

    #[test]
    fn a_real_vitest_line_keeps_its_colour() {
        // As GitHub serves it: a stamp, two spaces, then the escapes.
        let line = concat!(
            "2026-09-26T19:13:20.0153062Z  \u{1b}[32m\u{2713}\u{1b}[39m ",
            "tests/components/window-reveal.test.tsx ",
            "\u{1b}[2m(\u{1b}[22m\u{1b}[2m3 tests\u{1b}[22m\u{1b}[2m)\u{1b}[22m",
            "\u{1b}[32m 35\u{1b}[2mms\u{1b}[39m"
        );
        let (lines, styles) = parse(line);
        assert!(!lines[0].text.contains('\u{1b}'));
        assert!(!lines[0].text.contains("[32m"));
        // The stamp is stripped by the caller, which routes on it.
        assert!(
            lines[0]
                .text
                .ends_with("2026-09-26T19:13:20.0153062Z  \u{2713} tests/components/window-reveal.test.tsx (3 tests) 35ms")
        );
        assert!(!lines[0].spans.is_empty());
        assert!(!styles.is_empty());
    }

    #[test]
    fn a_line_whose_escapes_were_already_stripped_stays_plain() {
        // Nothing is invented when the log carries no escape bytes at all, so
        // a literal marker in real output is left as the text it is.
        let (lines, styles) = parse("2026-01-01T00:00:00Z  [32m\u{2713}[39m done");
        assert!(lines[0].text.contains("[32m"));
        assert!(lines[0].spans.is_empty());
        assert!(styles.is_empty());
    }

    #[test]
    fn a_bare_marker_is_honoured_only_in_a_log_that_is_coloured() {
        // The producer emitted ANSI, so a marker that lost its ESC byte on
        // the way through is still an escape.
        let log = "first line plain\n\u{1b}[32mcoloured\u{1b}[0m\n[32mrest\n";
        let (lines, _) = parse(log);
        assert_eq!(lines[2].text, "rest");
        assert_eq!(lines[2].spans, vec![0, 4, 1]);
        assert!(lines[0].spans.is_empty());
    }

    #[test]
    fn workflow_commands_do_not_corrupt_the_text() {
        // A bare `[` followed by a letter is not a colour, so `##[group]`
        // must survive intact for the caller to strip.
        let (lines, _) = parse("\u{1b}[32mset\u{1b}[0m ##[group]checkout ##[command]git init");
        assert!(lines[0].text.contains("##[group]checkout"));
        assert!(lines[0].text.contains("##[command]git init"));
        assert_eq!(
            strip_workflow_commands("##[group]Run actions/checkout@v7"),
            "Run actions/checkout@v7"
        );
        assert_eq!(strip_workflow_commands("##[endgroup]"), "");
    }

    #[test]
    fn a_command_prefix_is_dropped_and_the_line_is_blue() {
        let mut table = AnsiTable::default();
        let line = parse_command_line("[command]/usr/bin/git log -1 --format=%H", &mut table)
            .expect("the prefix marks a command");
        assert_eq!(line.text, "/usr/bin/git log -1 --format=%H");
        assert_eq!(line.spans.len(), 3);
        // The whole of what is left is the span, since the prefix is gone.
        let len = line.text.chars().map(char::len_utf16).sum::<usize>() as u32;
        assert_eq!(&line.spans[0..2], [0, len]);

        let id = line.spans[2] as usize;
        let style = table.styles()[id - 1].clone();
        // Blue in both themes, so the line reads the same either way.
        assert_eq!(style.light, "#0969da");
        assert_eq!(style.dark, "#58a6ff");
    }

    #[test]
    fn a_command_prefix_keeps_the_rest_of_the_line_intact() {
        // A command can carry a colour of its own, and the prefix must not
        // swallow the text or the escape that follows it.
        let mut table = AnsiTable::default();
        let line = parse_command_line("[command]\u{1b}[32mgit init", &mut table)
            .expect("the prefix marks a command");
        assert_eq!(strip_command_prefix("[command]git init"), Some("git init"));
        assert!(line.text.contains("git init"));
    }

    #[test]
    fn a_line_without_the_prefix_is_not_a_command() {
        let mut table = AnsiTable::default();
        assert!(parse_command_line("git init", &mut table).is_none());
        assert!(parse_command_line("##[command]git init", &mut table).is_none());
        assert_eq!(strip_command_prefix("##[command]git init"), None);
    }

    #[test]
    fn an_empty_command_is_dropped() {
        let mut table = AnsiTable::default();
        let line = parse_command_line("[command]", &mut table).expect("still a command");
        assert_eq!(line.text, "");
        assert!(line.spans.is_empty(), "an empty line has no span");
    }

    #[test]
    fn bright_variants_differ_from_the_standard_hue() {
        let (bright_lines, bright) = parse("\u{1b}[92mbright green\u{1b}[0m");
        let (plain_lines, plain) = parse("\u{1b}[32mgreen\u{1b}[0m");
        assert_eq!(bright_lines[0].text, "bright green");
        assert_eq!(plain_lines[0].text, "green");
        assert_ne!(bright[0].dark, plain[0].dark);
    }
}
