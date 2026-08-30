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
    /// A signal-graph animation wrapper (FLUX-042): drives a signal through a
    /// spring/timing curve. The release backends emit the host-native
    /// `withAnimation(<spec>) { … }` call (SwiftUI `withAnimation` /
    /// Compose `withAnimation`) wrapping the child subtree; the curve is data the
    /// host consumes, never animation frames on the wire. Parity reduces both
    /// backends' `withAnimation` to the flux surface name `Animate`.
    Animate,
}

/// A design-token group, used to pick the native value spelling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenGroup {
    /// A color token (`Color(0xRRGGBB)` / `Color(...)`).
    Color,
    /// A spacing token (`N.dp` / `N`).
    Spacing,
    /// A typography token (point size: `N.sp` / `N`).
    Typography,
}

/// A single design token, declared once and emitted into a native theme
/// extension on both backends (FLUX-043). `kotlin`/`swift` carry the
/// already-spelled native literal so codegen stays a pure table read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DesignToken {
    /// The token's surface name (referenced by components, not hardcoded).
    pub name: &'static str,
    /// The token group, driving the native value type.
    pub group: TokenGroup,
    /// The native Kotlin/Compose literal (e.g. `Color(0xFF6750A4)`, `8.dp`).
    pub kotlin: &'static str,
    /// The native Swift/SwiftUI literal (e.g. `Color(...)`, `8`).
    pub swift: &'static str,
}

/// The single source of truth for design tokens (FLUX-043, ADR-0047).
///
/// Codegen emits every token here into a native theme extension on both
/// backends so components reference tokens by name rather than per-component
/// literals. Mirrors `PRIMITIVES`: one declarative table, two backends reading
/// it. A test asserts both backends emit every token's name.
pub fn theme_tokens() -> &'static [DesignToken] {
    TOKENS
}

/// The design-token table — color / spacing / typography scales.
const TOKENS: &[DesignToken] = &[
    DesignToken {
        name: "colorPrimary",
        group: TokenGroup::Color,
        kotlin: "Color(0xFF6750A4)",
        swift: "Color(red: 0.404, green: 0.314, blue: 0.643)",
    },
    DesignToken {
        name: "colorSecondary",
        group: TokenGroup::Color,
        kotlin: "Color(0xFF625B71)",
        swift: "Color(red: 0.384, green: 0.357, blue: 0.443)",
    },
    DesignToken {
        name: "colorSurface",
        group: TokenGroup::Color,
        kotlin: "Color(0xFFFEF7FF)",
        swift: "Color(red: 0.996, green: 0.969, blue: 1.0)",
    },
    DesignToken {
        name: "spaceSm",
        group: TokenGroup::Spacing,
        kotlin: "4.dp",
        swift: "4",
    },
    DesignToken {
        name: "spaceMd",
        group: TokenGroup::Spacing,
        kotlin: "8.dp",
        swift: "8",
    },
    DesignToken {
        name: "spaceLg",
        group: TokenGroup::Spacing,
        kotlin: "16.dp",
        swift: "16",
    },
    DesignToken {
        name: "textBody",
        group: TokenGroup::Typography,
        kotlin: "17.sp",
        swift: "17",
    },
    DesignToken {
        name: "textTitle",
        group: TokenGroup::Typography,
        kotlin: "28.sp",
        swift: "28",
    },
];

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
    /// The presentation/transition contract for overlay containers
    /// (`Modal`/`Sheet`/`Dialog`, FLUX-038). `None` for non-overlay primitives.
    /// This is **data the host consumes** — a named transition it maps to the
    /// native equivalent (`.sheet`/`.fullScreenCover`, `ModalBottomSheet`,
    /// `AlertDialog`) — never a wire animation frame. The codegen emits it as a
    /// code comment documenting the intended presentation; the host adapter kit
    /// resolves the actual native surface (host wiring is gated on ADR-0048).
    pub presentation: Option<Presentation>,
}

/// The named transition an overlay container maps to on each host.
///
/// Mirrors the FLUX-038 design: animation is specified as a named transition the
/// host maps to its native equivalent, not as frame-by-frame wire data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Presentation {
    /// Full-screen centered dialog with a dimmed scrim (e.g. iOS
    /// `.fullScreenCover`, Compose `AlertDialog` / `Dialog`).
    Dialog,
    /// Bottom-anchored sheet that slides up (iOS `.sheet`, Compose
    /// `ModalBottomSheet`).
    Sheet,
    /// Centered modal over a scrim, dismissal by tap-outside (iOS
    /// `.presentationDetents`-free `fullScreenCover`, Compose `Dialog`).
    Modal,
}

impl PrimitiveSpec {
    /// Looks up a primitive by its Flux surface name.
    #[must_use]
    pub(crate) fn by_name(name: &str) -> Option<&'static PrimitiveSpec> {
        PRIMITIVES.iter().find(|p| p.flux_name == name)
    }

    /// The surface prop names this primitive exposes, in registry order.
    ///
    /// Built from the declared `primary_prop` / `handler_prop` / `label_prop`
    /// fields so the set can never drift from what the emitters actually read.
    /// This is the authoritative prop-name list consumed by the LSP completion
    /// provider (FLUX-027); callers must not hand-maintain a duplicate copy.
    #[must_use]
    pub fn prop_names(&self) -> Vec<&'static str> {
        let mut names: Vec<&'static str> = Vec::new();
        if let Some(p) = self.primary_prop {
            names.push(p);
        }
        if let Some(p) = self.handler_prop {
            names.push(p);
        }
        if let Some(p) = self.label_prop {
            names.push(p);
        }
        names
    }

    /// All registered primitive specs, in source order.
    ///
    /// This is the authoritative, single-source-of-truth list of every built-in
    /// primitive's surface name and prop surface (its `primary_prop` /
    /// `handler_prop` / `label_prop`). Consumers that need the set of known
    /// prop names — e.g. the LSP completion provider (FLUX-027) — must read it
    /// from here rather than maintaining a parallel list that can drift from
    /// the registry. The parity test `registry_covers_every_prelude_primitive`
    /// fails if `PRIMITIVES` ever desyncs from what `flux_types::prelude`
    /// registers, so this list is guaranteed to match the compiler.
    #[must_use]
    pub fn all() -> &'static [PrimitiveSpec] {
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
        presentation: None,
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
        presentation: None,
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
        presentation: None,
    },
    PrimitiveSpec {
        flux_name: "Image",
        node_kind: NodeKind::Primitive,
        kind: PrimitiveKind::Leaf,
        kotlin_view: "Image",
        swift_view: "Image",
        primary_prop: Some("source"),
        handler_prop: None,
        label_prop: None,
        presentation: None,
    },
    PrimitiveSpec {
        flux_name: "Button",
        node_kind: NodeKind::Primitive,
        kind: PrimitiveKind::Button,
        kotlin_view: "Button",
        swift_view: "Button",
        primary_prop: None,
        handler_prop: Some("onPress"),
        label_prop: Some("text"),
        presentation: None,
    },
    PrimitiveSpec {
        flux_name: "CupertinoButton",
        node_kind: NodeKind::Primitive,
        kind: PrimitiveKind::Button,
        kotlin_view: "Button",
        swift_view: "Button",
        primary_prop: None,
        handler_prop: Some("onPress"),
        label_prop: Some("text"),
        presentation: None,
    },
    PrimitiveSpec {
        flux_name: "MaterialButton",
        node_kind: NodeKind::Primitive,
        kind: PrimitiveKind::Button,
        kotlin_view: "Button",
        swift_view: "Button",
        primary_prop: None,
        handler_prop: Some("onPress"),
        label_prop: Some("text"),
        presentation: None,
    },
    PrimitiveSpec {
        flux_name: "TextInput",
        node_kind: NodeKind::Primitive,
        kind: PrimitiveKind::TextField,
        kotlin_view: "TextField",
        swift_view: "TextField",
        primary_prop: Some("text"),
        handler_prop: Some("onChangeText"),
        label_prop: None,
        presentation: None,
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
        presentation: None,
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
        presentation: None,
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
        presentation: None,
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
        presentation: None,
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
        presentation: None,
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
        presentation: None,
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
        presentation: None,
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
        presentation: None,
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
        presentation: None,
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
        presentation: None,
    },
    // --- FLUX-040 form primitives (PRD-N family) ---
    // Each carries a `value` signal + `onChange` callback (same contract as
    // `TextField`). Native hosts map these registry names to their platform
    // controls.
    PrimitiveSpec {
        flux_name: "Switch",
        node_kind: NodeKind::Primitive,
        kind: PrimitiveKind::Leaf,
        kotlin_view: "Switch",
        swift_view: "Toggle",
        primary_prop: Some("value"),
        handler_prop: Some("onChange"),
        label_prop: None,
        presentation: None,
    },
    PrimitiveSpec {
        flux_name: "Checkbox",
        node_kind: NodeKind::Primitive,
        kind: PrimitiveKind::Leaf,
        kotlin_view: "Checkbox",
        swift_view: "Toggle",
        primary_prop: Some("value"),
        handler_prop: Some("onChange"),
        label_prop: None,
        presentation: None,
    },
    PrimitiveSpec {
        flux_name: "Slider",
        node_kind: NodeKind::Primitive,
        kind: PrimitiveKind::Leaf,
        kotlin_view: "Slider",
        swift_view: "Slider",
        primary_prop: Some("value"),
        handler_prop: Some("onChange"),
        label_prop: None,
        presentation: None,
    },
    PrimitiveSpec {
        flux_name: "Picker",
        node_kind: NodeKind::Primitive,
        kind: PrimitiveKind::Leaf,
        kotlin_view: "DropdownMenu",
        swift_view: "Picker",
        primary_prop: Some("value"),
        handler_prop: Some("onChange"),
        label_prop: None,
        presentation: None,
    },
    PrimitiveSpec {
        flux_name: "DatePicker",
        node_kind: NodeKind::Primitive,
        kind: PrimitiveKind::Leaf,
        kotlin_view: "DatePickerDialog",
        swift_view: "DatePicker",
        primary_prop: Some("value"),
        handler_prop: Some("onChange"),
        label_prop: None,
        presentation: None,
    },
    PrimitiveSpec {
        flux_name: "TextArea",
        node_kind: NodeKind::Primitive,
        kind: PrimitiveKind::Leaf,
        kotlin_view: "TextField",
        swift_view: "TextEditor",
        primary_prop: Some("value"),
        handler_prop: Some("onChange"),
        label_prop: None,
        presentation: None,
    },
    // --- FLUX-041 gestures (PRD-N family) ---
    // A `Gesture` wrapper carrying a `kind` (longPress/swipe/drag/pinch) + an
    // `onGesture` callback (reuses the onClick contract). Native hosts attach
    // the matching recognizer/modifier.
    PrimitiveSpec {
        flux_name: "Gesture",
        node_kind: NodeKind::Primitive,
        kind: PrimitiveKind::Container,
        kotlin_view: "Box",
        swift_view: "VStack",
        primary_prop: Some("kind"),
        handler_prop: Some("onGesture"),
        label_prop: None,
        presentation: None,
    },
    // --- FLUX-038 overlay container primitives (PRD-N family) ---
    // Each is an overlay surface that presents `content` above the current
    // scene. The `presentation` field is data the host consumes: a named
    // transition it maps to the native equivalent (`.sheet` / `.fullScreenCover`,
    // `ModalBottomSheet` / `AlertDialog`). Animation is never a wire frame — it is
    // the host's native transition. Host adapter wiring is deferred pending
    // ADR-0048 (iOS dev-tier convergence); the registry entry exists so codegen
    // and the type-checker prelude can name the primitive today.
    PrimitiveSpec {
        flux_name: "Sheet",
        node_kind: NodeKind::Primitive,
        kind: PrimitiveKind::Container,
        // Bottom-anchored sheet that slides up.
        kotlin_view: "ModalBottomSheet",
        swift_view: "Sheet",
        primary_prop: None,
        handler_prop: Some("onDismiss"),
        label_prop: None,
        presentation: Some(Presentation::Sheet),
    },
    PrimitiveSpec {
        flux_name: "Dialog",
        node_kind: NodeKind::Primitive,
        kind: PrimitiveKind::Container,
        // Modal dialog with a dimmed scrim. On Swift the canonical overlay is
        // `Alert`; on Kotlin it is `AlertDialog`. Neither collides with `Modal`'s
        // native tokens (`Dialog` / `FullScreenCover`), so the parity reducer maps
        // each overlay back to its own Flux surface unambiguously.
        kotlin_view: "AlertDialog",
        swift_view: "Alert",
        primary_prop: None,
        handler_prop: Some("onDismiss"),
        label_prop: None,
        presentation: Some(Presentation::Dialog),
    },
    PrimitiveSpec {
        flux_name: "Modal",
        node_kind: NodeKind::Primitive,
        kind: PrimitiveKind::Container,
        // Centered modal over a scrim; dismiss on tap-outside.
        kotlin_view: "Dialog",
        swift_view: "FullScreenCover",
        primary_prop: None,
        handler_prop: Some("onDismiss"),
        label_prop: None,
        presentation: Some(Presentation::Modal),
    },
    // --- FLUX-042 signal-graph animation primitive (PRD-N family) ---
    // Wraps a child subtree and drives a signal through a spring/timing curve.
    // The release backends emit the host-native `withAnimation(<spec>) { … }`
    // call (SwiftUI `withAnimation` / Compose `withAnimation`) wrapping the
    // children; the curve is data the host consumes, never animation frames on
    // the wire. Parity reduces both backends' `withAnimation` to the flux
    // surface name `Animate` (see `flux-parity` `normalize_view_name`).
    PrimitiveSpec {
        flux_name: "Animate",
        node_kind: NodeKind::Primitive,
        kind: PrimitiveKind::Animate,
        // SwiftUI `withAnimation(spec) { … }` wrapping the child subtree.
        kotlin_view: "withAnimation",
        swift_view: "withAnimation",
        primary_prop: Some("signal"),
        handler_prop: None,
        label_prop: None,
        presentation: None,
    },
    // --- FLUX-043 design-token theme primitive (PRD-N family) ---
    // Declares the design-token theme; codegen emits the token table into a
    // native theme extension on both backends (see `theme_tokens`). The node
    // itself is a thin container that applies the active theme to its children
    // (e.g. `MaterialTheme`/`ColorScheme`). It is a control-flow-free
    // container so the parity reducer keeps its children as a subtree.
    PrimitiveSpec {
        flux_name: "Theme",
        node_kind: NodeKind::Primitive,
        kind: PrimitiveKind::Container,
        kotlin_view: "MaterialTheme",
        swift_view: "FluxTheme",
        primary_prop: None,
        handler_prop: None,
        label_prop: None,
        presentation: None,
    },
];

/// Intent for a single built-in primitive's **dev-host adapter** (FLUX-078).
///
/// The release codegen path is fully data-driven from [`PRIMITIVES`]
/// (ADR-0047): one table row drives both Kotlin + Swift emitters. The dev-host
/// adapter kits (`adapters/ui-kotlin`, `adapters/ui-swift`) are the remaining
/// hand-maintained half — each primitive that needs a live dev-render adapter
/// carries one hand-written adapter class per platform, and both kits register
/// it in a name→factory map. [`HostAdapterSpec`] is the *single source of truth*
/// for that registration: it records, per primitive, the adapter class name the
/// kit generates for each platform, and [`crate::native_gen`] emits the
/// registry blocks (Kotlin `FluxUiKit.adapters` map, iOS `AdapterKit.AdapterRegistry`)
/// from it. A [`flux-parity`] guard fails if a checked-in kit drifts from this
/// table, so the two hosts can never silently desync again (the FLUX-040 /
/// FLUX-076 class of bug).
///
/// Only primitives with an actual hand-written adapter appear here. Structural
/// and control-flow forms (`CupertinoButton`, `MaterialButton`, `ForEach`,
/// `Provider`, `When`) lower to existing nodes and have no dedicated adapter, so
/// they are intentionally absent — the generator skips them, matching the
/// checked-in kits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// A primitive's dev-host adapter wiring — the single source of truth that the
/// codegen-driven native glue (`flux_codegen_core::native_gen`) and the parity
/// guard (`flux-parity/tests/native_kit_parity.rs`) compare the checked-in
/// `FluxUiKit.adapters` / `AdapterKit.AdapterRegistry` maps against (FLUX-078).
///
/// `kotlin_adapter` / `swift_adapter` are `Option<&str>` because a primitive may
/// be registered on only one host (e.g. `Container` is Kotlin-only: the iOS kit
/// routes unknown component names through its reconciler container fallback, so
/// it has no `ContainerAdapter` in `byName`). `None` on a platform means the
/// generator emits nothing for that platform and the guard does not require it.
pub struct HostAdapterSpec {
    /// The Flux surface name (must match a [`PRIMITIVES`] `flux_name`).
    pub flux_name: &'static str,
    /// The Kotlin adapter class name the kit registers (e.g. `TextAdapter`), or
    /// `None` if the Kotlin kit does not register a dedicated adapter.
    pub kotlin_adapter: Option<&'static str>,
    /// The Swift adapter class name the kit registers (e.g. `TextAdapter`), or
    /// `None` if the Swift kit does not register a dedicated adapter.
    pub swift_adapter: Option<&'static str>,
}

impl HostAdapterSpec {
    /// Looks up a host-adapter spec by Flux surface name.
    #[must_use]
    pub fn by_name(name: &str) -> Option<&'static HostAdapterSpec> {
        HOST_ADAPTERS.iter().find(|a| a.flux_name == name)
    }

    /// Every host-adapter spec, in source order.
    #[must_use]
    pub fn all() -> &'static [HostAdapterSpec] {
        HOST_ADAPTERS
    }
}

/// The dev-host adapter registry — single source of truth for the name→adapter
/// wiring in both kits (FLUX-078).
///
/// Kept in lockstep with `PRIMITIVES` by the `host_adapters_cover_primitives`
/// parity test (a missing adapter for a primitive that should have one is a
/// build failure), and with the checked-in `FluxUiKit.adapters` /
/// `AdapterKit.AdapterRegistry` maps by the `host_kits_match_generated` parity
/// test. Rows were taken verbatim from the two kits' registry blocks
/// (`adapters/ui-kotlin/src/main/kotlin/dev/flux/ui/FluxUiKit.kt`,
/// `runtimes/ios/FluxHost/Sources/FluxHost/AdapterKit.swift`).
const HOST_ADAPTERS: &[HostAdapterSpec] = &[
    HostAdapterSpec {
        flux_name: "Text",
        kotlin_adapter: Some("TextAdapter"),
        swift_adapter: Some("TextAdapter"),
    },
    HostAdapterSpec {
        flux_name: "Image",
        kotlin_adapter: Some("ImageAdapter"),
        swift_adapter: Some("ImageAdapter"),
    },
    HostAdapterSpec {
        flux_name: "Button",
        kotlin_adapter: Some("ButtonAdapter"),
        swift_adapter: Some("ButtonAdapter"),
    },
    HostAdapterSpec {
        flux_name: "Column",
        kotlin_adapter: Some("ColumnAdapter"),
        swift_adapter: Some("ColumnAdapter"),
    },
    HostAdapterSpec {
        flux_name: "Row",
        kotlin_adapter: Some("RowAdapter"),
        swift_adapter: Some("RowAdapter"),
    },
    HostAdapterSpec {
        flux_name: "TextInput",
        kotlin_adapter: Some("TextInputAdapter"),
        swift_adapter: Some("TextInputAdapter"),
    },
    HostAdapterSpec {
        flux_name: "Router",
        kotlin_adapter: Some("RouterAdapter"),
        swift_adapter: Some("RouterAdapter"),
    },
    HostAdapterSpec {
        flux_name: "Screen",
        kotlin_adapter: Some("ScreenAdapter"),
        swift_adapter: Some("ScreenAdapter"),
    },
    HostAdapterSpec {
        flux_name: "Stack",
        kotlin_adapter: Some("StackAdapter"),
        swift_adapter: Some("StackAdapter"),
    },
    HostAdapterSpec {
        flux_name: "Grid",
        kotlin_adapter: Some("GridAdapter"),
        swift_adapter: Some("GridAdapter"),
    },
    HostAdapterSpec {
        flux_name: "Spacer",
        kotlin_adapter: Some("SpacerAdapter"),
        swift_adapter: Some("SpacerAdapter"),
    },
    HostAdapterSpec {
        flux_name: "SafeArea",
        kotlin_adapter: Some("SafeAreaAdapter"),
        swift_adapter: Some("SafeAreaAdapter"),
    },
    HostAdapterSpec {
        flux_name: "Container",
        kotlin_adapter: Some("ContainerAdapter"),
        swift_adapter: None,
    },
    HostAdapterSpec {
        flux_name: "Modal",
        kotlin_adapter: Some("ModalAdapter"),
        swift_adapter: Some("ModalAdapter"),
    },
    HostAdapterSpec {
        flux_name: "Sheet",
        kotlin_adapter: Some("SheetAdapter"),
        swift_adapter: Some("SheetAdapter"),
    },
    HostAdapterSpec {
        flux_name: "Dialog",
        kotlin_adapter: Some("DialogAdapter"),
        swift_adapter: Some("DialogAdapter"),
    },
    HostAdapterSpec {
        flux_name: "Animate",
        kotlin_adapter: Some("AnimateAdapter"),
        swift_adapter: Some("AnimateAdapter"),
    },
    HostAdapterSpec {
        flux_name: "Switch",
        kotlin_adapter: Some("SwitchAdapter"),
        swift_adapter: Some("SwitchAdapter"),
    },
    HostAdapterSpec {
        flux_name: "Toggle",
        kotlin_adapter: Some("ToggleAdapter"),
        swift_adapter: Some("ToggleAdapter"),
    },
    HostAdapterSpec {
        flux_name: "Checkbox",
        kotlin_adapter: Some("CheckboxAdapter"),
        swift_adapter: Some("CheckboxAdapter"),
    },
    HostAdapterSpec {
        flux_name: "Slider",
        kotlin_adapter: Some("SliderAdapter"),
        swift_adapter: Some("SliderAdapter"),
    },
    HostAdapterSpec {
        flux_name: "Picker",
        kotlin_adapter: Some("PickerAdapter"),
        swift_adapter: Some("PickerAdapter"),
    },
    HostAdapterSpec {
        flux_name: "DatePicker",
        kotlin_adapter: Some("DatePickerAdapter"),
        swift_adapter: Some("DatePickerAdapter"),
    },
    HostAdapterSpec {
        flux_name: "TextArea",
        kotlin_adapter: Some("TextAreaAdapter"),
        swift_adapter: Some("TextAreaAdapter"),
    },
    HostAdapterSpec {
        flux_name: "Gesture",
        kotlin_adapter: Some("GestureAdapter"),
        swift_adapter: Some("GestureAdapter"),
    },
];
