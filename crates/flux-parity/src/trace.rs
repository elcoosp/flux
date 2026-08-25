//! T16 trace-diff comparison (ADR-0027 / reconcile-trace-format v1).
//!
//! Swift and Kotlin hosts emit per-phase signal-propagation traces as JSONL.
//! This module canonicalizes each line and compares two traces *exactly*,
//! returning the first divergence together with surrounding line context so a
//! developer can localize a cross-platform reconciliation bug.
//!
//! Canonical form per line (see `reconcile-trace-format.md` v1):
//! - `span` is rendered as `file:start-end` (`file` already defaults to the
//!   module path; the host-side `span` key is dropped because it is a
//!   platform-specific line:col pair and is *not* part of the canonical frame).
//! - JSON object key order is normalized (sorted keys).
//! - Whitespace is compacted.

mod json;

use std::fmt;
use std::path::Path;
use std::str::FromStr;

/// A trace phase: `1` dirty-set reconciliation, `2` topo propagation, `3` commit.
///
/// Phases are fixed by the reconcile protocol; only `1..=3` are valid.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Phase(u8);

impl Phase {
    /// Creates a phase from its `1..=3` numeric value.
    #[must_use]
    pub fn new(value: u8) -> Option<Phase> {
        match value {
            1..=3 => Some(Phase(value)),
            _ => None,
        }
    }

    /// Returns the numeric phase value.
    #[must_use]
    pub fn as_u8(self) -> u8 {
        self.0
    }
}

impl fmt::Display for Phase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for Phase {
    type Err = TraceError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let value: u8 = s
            .trim()
            .parse()
            .map_err(|_| TraceError::InvalidPhase(s.to_owned()))?;
        Phase::new(value).ok_or(TraceError::InvalidPhase(s.to_owned()))
    }
}

/// Extracts the phase from a trace filename suffix (`…p1.jsonl` / `…p2.jsonl` /
/// `…p3.jsonl`). Returns `None` when the suffix is absent or out of range.
#[must_use]
pub fn phase_from_filename(path: &Path) -> Option<Phase> {
    let stem = path.file_stem()?.to_str()?;
    let tail = stem.rsplit('.').next()?;
    let digit = tail.strip_prefix('p')?;
    let value: u8 = digit.parse().ok()?;
    Phase::new(value)
}

/// A single canonicalized trace line (stable, order-independent JSON).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Frame {
    /// 1-based line number in the source file.
    pub line: usize,
    /// Canonical JSON text for the line.
    pub canonical: String,
}

impl Frame {
    /// Canonicalizes one raw JSONL line.
    ///
    /// # Errors
    /// Returns [`TraceError::Json`] when the line is not valid JSON.
    pub fn canonicalize(line: usize, raw: &str) -> Result<Frame, TraceError> {
        let value = json::Json::parse(raw.trim()).map_err(|e| TraceError::Json(line, e))?;
        let canonical = value.canonical();
        Ok(Frame { line, canonical })
    }
}

/// Loads and canonicalizes every line of a trace file, preserving line numbers.
///
/// Blank lines are skipped. Line numbers are 1-based and count every non-empty
/// line in the source.
///
/// # Errors
/// Returns [`TraceError::Io`] when the file cannot be read, or [`TraceError::Json`]
/// when any non-blank line is not valid JSON.
pub fn load_trace(path: &Path) -> Result<Vec<Frame>, TraceError> {
    let text = std::fs::read_to_string(path).map_err(TraceError::Io)?;
    load_trace_str(&text)
}

/// Canonicalizes an in-memory trace buffer (used by tests and the CLI).
///
/// # Errors
/// Returns [`TraceError::Json`] when any non-blank line is not valid JSON.
pub fn load_trace_str(buffer: &str) -> Result<Vec<Frame>, TraceError> {
    let mut frames = Vec::new();
    for (idx, raw) in buffer.lines().enumerate() {
        if raw.trim().is_empty() {
            continue;
        }
        frames.push(Frame::canonicalize(idx + 1, raw)?);
    }
    Ok(frames)
}

/// The first point at which two traces differ, with surrounding context.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Divergence {
    /// 1-based line number of the divergence in the *first* (left) trace.
    pub left_line: usize,
    /// Canonical left frame at the divergence (if the left trace is longer).
    pub left: Option<String>,
    /// Canonical right frame at the divergence (if the right trace is longer).
    pub right: Option<String>,
    /// Number of trailing lines of surrounding context to render.
    pub context: usize,
}

impl Divergence {
    /// Renders a human-readable report with surrounding line context.
    #[must_use]
    pub fn render(&self, left: &[Frame], right: &[Frame]) -> String {
        let mut out = String::new();
        let n = self.context;
        let start = self.left_line.saturating_sub(n);
        out.push_str(&format!("trace divergence at line {}:\n", self.left_line));
        for l in start..self.left_line {
            if let Some(f) = left.get(l.wrapping_sub(1)) {
                out.push_str(&format!("  left  {}: {}\n", f.line, f.canonical));
            }
        }
        out.push_str(&format!(
            "  left  {}: {}\n",
            self.left_line,
            self.left.as_deref().unwrap_or("<end of trace>")
        ));
        out.push_str(&format!(
            "  right {}: {}\n",
            self.left_line,
            self.right.as_deref().unwrap_or("<end of trace>")
        ));
        let after = self.left_line;
        for l in after..(after + n).min(right.len()) {
            if let Some(f) = right.get(l) {
                out.push_str(&format!("  right {}: {}\n", f.line, f.canonical));
            }
        }
        out
    }
}

/// Compares two canonicalized traces exactly.
///
/// Returns `Ok(())` when every frame matches in order and length; otherwise
/// returns the first [`Divergence`].
pub fn compare(left: &[Frame], right: &[Frame]) -> Result<(), Divergence> {
    let context = 2;
    let len = left.len().min(right.len());
    for i in 0..len {
        if left[i].canonical != right[i].canonical {
            return Err(Divergence {
                left_line: left[i].line,
                left: Some(left[i].canonical.clone()),
                right: Some(right[i].canonical.clone()),
                context,
            });
        }
    }
    if left.len() != right.len() {
        let at = len + 1;
        return Err(Divergence {
            left_line: at,
            left: left.get(len).map(|f| f.canonical.clone()),
            right: right.get(len).map(|f| f.canonical.clone()),
            context,
        });
    }
    Ok(())
}

/// Loads two traces, compares them, and reports the result.
///
/// # Errors
/// Returns [`TraceError::Io`] / [`TraceError::Json`] for load failures, or
/// [`TraceError::Divergence`] for a real mismatch (with the rendered report).
pub fn diff_traces(left_path: &Path, right_path: &Path) -> Result<(), TraceError> {
    let left = load_trace(left_path)?;
    let right = load_trace(right_path)?;
    match compare(&left, &right) {
        Ok(()) => Ok(()),
        Err(div) => Err(TraceError::Divergence(div.render(&left, &right))),
    }
}

/// Errors produced by the trace-diff tool.
#[derive(Debug)]
pub enum TraceError {
    /// An I/O failure reading a trace file.
    Io(std::io::Error),
    /// A trace line was not valid JSON (carries the 1-based line number).
    Json(usize, String),
    /// The two traces diverged (carries the rendered report).
    Divergence(String),
    /// A phase string was not `1`, `2`, or `3`.
    InvalidPhase(String),
}

impl fmt::Display for TraceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TraceError::Io(e) => write!(f, "trace read error: {e}"),
            TraceError::Json(line, e) => write!(f, "trace JSON error at line {line}: {e}"),
            TraceError::Divergence(report) => write!(f, "{report}"),
            TraceError::InvalidPhase(s) => {
                write!(f, "invalid phase {s}: expected 1, 2, or 3")
            }
        }
    }
}

impl std::error::Error for TraceError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phase_parsing_bounds() {
        assert_eq!(Phase::new(1), Some(Phase(1)));
        assert_eq!(Phase::new(3), Some(Phase(3)));
        assert_eq!(Phase::new(0), None);
        assert_eq!(Phase::new(4), None);
        assert_eq!("2".parse::<Phase>().unwrap(), Phase(2));
        assert!("9".parse::<Phase>().is_err());
    }

    #[test]
    fn phase_from_filename_variants() {
        assert_eq!(
            phase_from_filename(Path::new("counter_1000.p1.jsonl")),
            Some(Phase(1))
        );
        assert_eq!(
            phase_from_filename(Path::new("a/b/noop_dispatch.p3.jsonl")),
            Some(Phase(3))
        );
        assert_eq!(phase_from_filename(Path::new("scenario.jsonl")), None);
        assert_eq!(phase_from_filename(Path::new("x.p9.jsonl")), None);
    }

    #[test]
    fn canonicalizes_key_order_and_span() {
        let f = Frame::canonicalize(
            1,
            r#"{"phase":1,"event":"mark","span":"flux://m#L1","kind":"state","node":"n1","signal":"s1"}"#,
        )
        .unwrap();
        // Keys are sorted; span is dropped because it is platform-specific.
        assert_eq!(
            f.canonical,
            r#"{"event":"mark","kind":"state","node":"n1","phase":1,"signal":"s1"}"#
        );
    }

    #[test]
    fn identical_traces_compare_equal() {
        let a = load_trace_str(
            "{\"event\":\"mark\",\"node\":\"n1\"}\n{\"event\":\"mark\",\"node\":\"n2\"}\n",
        )
        .unwrap();
        let b = load_trace_str(
            "{\"node\":\"n1\",\"event\":\"mark\"}\n{\"node\":\"n2\",\"event\":\"mark\"}\n",
        )
        .unwrap();
        assert!(compare(&a, &b).is_ok());
    }

    #[test]
    fn differing_value_diverges() {
        let a = load_trace_str("{\"event\":\"mark\",\"node\":\"n1\"}").unwrap();
        let b = load_trace_str("{\"event\":\"mark\",\"node\":\"n2\"}").unwrap();
        let err = compare(&a, &b).unwrap_err();
        assert_eq!(err.left_line, 1);
        assert_eq!(err.left.as_deref(), Some(r#"{"event":"mark","node":"n1"}"#));
        assert_eq!(
            err.right.as_deref(),
            Some(r#"{"event":"mark","node":"n2"}"#)
        );
    }

    #[test]
    fn length_mismatch_diverges() {
        let a = load_trace_str("{\"event\":\"a\"}\n{\"event\":\"b\"}").unwrap();
        let b = load_trace_str("{\"event\":\"a\"}").unwrap();
        let err = compare(&a, &b).unwrap_err();
        assert_eq!(err.left_line, 2);
        assert_eq!(err.right, None);
    }
}
