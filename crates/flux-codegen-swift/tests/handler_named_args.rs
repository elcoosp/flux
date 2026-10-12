//! FLUX-F63 regression: a named-arg call inside a handler must keep its
//! parameter label in the emitted Swift.

use flux_parser::parse;
use flux_types::type_check;

fn emit_swift(src: &str) -> String {
    let ast = parse(src, 0, "f.flux").expect("parse");
    let typed = type_check(&ast).expect("type-check");
    let lowered = flux_ir::lower(&ast, &typed).expect("lower");
    flux_codegen_swift::codegen(&lowered, &ast)
}

#[test]
fn named_arg_call_in_handler_preserves_label() {
    let src = "fn f(n: Int) -> Int { n }\n\ncompo C\n  Button(text: \"x\", onPress: { f(n: 1) })";
    let swift = emit_swift(src);
    assert!(
        swift.contains("f(n: 1)"),
        "emitted Swift must preserve the parameter label `n:`; got:\n{swift}"
    );
}

/// FLUX-F64 (documented gap, pinned for regression detection): a Flux
/// **positional** call to a Swift function declared with named params
/// still emits `f(1)` (no label), which Swift does not accept. When the
/// emitter is updated to synthesise labels from the callee's parameter
/// names, flip this assertion to `f(x: 1)`.
#[test]
fn positional_arg_call_in_handler_emits_bare_positional_today() {
    let src = "fn f(x: Int) -> Int { x }\n\ncompo C\n  Button(text: \"x\", onPress: { f(1) })";
    let swift = emit_swift(src);
    assert!(
        swift.contains("f(1)"),
        "documented current behavior: positional Flux call emits bare `f(1)`; got:\n{swift}"
    );
}
