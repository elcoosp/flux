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
    
    // Check if this is a screen selection pattern: `if route == "route_name" { ... }`
    // When inside a NavigationStack (Router), screens are emitted as conditionals
    // that match on the route state. Convert these to Screen nodes for parity.
    let if_tokens: Vec<String> = cond.split_whitespace().map(|s| s.to_string()).collect();
    if if_tokens.len() == 3 
        && if_tokens[0] == "route" 
        && (if_tokens[1] == "==" || if_tokens[1] == "!=") 
    {
        // Extract the route from the condition (third token, which may be a quoted string)
        let route_text = &if_tokens[2];
        let route = route_text
            .trim_start_matches('"')
            .trim_end_matches('"')
            .to_owned();
        if !route.is_empty() {
            return Ok((
                ViewNode::Screen {
                    route: canonicalize_expr(&route),
                    children: then_branch,
                },
                i,
            ));
        }
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
/// now emitted as `if route == "..."` conditionals (matching the new codegen pattern).
///
/// ```text
/// NavigationStack(path: $route) {
///     if route == "home" { ... }
///     if route == "settings" { ... }
/// }
/// ```
///
/// Each `if route == "..."` conditional opens a new `Screen` node; the view call(s)
/// inside the conditional become that screen's children — mirroring the dev path,
/// where `Screen` nodes are nested directly under `Router`.
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

    // Parse children inside NavigationStack. The emit_router function now
    // emits screens as `if route == "name" { ... }` conditionals, which
    // parse_if will recognize and convert to Screen nodes.
    let (children, _) = parse_body(tokens, open)?;
    
    Ok((ViewNode::Router { children }, close + 1))
}

/// Parses a `// Screen route: "..."` comment into a [`ViewNode::Screen`] with an
/// empty body (the screen's content component call follows as a sibling and is
/// recovered as a normal child of the Router).
///
/// NOTE: This is kept for backwards compatibility with existing fixtures that
/// may still use the old emission pattern. The new pattern uses `if route == "..."` 
/// conditionals instead.
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
///
/// Overlay containers (`Modal`/`Sheet`/`Dialog`, FLUX-038) are emitted with no
/// argument list — `FullScreenCover { … }`, `Sheet { … }`, `Alert { … }` — so the
/// trailing `{` may follow the name directly (no `( … )`). The same applies to
/// their Kotlin surface names (`Dialog {`, `ModalBottomSheet {`, `AlertDialog {`).
pub(crate) fn parse_view(
    tokens: &[Token],
    start: usize,
) -> Result<(ViewNode, usize), SwiftRecognitionError> {
    let name = tokens[start].text.clone();
    let normalized = normalize_view_name(&name);
    let mut i = start + 1;
    // Skip an optional `( ... )` argument list (present for normal adapters;
    // absent for no-arg overlay containers like `FullScreenCover {`).
    if tokens.get(i).map(|t| t.text.as_str()) == Some("(") {
        let end = match_paren(tokens, i)
            .ok_or_else(|| SwiftRecognitionError(format!("unbalanced args in {normalized}")))?;
        i = end + 1;
    }
    if tokens.get(i).map(|t| t.text.as_str()) == Some("{") {
        // Container layouts (Column/Row/VStack/HStack/Stack) and FLUX-038 overlay
        // containers (Modal/Sheet/Dialog) carry real structural children. Every
        // other adapter is a leaf: its trailing block is a codegen placeholder
        // (e.g. `Button(action: {}) { Text("") }`) and must not be recovered as a
        // child — the dev path models those adapters as childless.
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