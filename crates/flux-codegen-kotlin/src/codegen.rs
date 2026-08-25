//! Top-level codegen orchestration for FLUX-021.
//!
//! [`codegen`] turns a lowered Flux reactive tree plus its originating surface
//! AST into idiomatic Kotlin/Compose source (spec FR-011, Appendix F). The
//! lowered arena provides tree *structure*; the AST (reached via the ADR-0027
//! node-ID bridge) provides *semantics* — component names, generics, `@pure`,
//! props, `state`, string interpolations — that the arena deliberately drops to
//! stay compact.

use flux_ir::LoweredIr;
use flux_parser::Ast;

use crate::bridge::Bridge;
use crate::program::Emitter;

/// Generates Kotlin/Compose source from a lowered Flux program and its surface AST.
///
/// The two inputs are complementary: `lowered` is the packed reactive tree
/// (structure — what nodes exist and how they nest), while `ast` is the
/// original surface syntax. Because the arena stores only numeric component
/// identifiers and drops runtime values, names, generics and interpolations
/// are recovered from `ast` through the ADR-0027 node-ID bridge (the `bridge`
/// module).
///
/// # Panics
///
/// Does not panic on well-formed input: every construct that cannot be
/// represented degrades to a parseable placeholder (a Kotlin comment or empty
/// composable) rather than aborting the whole file.
///
/// # Examples
///
/// ```rust
/// use flux_codegen_kotlin::codegen;
/// use flux_parser::parse;
/// use flux_types::type_check;
/// use flux_ir::lower;
///
/// let src = "component Hello { state count: Int = 0 Text(\"hi\") }";
/// let ast = parse(src, 0, "hello.flux").unwrap();
/// let typed = type_check(&ast).expect("well-typed");
/// let lowered = lower(&ast, &typed).expect("lowers");
/// let kotlin = codegen(&lowered, &ast);
/// assert!(kotlin.contains("@Composable fun Hello"));
/// assert!(kotlin.contains("var count by remember { mutableStateOf<Int>(0) }"));
/// ```
#[must_use]
pub fn codegen(lowered: &LoweredIr, ast: &Ast) -> String {
    let bridge = Bridge::build(ast);
    let mut emitter = Emitter::new(lowered, &bridge);
    emitter.emit_program();
    emitter.finish()
}
