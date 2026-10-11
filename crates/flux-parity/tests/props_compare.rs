//! T-501 regression tests: the parity harness must compare view props, not
//! just structure. The AST reducer, Swift recognizer, and Kotlin recognizer
//! must all surface the same canonical prop subset so dev-vs-release prop
//! drift is caught.
//!
//! Recognized props: **currently none** — see FLUX-F22. The codegen emitters
//! (`flux-codegen-core::emitter`) read only `gap` from the container props
//! map; `label`, `color`, and `alignment` are collected then silently dropped,
//! so tracking them on the dev side made the reduced dev tree carry props the
//! release trees could never have and `check_parity` reported a divergence for
//! any program using them. Round 86 emptied the `RECOGNISED` list on both
//! sides. When the emitters actually emit these props, restore them here and
//! in `recognize_swift::swift_views::extract_swift_props`.
//!
//! `text` is excluded (independently of F22) because codegen renders it
//! positionally or as a child Text node — it never survives as a named arg in
//! generated source.

use flux_parity::ViewNode;
use flux_parity::compile;
use flux_parity::from_ast;

/// Helper: find the first Primitive node with the given name in a subtree
/// and return its props.
fn find_props<'a>(nodes: &'a [ViewNode], name: &str) -> Option<&'a [(String, String)]> {
    for node in nodes {
        match node {
            ViewNode::Primitive {
                name: n,
                props,
                children,
            } if n == name => return Some(props),
            ViewNode::Primitive { children, .. } => {
                if let Some(found) = find_props(children, name) {
                    return Some(found);
                }
            }
            ViewNode::Component { children, .. } => {
                if let Some(found) = find_props(children, name) {
                    return Some(found);
                }
            }
            _ => {}
        }
    }
    None
}

/// Helper: parse + type-check + AST-reduce a `.flux` source fragment.
fn from_src(src: &str) -> Vec<ViewNode> {
    let (ast, _, _) = compile(src, 1).expect("compile succeeded");
    from_ast(&ast)
}

/// FLUX-F22: the dev-side AST reducer currently drops `label` — the codegen
/// backends silently ignore it, so extracting it here would make the dev and
/// release reduced trees diverge (the very bug F22 filed). This test asserts
/// the *current* invariant; the F22 fix will invert it (label becomes
/// extracted on both sides) and this test should then be flipped back.
#[test]
fn label_prop_is_not_extracted_until_f22_lands() {
    let src = "compo Hello\n  Button(label: \"Save\", onPress: {})";
    let dev = from_src(src);
    let props = find_props(&dev, "Button").expect("Button node exists");
    assert!(
        props.iter().all(|(k, _)| k != "label"),
        "label prop must be dropped until F22 lands (codegen cannot emit it): got {:?}",
        props
    );
}

/// The recognized prop subset excludes `text` — it is rendered positionally
/// (Text/Image) or as a child Text node (Button), never as a named arg
/// in generated source.
#[test]
fn text_prop_is_not_extracted() {
    let src = "compo Hello\n  Button(text: \"Save\", onPress: {})";
    let dev = from_src(src);
    let props = find_props(&dev, "Button").expect("Button node exists");
    assert!(
        props.is_empty(),
        "text prop should not be in recognized subset: got {:?}",
        props
    );
}

/// FLUX-F22: the dev-side AST reducer currently drops `color` — see
/// `label_prop_is_not_extracted_until_f22_lands`. Same reasoning: codegen
/// silently ignores the prop, so extracting it only on the dev side would
/// make the reduced trees diverge.
#[test]
fn color_prop_is_not_extracted_until_f22_lands() {
    let src = "compo Hello\n  Text(\"Hi\", color: \"red\")";
    let dev = from_src(src);
    let props = find_props(&dev, "Text").expect("Text node exists");
    assert!(
        props.iter().all(|(k, _)| k != "color"),
        "color prop must be dropped until F22 lands (codegen cannot emit it): got {:?}",
        props
    );
}

/// An unrecognized prop like `width` must NOT be extracted.
#[test]
fn unrecognized_prop_is_not_extracted() {
    let src = "compo Hello\n  Text(\"Hi\", width: 100)";
    let dev = from_src(src);
    let props = find_props(&dev, "Text").expect("Text node exists");
    assert!(
        props.iter().all(|(k, _)| k != "width"),
        "width should not be in recognized subset: got {:?}",
        props
    );
}
