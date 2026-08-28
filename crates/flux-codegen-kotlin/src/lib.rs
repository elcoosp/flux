//! `flux-codegen-kotlin` — release-mode Jetpack Compose code generation for Flux.
//!
//! This crate consumes the lowered Flux reactive tree ([`flux_ir::LoweredIr`])
//! together with its originating surface AST ([`flux_parser::Ast`]) and emits
//! idiomatic Kotlin/Compose source (spec FR-011, Appendix F, ADR-0003 /
//! ADR-0004). The shared traversal, primitive registry, node-ID bridge and
//! expression renderer live in `flux-codegen-core`; this crate supplies only the
//! Kotlin-specific syntax via the [`Backend`](flux_codegen_core::Backend) impl
//! in `backend_impl` and the component/sum-type header forms in `component`.
//!
//! # Examples
//!
//! ```rust
//! use flux_codegen_kotlin::codegen;
//! use flux_parser::parse;
//! use flux_types::type_check;
//! use flux_ir::lower;
//!
//! let src = "compo Hello\n  state count: Int = 0\n  Text(\"hi\")\n";
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

mod backend_impl;
mod codegen;

pub use codegen::codegen;
pub use flux_codegen_core::CodegenError;
