//! Parse diagnostics in the Rust-style format required by AGENTS.md §3.7.

use std::fmt;

use flux_syntax::Span;
use thiserror::Error;

/// A 1-based line and column position within a source file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Location {
    /// 1-based line number.
    pub line: u32,
    /// 1-based column number, counted in characters.
    pub column: u32,
}

impl Location {
    /// Resolves the byte `offset` within `source` into a line and column.
    ///
    /// Offsets past the end of `source` resolve to the position just after the
    /// final character, which is what a parser reports for unexpected EOF.
    ///
    /// # Examples
    ///
    /// ```
    /// use flux_parser::Location;
    ///
    /// let at = Location::from_offset("ab\ncd", 3);
    /// assert_eq!((at.line, at.column), (2, 1));
    /// ```
    #[must_use]
    pub fn from_offset(source: &str, offset: usize) -> Self {
        let mut line = 1;
        let mut column = 1;
        for (index, character) in source.char_indices() {
            if index >= offset {
                break;
            }
            if character == '\n' {
                line += 1;
                column = 1;
            } else {
                column += 1;
            }
        }
        Self { line, column }
    }
}

impl fmt::Display for Location {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}:{}", self.line, self.column)
    }
}

/// A parse failure, carrying everything needed for an actionable diagnostic.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
#[error("{message} at {path}:{location}")]
pub struct ParseError {
    /// Short description of what went wrong.
    pub message: String,
    /// Optional hint explaining why it may have gone wrong and how to fix it.
    pub hint: Option<String>,
    /// Byte span the error points at.
    pub span: Span,
    /// Resolved line/column of `span.start`.
    pub location: Location,
    /// Display path of the offending file.
    pub path: String,
    /// The offending source line, with its trailing newline removed.
    pub line_text: String,
}

impl ParseError {
    /// Renders the error as a Rust-style diagnostic block.
    ///
    /// The output carries what went wrong, where, an optional hint, and a
    /// caret run under the offending bytes, per AGENTS.md §3.7.
    ///
    /// # Examples
    ///
    /// ```
    /// use flux_parser::parse;
    ///
    /// let error = parse("component Broken {", 0, "a.flux")
    ///     .expect_err("an unclosed brace must not parse");
    /// assert!(error.render().contains("a.flux:1:"));
    /// ```
    #[must_use]
    pub fn render(&self) -> String {
        let line_number = self.location.line.to_string();
        let gutter = " ".repeat(line_number.len());
        let caret_pad = " ".repeat(self.location.column.saturating_sub(1) as usize);
        let caret_width = usize::max(
            1,
            self.line_text
                .chars()
                .count()
                .saturating_sub(self.location.column.saturating_sub(1) as usize)
                .min(usize::max(1, self.span.len() as usize)),
        );
        let mut rendered = String::with_capacity(self.line_text.len() + 160);
        rendered.push_str(&format!("error: {}\n", self.message));
        rendered.push_str(&format!("  --> {}:{}\n", self.path, self.location));
        rendered.push_str(&format!("{gutter}   |\n"));
        rendered.push_str(&format!("{line_number} | {}\n", self.line_text));
        rendered.push_str(&format!(
            "{gutter}   | {caret_pad}{}\n",
            "^".repeat(caret_width)
        ));
        if let Some(hint) = &self.hint {
            rendered.push_str(&format!("{gutter}   |\n"));
            rendered.push_str(&format!("{gutter}   = hint: {hint}\n"));
        }
        rendered
    }
}

/// Extracts the line containing `offset`, without its line terminator.
pub(crate) fn line_at(source: &str, offset: usize) -> String {
    let clamped = offset.min(source.len());
    let start = source[..clamped].rfind('\n').map_or(0, |index| index + 1);
    let end = source[clamped..]
        .find('\n')
        .map_or(source.len(), |index| clamped + index);
    source[start..end].trim_end_matches('\r').to_owned()
}
