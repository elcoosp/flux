//! `flux-codegen-kotlin` — release-mode Jetpack Compose code generation for Flux.
//!
//! This crate consumes the lowered Flux reactive tree ([`flux_ir::LoweredIr`])
//! together with its originating surface AST ([`flux_parser::Ast`]) and emits
//! idiomatic Kotlin/Compose source (spec FR-011, Appendix F, ADR-0003 /
//! ADR-0004). The dev server ships the same IR as binary patches; in release,
//! this crate codegen's it to a native Compose app.
//!
//! The lowered arena carries only numeric component identifiers and no
//! runtime values (it is kept compact on purpose — Appendix C §C.1). Component
//! names, generics, `@pure` annotations, prop/state types and string
//! interpolations are therefore recovered from the AST through the ADR-0027
//! node-ID bridge exposed in the `bridge` module.
//!
//! # Examples
//!
//! ```rust
//! use flux_codegen_kotlin::codegen;
//! use flux_parser::parse;
//! use flux_types::type_check;
//! use flux_ir::lower;
//!
//! let src = "component Hello { state count: Int = 0 Text(\"hi\") }";
//! let ast = parse(src, 0, "hello.flux").unwrap();
//! let typed = type_check(&ast).expect("well-typed");
//! let lowered = lower(&ast, &typed).expect("lowers");
//! let kotlin = codegen(&lowered, &ast);
//! assert!(kotlin.contains("@Composable fun Hello"));
//! ```
#![forbid(unsafe_code)]
#![warn(
    missing_docs,
    missing_debug_implementations,
    rust_2018_idioms,
    unreachable_pub
)]

mod bridge;
mod codegen;
mod error;
mod expressions;
mod model;
mod nodes;
mod printers;
mod program;

pub use codegen::codegen;
pub use error::CodegenError;
