//! Single-source-of-truth parity guard between the primitive registry and the
//! type-checker prelude (FLUX-047).
//!
//! `flux_types::prelude` seeds the environment with the adapter/primitive
//! component names the MLP supports. Every such name must also appear in
//! [`crate::primitives::PRIMITIVES`], otherwise codegen would hit an unknown
//! primitive and emit an honest-but-broken comment instead of a real view. This
//! mirrors the capability-IDL parity guard in `flux-devserver`, keeping one
//! table of truth and a test that fails on drift.

use crate::primitives::PrimitiveSpec;

#[test]
fn registry_covers_every_prelude_primitive() {
    // The prelude registers these adapter/primitive component names
    // (see flux_types::prelude::prelude). The registry is the codegen source of
    // truth; any omission here means codegen can't emit that primitive.
    let prelude_primitives: &[&str] = &[
        "Column",
        "Row",
        "Text",
        "Button",
        "Image",
        "Router",
        "Screen",
        "ForEach",
        "CupertinoButton",
        "MaterialButton",
        "TextField",
        "Provider",
        "When",
        "Switch",
        // FLUX-037 layout primitives (PRD-N family).
        "Stack",
        "Grid",
        "Spacer",
        "SafeArea",
        // FLUX-040 form primitives (PRD-N family).
        "Switch",
        "Checkbox",
        "Slider",
        "Picker",
        "DatePicker",
        "TextArea",
    ];
    for name in prelude_primitives {
        assert!(
            PrimitiveSpec::by_name(name).is_some(),
            "primitive `{name}` is registered in the prelude but missing from the codegen registry"
        );
    }
}

#[test]
fn registry_has_no_unknown_entries() {
    // Defensive: the registry should not list a name the prelude does not know,
    // or codegen would emit a view the checker can't type.
    let prelude_primitives: &[&str] = &[
        "Column",
        "Row",
        "Text",
        "Button",
        "Image",
        "Router",
        "Screen",
        "ForEach",
        "CupertinoButton",
        "MaterialButton",
        "TextField",
        "Provider",
        "When",
        "Switch",
        // FLUX-037 layout primitives (PRD-N family).
        "Stack",
        "Grid",
        "Spacer",
        "SafeArea",
        // FLUX-040 form primitives (PRD-N family).
        "Switch",
        "Checkbox",
        "Slider",
        "Picker",
        "DatePicker",
        "TextArea",
    ];
    for spec in PrimitiveSpec::all() {
        assert!(
            prelude_primitives.contains(&spec.flux_name),
            "registry lists `{}` which is not in the prelude",
            spec.flux_name
        );
    }
}
