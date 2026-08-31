//! The backend abstraction that decouples the shared structural emitter from
//! each native language's syntax (FLUX-047).
//!
//! The traversal logic in [`crate::emitter`] (primitives, `if`/`when`,
//! `ForEach`, `Router`, `Screen`, algebraic `match`) is identical across Kotlin
//! and Swift. Only a handful of syntax points differ — indentation width,
//! container spacing, image resource binding, the navigation API, the `Screen`
//! destination form, the scalar/collection spellings, and the component/sum-type
//! header forms. Those are pulled out into this trait so the two backends supply
//! them and the shared emitter stays free of `if swift { … }` branches.
//!
//! The trait is an associated-const + associated-fn vehicle (no per-node state
//! needed). Component/sum-type hooks take `&mut Emitter<Self>` so they can write
//! into the shared output buffer through the emitter's `line`/`push_raw` helpers.

use std::collections::HashMap;

use flux_parser::{Expr, TypeDecl};

use crate::emitter::Emitter;
use crate::model::ComponentMeta;
use crate::primitives::PrimitiveSpec;

/// A codegen backend: the per-language syntax the shared emitter needs.
pub trait Backend {
    /// Spaces of indentation per logical nesting level (Kotlin 4, Swift 1).
    const INDENT_UNIT: usize;

    /// How far child content is indented relative to a container open line.
    /// Kotlin emits children one level in (`+1` unit); Swift emits them four
    /// spaces in (`+4` spaces = `+4` units because its unit is 1).
    const CHILD_STEP: usize;

    /// Native spelling of the `Int` scalar.
    #[must_use]
    fn int_type() -> &'static str;
    /// Native spelling of the `Float` scalar.
    #[must_use]
    fn float_type() -> &'static str;
    /// Native spelling of the `Bool` scalar.
    #[must_use]
    fn bool_type() -> &'static str;
    /// Native spelling of the `String` scalar.
    #[must_use]
    fn string_type() -> &'static str;
    /// Native spelling of the `Unit` scalar.
    #[must_use]
    fn unit_type() -> &'static str;
    /// Native spelling of an opaque/`Any` fallback type.
    #[must_use]
    fn any_type() -> &'static str;
    /// Wraps rendered record fields (`a: T, b: U`) into a record type spell.
    #[must_use]
    fn record_type(fields: &[String]) -> String;

    /// Renders the trailing arguments of a layout container given its `gap`
    /// prop value (e.g. `(spacing: 8)` for Swift, the longer Compose
    /// `Arrangement` form for Kotlin). Empty string when there is no gap.
    #[must_use]
    fn container_spacing(gap: &str) -> String;

    /// Renders the body of an `Image(primary)` call: the resource binding.
    /// Kotlin: `painter = painterResource(value), contentDescription = null`.
    /// Swift: `uiImage: UIImage(named: value) ?? UIImage()`.
    #[must_use]
    fn image_expr(value: &str) -> String;

    /// Opens a `Router` navigation container (`NavHost(…) {` / `NavigationStack {`).
    #[must_use]
    fn router_open() -> String;

    /// Closes a `Router` navigation container (`}`).
    #[must_use]
    fn router_close() -> String;

    /// How far a `Screen` body is indented relative to its `screen_open` line.
    /// Swift inlines the body at the same indent (0); Kotlin nests it one level.
    const SCREEN_BODY_STEP: usize;

    /// Renders a `Screen` destination's opening line(s) given its route string.
    /// The shared emitter emits the screen's children and [`screen_close`]
    /// afterwards. Swift emits the route as a comment and no brace; Kotlin opens
    /// a `composable(route) {` block.
    ///
    /// [`screen_close`]: Backend::screen_close
    #[must_use]
    fn screen_open(route: &str) -> String;

    /// The closing line for a `Screen` destination body (`}` for Kotlin, empty
    /// for the Swift comment form, which needs no brace).
    #[must_use]
    fn screen_close() -> String;

    /// Renders the opening of an `if`/`when` block given the rendered condition.
    /// Differs per backend (`if ({cond}) {` / `if {cond} {`).
    #[must_use]
    fn if_open(cond: &str) -> String;

    /// Renders the opening line of a `ForEach` collection view given its
    /// rendered collection, key-extractor fragment, and element name. Differs per
    /// backend (`ForEach(c, id: k) { e in` / `items(c, key = k) { e ->`).
    #[must_use]
    fn for_each_open(collection: &str, key: &str, element: &str) -> String;

    /// Renders the closing line of a `ForEach` collection view (`}`).
    #[must_use]
    fn for_each_close() -> String;

    /// Renders a `Button` open line given the primitive name and the rendered
    /// handler body. The label is emitted separately as a `Text(...)` child; only
    /// the opening (and the handler lambda, plus any platform style) differ per
    /// backend and per button style (`Button` / `CupertinoButton` / `MaterialButton`).
    #[must_use]
    fn button_open(name: &str, handler: &str) -> String;

    /// A trailing style modifier appended after a `Button`'s label block
    /// (e.g. Swift `.buttonStyle(.borderedProminent)`). Empty for backends whose
    /// style is already part of [`Backend::button_open`]. Emitting it *after* the closing
    /// `}` keeps the parity recognizer from mistaking the label block for a
    /// sibling child.
    #[must_use]
    fn button_style(name: &str) -> &'static str;

    /// Renders a `TextField` leaf given its bound value, `onValueChange` handler,
    /// and placeholder. When `value`/`on_change` are empty, sensible defaults are
    /// emitted so the view still compiles.
    #[must_use]
    fn text_field(value: &str, on_change: &str, placeholder: &str) -> String;

    /// The `key`-extractor fragment for a `ForEach` collection (`{ it.id }` /
    /// `\.id`).
    #[must_use]
    fn key_extractor(key: &Expr) -> String;

    /// The opening punctuation of a string interpolation (`${` for Kotlin,
    /// `\(` for Swift). The closing `}` is shared.
    #[must_use]
    fn interp_open() -> &'static str;

    /// The closing punctuation of a string interpolation (`}` for Kotlin,
    /// `)` for Swift). Paired with [`interp_open`].
    ///
    /// [`interp_open`]: Backend::interp_open
    #[must_use]
    fn interp_close() -> &'static str;

    /// Wraps the rendered elements of a list literal into a collection
    /// expression (`listOf(a, b)` / `[a, b]`).
    #[must_use]
    fn list_literal(elements: &[String]) -> String;

    /// Renders the native spelling of the `List[T]` collection type
    /// (`List<T>` for Kotlin, `[T]` for Swift).
    #[must_use]
    fn list_type(element: &str) -> String {
        format!("List<{element}>")
    }

    /// Renders a `Spacer` leaf (e.g. `Spacer()` for SwiftUI, `Spacer("")` for Compose).
    #[must_use]
    fn spacer() -> &'static str {
        "Spacer()"
    }

    /// Renders the `unsupported expr` placeholder so generated code stays
    /// honest and parses (`/* unsupported expr */ 0` / `0 /* unsupported */`).
    #[must_use]
    fn unsupported_placeholder() -> String;

    /// Maps a Flux primitive's native name for this backend (reads the
    /// [`PrimitiveSpec`] table).
    #[must_use]
    fn native_name(spec: &PrimitiveSpec) -> &'static str;

    /// Renders the animation-curve spec literal for an `Animate` wrapper
    /// (FLUX-042). `curve` is the `Animate` node's `curve` prop value rendered
    /// as a Flux expression (e.g. `"easeInOut"` / `"spring"`); the backend maps
    /// it onto the host-native curve spelling. SwiftUI: `withAnimation(.easeInOut)`
    /// / `.spring()`; Compose: `withAnimation(...)`. The signal the animation
    /// drives is data the host consumes; this returns only the spec that wraps
    /// the child subtree.
    #[must_use]
    fn animation_spec(curve: &str) -> String;

    /// Emits the native design-token theme extension (FLUX-043) covering every
    /// token in `tokens`. The extension is a top-level declaration the generated
    /// module ships once so components reference tokens by name rather than
    /// per-component literals. Returns the full native source for the extension.
    #[must_use]
    fn theme_extension(tokens: &[crate::primitives::DesignToken]) -> String;

    // ----- Component / sum-type header hooks (write into the emitter) -----

    /// The indentation level at which a component body's children are emitted.
    #[must_use]
    fn component_body_indent() -> usize
    where
        Self: Sized;

    /// Emits the component declaration header up to and including the props
    /// (for Kotlin also the `) {` open; for Swift just `struct …: View {`).
    /// `subst` maps generic parameters to their concrete arguments for a
    /// specialised monomorphisation (so `initial: T` renders `initial: Int`).
    fn emit_component_header(
        em: &mut Emitter<'_, Self>,
        name: &str,
        generics: &str,
        meta: &ComponentMeta<'_>,
        subst: &HashMap<String, String>,
    ) where
        Self: Sized;

    /// Emits the `body` opening line (Swift `var body: some View {`; a no-op
    /// for Kotlin, whose header already opened the function body).
    fn emit_body_open(em: &mut Emitter<'_, Self>)
    where
        Self: Sized;

    /// Emits the component footer (closing brace(s)).
    fn emit_component_footer(em: &mut Emitter<'_, Self>)
    where
        Self: Sized;

    /// Emits a placeholder component when no AST declaration is available.
    fn emit_placeholder_component(em: &mut Emitter<'_, Self>, id: flux_syntax::NodeId)
    where
        Self: Sized;

    /// Emits one state-cell declaration (`var … by remember` / `@State private var`).
    /// `subst` maps generic parameters to their concrete arguments.
    fn emit_state_cell(
        em: &mut Emitter<'_, Self>,
        name: &str,
        ty: &str,
        init: &str,
        subst: &HashMap<String, String>,
    ) where
        Self: Sized;

    /// Emits one algebraic data type as a native `sealed`/`enum` declaration.
    fn emit_sum_type(em: &mut Emitter<'_, Self>, sum: &TypeDecl)
    where
        Self: Sized;

    /// Emits a `match` over an algebraic data type. The `when`/`switch` syntax
    /// and per-variant binding form differ per backend.
    fn emit_match(em: &mut Emitter<'_, Self>, id: flux_syntax::NodeId, indent: usize)
    where
        Self: Sized;

    /// Source code emitted at the very top of the generated file before any
    /// other declaration (e.g. `import` statements, package declarations).
    /// Defaults to the empty string so backends that need no header are not
    /// forced to implement it.
    #[must_use]
    fn prelude() -> &'static str {
        ""
    }
}
