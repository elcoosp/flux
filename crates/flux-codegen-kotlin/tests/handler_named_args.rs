//! FLUX-F63 regression: a named-arg call inside a handler must keep its
//! parameter name in the emitted Kotlin.

use flux_parser::parse;
use flux_types::type_check;

fn emit_kotlin(src: &str) -> String {
    let ast = parse(src, 0, "f.flux").expect("parse");
    let typed = type_check(&ast).expect("type-check");
    let lowered = flux_ir::lower(&ast, &typed).expect("lower");
    flux_codegen_kotlin::codegen(&lowered, &ast)
}

#[test]
fn named_arg_call_in_handler_preserves_name() {
    let src = "fn f(n: Int) -> Int { n }\n\ncompo C\n  Button(text: \"x\", onPress: { f(n: 1) })";
    let kt = emit_kotlin(src);
    assert!(
        kt.contains("f(n = 1)"),
        "emitted Kotlin must preserve the arg name `n =`; got:\n{kt}"
    );
}
