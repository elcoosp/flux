//! Recognizer: SwiftUI emitted source → structural [`ViewNode`] tree.
//!
//! The `flux-codegen-swift` backend emits a deterministic shape:
//!
//! ```text
//! struct Name: View {
//!     @State private var count: Int = 0        // skipped
//!     var body: some View {
//!         VStack(spacing: 12) {                 // adapter
//!             Text("Count: \(count)")           // adapter
//!             Button(action: {}) { Text("") }   // adapter
//!         }
//!     }
//! }
//! ```
//!
//! This module parses that emitted text — *not* arbitrary Swift — into the same
//! [`ViewNode`] shape the dev-path lowered IR is reduced to, so the two can be
//! compared for structural parity. Declarations of `state`/`props` and the
//! `var body: some View` wrapper are skipped; only the view tree is recovered.

use crate::model::{ViewNode, is_container, normalize_view_name};
use crate::tokenize::{Token, match_brace, tokenize};

mod swift_views;
use swift_views::{
    parse_for_each, parse_if, parse_navigation_stack, parse_screen_comment, parse_view,
};

/// Recognizes a complete Swift program (the output of `codegen`) into a list of
/// top-level [`ViewNode::Component`] trees.
///
/// # Errors
///
/// Returns [`SwiftRecognitionError`] if a component's `body` block cannot be
/// located or the emitted tree is unbalanced — which would indicate the codegen
/// output drifted away from the documented grammar for the B.3 examples.
pub fn recognize(
    _lowered: &[ViewNode],
    swift: &str,
) -> Result<Vec<ViewNode>, SwiftRecognitionError> {
    let tokens = tokenize(swift);
    let mut roots = Vec::new();
    let mut idx = 0;
    while idx < tokens.len() {
        if tokens[idx].text == "struct" {
            let (node, next) = parse_struct(&tokens, idx)?;
            roots.push(node);
            idx = next;
        } else {
            idx += 1;
        }
    }
    Ok(roots)
}

/// An error produced while recognizing Swift emitted source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SwiftRecognitionError(pub String);

impl std::fmt::Display for SwiftRecognitionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "swift recognition error: {}", self.0)
    }
}

impl std::error::Error for SwiftRecognitionError {}

/// Parses a `struct …: View { … }` component. The structural children are the
/// contents of the `var body: some View { … }` block (or the whole body when no
/// `body` wrapper is present, e.g. adapter-only components).
fn parse_struct(
    tokens: &[Token],
    start: usize,
) -> Result<(ViewNode, usize), SwiftRecognitionError> {
    let name = tokens
        .get(start + 1)
        .ok_or_else(|| SwiftRecognitionError("struct missing name".into()))?
        .text
        .split(['<', ' '])
        .next()
        .unwrap_or("")
        .to_owned();

    // Find the component's closing brace: the matching `}` for the first `{`.
    let open = tokens[start..]
        .iter()
        .position(|t| t.text == "{")
        .map(|i| start + i)
        .ok_or_else(|| SwiftRecognitionError("struct missing body".into()))?;
    let close = match_brace(tokens, open)
        .ok_or_else(|| SwiftRecognitionError("struct unbalanced".into()))?;

    // Locate the `var body: some View {` block if present; otherwise the whole
    // struct body is the view tree.
    let body_open = tokens[open..close]
        .iter()
        .position(|t| t.text == "body")
        .and_then(|rel| {
            // find the `{` after `body : some View`
            let from = open + rel;
            tokens[from..close]
                .iter()
                .position(|t| t.text == "{")
                .map(|i| from + i)
        });

    let (children, _) = if let Some(bo) = body_open {
        parse_body(tokens, bo)?
    } else {
        parse_body(tokens, open)?
    };

    Ok((ViewNode::Component { name, children }, close + 1))
}

/// Parses the structural children inside a body block (whose opening brace is at
/// `open`), returning the children and the index just past the matching `}`.
pub(crate) fn parse_body(
    tokens: &[Token],
    open: usize,
) -> Result<(Vec<ViewNode>, usize), SwiftRecognitionError> {
    let mut children = Vec::new();
    let mut i = open + 1;
    let mut depth = 1usize;
    while i < tokens.len() {
        let tok = &tokens[i].text;
        if tok == "{" {
            depth += 1;
            i += 1;
            continue;
        }
        if tok == "}" {
            depth -= 1;
            if depth == 0 {
                return Ok((children, i + 1));
            }
            i += 1;
            continue;
        }
        if tok == "if" {
            let (node, next) = parse_if(tokens, i)?;
            children.push(node);
            i = next;
            continue;
        }
        if tok == "ForEach" {
            let (node, next) = parse_for_each(tokens, i)?;
            children.push(node);
            i = next;
            continue;
        }
        if tok == "NavigationStack" {
            let (node, next) = parse_navigation_stack(tokens, i)?;
            children.push(node);
            i = next;
            continue;
        }
        if tok == "//" {
            if let Some(node) = parse_screen_comment(tokens, i) {
                children.push(node);
            }
            i += 1;
            continue;
        }
        // A generic view expression is `Name(...)` (identifier immediately
        // followed by `(`) or, for no-arg overlay containers (FLUX-038), a bare
        // `Name {` (e.g. `FullScreenCover {`). `parse_view` accepts both forms;
        // here we dispatch when the identifier is followed by `(` or by `{` and is
        // a known container.
        let is_ident = tok
            .chars()
            .next()
            .is_some_and(|c| c.is_alphabetic() || c == '_');
        let next = tokens.get(i + 1).map(|t| t.text.as_str());
        if is_ident
            && (next == Some("(") || (next == Some("{") && is_container(&normalize_view_name(tok))))
        {
            let (node, next_idx) = parse_view(tokens, i)?;
            children.push(node);
            i = next_idx;
            continue;
        }
        i += 1;
    }
    Err(SwiftRecognitionError("unbalanced braces in body".into()))
}
