//! Top-level codegen orchestration for FLUX-021 (Kotlin/Compose).
//!
//! [`codegen`] turns a lowered Flux reactive tree plus its originating surface
//! AST into idiomatic Kotlin/Compose source (spec FR-011, Appendix F). The
//! lowered arena provides tree *structure*; the AST (reached via the ADR-0027
//! node-ID bridge) provides *semantics* — component names, generics, `@pure`,
//! props, `state`, string interpolations — that the arena deliberately drops to
//! stay compact. The shared emitter and primitive registry live in
//! `flux-codegen-core`; this function only wires the Kotlin [`Backend`].

use flux_codegen_core::{Bridge, Emitter};
use flux_ir::LoweredIr;
use flux_parser::Ast;

/// Generates Kotlin/Compose source from a lowered Flux program and its surface AST.
///
/// # Examples
///
/// ```rust
/// use flux_codegen_kotlin::codegen;
/// use flux_parser::parse;
/// use flux_types::type_check;
/// use flux_ir::lower;
///
/// let src = "compo Hello\n  state count: Int = 0\n  Text(\"hi\")\n";
/// let ast = parse(src, 0, "hello.flux").unwrap();
/// let typed = type_check(&ast).expect("well-typed");
/// let lowered = lower(&ast, &typed).expect("lowers");
/// let kotlin = codegen(&lowered, &ast);
/// assert!(kotlin.contains("var count by remember { mutableStateOf<Int>(0) }"));
/// ```
#[must_use]
pub fn codegen(lowered: &LoweredIr, ast: &Ast) -> String {
    let bridge = Bridge::build(ast);
    let mut emitter = Emitter::<crate::backend_impl::Kotlin>::new(lowered, &bridge);
    emitter.emit_program();
    emitter.finish()
}
