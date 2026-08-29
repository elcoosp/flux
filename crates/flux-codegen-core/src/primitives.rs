//! The single source of truth for Flux primitive components (FLUX-047).
//!
//! Every built-in adapter/primitive the type checker seeds in
//! `flux_types::prelude` (`Column`, `Row`, `Text`, `Button`, `Image`, `Router`,
//! `Screen`, `ForEach`, `CupertinoButton`, `MaterialButton`, `TextField`,
//! `Provider`, `When`, `Switch`) is described exactly once here. The Kotlin and
//! Swift codegen backends derive their emitters from this table through the
//! [`Backend`](crate::backend::Backend) trait, so adding a primitive is a
//! one-line edit to `PRIMITIVES` — not a touch to two duplicated `match`
//! statements.
//!
//! The shape mirrors the project's existing capability registry
//! (`flux_types::capabilities::CAPABILITY_IDL`): one declarative table, two
//! backends reading it. A parity test
//! (`registry_covers_every_prelude_primitive` in the `parity` module) fails if
//! `PRIMITIVES` ever
//! drifts from what the prelude registers, so the two cannot silently diverge.

use flux_syntax::NodeKind;

/// The kind of a Flux primitive: how the structural emitter treats it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrimitiveKind {
    /// A layout container that wraps child nodes (`Column`, `Row`).
    Container,
    /// A leaf view carrying a primary value (`Text`, `Image`).
    Leaf,
    /// A navigation container (`Router`); rendered by the backend's nav API.
    Router,
    /// A navigation destination (`Screen`); rendered by the backend's nav API.
    Screen,
    /// An interactive leaf (`Button` and its style aliases).
    Button,
    /// An editor leaf (`TextField`): a native editable text field bound to a
    /// `text`/`onChange` (Kotlin) or `text:`/`onEditingChanged:` (Swift) pair,
    /// with an optional `placeholder`.
    TextField,
    /// A built-in primitive the release backends emit as a bare call (no special
    /// shaping yet): `When`, `Switch`. These are control-flow forms that lower to
    /// `NodeKind::If`/`NodeKind::Match` and are emitted structurally by
    /// `emit_if`/`emit_match`, so they never reach `emit_primitive` as a primitive
    /// call; the registry entry exists only so `PRIMITIVES` covers every prelude
    /// name and the parity guard stays honest.
    Other,
}

/// Declarative metadata for one Flux primitive, shared by both backends.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PrimitiveSpec {
    /// The Flux surface name as written in source (and registered in the
    /// prelude). Matched against the call callee by the structural emitter.
    pub flux_name: &'static str,
    /// The lowered [`NodeKind`] family this primitive lowers into. Primitive
    /// calls lower under [`NodeKind::Primitive`]; control-flow / navigation
    /// forms lower under their own dedicated kinds.
    pub node_kind: NodeKind,
    /// The primitive category, driving shared emission logic.
    pub kind: PrimitiveKind,
    /// The native view name on the Kotlin backend (e.g. `Column` for `Column`).
    /// Backends select between this and [`Self::swift_view`] via
    /// [`Backend::native_name`](crate::Backend::native_name).
    pub kotlin_view: &'static str,
    /// The native view name on the Swift backend.
    pub swift_view: &'static str,
    /// The prop whose value is the view's primary argument (`text`, `url`).
    pub primary_prop: Option<&'static str>,
    /// The prop naming the tap/click handler (`onClick`, `onTap`).
    pub handler_prop: Option<&'static str>,
    /// The prop naming the button's label (`text`).
    pub label_prop: Option<&'static str>,
}

impl PrimitiveSpec {
    /// Looks up a primitive by its Flux surface name.
    #[must_use]
    pub(crate) fn by_name(name: &str) -> Option<&'static PrimitiveSpec> {
        PRIMITIVES.iter().find(|p| p.flux_name == name)
    }

    /// All registered primitive specs, in source order.
    #[cfg_attr(not(test), allow(dead_code))]
    #[must_use]
    pub(crate) fn all() -> &'static [PrimitiveSpec] {
        PRIMITIVES
    }
}

/// The declarative primitive registry — the single source of truth.
///
/// Keeping this in one place (rather than two duplicated `match` arms in
/// `emit_primitive`) is the core of the data-driven codegen refactor (FLUX-047).
/// `flux_types::prelude` must register every name listed here; the parity test
/// enforces it.
const PRIMITIVES: &[PrimitiveSpec] = &[
    PrimitiveSpec {
        flux_name: "Column",
        node_kind: NodeKind::Primitive,
        kind: PrimitiveKind::Container,
        kotlin_view: "Column",
        swift_view: "VStack",
        primary_prop: None,
        handler_prop: None,
        label_prop: None,
    },
    PrimitiveSpec {
        flux_name: "Row",
        node_kind: NodeKind::Primitive,
        kind: PrimitiveKind::Container,
        kotlin_view: "Row",
        swift_view: "HStack",
        primary_prop: None,
        handler_prop: None,
        label_prop: None,
    },
    PrimitiveSpec {
        flux_name: "Text",
        node_kind: NodeKind::Primitive,
        kind: PrimitiveKind::Leaf,
        kotlin_view: "Text",
        swift_view: "Text",
        primary_prop: Some("text"),
        handler_prop: None,
        label_prop: None,
    },
    PrimitiveSpec {
        flux_name: "Image",
        node_kind: NodeKind::Primitive,
        kind: PrimitiveKind::Leaf,
        kotlin_view: "Image",
        swift_view: "Image",
        primary_prop: Some("url"),
        handler_prop: None,
        label_prop: None,
    },
    PrimitiveSpec {
        flux_name: "Button",
        node_kind: NodeKind::Primitive,
        kind: PrimitiveKind::Button,
        kotlin_view: "Button",
        swift_view: "Button",
        primary_prop: None,
        handler_prop: Some("onClick"),
        label_prop: Some("text"),
    },
    PrimitiveSpec {
        flux_name: "CupertinoButton",
        node_kind: NodeKind::Primitive,
        kind: PrimitiveKind::Button,
        kotlin_view: "Button",
        swift_view: "Button",
        primary_prop: None,
        handler_prop: Some("onClick"),
        label_prop: Some("text"),
    },
    PrimitiveSpec {
        flux_name: "MaterialButton",
        node_kind: NodeKind::Primitive,
        kind: PrimitiveKind::Button,
        kotlin_view: "Button",
        swift_view: "Button",
        primary_prop: None,
        handler_prop: Some("onClick"),
        label_prop: Some("text"),
    },
    PrimitiveSpec {
        flux_name: "TextField",
        node_kind: NodeKind::Primitive,
        kind: PrimitiveKind::TextField,
        kotlin_view: "TextField",
        swift_view: "TextField",
        primary_prop: Some("text"),
        handler_prop: Some("onChange"),
        label_prop: None,
    },
    PrimitiveSpec {
        flux_name: "Provider",
        node_kind: NodeKind::Primitive,
        kind: PrimitiveKind::Container,
        kotlin_view: "Provider",
        swift_view: "Provider",
        primary_prop: None,
        handler_prop: None,
        label_prop: None,
    },
    PrimitiveSpec {
        flux_name: "When",
        node_kind: NodeKind::Primitive,
        kind: PrimitiveKind::Other,
        kotlin_view: "When",
        swift_view: "When",
        primary_prop: None,
        handler_prop: None,
        label_prop: None,
    },
    PrimitiveSpec {
        flux_name: "Switch",
        node_kind: NodeKind::Primitive,
        kind: PrimitiveKind::Other,
        kotlin_view: "Switch",
        swift_view: "Switch",
        primary_prop: None,
        handler_prop: None,
        label_prop: None,
    },
    PrimitiveSpec {
        flux_name: "Router",
        node_kind: NodeKind::Router,
        kind: PrimitiveKind::Router,
        kotlin_view: "NavHost",
        swift_view: "NavigationStack",
        primary_prop: None,
        handler_prop: None,
        label_prop: None,
    },
    PrimitiveSpec {
        flux_name: "Screen",
        node_kind: NodeKind::Screen,
        kind: PrimitiveKind::Screen,
        kotlin_view: "composable",
        swift_view: "Screen",
        primary_prop: None,
        handler_prop: None,
        label_prop: None,
    },
    PrimitiveSpec {
        flux_name: "ForEach",
        node_kind: NodeKind::ForEach,
        kind: PrimitiveKind::Other,
        kotlin_view: "ForEach",
        swift_view: "ForEach",
        primary_prop: None,
        handler_prop: None,
        label_prop: None,
    },
    // --- FLUX-037 layout primitives (PRD-N family) ---
    PrimitiveSpec {
        flux_name: "Stack",
        node_kind: NodeKind::Primitive,
        kind: PrimitiveKind::Container,
        // Z-order overlay container.
        kotlin_view: "Box",
        swift_view: "ZStack",
        primary_prop: None,
        handler_prop: None,
        label_prop: None,
    },
    PrimitiveSpec {
        flux_name: "Grid",
        node_kind: NodeKind::Primitive,
        kind: PrimitiveKind::Container,
        kotlin_view: "LazyVerticalGrid",
        swift_view: "Grid",
        primary_prop: None,
        handler_prop: None,
        label_prop: None,
    },
    PrimitiveSpec {
        flux_name: "Spacer",
        node_kind: NodeKind::Primitive,
        kind: PrimitiveKind::Leaf,
        kotlin_view: "Spacer",
        swift_view: "Spacer",
        primary_prop: None,
        handler_prop: None,
        label_prop: None,
    },
    PrimitiveSpec {
        flux_name: "SafeArea",
        node_kind: NodeKind::Primitive,
        kind: PrimitiveKind::Container,
        // Insets the content within the platform safe area.
        kotlin_view: "Scaffold",
        swift_view: "SafeArea",
        primary_prop: None,
        handler_prop: None,
        label_prop: None,
    },
];
