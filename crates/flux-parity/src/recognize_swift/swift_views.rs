//! Swift view-tree parsers: recovers individual [`ViewNode`] subtrees from
//! emitted SwiftUI source (after the component/`body` wrapper has been stripped).
//!
//! These helpers are invoked by [`super::parse_body`] while walking a component's
//! `body` block. They are private to the Swift recognizer.

use crate::bridge::canonicalize_expr;
use crate::model::{ViewNode, is_container, normalize_view_name};
use crate::tokenize::{Token, match_brace, match_paren};

use super::{SwiftRecognitionError, parse_body};

pub(crate) fn parse_if(
    tokens: &[Token],
    start: usize,
) -> Result<(ViewNode, usize), SwiftRecognitionError> {
    let mut j = start + 1;
    while j < tokens.len() && tokens[j].text != "{" {
        j += 1;
    }
    let cond = tokens[start + 1..j]
        .iter()
        .map(|t| t.text.as_str())
        .collect::<Vec<_>>()
        .join(" ");
    let (then_branch, after_then) = parse_body(tokens, j)?;
    let mut i = after_then;
    let mut else_branch = Vec::new();
    if tokens.get(i).map(|t| t.text.as_str()) == Some("else") {
        let mut k = i + 1;
        while k < tokens.len() && tokens[k].text != "{" {
            k += 1;
        }
        let (els, after_else) = parse_body(tokens, k)?;
        else_branch = els;
        i = after_else;
    }
    Ok((
        ViewNode::If {
            cond: canonicalize_expr(&cond),
            then_branch,
            else_branch,
        },
        i,
    ))
}

/// Parses `ForEach(<coll>, id: <key>) { item in … }`. The `item in` body is
/// emitted empty by the codegen (FLUX-014); parity treats an empty body as the
/// expected, faithful shape.
pub(crate) fn parse_for_each(
    tokens: &[Token],
    start: usize,
) -> Result<(ViewNode, usize), SwiftRecognitionError> {
    let mut j = start + 1;
    while j < tokens.len() && tokens[j].text != "(" {
        j += 1;
    }
    let mut depth = 0usize;
    let mut k = j;
    let mut args = Vec::new();
    let mut cur = String::new();
    while k < tokens.len() {
        let t = &tokens[k].text;
        if t == "(" {
            depth += 1;
            if depth == 1 {
                k += 1;
                continue;
            }
        } else if t == ")" {
            depth -= 1;
            if depth == 0 {
                if !cur.is_empty() {
                    args.push(std::mem::take(&mut cur));
                }
                break;
            }
        } else if t == "," && depth == 1 {
            args.push(std::mem::take(&mut cur));
            k += 1;
            continue;
        }
        cur.push_str(t);
        k += 1;
    }
    let collection = args
        .first()
        .map(|s| canonicalize_expr(s.trim()))
        .unwrap_or_default();
    let key_path = args
        .iter()
        .find_map(|a| a.trim().strip_prefix("id:").map(str::trim))
        .map(canonicalize_expr)
        .unwrap_or_default();
    let mut m = k + 1;
    while m < tokens.len() && tokens[m].text != "{" {
        m += 1;
    }
    let (_body, end) = if m < tokens.len() {
        parse_body(tokens, m).unwrap_or_default()
    } else {
        (vec![], m + 1)
    };
    let _ = _body;
    Ok((
        ViewNode::ForEach {
            collection,
            key_path,
        },
        end,
    ))
}

/// Parses `NavigationStack { … }` (the `Router` node). Screen destinations are
/// emitted as `// Screen route: "..."` comments followed by the screen component
/// call on the next line, e.g.:
///
/// ```text
/// NavigationStack {
///     // Screen route: "home"
///     Home()
///     // Screen route: "profile"
///     Profile()
/// }
/// ```
///
/// Each `// Screen route:` comment opens a new `Screen` node; the view call(s)
/// that follow (until the next comment or the closing brace) become that
/// screen's children — mirroring the dev path, where `Screen` nodes are nested
/// directly under `Router`.
pub(crate) fn parse_navigation_stack(
    tokens: &[Token],
    start: usize,
) -> Result<(ViewNode, usize), SwiftRecognitionError> {
    let mut j = start + 1;
    while j < tokens.len() && tokens[j].text != "{" {
        j += 1;
    }
    let open = j;
    let close =
        match_brace(tokens, open).ok_or_else(|| SwiftRecognitionError("nav unbalanced".into()))?;

    let mut screens = Vec::new();
    let mut current: Option<ViewNode> = None;
    let mut i = open + 1;
    while i < close {
        let tok = &tokens[i].text;
        if tok == "{" || tok == "}" {
            i += 1;
            continue;
        }
        if tok == "//" {
            // Close any open screen and start a new one.
            if let Some(s) = current.take() {
                screens.push(s);
            }
            if let Some(node) = parse_screen_comment(tokens, i) {
                current = Some(node);
            }
            i += 1;
            continue;
        }
        // A view expression `Name(...)` becomes a child of the current screen
        // (or is dropped if no screen is open, which should not happen).
        if tok
            .chars()
            .next()
            .is_some_and(|c| c.is_alphabetic() || c == '_')
            && tokens.get(i + 1).map(|t| t.text.as_str()) == Some("(")
        {
            if let Some(screen) = current.as_mut() {
                let (child, next) = parse_view(tokens, i)?;
                if let ViewNode::Screen { children, .. } = screen {
                    children.push(child);
                }
                i = next;
                continue;
            }
        }
        i += 1;
    }
    if let Some(s) = current.take() {
        screens.push(s);
    }
    Ok((ViewNode::Router { children: screens }, close + 1))
}

/// Parses a `// Screen route: "..."` comment into a [`ViewNode::Screen`] with an
/// empty body (the screen's content component call follows as a sibling and is
/// recovered as a normal child of the Router). The colon is a token delimiter, so
/// the comment is `// Screen route : "home"` in token form; we extract the quoted
/// route substring rather than relying on a literal `Screen route:`.
pub(crate) fn parse_screen_comment(tokens: &[Token], start: usize) -> Option<ViewNode> {
    let joined = tokens[start..]
        .iter()
        .take_while(|t| t.text == "//" || t.line == tokens[start].line)
        .map(|t| t.text.clone())
        .collect::<String>();
    // The comment is `// Screen route: "home"` in source, but the colon is a token
    // delimiter and `route`/`"home"` may be concatenated (`routeroute"home"`), so
    // we simply take the first quoted substring as the route literal.
    let route = joined
        .find('"')
        .and_then(|open| {
            let close = joined[open + 1..].find('"')? + open + 1;
            Some(joined[open + 1..close].to_owned())
        })
        .unwrap_or_default();
    Some(ViewNode::Screen {
        route: canonicalize_expr(&route),
        children: vec![],
    })
}

/// Parses a view expression `Name(...)` possibly with a `{ … }` trailing block —
/// e.g. `Text("…")`, `VStack(spacing: 12) { … }`, `Button(action: {}) { … }`.
pub(crate) fn parse_view(
    tokens: &[Token],
    start: usize,
) -> Result<(ViewNode, usize), SwiftRecognitionError> {
    let name = tokens[start].text.clone();
    let normalized = normalize_view_name(&name);
    let mut i = start + 1;
    // Skip the `( ... )` argument list (it may be empty, e.g. `Avatar()`,
    // `Home()`, or contain the `action: {}` closure — none affect structure).
    if tokens.get(i).map(|t| t.text.as_str()) == Some("(") {
        let end = match_paren(tokens, i)
            .ok_or_else(|| SwiftRecognitionError(format!("unbalanced args in {normalized}")))?;
        i = end + 1;
    }
    if tokens.get(i).map(|t| t.text.as_str()) == Some("{") {
        // Container layouts (Column/Row/VStack/HStack/Stack) carry real structural
        // children. Every other adapter is a leaf: its trailing block is a codegen
        // placeholder (e.g. `Button(action: {}) { Text("") }`) and must not be
        // recovered as a child — the dev path models those adapters as childless.
        if is_container(&normalized) {
            let (children, after) = parse_body(tokens, i)?;
            return Ok((
                ViewNode::Primitive {
                    name: normalized,
                    props: vec![],
                    children,
                },
                after,
            ));
        }
        // Leaf adapter: consume the (empty or placeholder) block and stay childless.
        let end = match_brace(tokens, i)
            .ok_or_else(|| SwiftRecognitionError(format!("unbalanced body in {normalized}")))?;
        i = end + 1;
    }
    Ok((
        ViewNode::Primitive {
            name: normalized,
            props: vec![],
            children: vec![],
        },
        i,
    ))
}
