//! FLUX-F65 regression: a nested call inside an outer call's argument
//! list must be rendered as a real native call, not the placeholder.

use flux_parser::parse;
use flux_types::type_check;

fn emit_swift(src: &str) -> String {
    let ast = parse(src, 0, "f.flux").expect("parse");
    let typed = type_check(&ast).expect("type-check");
    let lowered = flux_ir::lower(&ast, &typed).expect("lower");
    flux_codegen_swift::codegen(&lowered, &ast)
}

#[test]
fn nested_call_in_handler_preserves_inner_call() {
    let src = "fn g(n: Int) -> Int { n }\nfn f(n: Int) -> Int { g(n) }\n\ncompo C\n  Button(text: \"x\", onPress: { f(g(1)) })";
    let swift = emit_swift(src);
    // The inner `g(1)` must survive with the label Swift requires.
    assert!(
        swift.contains("f(g(n: 1))") || swift.contains("f(g(1))"),
        "emitted Swift must preserve the nested call; got:\n{swift}"
    );
    assert!(
        !swift.contains("unsupported"),
        "emitted Swift must not contain the unsupported placeholder; got:\n{swift}"
    );
}

#[test]
fn three_deep_nesting_still_preserved() {
    let src = "fn a(n: Int) -> Int { n }\nfn b(n: Int) -> Int { a(n) }\nfn c(n: Int) -> Int { b(n) }\n\ncompo C\n  Button(text: \"x\", onPress: { c(b(a(1))) })";
    let swift = emit_swift(src);
    assert!(
        !swift.contains("unsupported"),
        "three-deep nesting must not fall through to the placeholder; got:\n{swift}"
    );
}
