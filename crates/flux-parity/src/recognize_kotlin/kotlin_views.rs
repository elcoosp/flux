//! Kotlin view-tree parsers: recovers individual [`ViewNode`] subtrees from
//! emitted Compose source (after the component/`remember` wrapper has been stripped).
//!
//! These helpers are invoked by [`super::parse_body`] while walking a `@Composable
//! fun`'s body block. They are private to the Kotlin recognizer.

use crate::bridge::canonicalize_release_expr;
use crate::model::{ViewNode, is_container, normalize_view_name};
use crate::tokenize::{Token, match_brace, match_paren};

use super::{KotlinRecognitionError, parse_body};

pub(crate) fn parse_if(
    tokens: &[Token],
    start: usize,
) -> Result<(ViewNode, usize), KotlinRecognitionError> {
    let mut j = start + 1;
    while j < tokens.len() && tokens[j].text != "(" {
        j += 1;
    }
    let mut depth = 0usize;
    let mut k = j;
    let mut cond = String::new();
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
                break;
            }
        }
        cond.push_str(t);
        k += 1;
    }
    let mut m = k + 1;
    while m < tokens.len() && tokens[m].text != "{" {
        m += 1;
    }
    let (then_branch, after_then) = parse_body(tokens, m)?;
    let mut i = after_then;
    let mut else_branch = Vec::new();
    if tokens.get(i).map(|t| t.text.as_str()) == Some("else") {
        let mut q = i + 1;
        while q < tokens.len() && tokens[q].text != "{" {
            q += 1;
        }
        let (els, after_else) = parse_body(tokens, q)?;
        else_branch = els;
        i = after_else;
    }
    Ok((
        ViewNode::If {
            cond: canonicalize_release_expr(cond.trim()),
            then_branch,
            else_branch,
        },
        i,
    ))
}

/// Parses `items(<coll>, key = <key>) { item -> … }`. The `item ->` body is
/// emitted empty by the codegen (FLUX-014); parity treats an empty body as the
/// expected, faithful shape.
pub(crate) fn parse_items(
    tokens: &[Token],
    start: usize,
) -> Result<(ViewNode, usize), KotlinRecognitionError> {
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
        .map(|s| canonicalize_release_expr(s.trim()))
        .unwrap_or_default();
    let key_path = args
        .iter()
        .find_map(|a| {
            let a = a.trim();
            a.strip_prefix("key")
                .and_then(|r| r.strip_prefix('='))
                .map(str::trim)
        })
        .map(canonicalize_release_expr)
        .unwrap_or_default();
    let mut m = k + 1;
    while m < tokens.len() && tokens[m].text != "{" {
        m += 1;
    }
    // Propagate parse_body's error instead of swallowing it via
    // `unwrap_or_default()`. Before this fix, an unbalanced body returned
    // `(vec![], 0)` — i.e. `end = 0` — which the caller (parse_body) would
    // then assign to its own `i`, causing an infinite loop over the token
    // stream. Errors here mean the emitted source drifted from the grammar,
    // which is exactly what the recognizer's error type is for.
    let (_, end) = if m < tokens.len() {
        parse_body(tokens, m)?
    } else {
        (vec![], m + 1)
    };
    Ok((
        ViewNode::ForEach {
            collection,
            key_path,
        },
        end,
    ))
}

/// Parses `NavHost(...) { … }` (the `Router` node).
pub(crate) fn parse_nav_host(
    tokens: &[Token],
    start: usize,
) -> Result<(ViewNode, usize), KotlinRecognitionError> {
    let mut j = start + 1;
    while j < tokens.len() && tokens[j].text != "{" {
        j += 1;
    }
    let (children, end) = parse_body(tokens, j)?;
    Ok((ViewNode::Router { children }, end))
}

/// Parses `composable("route") { … }` (the `Screen` node). The body's children
/// are the screen's content components.
pub(crate) fn parse_composable_dest(
    tokens: &[Token],
    start: usize,
) -> Result<(ViewNode, usize), KotlinRecognitionError> {
    let mut j = start + 1;
    while j < tokens.len() && tokens[j].text != "(" {
        j += 1;
    }
    let mut depth = 0usize;
    let mut k = j;
    let mut arg = String::new();
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
                break;
            }
        }
        arg.push_str(t);
        k += 1;
    }
    let route = canonicalize_release_expr(arg.trim());
    let mut m = k + 1;
    while m < tokens.len() && tokens[m].text != "{" {
        m += 1;
    }
    let (children, end) = parse_body(tokens, m)?;
    Ok((ViewNode::Screen { route, children }, end))
}

/// Parses a view expression `Name(...)` possibly with a `{ … }` trailing block —
/// e.g. `Text("…")`, `Column(...) { … }`, `Home()`.
///
/// Overlay containers (`Modal`/`Sheet`/`Dialog`, FLUX-038) are emitted with no
/// argument list — `Dialog { … }`, `ModalBottomSheet { … }`, `AlertDialog { … }`
/// — so the trailing `{` may follow the name directly (no `( … )`). `parse_body`
/// only routes `Name(` forms here; this helper additionally accepts the bare
/// `Name {` form for known containers.
pub(crate) fn parse_view(
    tokens: &[Token],
    start: usize,
) -> Result<(ViewNode, usize), KotlinRecognitionError> {
    let name = tokens[start].text.clone();
    let normalized = normalize_view_name(&name);
    let mut i = start + 1;
    // Extract recognised props from the argument list, if any.
    let mut props: Vec<(String, String)> = Vec::new();
    if tokens.get(i).map(|t| t.text.as_str()) == Some("(") {
        let end = match_paren(tokens, i)
            .ok_or_else(|| KotlinRecognitionError(format!("unbalanced args in {normalized}")))?;
        props =
            crate::recognize_swift::swift_views::extract_swift_props(&normalized, &tokens[i..=end]);
        i = end + 1;
    }
    if tokens.get(i).map(|t| t.text.as_str()) == Some("{") {
        // Container layouts (Column/Row/VStack/HStack/Stack) and FLUX-038 overlay
        // containers (Modal/Sheet/Dialog) carry real structural children. Every
        // other adapter is a leaf: its trailing block is a codegen placeholder and
        // must not be recovered as a child.
        if is_container(&normalized) {
            let (children, after) = parse_body(tokens, i)?;
            return Ok((
                ViewNode::Primitive {
                    name: normalized,
                    props: props.clone(),
                    children,
                },
                after,
            ));
        }
        // Leaf adapter: consume the (empty or placeholder) block and stay childless.
        let end = match_brace(tokens, i)
            .ok_or_else(|| KotlinRecognitionError(format!("unbalanced body in {normalized}")))?;
        i = end + 1;
    }
    Ok((
        ViewNode::Primitive {
            name: normalized,
            props,
            children: vec![],
        },
        i,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokenize::tokenize;

    /// Regression (round 31): `parse_items` must propagate an error from the
    /// nested `parse_body` call. Before the fix, `unwrap_or_default()` turned a
    /// body-parsing failure into `(vec![], 0)` — returning `end = 0` to the
    /// caller, which assigns it to its own loop index `i`, causing an infinite
    /// loop over the token stream.
    #[test]
    fn parse_items_propagates_unbalanced_body_error() {
        // An `items(...)` whose body brace is never closed.
        let tokens = tokenize("items(list, key = it) {");
        match parse_items(&tokens, 0) {
            Err(_) => {}
            Ok((_, end)) => panic!(
                "unbalanced items must propagate Err, got Ok with end={end}"
            ),
        }
    }
}
