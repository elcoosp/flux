//! The structural model both paths are reduced to before comparison.
//!
//! Parity is proven by reducing the dev-path surface AST ([`flux_parser::Ast`]) and
//! each release-path emitted source (SwiftUI / Compose text) to the *same*
//! discriminated [`ViewNode`] tree, then asserting the three trees are
//! structurally identical. We compare structure — component/view graph, control
//! flow (`if`/`when`/`ForEach`/`match`) and value bindings (string literals) — not
//! source text, so cosmetic backend differences (Swift `VStack` vs Kotlin
//! `Column`, `\()` vs `${}`) are correctly normalized away.
//!
//! The dev path drives the tree directly from the parsed AST: the AST is the
//! authoritative "what the user wrote" and is exactly what the release codegen
//! derives from, so reducing it to the structural [`ViewNode`] tree is the
//! faithful dev-side equivalent. State/handler/prop/lifecycle declarations are
//! skipped; only the view graph and control flow are retained.

/// A single node in the language-neutral structural view tree.
///
/// Only the facts that must match between the dev path's surface AST and the
/// release codegen's emitted component are retained. Condition / collection /
/// key / prop-value text is stored in a canonical form (produced by the internal
/// `canonicalize_expr` helper) so that Swift and Kotlin backends (and the dev
/// path) compare equal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ViewNode {
    /// A top-level component (dev `Component` node, release `struct …: View` /
    /// `@Composable fun …`).
    Component {
        /// Surface component name (e.g. `HelloWorld`).
        name: String,
        /// Structural children.
        children: Vec<ViewNode>,
    },
    /// A leaf or container adapter view (`Text`, `Button`, `Column`, `VStack`,
    /// `Image`, `Row`, `HStack`, …). The name is the normalized Flux surface
    /// spelling (see [`normalize_view_name`]).
    Primitive {
        /// Normalized Flux surface name.
        name: String,
        /// Trailing-block prop entries, keyed by name (e.g. `width: size`).
        props: Vec<(String, String)>,
        /// Structural children.
        children: Vec<ViewNode>,
    },
    /// A conditional (`if` or `when … otherwise`).
    If {
        /// The condition, in canonical form.
        cond: String,
        /// Then-branch children.
        then_branch: Vec<ViewNode>,
        /// Else/otherwise children (empty when absent).
        else_branch: Vec<ViewNode>,
    },
    /// A keyed collection repeater. The body is intentionally empty in the MLP:
    /// keyed items are reconciled at runtime by the host (FLUX-014), so the
    /// lowered IR emits a `ForEach` node with an empty splice and both codegen
    /// backends render an empty `ForEach`/`items` wrapper. Parity therefore
    /// asserts the wrapper renders with an empty body in *all three* paths — an
    /// empty body is the expected, faithful shape, never a divergence.
    ForEach {
        /// The collection expression, in canonical form.
        collection: String,
        /// The stable key extractor, in canonical form.
        key_path: String,
    },
    /// An algebraic-data-type match (`switch`/`when`), one arm per body.
    Match {
        /// The scrutinee expression, in canonical form.
        scrutinee: String,
        /// One entry per arm: its pattern label and its body children.
        arms: Vec<(String, Vec<ViewNode>)>,
    },
    /// A `Router` navigation container.
    Router {
        /// Destination screens.
        children: Vec<ViewNode>,
    },
    /// A `Screen` destination inside a `Router`.
    Screen {
        /// The route string.
        route: String,
        /// Screen body children.
        children: Vec<ViewNode>,
    },
}

impl ViewNode {
    /// Returns the human-readable kind label, used in diagnostics.
    #[must_use]
    pub fn kind_label(&self) -> &'static str {
        match self {
            ViewNode::Component { .. } => "Component",
            ViewNode::Primitive { .. } => "Primitive",
            ViewNode::If { .. } => "If",
            ViewNode::ForEach { .. } => "ForEach",
            ViewNode::Match { .. } => "Match",
            ViewNode::Router { .. } => "Router",
            ViewNode::Screen { .. } => "Screen",
        }
    }
}

pub(crate) use crate::reduce::is_container;
pub use crate::reduce::{from_ast, normalize_view_name};
