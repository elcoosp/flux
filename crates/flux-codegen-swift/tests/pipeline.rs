//! Full-pipeline integration tests for `flux-codegen-swift` (FLUX-020).
//!
//! Each test runs the documented pipeline — `parse` → `type_check` → `lower`
//! → `codegen` — over one of the Appendix B.3 grammar examples (completed
//! where the spec elides bodies with `{ … }` or omits sibling declarations),
//! then asserts the generated SwiftUI via an [`insta`] snapshot. A determinism
//! check and a `swiftc -parse` parse-check (when a Swift toolchain is present)
//! round out the suite.

use flux_codegen_swift::codegen;
use flux_ir::lower;
use flux_parser::parse;
use flux_types::type_check;
use insta::assert_snapshot;

/// Runs parse → type-check → lower → codegen, panicking with context on the
/// first stage that fails (the pipeline is the contract under test).
fn codegen_example(name: &str, src: &str) -> String {
    let ast = parse(src, 0, &format!("{name}.flux"))
        .unwrap_or_else(|e| panic!("parse failed for {name}: {e:?}"));
    let typed = type_check(&ast).unwrap_or_else(|e| panic!("type_check failed for {name}: {e:?}"));
    let lowered = lower(&ast, &typed).unwrap_or_else(|e| panic!("lower failed for {name}: {e:?}"));
    codegen(&lowered, &ast)
}

/// The 10 Appendix B.3 grammar examples, written in the project's actual
/// grammar (props in a parenthesized block before the body, `[T]` for generic
/// parameters/arguments, `when/otherwise` for conditionals). Where the spec
/// elides a sibling declaration it is supplied here so the pipeline is whole.
fn examples() -> Vec<(&'static str, &'static str)> {
    vec![
        (
            "b3_1_counter",
            "compo Counter\n  state count: Int = 0\n  Column {\n    Text(\"Count: {count}\")\n    Button(onPress: { count = count + 1 }) { Text(\"Increment\") }\n  }\n\n",
        ),
        (
            "b3_2_button",
            "compo Tapped\n  state taps: Int = 0\n  Button(onPress: { taps = taps + 1 }) { Text(\"Tapped {taps} times\") }\n\n",
        ),
        (
            "b3_3_match",
            "type Shape = Circle(Int) | Rect(Int, Int)\ncompo AreaView(shape: Shape)\n  Column {\n    match shape {\n      Circle(r) => Text(\"circle\")\n      Rect(w, h) => Text(\"rect\")\n    }\n  }\n\n",
        ),
        (
            "b3_4_router",
            "compo App\n  state route: String = \"home\"\n  Router {\n    Screen(\"home\") { Text(\"Home\") }\n    Screen(\"settings\") { Text(\"Settings\") }\n  }\n\n",
        ),
        (
            "b3_5_conditional",
            "compo App\n  state show: Bool = false\n  Column {\n    when show {\n      Text(\"visible\")\n    } otherwise {\n      Text(\"hidden\")\n    }\n  }\n\n",
        ),
        (
            "b3_6_fetch",
            "compo Feed\n  state items: List[String] = [\"a\", \"b\"]\n  Column {\n    ForEach(items, key: fn(s) { s.id }) { item =>\n      Text(item)\n    }\n  }\n\n",
        ),
        (
            "b3_7_optional",
            "compo Detail(model: Model)\n  Column {\n    Text(model.title)\n  }\n\n",
        ),
        (
            "b3_8_form",
            "compo Login\n  state value: String = \"\"\n  Column {\n    Text(\"Login\")\n    Button(onPress: { value = \"\" }) { Text(\"Reset\") }\n  }\n\n",
        ),
        (
            "b3_9_state",
            "compo Toggle\n  state on: Bool = false\n  Button(onPress: { on = true }) { Text(\"on = {on}\") }\n\n",
        ),
        (
            "b3_10_generics",
            "compo List[T](items: List[T])\n  Column {\n    ForEach(items, key: fn(t) { t.id }) { item =>\n      Text(item)\n    }\n  }\n\n",
        ),
    ]
}

#[test]
fn pipeline_b3_1_counter() {
    let (name, src) = &examples()[0];
    assert_snapshot!(codegen_example(name, src));
}

#[test]
fn pipeline_b3_2_button() {
    let (name, src) = &examples()[1];
    assert_snapshot!(codegen_example(name, src));
}

#[test]
fn pipeline_b3_3_match() {
    let (name, src) = &examples()[2];
    assert_snapshot!(codegen_example(name, src));
}

#[test]
fn pipeline_b3_4_router() {
    let (name, src) = &examples()[3];
    assert_snapshot!(codegen_example(name, src));
}

#[test]
fn pipeline_b3_5_conditional() {
    let (name, src) = &examples()[4];
    assert_snapshot!(codegen_example(name, src));
}

#[test]
fn pipeline_b3_6_fetch() {
    let (name, src) = &examples()[5];
    assert_snapshot!(codegen_example(name, src));
}

#[test]
fn pipeline_b3_7_optional() {
    let (name, src) = &examples()[6];
    assert_snapshot!(codegen_example(name, src));
}

#[test]
fn pipeline_b3_8_form() {
    let (name, src) = &examples()[7];
    assert_snapshot!(codegen_example(name, src));
}

#[test]
fn pipeline_b3_9_state() {
    let (name, src) = &examples()[8];
    assert_snapshot!(codegen_example(name, src));
}

#[test]
fn pipeline_b3_10_generics() {
    let (name, src) = &examples()[9];
    assert_snapshot!(codegen_example(name, src));
}

/// The generated Swift must be stable across runs (no hash/address leakage).
#[test]
fn codegen_is_deterministic() {
    let (name, src) = &examples()[0];
    let first = codegen_example(name, src);
    let second = codegen_example(name, src);
    assert_eq!(
        first, second,
        "codegen output differs between identical runs"
    );
}

/// A few structural invariants every component emission must satisfy.
#[test]
fn emits_view_structs_and_state() {
    let (name, src) = &examples()[0];
    let out = codegen_example(name, src);
    assert!(out.contains("struct Counter: View"), "missing View struct");
    assert!(
        out.contains("@State private var count: Int = 0"),
        "missing @State"
    );
    assert!(
        out.contains("Text(\"Count: \\(count)\")"),
        "missing interpolation"
    );
    assert!(out.contains("VStack {"), "missing VStack container");
    assert!(
        out.contains("Button(action: { count = (count + 1) })"),
        "Button must emit its onClick handler body, not an empty closure: {out}"
    );
}

/// `swiftc -parse` must accept the generated SwiftUI (syntax-only), per the
/// issue's acceptance bar. Skipped when no Swift toolchain is on PATH.
#[test]
fn generated_swift_parses() {
    let mut combined = String::new();
    for (name, src) in examples() {
        combined.push_str(&codegen_example(name, src));
        combined.push('\n');
    }
    if std::process::Command::new("swiftc")
        .arg("--version")
        .output()
        .is_err()
    {
        eprintln!("swiftc not on PATH; skipping swiftc -parse check");
        return;
    }
    let dir = std::env::temp_dir();
    let path = dir.join("flux_codegen_swift_generated.swift");
    std::fs::write(&path, &combined).expect("write temp swift file");
    let status = std::process::Command::new("swiftc")
        .arg("-parse")
        .arg(&path)
        .status()
        .expect("spawn swiftc");
    assert!(
        status.success(),
        "swiftc -parse rejected generated Swift:\n{combined}"
    );
}

/// Regression test for the Button codegen defect: the `onClick` handler body and
/// the `text:` label must both reach the generated output. A prior build emitted
/// an empty `Button(action: {}) { Text("") }`, dropping the tap behaviour. This
/// locks the correct behaviour for both the named-arg form
/// (`Button(text:, onClick:)`) used by `examples/counter` and the trailing-block
/// form (`Button(...) { Text(...) }`).
#[test]
fn button_emits_handler_and_label() {
    let src = "compo Tapped\n  state taps: Int = 0\n  Button(text: \"Tap me\", onPress: fn() { taps = taps + 1 })\n\n";
    let out = codegen_example("button_regression", src);
    assert!(
        out.contains("Button(action: { taps = (taps + 1) })"),
        "missing onClick handler body in:\n{out}"
    );
    assert!(
        out.contains("Text(\"Tap me\")"),
        "missing button label in:\n{out}"
    );

    // Trailing-block label form must also work.
    let src2 = "compo Tapped2\n  state taps: Int = 0\n  Button(onPress: fn() { taps = taps + 1 }) { Text(\"Block\") }\n\n";
    let out2 = codegen_example("button_regression_2", src2);
    assert!(
        out2.contains("Button(action: { taps = (taps + 1) })"),
        "missing onClick handler body in trailing form:\n{out2}"
    );
    assert!(
        out2.contains("Text(\"Block\")"),
        "missing trailing-block label in:\n{out2}"
    );
}

/// Roadmap Phase 1: a generic component emits one specialised native struct per
/// instantiation, and call sites resolve to the specialised name (so the runtime
/// keeps the type argument and the host sees distinct component kinds).
#[test]
fn generic_component_emits_specialised_structs() {
    let src = "trait Numeric[T] { fn zero() -> T }\n\ncompo Counter[T: Numeric](initial: T)\n  state count: T = initial\n\ncompo IntCase\n  Counter(initial: 0)\n\ncompo FloatCase\n  Counter(initial: 0.0)\n\n";
    let out = codegen_example("generic_mono", src);
    assert!(
        out.contains("struct Counter_Int: View"),
        "expected a specialised Int struct in:\n{out}"
    );
    assert!(
        out.contains("struct Counter_Float: View"),
        "expected a specialised Float struct in:\n{out}"
    );
    // The generic template must NOT ship as a parametric native type.
    assert!(
        !out.contains("struct Counter<T>"),
        "generic template must not emit a parametric native type in:\n{out}"
    );
    // Caller sites resolve to the specialised names via component_names.
    assert!(
        out.contains("Counter_Int(initial: 0)"),
        "Int call site must resolve to Counter_Int(initial: 0) in:\n{out}"
    );
    assert!(
        out.contains("Counter_Float(initial: 0.0)"),
        "Float call site must resolve to Counter_Float(initial: 0.0) in:\n{out}"
    );
}
