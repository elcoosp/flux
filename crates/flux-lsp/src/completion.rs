//! Prop / capability completion over the compiler's authoritative registries
//! (FLUX-027).
//!
//! Completion suggestions are drawn from two single-source-of-truth tables,
//! never a hand-maintained list that can desync from the compiler:
//!
//! * **Built-in prop names** come from the ADR-0047 primitive registry
//!   ([`flux_codegen_core::primitives::PrimitiveSpec::all`]) — every primitive's
//!   `primary_prop` / `handler_prop` / `label_prop`.
//! * **Capability method names** come from the capability prelude
//!   ([`flux_types::capabilities::CAPABILITY_IDL`]) — every capability's method
//!   names.
//!
//! The provider is a pure function of the document text and the cursor, so it
//! is directly unit-testable without a socket.

use async_lsp::lsp_types::{CompletionItem, CompletionItemKind, CompletionResponse, Position};
use flux_codegen_core::primitives::PrimitiveSpec;
use flux_types::capabilities::CAPABILITY_IDL;

/// Builds the completion response for the cursor position in `text`.
///
/// Returns all known built-in prop names and capability method names. The LSP
/// client filters this list against the partial token before the user sees it,
/// so returning the full authoritative set is correct and cheap (it is a fixed
/// registry, not file-dependent).
///
/// The cursor is accepted for call-shape uniformity with the other providers
/// but is not used for filtering.
#[must_use]
pub(crate) fn completions_at(_text: &str, _cursor: Position) -> Option<CompletionResponse> {
    Some(CompletionResponse::Array(all_completion_items()))
}

/// Returns the full, registry-derived completion item set.
///
/// Exposed separately so unit tests can assert membership without constructing
/// an LSP [`Position`].
#[must_use]
pub(crate) fn all_completion_items() -> Vec<CompletionItem> {
    let mut items: Vec<CompletionItem> = Vec::new();
    for spec in PrimitiveSpec::all() {
        for prop in spec.prop_names() {
            items.push(CompletionItem {
                label: prop.to_owned(),
                kind: Some(CompletionItemKind::PROPERTY),
                detail: Some(format!("built-in prop of `{}`", spec.flux_name)),
                ..Default::default()
            });
        }
    }
    for cap in CAPABILITY_IDL {
        for method in cap.methods {
            items.push(CompletionItem {
                label: method.name.to_owned(),
                kind: Some(CompletionItemKind::METHOD),
                detail: Some(format!("method of capability `{}`", cap.name)),
                ..Default::default()
            });
        }
    }
    items
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn includes_primitive_props_from_registry() {
        let items = all_completion_items();
        let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
        // `text` is the primary prop of `Text`/`Image`/`TextInput` in the
        // ADR-0047 registry; it must be suggested, never hardcoded.
        assert!(labels.contains(&"text"), "missing primitive prop `text`");
        // `onPress` is the handler prop of the Button family.
        assert!(
            labels.contains(&"onPress"),
            "missing handler prop `onPress`"
        );
    }

    #[test]
    fn includes_capability_methods_from_prelude() {
        let items = all_completion_items();
        let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
        // `navigate` is a Router capability method in CAPABILITY_IDL.
        assert!(
            labels.contains(&"navigate"),
            "missing capability method `navigate`"
        );
        assert!(
            labels.contains(&"takePicture"),
            "missing Camera method `takePicture`"
        );
    }

    #[test]
    fn completions_at_returns_full_set() {
        let resp = completions_at(
            "",
            Position {
                line: 0,
                character: 0,
            },
        );
        let CompletionResponse::Array(items) = resp.expect("some response") else {
            panic!("expected array response");
        };
        assert!(
            !items.is_empty(),
            "registry-derived completions must be non-empty"
        );
    }
}
