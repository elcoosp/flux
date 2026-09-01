//! Recognizer: Kotlin/Compose emitted source → structural [`ViewNode`] tree.
//!
//! The `flux-codegen-kotlin` backend emits a deterministic shape:
//!
//! ```text
//! @Composable fun Name(...) {
//!     var count by remember { mutableStateOf<Int>(0) }   // skipped
//!     Column(horizontalAlignment = ..., verticalArrangement = ...) {
//!         Text("Count: ${count}")
//!         Button(onClick = { }) { Text("") }
//!     }
//! }
//! ```
//!
//! This module parses that emitted text — *not* arbitrary Kotlin — into the same
//! [`ViewNode`] shape the dev-path lowered IR is reduced to, so the two can be
//! compared for structural parity. `state`/`prop` declarations and the
//! `by remember { … }` wrappers are skipped; only the view tree is recovered.

use crate::model::{ViewNode, is_container, normalize_view_name};
use crate::tokenize::{Token, tokenize};

mod kotlin_views;
use kotlin_views::{parse_composable_dest, parse_if, parse_items, parse_nav_host, parse_view};

/// Recognizes a complete Kotlin program (the output of `codegen`) into a list of
/// top-level [`ViewNode::Component`] trees.
///
/// # Errors
///
/// Returns [`KotlinRecognitionError`] if a component's body cannot be located or
/// the emitted tree is unbalanced — which would indicate the codegen output
/// drifted away from the documented grammar for the B.3 examples.
pub fn recognize(
    _lowered: &[ViewNode],
    kotlin: &str,
) -> Result<Vec<ViewNode>, KotlinRecognitionError> {
    let tokens = tokenize(kotlin);
    let mut roots = Vec::new();
    let mut idx = 0;
    while idx < tokens.len() {
        if tokens[idx].text == "@Composable" {
            let (node, next) = parse_composable(&tokens, idx)?;
            roots.push(node);
            idx = next;
        } else {
            idx += 1;
        }
    }
    Ok(roots)
}

/// An error produced while recognizing Kotlin emitted source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KotlinRecognitionError(pub String);

impl std::fmt::Display for KotlinRecognitionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "kotlin recognition error: {}", self.0)
    }
}

impl std::error::Error for KotlinRecognitionError {}

/// Parses a `@Composable fun <Name>(...) { … }` component. `state`/`prop`
/// declarations (`var x by remember { … }`, `let y: T`) are skipped; only the
/// trailing view tree is recovered.
fn parse_composable(
    tokens: &[Token],
    start: usize,
) -> Result<(ViewNode, usize), KotlinRecognitionError> {
    let mut j = start + 1;
    while j < tokens.len() && tokens[j].text != "fun" {
        j += 1;
    }
    let name = tokens
        .get(j + 1)
        .ok_or_else(|| KotlinRecognitionError("composable missing name".into()))?
        .text
        .split(['<', '('])
        .next()
        .unwrap_or("")
        .to_owned();

    // Find the body `{` after the parameter list.
    let mut o = j + 2;
    while o < tokens.len() && tokens[o].text != "{" {
        o += 1;
    }
    let (children, end) = parse_body(tokens, o)?;
    Ok((ViewNode::Component { name, children }, end))
}

/// Parses the structural children inside a body block (whose opening brace is at
/// `open`), returning the children and the index just past the matching `}`.
pub(crate) fn parse_body(
    tokens: &[Token],
    open: usize,
) -> Result<(Vec<ViewNode>, usize), KotlinRecognitionError> {
    let mut children = Vec::new();
    let mut i = open + 1;
    let mut depth = 1usize;
    while i < tokens.len() {
        let tok = tokens[i].text.clone();
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
        // Skip declaration keywords and their binding (`var x by remember …`).
        if matches!(tok.as_str(), "var" | "val" | "let" | "by") {
            i += 1;
            continue;
        }
        if tok == "if" {
            let (node, next) = parse_if(tokens, i)?;
            children.push(node);
            i = next;
            continue;
        }
        if tok == "items" {
            let (node, next) = parse_items(tokens, i)?;
            children.push(node);
            i = next;
            continue;
        }
        if tok == "NavHost" {
            let (node, next) = parse_nav_host(tokens, i)?;
            children.push(node);
            i = next;
            continue;
        }
        if tok == "composable" {
            let (node, next) = parse_composable_dest(tokens, i)?;
            children.push(node);
            i = next;
            continue;
        }
        // A generic view expression is `Name(...)` where the identifier is not a
        // known non-view token. FLUX-038 overlay containers are emitted with no
        // argument list (`Dialog {`, `ModalBottomSheet {`, `AlertDialog {`), so we
        // also dispatch a bare `Name {` when `Name` is a known container.
        let is_ident = tok
            .chars()
            .next()
            .is_some_and(|c| c.is_alphabetic() || c == '_');
        let next = tokens.get(i + 1).map(|t| t.text.as_str());
        if is_ident
            && ((next == Some("(") && !is_non_view(strip_generics(&tok)))
                || (next == Some("{") && is_container(&normalize_view_name(&tok))))
        {
            let (node, next_idx) = parse_view(tokens, i)?;
            children.push(node);
            i = next_idx;
            continue;
        }
        i += 1;
    }
    Err(KotlinRecognitionError("unbalanced braces in body".into()))
}

/// Identifiers that may be followed by `(` in emitted Kotlin but are NOT view
/// expressions (they are declarations, type-constructors, or layout helpers
/// nested inside adapter argument lists).
pub(crate) fn is_non_view(text: &str) -> bool {
    matches!(
        text,
        "remember"
            | "mutableStateOf"
            | "painterResource"
            | "Alignment"
            | "Arrangement"
            | "named"
            | "listOf"
            | "mutableStateListOf"
            | "rememberNavController"
            | "navController"
    )
}

/// Strips a trailing `<...>` generic suffix so `mutableStateOf<Int>` matches the
/// `mutableStateOf` entry in [`is_non_view`] and view names compare uniformly.
pub(crate) fn strip_generics(name: &str) -> &str {
    name.split('<').next().unwrap_or(name)
}
