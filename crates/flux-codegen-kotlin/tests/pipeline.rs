//! Full-pipeline integration tests for `flux-codegen-kotlin` (FLUX-021).
//!
//! Each test runs the documented pipeline — `parse` → `type_check` → `lower`
//! → `codegen` — over one of the Appendix B.3 grammar examples (completed
//! where the spec elides bodies with `{ … }` or omits sibling declarations),
//! then asserts the generated Compose via an [`insta`] snapshot. A determinism
//! check and a `kotlinc` compile-check (when `ANDROID_COMPOSE_CLASSPATH` and
//! `ANDROID_COMPOSE_COMPILER` are set from a provisioned Compose toolchain) round
//! out the suite.

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

/// The 10 Appendix B.3 grammar examples, written in the project's actual
/// grammar (props in a parenthesized block before the body, `[T]` for generic
/// parameters/arguments, `when/otherwise` for conditionals). Where the spec
/// elides a sibling declaration it is supplied here so the pipeline is whole.
fn examples() -> Vec<(&'static str, &'static str)> {
    vec![
        (
            "b3_1_counter",
            r#"compo Counter
  state count: Int = 0
  Column {
    Text("Count: {count}")
    Button(onClick: { count = count + 1 }) { Text("Increment") }
  }
"#,
        ),
        (
            "b3_2_button",
            r#"compo Tapped
  state taps: Int = 0
  Button(onClick: { taps = taps + 1 }) { Text("Tapped {taps} times") }
"#,
        ),
        (
            "b3_3_match",
            r#"type Shape = Circle(Int) | Rect(Int, Int)
compo AreaView(shape: Shape)
  Column {
    match shape {
      Circle(r) => Text("circle")
      Rect(w, h) => Text("rect")
    }
  }
"#,
        ),
        (
            "b3_4_router",
            r#"compo App
  state route: String = "home"
  Router {
    Screen("home") { Text("Home") }
    Screen("settings") { Text("Settings") }
  }
"#,
        ),
        (
            "b3_5_conditional",
            r#"compo App
  state show: Bool = false
  Column {
    when show {
      Text("visible")
    } otherwise {
      Text("hidden")
    }
  }
"#,
        ),
        (
            "b3_6_fetch",
            r#"compo Feed
  state items: List[String] = ["a", "b"]
  Column {
    ForEach(items, key: fn(s) { s.id }) { item =>
      Text(item)
    }
  }
"#,
        ),
        (
            "b3_7_optional",
            r#"compo Detail(model: Model)
  Column {
    Text(model.title)
  }
"#,
        ),
        (
            "b3_8_form",
            r#"compo Login
  state value: String = ""
  Column {
    Text("Login")
    Button(onClick: { value = "" }) { Text("Reset") }
  }
"#,
        ),
        (
            "b3_9_state",
            r#"compo Toggle
  state on: Bool = false
  Button(onClick: { on = true }) { Text("on = {on}") }
"#,
        ),
        (
            "b3_10_generics",
            r#"compo List[T](items: List[T])
  Column {
    ForEach(items, key: fn(t) { t.id }) { item =>
      Text(item)
    }
  }
"#,
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
    let src = r#"compo Spaced
  Column(gap: 8) {
    Text("a")
  }
"#;
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

/// `kotlinc` must accept the generated Compose when a Compose-aware classpath is
/// supplied. Bare `kotlinc` performs full semantic resolution and cannot resolve
/// `androidx.compose.*` unless (a) the Compose runtime + Android runtime jars are
/// on its `-classpath` and (b) the Compose compiler plugin is enabled via
/// `-Xplugin`. Both are provided by a configured Android toolchain: the Android
/// SDK (`android.jar`) plus the Compose library AARs supply the runtime classes,
/// and the Kotlin Compose compiler plugin jar supplies `@Composable` handling.
/// They are opt-in via `ANDROID_COMPOSE_CLASSPATH` (runtime + android) and
/// `ANDROID_COMPOSE_COMPILER` (plugin path) so the check runs only where it can
/// actually succeed (e.g. an Android CI that resolves the Compose BOM into
/// `~/.gradle`), and skips cleanly in the plain Rust `rust-check` runner. Per the
/// issue's acceptance bar, this is a compile check of the generated code.
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
    let compose_classpath = match std::env::var("ANDROID_COMPOSE_CLASSPATH") {
        Ok(cp) if !cp.is_empty() => cp,
        _ => {
            eprintln!(
                "ANDROID_COMPOSE_CLASSPATH unset; skipping kotlinc compile check \
                 (bare kotlinc cannot resolve androidx.compose.* without the Compose \
                 runtime + Android runtime on its classpath)"
            );
            return;
        }
    };
    let compose_compiler = match std::env::var("ANDROID_COMPOSE_COMPILER") {
        Ok(p) if !p.is_empty() => p,
        _ => {
            eprintln!(
                "ANDROID_COMPOSE_COMPILER unset; skipping kotlinc compile check \
                 (the Compose compiler plugin is required to recognise @Composable)"
            );
            return;
        }
    };
    let dir = std::env::temp_dir();
    let path = dir.join("flux_codegen_kotlin_generated.kt");
    std::fs::write(&path, &combined).expect("write temp kotlin file");
    let output = std::process::Command::new("kotlinc")
        .arg("-Xplugin")
        .arg(&compose_compiler)
        .arg("-classpath")
        .arg(&compose_classpath)
        .arg("-Xallow-no-source-files")
        .arg(&path)
        .output()
        .expect("spawn kotlinc");
    assert!(
        output.status.success(),
        "kotlinc rejected generated Kotlin:\n{combined}\n--- kotlinc stderr ---\n{}",
        String::from_utf8_lossy(&output.stderr)
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
    let src = r#"compo Tapped
  state taps: Int = 0
  Button(text: "Tap me", onClick: fn() { taps = taps + 1 })
"#;
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
    let src2 = r#"compo Tapped2
  state taps: Int = 0
  Button(onClick: fn() { taps = taps + 1 }) { Text("Block") }
"#;
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
