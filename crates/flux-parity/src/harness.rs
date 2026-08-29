//! Headless `.flux` app-test harness (FLUX-034).
//!
//! Exposes a small, user-facing test API on top of the parity pipeline so that
//! component-level tests can run a `.flux` source through the dev compiler
//! headlessly and assert on its structural shadow tree and on signal state after
//! a synthetic interaction.
//!
//! The harness deliberately reuses the existing compilation/execution machinery
//! rather than inventing a new VM:
//! - the dev path lowers a `.flux` source to a structural [`ViewNode`] tree via
//!   [`crate::compile`] + [`crate::from_ast`] (exactly what `flux-parity` already
//!   does for parity);
//! - interaction effects are asserted by running an Appendix-E ISA program
//!   through [`flux_vm_ref::run`] and inspecting the resulting signal cells.
//!
//! The section marked `SignalAssertion` is the "assert on emitted signals" half
//! of the roadmap §9 ask. It consumes a *caller-supplied* ISA program because
//! this tree has no `.flux → ISA` lowering pass yet (codegen targets Swift/Kotlin
//! source, not bytecode). When that compiler lands, `ComponentUnderTest` can gain
//! a `tap(signal)` helper that lowers a `Tap` handler to bytes; until then the
//! caller provides the bytes, which keeps the assertion path honest and tested.

use flux_codegen_core::ViewNode;
use flux_parser::Ast;
use flux_syntax::SignalId;
use flux_syntax::Value;
use flux_types::TypedAST;
use flux_vm_ref::{InMemorySignals, VmOutcome, run};

use crate::relation::{ParityPipelineError, compile};

/// A component loaded headlessly, ready for structural and signal assertions.
#[derive(Clone, Debug)]
pub struct ComponentUnderTest {
    /// The parsed surface AST (authoritative "what the user wrote").
    pub ast: Ast,
    /// The type-checked AST.
    pub typed: TypedAST,
    /// The dev-path structural shadow tree.
    view_tree: Vec<ViewNode>,
}

impl ComponentUnderTest {
    /// Returns the dev-path structural view tree (the headless shadow tree).
    #[must_use]
    pub fn view_tree(&self) -> &[ViewNode] {
        &self.view_tree
    }

    /// Returns every node whose normalized surface name equals `name`.
    #[must_use]
    pub fn find_all(&self, name: &str) -> Vec<&ViewNode> {
        let mut out = Vec::new();
        collect_by_name(&self.view_tree, name, &mut out);
        out
    }

    /// Returns the first node whose normalized surface name equals `name`, if any.
    #[must_use]
    pub fn find_first(&self, name: &str) -> Option<&ViewNode> {
        self.find_all(name).into_iter().next()
    }

    /// Counts nodes whose normalized surface name equals `name`.
    #[must_use]
    pub fn count(&self, name: &str) -> usize {
        self.find_all(name).len()
    }
}

fn collect_by_name<'a>(nodes: &'a [ViewNode], name: &str, out: &mut Vec<&'a ViewNode>) {
    for node in nodes {
        match node {
            ViewNode::Component { name: n, children }
            | ViewNode::Primitive {
                name: n, children, ..
            } => {
                if n == name {
                    out.push(node);
                }
                collect_by_name(children, name, out);
            }
            ViewNode::If {
                then_branch,
                else_branch,
                ..
            } => {
                collect_by_name(then_branch, name, out);
                collect_by_name(else_branch, name, out);
            }
            ViewNode::Match { arms, .. } => {
                for (_, body) in arms {
                    collect_by_name(body, name, out);
                }
            }
            ViewNode::ForEach { .. } => {
                // Empty body by design (FLUX-014); nothing to recurse into.
            }
            ViewNode::Router { children } | ViewNode::Screen { children, .. } => {
                collect_by_name(children, name, out);
            }
        }
    }
}

/// Error produced while rendering a component headlessly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderError(pub String);

impl std::fmt::Display for RenderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "headless render error: {}", self.0)
    }
}

impl std::error::Error for RenderError {}

impl From<ParityPipelineError> for RenderError {
    fn from(e: ParityPipelineError) -> Self {
        RenderError(e.0)
    }
}

/// Renders a `.flux` source headlessly: parse → type-check → lower → dev view tree.
///
/// # Errors
///
/// Surfaces any parse, type-check, or lowering failure (e.g. a missing lowering
/// pass for a new primitive) as [`RenderError`].
pub fn render_component(source: &str, file_id: u32) -> Result<ComponentUnderTest, RenderError> {
    let (ast, typed, _lowered) = compile(source, file_id)?;
    let view_tree = crate::from_ast(&ast);
    Ok(ComponentUnderTest {
        ast,
        typed,
        view_tree,
    })
}

/// The outcome of a synthetic interaction: the final signal cells plus the raw VM outcome.
#[derive(Clone, Debug)]
pub struct InteractionOutcome {
    /// Final values of every signal cell that was written.
    pub signals: Vec<(SignalId, Value)>,
    /// The raw reference-VM outcome (registers, gas) for deeper assertions.
    pub vm: VmOutcome,
}

/// Runs an Appendix-E ISA program as a synthetic "tap" and returns the resulting
/// signal state.
///
/// `program` is the bytecode for the handler under test (e.g. the compiled body
/// of a `Tap` handler). `entry` is the payload delivered to register `r0`. This
/// is the honest, tested path for "assert a signal updates after a synthetic tap"
/// until a `.flux → ISA` lowering pass exists (see module docs).
///
/// # Errors
///
/// Surfaces any VM fault (e.g. `DivByZero`, `InvalidDispatch`) as [`RenderError`].
pub fn run_tap(program: &[u8], entry: Value) -> Result<InteractionOutcome, RenderError> {
    let mut signals = InMemorySignals::default();
    let vm = run(program, &mut signals, entry).map_err(|e| RenderError(format!("vm: {e}")))?;
    Ok(InteractionOutcome {
        signals: vm.signals.clone(),
        vm,
    })
}

/// Convenience: the value of `signal` after running `program` with `entry`, or
/// `None` if that cell was never written.
///
/// # Errors
///
/// Surfaces any VM fault via [`RenderError`].
pub fn signal_after_tap(
    program: &[u8],
    entry: Value,
    signal: SignalId,
) -> Result<Option<Value>, RenderError> {
    let outcome = run_tap(program, entry)?;
    Ok(outcome
        .signals
        .into_iter()
        .find(|(id, _)| *id == signal)
        .map(|(_, v)| v))
}
