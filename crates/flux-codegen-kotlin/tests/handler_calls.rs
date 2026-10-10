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
