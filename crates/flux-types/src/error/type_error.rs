use flux_syntax::{Span, TypeKind};
use std::fmt;
/// A type-checking failure.
///
/// Construct one with [`TypeError::mismatch`] / [`TypeError::new`] and render it
/// for a human with [`TypeError::render`], which needs the original source text
/// to turn the [`Span`] offsets into `file:line:col`.
#[derive(Clone, Debug, PartialEq)]
pub struct TypeError {
    /// What went wrong, in prose (e.g. "type mismatch in `Counter`").
    pub message: String,
    /// Why it went wrong and how to fix it, when known.
    pub hint: Option<String>,
    /// Where it went wrong, as raw byte offsets into the source file.
    pub span: Span,
    /// The type that was expected at this site, when applicable.
    pub expected: Option<Box<TypeKind>>,
    /// The type that was actually found, when applicable.
    pub actual: Option<Box<TypeKind>>,
}

impl TypeError {
    /// Builds a diagnostic with only a message and span.
    #[must_use]
    pub fn new(message: impl Into<String>, span: Span) -> Self {
        Self {
            message: message.into(),
            hint: None,
            span,
            expected: None,
            actual: None,
        }
    }

    /// Builds a type-mismatch diagnostic, capturing expected/actual.
    #[must_use]
    pub fn mismatch(expected: &crate::TcType, actual: &crate::TcType, span: Span) -> Self {
        let message = format!("expected `{expected}`, got `{actual}`");
        Self {
            message,
            hint: Some(
                "check that the expression on the right has the type declared \
                 on the left, or adjust the annotation"
                    .to_owned(),
            ),
            span,
            expected: Some(Box::new(expected.to_typekind())),
            actual: Some(Box::new(actual.to_typekind())),
        }
    }

    /// Attaches a "how" hint, returning `self` for chaining.
    #[must_use]
    pub fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    /// Renders the diagnostic as a Rust-style `file:line:col` message.
    ///
    /// `source` must be the text that was parsed to produce [`Span`]; line and
    /// column are computed from the span's start offset. The `path` is shown in
    /// the gutter.
    #[must_use]
    pub fn render(&self, source: &str, path: &str) -> String {
        let (line, col) = line_col(source, self.span.start as usize);
        let line_text = source.lines().nth(line.saturating_sub(1)).unwrap_or("");
        let mut out = String::new();
        out.push_str(&format!("error: {}\n", self.message));
        out.push_str(&format!("  --> {path}:{line}:{col}\n"));
        out.push_str("   |\n");
        out.push_str(&format!("{} | {}\n", line, line_text));
        out.push_str(&format!(
            "   | {}{}\n",
            " ".repeat(col.saturating_sub(1)),
            "^".repeat(usize::max(1, (self.span.end - self.span.start) as usize))
        ));
        if let Some(hint) = &self.hint {
            out.push_str(&format!("   |\n   = hint: {hint}\n"));
        }
        out
    }
}

/// Converts a byte `offset` into 1-based `(line, column)` within `source`.
#[must_use]
pub(crate) fn line_col(source: &str, offset: usize) -> (usize, usize) {
    let mut line = 1usize;
    let mut col = 1usize;
    for (i, ch) in source.char_indices() {
        if i >= offset {
            break;
        }
        if ch == '\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
    }
    (line, col)
}

impl fmt::Display for TypeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)?;
        if let Some(hint) = &self.hint {
            write!(f, " ({hint})")?;
        }
        Ok(())
    }
}
