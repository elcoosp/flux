//! Full-pipeline integration tests for `flux-codegen-kotlin` (FLUX-021).
//!
//! Each test runs the documented pipeline — `parse` → `type_check` → `lower`
//! → `codegen` — over one of the Appendix B.3 grammar examples (completed
//! where the spec elides bodies with `{ … }` or omits sibling declarations),
//! then asserts the generated Compose via an [`insta`] snapshot. A determinism
//! check and a `kotlinc` compile-check (when a full Android/Compose toolchain
//! is present — see [`android_toolchain_present`]) round out the suite.

use flux_codegen_kotlin::codegen;
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

/// `kotlinc` resolves `androidx.compose.*` imports only when the Android SDK and
/// the Compose compiler are on its classpath — which only a configured Android
/// toolchain provides (e.g. the Gradle `android-check` job). The plain Rust
/// `rust-check` runner has `kotlinc` on PATH but no Android SDK, so a bare
/// `kotlinc` invocation fails to resolve `androidx`. This guard lets the
/// `kotlinc` compile-check run only where it can actually succeed.
fn android_toolchain_present() -> bool {
    std::env::var_os("ANDROID_HOME").is_some() || std::env::var_os("ANDROID_SDK_ROOT").is_some()
}

/// The 10 Appendix B.3 grammar examples, written in the project's actual
/// grammar (props in a parenthesized block before the body, `[T]` for generic
/// parameters/arguments, `when/otherwise` for conditionals). Where the spec
/// elides a sibling declaration it is supplied here so the pipeline is whole.
fn examples() -> Vec<(&'static str, &'static str)> {
    vec![
        (
            "b3_1_counter",
            "component Counter {\n  state count: Int = 0\n  Column {\n    Text(\"Count: {count}\")\n    Button(onClick: { count = count + 1 }) { Text(\"Increment\") }\n  }\n}\n",
        ),
        (
            "b3_2_button",
            "component Tapped {\n  state taps: Int = 0\n  Button(onClick: { taps = taps + 1 }) { Text(\"Tapped {taps} times\") }\n}\n",
        ),
        (
            "b3_3_match",
            "type Shape = Circle(Int) | Rect(Int, Int)\n\
             component AreaView(shape: Shape) {\n  Column {\n    match shape {\n      Circle(r) => Text(\"circle\")\n      Rect(w, h) => Text(\"rect\")\n    }\n  }\n}\n",
        ),
        (
            "b3_4_router",
            "component App {\n  state route: String = \"home\"\n  Router {\n    Screen(\"home\") { Text(\"Home\") }\n    Screen(\"settings\") { Text(\"Settings\") }\n  }\n}\n",
        ),
        (
            "b3_5_conditional",
            "component App {\n  state show: Bool = false\n  Column {\n    when show {\n      Text(\"visible\")\n    } otherwise {\n      Text(\"hidden\")\n    }\n  }\n}\n",
        ),
        (
            "b3_6_fetch",
            "component Feed {\n  state items: List[String] = [\"a\", \"b\"]\n  Column {\n    ForEach(items, key: fn(s) { s.id }) { item =>\n      Text(item)\n    }\n  }\n}\n",
        ),
        (
            "b3_7_optional",
            "component Detail(model: Model) {\n  Column {\n    Text(model.title)\n  }\n}\n",
        ),
        (
            "b3_8_form",
            "component Login {\n  state value: String = \"\"\n  Column {\n    Text(\"Login\")\n    Button(onClick: { value = \"\" }) { Text(\"Reset\") }\n  }\n}\n",
        ),
        (
            "b3_9_state",
            "component Toggle {\n  state on: Bool = false\n  Button(onClick: { on = true }) { Text(\"on = {on}\") }\n}\n",
        ),
        (
            "b3_10_generics",
            "component List[T](items: List[T]) {\n  Column {\n    ForEach(items, key: fn(t) { t.id }) { item =>\n      Text(item)\n    }\n  }\n}\n",
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

/// The generated Kotlin must be stable across runs (no hash/address leakage).
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
fn emits_composable_and_state() {
    let (name, src) = &examples()[0];
    let out = codegen_example(name, src);
    assert!(
        out.contains("@Composable fun Counter"),
        "missing composable"
    );
    assert!(
        out.contains("var count by remember { mutableStateOf<Int>(0) }"),
        "missing remembered state"
    );
    assert!(
        out.contains("Text(\"Count: ${count}\")"),
        "missing interpolation"
    );
    assert!(out.contains("Column {"), "missing Column container");
    assert!(
        out.contains("Button(onClick = { count = (count + 1) })"),
        "Button must emit its onClick handler body, not an empty closure: {out}"
    );
}

/// The `gap` prop becomes a Compose `Arrangement.spacedBy(N.dp)` argument
/// (Appendix F — flat props map to deterministic modifier chains).
#[test]
fn gap_becomes_spacing_modifier() {
    let src = "component Spaced {\n  Column(gap: 8) {\n    Text(\"a\")\n  }\n}\n";
    let out = codegen_example("gap", src);
    assert!(
        out.contains("Column(horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(8.dp)) {"),
        "gap not lowered to spacing: {out}"
    );
}

/// Key substrings the issue's acceptance bar requires to be present in the
/// generated Kotlin (used as the fallback parse check when no `kotlinc`
/// toolchain is available).
#[test]
fn generated_kotlin_contains_key_substrings() {
    let mut combined = String::new();
    for (name, src) in examples() {
        combined.push_str(&codegen_example(name, src));
        combined.push('\n');
    }
    assert!(combined.contains("@Composable fun"), "missing @Composable");
    assert!(combined.contains("items("), "missing items()");
    assert!(combined.contains("NavHost"), "missing NavHost");
    assert!(
        combined.contains("Button(onClick = { count = (count + 1) })"),
        "missing Button with onClick handler body: {combined}"
    );
}

/// `kotlinc -Xallow-no-source-files` must accept the generated Compose (syntax
/// only), per the issue's acceptance bar. Skipped when no Kotlin toolchain is
/// on PATH.
#[test]
fn generated_kotlin_parses() {
    let mut combined = String::new();
    combined.push_str("import androidx.compose.runtime.*\n");
    combined.push_str("import androidx.compose.foundation.layout.*\n");
    combined.push_str("import androidx.compose.material3.*\n");
    combined.push_str("import androidx.compose.material.icons.Icons\n");
    combined.push_str("import androidx.navigation.compose.*\n");
    for (name, src) in examples() {
        combined.push_str(&codegen_example(name, src));
        combined.push('\n');
    }
    if std::process::Command::new("kotlinc")
        .arg("--version")
        .output()
        .is_err()
    {
        eprintln!("kotlinc not on PATH; skipping kotlinc compile check");
        return;
    }
    if !android_toolchain_present() {
        eprintln!(
            "no Android SDK on PATH (ANDROID_HOME/ANDROID_SDK_ROOT unset); \
             skipping kotlinc compile check (cannot resolve androidx.compose.* without it)"
        );
        return;
    }
    let dir = std::env::temp_dir();
    let path = dir.join("flux_codegen_kotlin_generated.kt");
    std::fs::write(&path, &combined).expect("write temp kotlin file");
    let status = std::process::Command::new("kotlinc")
        .arg("-Xallow-no-source-files")
        .arg(&path)
        .status()
        .expect("spawn kotlinc");
    assert!(
        status.success(),
        "kotlinc rejected generated Kotlin:\n{combined}"
    );
}

/// Regression test for the Button codegen defect: the `onClick` handler body and
/// the `text:` label must both reach the generated output. A prior build emitted
/// an empty `Button(onClick = { }) { Text("") }`, dropping the tap behaviour.
/// This locks the correct behaviour for both the named-arg form
/// (`Button(text:, onClick:)`) used by `examples/counter` and the trailing-block
/// form (`Button(...) { Text(...) }`).
#[test]
fn button_emits_handler_and_label() {
    let src = "component Tapped {\n  state taps: Int = 0\n  Button(text: \"Tap me\", onClick: fn() { taps = taps + 1 })\n}\n";
    let out = codegen_example("button_regression", src);
    assert!(
        out.contains("Button(onClick = { taps = (taps + 1) })"),
        "missing onClick handler body in:\n{out}"
    );
    assert!(
        out.contains("Text(\"Tap me\")"),
        "missing button label in:\n{out}"
    );

    // Trailing-block label form must also work.
    let src2 = "component Tapped2 {\n  state taps: Int = 0\n  Button(onClick: fn() { taps = taps + 1 }) { Text(\"Block\") }\n}\n";
    let out2 = codegen_example("button_regression_2", src2);
    assert!(
        out2.contains("Button(onClick = { taps = (taps + 1) })"),
        "missing onClick handler body in trailing form:\n{out2}"
    );
    assert!(
        out2.contains("Text(\"Block\")"),
        "missing trailing-block label in:\n{out2}"
    );
}
