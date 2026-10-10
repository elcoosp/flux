//! Regression (round 34): a handler whose body is a plain capability-method
//! call must survive codegen. Before this fix, `render_stmt` fell through to
//! the `unsupported_placeholder` for any `ExprKind::Call` that was not
//! `RouterNav.navigate(...)`, so `onPress: { Auth.login("u") }` emitted
//! `/* unsupported expr */ 0` in place of the call.
//!
//! The `await`-of-a-call shape is intentionally not exercised here: the IR
//! lowering currently rejects `ExprKind::Await` in a handler (see
//! `docs/issues/FLUX-F19-async-handler-lowering.md`), so the codegen path is
//! unreachable for that shape until the lowering is extended.

use flux_codegen_kotlin::codegen;
use flux_ir::lower;
use flux_parser::parse;
use flux_types::type_check;

fn emit(src: &str) -> String {
    let ast = parse(src, 0, "t.flux").expect("parse");
    let typed = type_check(&ast).expect("type-check");
    let lowered = lower(&ast, &typed).expect("lower");
    codegen(&lowered, &ast)
}

#[test]
fn capability_call_handler_survives_codegen() {
    // `Auth.login("u")` is a capability method call — type-checks against the
    // capability IDL and lowers through `compile_call`.
    let out = emit("compo C\n  state n: Int = 0\n  Button(text: \"go\", onPress: { Auth.login(\"u\") })\n");
    assert!(
        out.contains("Auth.login(\"u\")"),
        "the capability call must appear in generated Kotlin:\n{out}"
    );
    assert!(
        !out.contains("/* unsupported expr */"),
        "handler call collapsed to a placeholder:\n{out}"
    );
}

/// Regression (round 57): with F19's lowering fix in place, a bare
/// `await Auth.login("u")` handler now reaches codegen and must be emitted
/// with the backend's await syntax inside the coroutine wrapper.
#[test]
fn awaited_handler_renders_await_expression() {
    // The IR lowering accepts this since round 56 (the bare `await` arm was
    // added to `compile_block_or_expr`).
    let out = emit("compo C\n  state n: Int = 0\n  Button(text: \"go\", onPress: { await Auth.login(\"u\") })\n");
    // The awaited call itself survives.
    assert!(
        out.contains("Auth.login(\"u\")"),
        "awaited call must appear in generated source:\n{out}"
    );
    // Both backends wrap an awaiting handler in a coroutine so the release
    // path can suspend without blocking the UI thread (FLUX-064).
    assert!(
        out.contains("launch") || out.contains("Task {"),
        "awaited handler must be wrapped in a coroutine:\n{out}"
    );
    // The placeholder never leaks.
    assert!(
        !out.contains("unsupported expr") && !out.contains("unsupported match"),
        "awaited handler collapsed to a placeholder:\n{out}"
    );
}

