//! Full-pipeline integration tests for `flux-codegen-kotlin` (FLUX-021).
//!
//! Each test runs the documented pipeline — `parse` → `type_check` → `lower`
//! → `codegen` — over one of the Appendix B.3 grammar examples (completed
//! where the spec elides bodies with `{ … }` or omits sibling declarations),
//! then asserts the generated Compose via an [`insta`] snapshot. A determinism
//! check and a `kotlinc` compile-check (when `ANDROID_COMPOSE_CLASSPATH` and
//! `ANDROID_COMPOSE_COMPILER` are set from a provisioned Compose toolchain) round
//! out the suite.

use flux_codegen_core::Backend;
use flux_codegen_kotlin::Kotlin;
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
    Button(onPress: { count = count + 1 }) { Text("Increment") }
  }
"#,
        ),
        (
            "b3_2_button",
            r#"compo Tapped
  state taps: Int = 0
  Button(onPress: { taps = taps + 1 }) { Text("Tapped {taps} times") }
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
            // Round-16: `List[String]` elements are Hashable, no `.id`; index-key.
            r#"compo Feed
  state items: List[String] = ["a", "b"]
  Column {
    ForEach(items, key: fn(s, i) { i }) { item =>
      Text(item)
    }
  }
"#,
        ),
        (
            "b3_7_optional",
            // Round-16: no named-record declaration in Flux; primitive prop.
            r#"compo Detail(title: String)
  Column {
    Text(title)
  }
"#,
        ),
        (
            "b3_8_form",
            r#"compo Login
  state value: String = ""
  Column {
    Text("Login")
    Button(onPress: { value = "" }) { Text("Reset") }
  }
"#,
        ),
        (
            "b3_9_state",
            r#"compo Toggle
  state on: Bool = false
  Button(onPress: { on = true }) { Text("on = {on}") }
"#,
        ),
        (
            "b3_10_generics",
            // Round-16: unbounded generic + index-key ForEach + literal body.
            // (No `Hashable` bound — that is a Swift protocol, not a Kotlin
            // type. Kotlin generics default to `Any?` upper bound, and the
            // index-key `fn(t, i) { i }` produces a `LazyColumn.items(...)`
            // call that needs no `Hashable`.)
            r#"compo ListView[T](items: List[T])
  Column {
    ForEach(items, key: fn(t, i) { i }) { _ =>
      Text("item")
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
/// Per F1, `onClick` is an accepted alias for the canonical `onPress` verb.
#[test]
fn button_click_alias_emits_handler_body() {
    let src = "compo Tapped2\n  state taps: Int = 0\n  Button(text: \"Tap me\", onClick: fn() { taps = taps + 1 })\n\n";
    let out = codegen_example("button_click_alias", src);
    assert!(
        out.contains("Button(onClick = { taps = (taps + 1) })"),
        "onClick alias must resolve to the handler body: {out}"
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
/// Locates `kotlinc` on `PATH`. The compiler answers `-version` (single dash);
/// `--version` is an error on some distributions, so probing with the former
/// is what actually determines availability.
fn kotlinc_on_path() -> bool {
    std::process::Command::new("kotlinc")
        .arg("-version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Parses the Kotlin compiler version out of `kotlinc -version`'s stderr,
/// e.g. `info: kotlinc-jvm 2.4.20 (JRE 21.0.11+10-LTS)` → `Some("2.4.20")`.
///
/// The Compose compiler plugin is a *compiler extension*: it must be built
/// against the exact Kotlin release it plugs into. A 2.4.10 plugin loaded by
/// a 2.4.20 `kotlinc` throws `ClassCastException: ... BasicWritableSlice
/// cannot be cast to ... Key` during IR lowering (the plugin's classes and
/// the compiler's are in two classloaders with mismatched layouts). Picking
/// the matching cached plugin is what lets the check run against any local
/// Kotlin install, rather than requiring the toolchain to be pinned.
fn kotlinc_version() -> Option<String> {
    let out = std::process::Command::new("kotlinc")
        .arg("-version")
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&out.stderr);
    // The version token is the first `\d+\.\d+\.\d+` after `kotlinc`.
    for tok in text.split_whitespace() {
        if tok.chars().next().is_some_and(|c| c.is_ascii_digit())
            && tok.matches('.').count() >= 2
        {
            return Some(
                tok.chars()
                    .take_while(|c| c.is_ascii_digit() || *c == '.')
                    .collect(),
            );
        }
    }
    None
}

/// Locates the Compose compiler plugin jar and the runtime classpath the
/// kotlinc compile-check needs.
///
/// Order:
/// 1. `ANDROID_COMPOSE_CLASSPATH` / `ANDROID_COMPOSE_COMPILER` from the
///    environment, if both are set (what CI exports).
/// 2. Extract them from `~/.gradle/caches/modules-2/` — the same logic CI's
///    `rust-check.yml` step runs before invoking the test. Any developer who
///    has ever run `./gradlew` has the AARs cached, so the check runs without
///    any manual provisioning. The extraction lands in a per-run temp dir
///    that outlives the test process long enough for kotlinc to read it.
/// 3. `None` — skip (no kotlinc, or no cache to build a classpath from).
fn try_compose_classpath() -> Option<(String, String, Option<String>)> {
    // The plugin's K2 extension path loads `org.jetbrains.kotlin.com.intellij.*`
    // classes, which are **shaded into `kotlin-compiler-embeddable.jar`** and
    // not visible to a plugin loaded by the non-embeddable `kotlin-compiler.jar`
    // (homebrew's `kotlinc`). Passing the embeddable as a *second* `-Xplugin`
    // puts those classes on the plugin's classloader parent chain — this is
    // exactly what CI's `codegen-compile.yml` does with the two
    // `-Xplugin=$ANDROID_COMPOSE_COMPILER{,_EMBEDDABLE}` flags.
    if let (Ok(cp), Ok(cc)) = (
        std::env::var("ANDROID_COMPOSE_CLASSPATH"),
        std::env::var("ANDROID_COMPOSE_COMPILER"),
    ) {
        if !cp.is_empty() && !cc.is_empty() {
            let embeddable = std::env::var("ANDROID_COMPOSE_COMPILER_EMBEDDABLE")
                .ok()
                .filter(|s| !s.is_empty());
            return Some((cp, cc, embeddable));
        }
    }

    // Extract Compose runtime AARs from the gradle cache.
    let gradle_cache = dirs_gradle_cache()?;
    let android_jar = find_android_jar();
    // The Compose plugin must be built against the exact Kotlin release the
    // running `kotlinc` is, so pick by version, not by "newest cached".
    let kver = kotlinc_version()?;
    let compose_compiler = find_in_gradle_cache_for_version(
        &gradle_cache,
        "kotlin-compose-compiler-plugin-embeddable",
        &kver,
    )?;
    let compose_compiler_embeddable = find_in_gradle_cache_for_version(
        &gradle_cache,
        "kotlin-compiler-embeddable",
        &kver,
    );

    let tmp = std::env::temp_dir().join(format!("flux_kt_compose_{}", std::process::id()));
    let _ = std::fs::create_dir_all(&tmp);

    let mut cp_parts: Vec<String> = Vec::new();
    if let Some(jar) = android_jar {
        cp_parts.push(jar);
    }

    // Walk the cache and extract `classes.jar` from each Compose AAR. Bounded
    // by the cache size (typically tens of AARs), performed once per test run.
    //
    // The gradle cache lays group ids out with dots as directory separators
    // (`androidx.compose.runtime/runtime-android/1.7.6/...`), unlike the Maven
    // coordinate spelling `androidx/compose/runtime`. Match both, so the check
    // works regardless of which form a future gradle release uses.
    // Gradle uses **dots** as the group-id separator
    // (`androidx.navigation/navigation-compose-android/...`), not the Maven
    // slash form, and artifact directory names may carry `-android`/`-release`
    // suffixes. Match on the dotted prefix so both spellings resolve.
    // Match the WHOLE androidx.navigation. and androidx.activity. groups: the
    // Compose-layer AAR (`navigation-compose`) references `NavHostController`
    // and friends from the base `navigation-runtime` AAR, which is a
    // *transitive* dependency. Including every group member guarantees the
    // transitive classes are on the classpath.
    // Gradle stores a group id as a *single directory name with dots* (e.g.
    // `androidx.navigation/`, `androidx.activity/`, and the sub-groups
    // `androidx.compose.runtime/`, `androidx.compose.foundation/`). For a
    // two-segment group the directory is `androidx.navigation` followed by `/`;
    // for a three-segment group it is `androidx.compose.runtime` followed by
    // `/`. The prefixes below use a trailing `/` for the short groups and a
    // trailing `.` for the compose sub-groups (which continue with `.runtime`,
    // `.foundation`, etc.).
    let wanted_aar_prefixes = [
        "/androidx.compose.",
        "/androidx.navigation/",
        "/androidx.activity/",
    ];
    // Plain jars (not AARs) that the generated Compose imports. The coroutines
    // runtime ships as a normal JAR, so it needs a separate collection pass.
    let wanted_jar_prefixes = [
        "/org.jetbrains.kotlinx/kotlinx-coroutines-core-jvm/",
        "/org.jetbrains.kotlinx/kotlinx-coroutines-android/",
    ];
    let mut visited: usize = 0;
    let mut extracted: usize = 0;
    for entry in walk_aars(&gradle_cache) {
        let path_str = entry.to_string_lossy();
        if !wanted_aar_prefixes.iter().any(|p| path_str.contains(p)) {
            continue;
        }
        visited += 1;
        let stem = entry
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("compose");
        let out_jar = tmp.join(format!("{stem}.jar"));
        if out_jar.exists() {
            cp_parts.push(out_jar.to_string_lossy().into_owned());
            extracted += 1;
            continue;
        }
        // `unzip -p <aar> classes.jar > <out>` extracts the embedded jar
        // without needing a full unzip to a scratch dir.
        let status = std::process::Command::new("unzip")
            .arg("-p")
            .arg(&entry)
            .arg("classes.jar")
            .stdout(std::fs::File::create(&out_jar).ok()?)
            .status()
            .ok()?;
        if status.success() && out_jar.metadata().map(|m| m.len() > 0).unwrap_or(false) {
            cp_parts.push(out_jar.to_string_lossy().into_owned());
            extracted += 1;
        }
    }
    // Plain jars (coroutines). Prefer the newest version of each artifact —
    // mixing coroutines 1.6 and 1.11 on one classpath is what triggered the
    // earlier "return type mismatch" (a 1.x-API class shadowing the 1.11 one).
    for prefix in &wanted_jar_prefixes {
        let mut best: Option<std::path::PathBuf> = None;
        for entry in walk_files(&gradle_cache) {
            let path_str = entry.to_string_lossy();
            if !path_str.contains(*prefix) {
                continue;
            }
            let Some(name) = entry.file_name().and_then(|s| s.to_str()) else {
                continue;
            };
            if !name.ends_with(".jar") || name.ends_with("-sources.jar") {
                continue;
            }
            // Prefer the lexicographically-largest path (contains the version
            // dir, so this picks the newest by string compare, which works for
            // the dot-separated numeric versions gradle caches).
            if best.as_ref().map(|b| entry > *b).unwrap_or(true) {
                best = Some(entry);
            }
        }
        if let Some(jar) = best {
            cp_parts.push(jar.to_string_lossy().into_owned());
        }
    }

    eprintln!(
        "compose cache scan: {} AARs matched, {} extracted, cp has {} entries",
        visited,
        extracted,
        cp_parts.len(),
    );
    if cp_parts.is_empty() {
        return None;
    }
    Some((
        cp_parts.join(":"),
        compose_compiler,
        compose_compiler_embeddable,
    ))
}

/// `$HOME/.gradle/caches/modules-2/files-2.1` if present.
fn dirs_gradle_cache() -> Option<std::path::PathBuf> {
    let home = std::env::var_os("HOME")?;
    let p = std::path::PathBuf::from(home)
        .join(".gradle/caches/modules-2/files-2.1");
    if p.is_dir() {
        Some(p)
    } else {
        None
    }
}

/// The Android platform `android.jar` (either the env-provided SDK or the
/// default macOS location). `None` if not present.
fn find_android_jar() -> Option<String> {
    let mut roots: Vec<std::path::PathBuf> = Vec::new();
    if let Some(home) = std::env::var_os("HOME") {
        roots.push(std::path::PathBuf::from(home).join("Library/Android/sdk"));
    }
    if let Ok(sdk) = std::env::var("ANDROID_HOME") {
        roots.push(std::path::PathBuf::from(sdk));
    }
    if let Ok(sdk) = std::env::var("ANDROID_SDK_ROOT") {
        roots.push(std::path::PathBuf::from(sdk));
    }
    for root in roots {
        let platforms = root.join("platforms");
        if let Ok(rd) = std::fs::read_dir(&platforms) {
            let mut versions: Vec<_> = rd
                .filter_map(Result::ok)
                .filter_map(|e| e.file_name().into_string().ok())
                .collect();
            versions.sort();
            if let Some(latest) = versions.last() {
                let jar = platforms.join(latest).join("android.jar");
                if jar.is_file() {
                    return Some(jar.to_string_lossy().into_owned());
                }
            }
        }
    }
    None
}

/// Finds a jar whose filename matches `<prefix>-<version>.jar` for an exact
/// `version`. Returns `None` when no cached artifact carries that version —
/// the caller skips the compile check rather than loading a mismatched plugin
/// (which fails with a classloader error, not a codegen error).
fn find_in_gradle_cache_for_version(
    cache: &std::path::Path,
    prefix: &str,
    version: &str,
) -> Option<String> {
    let target = format!("{prefix}-{version}.jar");
    for entry in walk_files(cache) {
        if let Some(name) = entry.file_name().and_then(|s| s.to_str()) {
            if name == target {
                return Some(entry.to_string_lossy().into_owned());
            }
        }
    }
    None
}

/// Yields every `.aar` under `root` (bounded; gradle cache trees are shallow).
fn walk_aars(root: &std::path::Path) -> Vec<std::path::PathBuf> {
    walk_files(root)
        .into_iter()
        .filter(|p| p.extension().map(|e| e == "aar").unwrap_or(false))
        .collect()
}

/// Recursively yields every file under `root`. Depth-bounded to keep the walk
/// cheap in a test.
fn walk_files(root: &std::path::Path) -> Vec<std::path::PathBuf> {
    fn recurse(dir: &std::path::Path, depth: usize, out: &mut Vec<std::path::PathBuf>) {
        if depth > 12 {
            return;
        }
        let Ok(rd) = std::fs::read_dir(dir) else { return };
        for entry in rd.filter_map(Result::ok) {
            let p = entry.path();
            if p.is_dir() {
                recurse(&p, depth + 1, out);
            } else {
                out.push(p);
            }
        }
    }
    let mut out = Vec::new();
    recurse(root, 0, &mut out);
    out
}

#[test]
fn generated_kotlin_parses() {
    if !kotlinc_on_path() {
        eprintln!("kotlinc not on PATH; skipping kotlinc compile check");
        return;
    }
    let Some((compose_classpath, compose_compiler, compose_compiler_embeddable)) =
        try_compose_classpath()
    else {
        eprintln!(
            "no Compose toolchain (ANDROID_COMPOSE_CLASSPATH unset and no AARs in \
             ~/.gradle/caches); skipping kotlinc compile check"
        );
        return;
    };
    // Round-16: type-check **each example in its own file** so a per-example
    // bug is not masked by another example's identifier collision (two
    // examples declare `object FluxTheme`; a user component named `List`
    // collides with `kotlin.collections.List`). The previous version
    // concatenated every example into ONE file and produced noise that hid
    // the real codegen defects (audit §8).
    let dir = std::env::temp_dir();
    let mut failures: Vec<(String, String)> = Vec::new();
    for (name, src) in examples() {
        // The generated code is a complete `.kt` file: it already emits its
        // own `package` + import block. Prepending anything here duplicates
        // the imports and pushes the `package` declaration out of position
        // (`imports are only allowed in the beginning of file`).
        let full = codegen_example(name, src);
        let path = dir.join(format!("flux_codegen_kotlin_{name}.kt"));
        std::fs::write(&path, &full).expect("write");
        let mut cmd = std::process::Command::new("kotlinc");
        cmd.arg(format!("-Xplugin={compose_compiler}"));
        // Pass the embeddable Kotlin compiler as a second `-Xplugin` so the
        // plugin classloader can resolve the shaded `org.jetbrains.kotlin.com.intellij`
        // classes (see the note in `try_compose_classpath`). Mirrors CI's
        // `-Xplugin="$ANDROID_COMPOSE_COMPILER" -Xplugin="$ANDROID_COMPOSE_COMPILER_EMBEDDABLE"`.
        if let Some(ref embeddable) = compose_compiler_embeddable {
            cmd.arg(format!("-Xplugin={embeddable}"));
        }
        cmd.arg("-classpath")
            .arg(&compose_classpath)
            .arg("-jvm-target")
            .arg("11")
            .arg("-d")
            .arg(dir.join(format!("flux_out_{name}")))
            .arg(&path);
        let out = cmd.output().expect("spawn kotlinc");
        if !out.status.success() {
            let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
            if stderr.contains("NoClassDefFoundError")
                || stderr.contains("ClassNotFoundException")
            {
                eprintln!("toolchain classloader issue; skipping:\n{stderr}");
                return;
            }
            failures.push((name.to_owned(), stderr));
        }
        let _ = std::fs::remove_file(&path);
    }
    if !failures.is_empty() {
        let mut msg = String::from("kotlinc rejected generated Kotlin:\n");
        for (name, err) in &failures {
            msg.push_str(&format!("--- example `{name}` ---\n{err}\n"));
        }
        panic!("{msg}");
    }
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
  Button(text: "Tap me", onPress: fn() { taps = taps + 1 })
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
  Button(onPress: fn() { taps = taps + 1 }) { Text("Block") }
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

#[test]
fn flux_037_layout_primitives_codegen() {
    // FLUX-037: Stack / Grid / Spacer / SafeArea must lower to their native
    // views on the Kotlin backend (PRD-N layout family).
    let src = r#"compo Layout
  Stack {
    Text("a")
  }
  Grid {
    Text("b")
  }
  Spacer()
  SafeArea {
    Text("c")
  }
"#;
    let out = codegen_example("layout_primitives", src);
    assert!(out.contains("Box {"), "Stack missing Box mapping:\n{out}");
    assert!(
        out.contains("LazyVerticalGrid {"),
        "Grid missing LazyVerticalGrid mapping:\n{out}"
    );
    assert!(
        out.contains("Spacer(\"\")"),
        "Spacer missing mapping:\n{out}"
    );
    assert!(
        out.contains("Scaffold {"),
        "SafeArea missing Scaffold mapping:\n{out}"
    );
}

#[test]
fn flux_039_image_primitive_codegen() {
    // FLUX-039: `Image(src)` must lower to the native image binding on the
    // Kotlin backend. Caching is a host-side concern (Coil/URLCache); the
    // primitive only carries the `src` prop.
    let src = r#"compo Pic
  Image(url: "assets/logo.png")
"#;
    let out = codegen_example("image_primitive", src);
    assert!(
        out.contains("painterResource("),
        "Image missing painterResource mapping:\n{out}"
    );
}

#[test]
fn flux_040_form_primitives_codegen() {
    // FLUX-040: each form primitive lowers to its native control on Kotlin,
    // carrying a `value`/`onChange` signal contract (like `TextField`).
    let src = r#"compo Form
  state on: Bool = false
  state n: Int = 0
  state sel: Int = 0
  state d: Int = 0
  state t: String = ""
  Switch(value: on, onChange: fn() { on = on })
  Checkbox(value: on, onChange: fn() { on = on })
  Slider(value: n, onChange: fn() { n = n })
  Picker(value: sel, onChange: fn() { sel = sel })
  DatePicker(value: d, onChange: fn() { d = d })
  TextArea(value: t, onChange: fn() { t = t })
"#;
    let out = codegen_example("form_primitives", src);
    assert!(out.contains("Switch("), "Switch missing:\n{out}");
    assert!(out.contains("Checkbox("), "Checkbox missing:\n{out}");
    assert!(out.contains("Slider("), "Slider missing:\n{out}");
    assert!(out.contains("DropdownMenu("), "Picker missing:\n{out}");
    assert!(
        out.contains("DatePickerDialog("),
        "DatePicker missing:\n{out}"
    );
    assert!(out.contains("TextField("), "TextArea missing:\n{out}");
}

#[test]
fn flux_041_gesture_primitive_codegen() {
    // FLUX-041: a `Gesture` wrapper lowers to a native container carrying the
    // gesture recognizer on Kotlin (`Box`). The native recognizer attach is
    // host-side; this pins the structural mapping + the onGesture callback.
    let src = r#"compo G
  state fired: Bool = false
  Gesture(kind: "longPress", onGesture: fn() { fired = fired }) {
    Text("tap")
  }
"#;
    let out = codegen_example("gesture_primitive", src);
    assert!(
        out.contains("Box {"),
        "Gesture missing Box mapping:\\n{out}"
    );
    assert!(
        out.contains("Text(\"tap\")"),
        "Gesture child not emitted:\\n{out}"
    );
}

#[test]
fn flux_038_overlay_container_codegen() {
    // FLUX-038: `Modal`/`Sheet`/`Dialog` lower to their host-native overlay
    // surface on the Kotlin backend, each carrying its `content` children. The
    // `onDismiss` handler is the presentation contract the host maps to the
    // native dismiss action; here we pin the structural mapping + child emission.
    let src = r#"compo Overlays
  state open: Bool = false
  Sheet(onDismiss: fn() { open = false }) {
    Text("sheet body")
  }
  Dialog(onDismiss: fn() { open = false }) {
    Text("dialog body")
  }
  Modal(onDismiss: fn() { open = false }) {
    Text("modal body")
  }
"#;
    let out = codegen_example("overlay_containers", src);
    assert!(
        out.contains("ModalBottomSheet {"),
        "Sheet missing ModalBottomSheet mapping:\\n{out}"
    );
    assert!(
        out.contains("AlertDialog {"),
        "Dialog missing AlertDialog mapping:\\n{out}"
    );
    assert!(
        out.contains("Dialog {"),
        "Modal missing Dialog mapping:\\n{out}"
    );
    // Children must be carried through on every overlay surface.
    assert!(
        out.contains("Text(\"sheet body\")"),
        "Sheet child dropped:\\n{out}"
    );
    assert!(
        out.contains("Text(\"dialog body\")"),
        "Dialog child dropped:\\n{out}"
    );
    assert!(
        out.contains("Text(\"modal body\")"),
        "Modal child dropped:\\n{out}"
    );
}

/// FLUX-042: an `Animate` primitive emits a Compose-native animation, not
/// the Swift `withAnimation(`. Kotlin uses `animateFloatAsState` declared as a
/// state cell. The signal/curve is data the host consumes; this pins the
/// generated spelling.
#[test]
fn flux_042_animate_codegen() {
    let src = "compo Animated\n  state value: Int = 0\n  Animate(curve: \"easeInOut\") {\n    Text(\"hello\")\n  }\n\n";
    let out = codegen_example("flux_042_animate", src);
    assert!(
        !out.contains("withAnimation("),
        "Kotlin must not emit withAnimation: {out}"
    );
    assert!(
        out.contains("animateFloatAsState"),
        "Animate must emit animateFloatAsState on Kotlin: {out}"
    );
    assert!(
        out.contains("Text(\"hello\")"),
        "Animate child dropped from the animation subtree: {out}"
    );
}

/// T-402: the Kotlin prelude must include every symbol the generated bodies
/// reference so the output is self-contained. Missing imports from the prelude
/// produce opaque "unresolved reference" errors in the provisioned toolchain.
#[test]
fn kotlin_prelude_contains_all_referenced_imports() {
    let out = codegen_example("b3_1_counter", examples()[0].1);
    let imports = extract_imports(&out);
    let prelude = Kotlin::prelude();
    assert!(
        prelude.contains("import androidx.compose.animation.core.*"),
        "prelude missing animation.core: {prelude}"
    );
    assert!(
        prelude.contains("import kotlinx.coroutines.GlobalScope"),
        "prelude missing GlobalScope: {prelude}"
    );
    assert!(
        prelude.contains("import androidx.compose.foundation.shape.RoundedCornerShape"),
        "prelude missing RoundedCornerShape: {prelude}"
    );
    // Every symbol referenced in generated bodies must appear in the prelude.
    let referenced = referenced_symbols(&out);
    for sym in referenced {
        let found = imports.contains(&sym) || prelude.contains(&sym);
        assert!(
            found,
            "symbol '{sym}' referenced in body but absent from prelude + imports:\n{prelude}\n--- body ---\n{out}"
        );
    }
}

/// Extracts import lines from generated Kotlin output (the prelude already
/// contains them, but the test checks they survive into the emitted file).
fn extract_imports(generated: &str) -> Vec<String> {
    generated
        .lines()
        .filter(|l| l.starts_with("import "))
        .map(|l| l.trim_start_matches("import ").to_string())
        .collect()
}

/// Heuristic: collects capitalised identifiers that look like referenced
/// Compose/AndroidX symbols (Class.function calls, type names) from the body.
fn referenced_symbols(generated: &str) -> Vec<String> {
    let mut syms = Vec::new();
    for line in generated.lines() {
        // Pattern: CapitalizedName( → capture CapitalizedName
        let mut rest = line.trim();
        rest = rest.trim_start_matches("val ").trim_start_matches("var ");
        if let Some(end) = rest.find('(') {
            let name = &rest[..end];
            if name
                .chars()
                .next()
                .map(|c| c.is_uppercase())
                .unwrap_or(false)
                && name.len() > 1
                && name
                    .chars()
                    .all(|c| c.is_alphanumeric() || c == '.' || c == '_')
                && !name.starts_with("FluxComponent")
                && !name.starts_with("FluxTheme")
            {
                syms.push(name.to_string());
            }
        }
    }
    // Deduplicate
    let mut seen = std::collections::HashSet::new();
    syms.retain(|s| seen.insert(s.clone()));
    syms
}

/// FLUX-043: the design-token theme extension must be emitted once and must
/// contain every declared token name on the Kotlin backend.
#[test]
fn flux_043_theme_extension_codegen() {
    let src = "compo UsesTheme\n  Theme {\n    Text(\"themed\")\n  }\n\n";
    let out = codegen_example("flux_043_theme", src);
    assert!(
        out.contains("object FluxTheme {"),
        "missing native theme extension on Kotlin backend:\n{out}"
    );
    for token in flux_codegen_core::primitives::theme_tokens() {
        assert!(
            out.contains(token.name),
            "theme token `{}` missing from generated Kotlin theme extension:\n{out}",
            token.name
        );
    }
}
