//! Regression (F20): a `match` with a guard arm must emit valid Kotlin.
//! The previous emission was `is <name> ->` — invalid Kotlin that also
//! silently dropped the guard condition.

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
fn guard_match_emits_valid_kotlin_when_chain() {
    let src = "compo Score\n  state score: Int = 95\n  Column {\n    match score {\n      s if s > 90 => Text(text: \"A\")\n      s if s > 80 => Text(text: \"B\")\n      _ => Text(text: \"C\")\n    }\n  }\n";
    let out = emit(src);
    // Valid Kotlin shape: a `run { val __flux_scrut = ...; when { ... } }`
    // with each guard's binding scoped via `.let`.
    assert!(
        out.contains("val __flux_scrut ="),
        "expected the subject to be hoisted into a `val`:\n{out}"
    );
    assert!(
        out.contains("__flux_scrut.let {"),
        "expected each guard to use `.let` to bind its name:\n{out}"
    );
    assert!(
        out.contains("else ->"),
        "expected the wildcard arm to emit an `else ->`:\n{out}"
    );
    // The old broken shape must be gone.
    assert!(
        !out.contains("/* unsupported match guard:"),
        "old guard-arm placeholder still present:\n{out}"
    );
    // The condition must actually appear, so the guard is not silently dropped.
    assert!(
        out.contains("s > 90") && out.contains("s > 80"),
        "guard conditions must appear in generated Kotlin:\n{out}"
    );
}
