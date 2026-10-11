//! Parity acceptance tests over all ten Appendix B.3 examples.
//!
//! Each test compiles the example through the full pipeline, reduces the dev (AST)
//! and both release (Swift / Kotlin codegen) paths to the structural [`ViewNode`]
//! model, asserts equivalence, and snapshots the relation with `insta`.
//!
//! A lowering failure is a hard error (issue 5): every B.3 example must ship with
//! a lowering pass, so `check_parity` returns `Err` rather than a silently
//! okayed "unsupported" result. The harness therefore proves parity for *every*
//! example, and CI fails loudly if any example cannot be fully lowered.

use flux_parity::{ParityReport, all_examples, check_parity};

/// Asserts the documented parity contract for one example and snapshots the
/// report via `insta`.
///
/// Panics (via `insta::assert_snapshot!` / `assert!`) if the snapshot diverges
/// from the committed baseline, or if the release backends could not be
/// exercised for the example — that is the intended failure mode for a parity
/// regression (and for a missing lowering pass, issue 5).
fn assert_parity(name: &str, source: &str, file_id: u32) {
    let report: ParityReport =
        check_parity(source, file_id).expect("example parses, type-checks and lowers");
    if std::env::var("PARITY_DEBUG").is_ok() {
        eprintln!(
            "=== MISMATCH {name} ===\nDEV={:#?}\nSW={:#?}\nKT={:#?}",
            report.dev, report.swift, report.kotlin
        );
    }
    assert!(
        report.is_equivalent(),
        "parity divergence for {name}: dev vs swift vs kotlin trees differ"
    );
    let serialized = format!(
        "verdict: {}\n\n dev    == {:#?}\nswift  == {:#?}\nkotlin == {:#?}\n",
        report.verdict(),
        report.dev,
        report.swift,
        report.kotlin
    );
    insta::assert_snapshot!(format!("parity_{name}"), serialized);
}

#[test]
fn b31_simple_parity() {
    let (name, src) = all_examples()[0];
    assert_parity(name, src, 31);
}

#[test]
fn b32_generic_parity() {
    let (name, src) = all_examples()[1];
    assert_parity(name, src, 32);
}

#[test]
fn b33_adt_parity() {
    let (name, src) = all_examples()[2];
    assert_parity(name, src, 33);
}

#[test]
fn b34_lifecycle_parity() {
    let (name, src) = all_examples()[3];
    assert_parity(name, src, 34);
}

#[test]
fn b35_navigation_parity() {
    let (name, src) = all_examples()[4];
    assert_parity(name, src, 35);
}

#[test]
fn b36_async_parity() {
    let (name, src) = all_examples()[5];
    assert_parity(name, src, 36);
}

#[test]
fn b37_pure_parity() {
    let (name, src) = all_examples()[6];
    assert_parity(name, src, 37);
}

#[test]
fn b38_platform_parity() {
    let (name, src) = all_examples()[7];
    assert_parity(name, src, 38);
}

#[test]
fn b39_capability_parity() {
    let (name, src) = all_examples()[8];
    assert_parity(name, src, 39);
}

#[test]
fn b310_refs_parity() {
    let (name, src) = all_examples()[9];
    assert_parity(name, src, 310);
}

/// The canonical `examples/router` app: a real capability-driven navigation
/// stack. Unlike the positional `Screen("home")` form in the B.3.5 parity
/// fixture (whose `route` is reconstructed syntactically by the reducer), this
/// example uses the named `Screen(route: "home")` form so the lowered IR carries
/// an actual `route` prop keyed by `FNV-1a("route")` — exactly the prop index
/// the iOS / Android host reconcilers read to match the active screen
/// (ADR-0045). This test fails loudly if the compiler ever stops emitting that
/// prop, which would silently break navigation on device.
#[test]
fn router_example_emits_route_prop_and_navigate_call() {
    let source = include_str!("../../../examples/router/main.flux");
    let (_ast, _typed, lowered) =
        flux_parity::compile(source, 900).expect("router example compiles");

    // The lowered IR represents `Router`/`Screen` as ordinary `Component` nodes
    // (the host matches them by resolved component name → adapter kind), so we
    // locate the Router node by its interned component name, not by `NodeKind`.
    let name_of = |lowered: &flux_ir::LoweredIr, id: flux_syntax::NodeId| -> String {
        let cid = lowered.arena.get(id).expect("node").component_id();
        lowered
            .component_names
            .iter()
            .find(|(c, _)| *c == cid)
            .map(|(_, name)| name.clone())
            .expect("component name interned")
    };

    let router_id = lowered
        .arena
        .all_ids()
        .find(|id| name_of(&lowered, *id) == "Router")
        .expect("lowered IR must contain a Router node");

    let route_prop_index = flux_ir::lower::prop_index_for_name("route");

    let mut screen_routes: Vec<String> = Vec::new();
    for child in lowered
        .arena
        .get(router_id)
        .expect("router node")
        .children()
    {
        if let flux_syntax::Child::Node(screen_id) = child {
            let screen = lowered.arena.get(screen_id).expect("screen node");
            assert_eq!(
                name_of(&lowered, screen_id),
                "Screen",
                "child of Router must be a Screen"
            );
            // The host reconcilers read the Screen's `route` prop keyed by
            // `FNV-1a("route")` (Android `ROUTE_PROP_INDEX`, iOS
            // `routePropIndex`), so the lowered IR MUST carry exactly that prop.
            let route = screen
                .props()
                .fields()
                .iter()
                .find(|(idx, _)| *idx == route_prop_index)
                .map(|(_, value)| value.clone())
                .expect("Screen must carry a `route` prop");
            let flux_syntax::Value::Str(route_id) = route else {
                panic!("route prop must be a string id");
            };
            screen_routes.push(
                lowered
                    .arena
                    .string_table()
                    .resolve(route_id)
                    .expect("route literal")
                    .to_owned(),
            );
        }
    }
    assert_eq!(
        screen_routes,
        vec!["home".to_owned(), "settings".to_owned()],
        "router must expose home + settings routes via `route` props"
    );

    // The two `onClick` handlers must each lower to `CALL_CAP(3,1)`.
    let mut navigate_calls = 0usize;
    for handler in lowered.closures.values() {
        if handler
            .bytecode
            .contains(&flux_syntax::opcode::raw::CALL_CAP)
        {
            let code = &handler.bytecode;
            let pos = code
                .iter()
                .position(|b| *b == flux_syntax::opcode::raw::CALL_CAP)
                .unwrap();
            let cap_id = u32::from_le_bytes(code[pos + 2..pos + 6].try_into().unwrap());
            let method_id = u16::from_le_bytes(code[pos + 6..pos + 8].try_into().unwrap());
            assert_eq!(
                (cap_id, method_id),
                (3, 1),
                "RouterNav.navigate → CALL_CAP(3,1)"
            );
            navigate_calls += 1;
        }
    }
    assert_eq!(
        navigate_calls, 2,
        "both buttons must call RouterNav.navigate"
    );
}

/// LANE-B (device-only blind spot): a POSITIONAL `Screen("home")` arg is the
/// documented trap. The dev-path reducer reconstructs `route` syntactically
/// (`flux_parity::reduce::screen_route_from_args`), so dev/release parity stays
/// GREEN, but the *lowered* IR carries the route at `PropIdx(0)` — NOT at
/// `FNV-1a("route")` — so the iOS / Android host reconcilers' `route` lookup
/// finds nothing and navigation silently never swaps on device (ADR-0045).
///
/// This gate inspects the ACTUAL lowered prop index (the true on-device
/// contract) and pins the trap so it can never regress silently: a positional
/// `Screen` must NOT emit a `route` prop keyed by `prop_index_for_name("route")`.
/// If this ever becomes non-zero, either the compiler started lowering
/// positional args to the named prop (closing the blind spot — the intended
/// fix) or a regression hid the trap. The correct fix for authors is the NAMED
/// `Screen(route:)` form (see `router_example_emits_route_prop_and_navigate_call`).
#[test]
fn positional_screen_does_not_emit_route_prop_at_fnv_index() {
    // Mirror the B.3.5 navigation fixture surface (brace syntax), but with a
    // POSITIONAL `Screen("home")` arg instead of a named `route:` prop. This is
    // exactly the device-only trap: the reducer reconstructs `route` from the
    // positional arg syntactically, so dev/release parity stays GREEN, but the
    // *lowered* IR carries the route at `PropIdx(0)` — NOT at `FNV-1a("route")`.
    let source = r#"compo App
  Router {
    Screen("home") {
      Column(gap: 16) {
        Text("Home")
        Button(text: "Go to Settings", onPress: {
          RouterNav.navigate("settings")
        })
      }
    }
    Screen("settings") {
      Column(gap: 16) {
        Text("Settings")
        Button(text: "Go to Home", onPress: {
          RouterNav.navigate("home")
        })
      }
    }
  }
"#;
    let (_ast, _typed, lowered) =
        flux_parity::compile(source, 901).expect("positional router example compiles");

    // Mirrors the node-locating helper above; the lowered IR represents
    // `Router`/`Screen` as ordinary `Component` nodes matched by name.
    let name_of = |lowered: &flux_ir::LoweredIr, id: flux_syntax::NodeId| -> String {
        let cid = lowered.arena.get(id).expect("node").component_id();
        lowered
            .component_names
            .iter()
            .find(|(c, _)| *c == cid)
            .map(|(_, name)| name.clone())
            .expect("component name interned")
    };

    let route_prop_index = flux_ir::lower::prop_index_for_name("route");

    let router_id = lowered
        .arena
        .all_ids()
        .find(|id| name_of(&lowered, *id) == "Router")
        .expect("lowered IR must contain a Router node");

    let mut positional_route_props = 0usize;
    for child in lowered
        .arena
        .get(router_id)
        .expect("router node")
        .children()
    {
        if let flux_syntax::Child::Node(screen_id) = child {
            let screen = lowered.arena.get(screen_id).expect("screen node");
            assert_eq!(
                name_of(&lowered, screen_id),
                "Screen",
                "child of Router must be a Screen"
            );
            // The host reconcilers read the `route` prop at `FNV-1a("route")`
            // (Android `ROUTE_PROP_INDEX`, iOS `routePropIndex`). A positional
            // `Screen("home")` must NOT emit a prop under that index — that is
            // the device-only trap this gate makes visible in Rust.
            if screen
                .props()
                .fields()
                .iter()
                .any(|(idx, _)| *idx == route_prop_index)
            {
                positional_route_props += 1;
            }
        }
    }

    assert_eq!(
        positional_route_props, 0,
        "POSITIONAL Screen must NOT carry a `route` prop at FNV-1a(\"route\") — the device-only blind spot"
    );
}

/// FLUX-038: a `Modal` (overlay container) open must pin the dev/release node
/// mapping on both backends. This is the issue's required parity trace test — it
/// proves the `Modal`/`Sheet`/`Dialog` overlay primitives lower and codegen to
/// structurally identical view trees across the dev (reduced AST) path and both
/// release (SwiftUI / Compose) backends, with their `content` children carried
/// through on every path.
#[test]
fn flux_038_modal_open_pins_dev_release_mapping() {
    let source = r#"compo SettingsScreen
  state open: Bool = false
  Column(gap: 16) {
    Button(text: "Open", onPress: { open = true })
    Modal(onDismiss: fn() { open = false }) {
      Column(gap: 8) {
        Text("Modal title")
        Text("Modal body")
      }
    }
    Sheet(onDismiss: fn() { open = false }) {
      Text("Sheet body")
    }
    Dialog(onDismiss: fn() { open = false }) {
      Text("Dialog body")
    }
  }
"#;
    let report = check_parity(source, 380)
        .expect("Modal/Sheet/Dialog example parses, type-checks and lowers");
    assert!(
        report.is_equivalent(),
        "parity divergence for FLUX-038 Modal/Sheet/Dialog: dev vs swift vs kotlin trees differ"
    );
    let serialized = format!(
        "verdict: {}\n\n dev    == {:#?}\nswift  == {:#?}\nkotlin == {:#?}\n",
        report.verdict(),
        report.dev,
        report.swift,
        report.kotlin
    );
    insta::assert_snapshot!("parity_flux_038_modal_open", serialized);
}

/// FLUX-037: the layout primitives (`Stack` / `Grid` / `Spacer` / `SafeArea`)
/// must lower and codegen to structurally identical view trees across the dev
/// (reduced AST) path and both release (SwiftUI / Compose) backends, with their
/// children carried through on every path. This is the issue's required parity
/// trace test — it proves the four layout primitives are first-class on all
/// three render paths, not just names registered in the codegen table.
#[test]
fn flux_037_layout_primitives_pin_dev_release_mapping() {
    let source = r#"compo LayoutShowcase
  Column(gap: 16) {
    Stack(gap: 8) {
      Text("bottom")
      Text("top")
    }
    Grid(columns: 2, gap: 4) {
      Text("a")
      Text("b")
      Text("c")
      Text("d")
    }
    SafeArea(edges: "top") {
      Text("inset content")
    }
    Spacer(flex: 1.0)
    Text("after spacer")
  }
"#;
    let report =
        check_parity(source, 370).expect("FLUX-037 example parses, type-checks and lowers");
    eprintln!(
        "=== FLUX-037 DEV ===\n{:#?}\n=== SWIFT ===\n{:#?}\n=== KOTLIN ===\n{:#?}",
        report.dev, report.swift, report.kotlin
    );
    assert!(
        report.is_equivalent(),
        "parity divergence for FLUX-037 layout primitives: dev vs swift vs kotlin trees differ"
    );
    let serialized = format!(
        "verdict: {}\n\n dev    == {:#?}\nswift  == {:#?}\nkotlin == {:#?}\n",
        report.verdict(),
        report.dev,
        report.swift,
        report.kotlin
    );
    insta::assert_snapshot!("parity_flux_037_layout", serialized);
}

/// FLUX-042: the signal-graph `Animate` wrapper must lower and codegen to
/// structurally identical view trees across the dev (reduced AST) path and both
/// release (SwiftUI `withAnimation` / Compose `withAnimation`) backends, with its
/// child subtree carried through on every path. The release backends emit the
/// host-native `withAnimation(spec) { … }` call (the curve is data the host
/// consumes); the parity reducer folds both backends' `withAnimation` back to the
/// common `Animate` surface, so this test pins dev/release equivalence for the
/// animation primitive.
#[test]
fn flux_042_animate_wrapper_pins_dev_release_mapping() {
    let source = r#"compo AnimatedCard
  state open: Bool = false
  Column(gap: 16) {
    Button(text: "Toggle", onPress: { open = !open })
    Animate(signal: open, curve: "spring", duration: 0.3) {
      Text("toggle content")
    }
  }
"#;
    let report =
        check_parity(source, 420).expect("FLUX-042 example parses, type-checks and lowers");
    assert!(
        report.is_equivalent(),
        "parity divergence for FLUX-042 Animate: dev vs swift vs kotlin trees differ"
    );
    let serialized = format!(
        "verdict: {}\n\n dev    == {:#?}\nswift  == {:#?}\nkotlin == {:#?}\n",
        report.verdict(),
        report.dev,
        report.swift,
        report.kotlin
    );
    insta::assert_snapshot!("parity_flux_042_animate", serialized);
}

/// FLUX-056: the `ScrollView` primitive (PRD-N) must lower and codegen to
/// structurally identical view trees across the dev (reduced AST) path and both
/// release (SwiftUI `ScrollView` / Compose `ScrollView`) backends, with its
/// children carried through on every path. The `orientation` prop selects the
/// scroll axis and is recorded as data the host consumes; this test pins the
/// dev/release equivalence (vertical default + explicit horizontal) for the
/// scroll primitive.
#[test]
fn flux_056_scrollview_pins_dev_release_mapping() {
    let source = r#"compo ScrollShowcase
  Column(gap: 16) {
    ScrollView {
      Text("scrolled item 1")
      Text("scrolled item 2")
    }
    ScrollView(orientation: "horizontal") {
      Text("h item 1")
      Text("h item 2")
    }
  }
"#;
    let report =
        check_parity(source, 560).expect("FLUX-056 example parses, type-checks and lowers");
    assert!(
        report.is_equivalent(),
        "parity divergence for FLUX-056 ScrollView: dev vs swift vs kotlin trees differ"
    );
    let serialized = format!(
        "verdict: {}\n\n dev    == {:#?}\nswift  == {:#?}\nkotlin == {:#?}\n",
        report.verdict(),
        report.dev,
        report.swift,
        report.kotlin
    );
    insta::assert_snapshot!("parity_flux_056_scrollview", serialized);
}

/// FLUX-F22 regression: a Flux program that sets one of the historically
/// tracked container/leaf props (`alignment`/`color`/`label`) must not
/// diverge across the three reduced trees. Before round 86, the dev reducer
/// kept these as props while the codegen backends silently dropped them, so
/// `check_parity` reported `is_equivalent() == false`. The fix emptied the
/// dev reducer's RECOGNISED list (restoring parity) and filed FLUX-F22 for
/// the codegen-emission gap. This test locks in the parity invariant.
#[test]
fn container_props_do_not_break_parity() {
    let src = "compo C\n  Column(alignment: \"center\", color: \"red\", label: \"hi\") { Text(\"x\") }";
    let report = check_parity(src, 9980).expect("compiles");
    assert!(
        report.is_equivalent(),
        "dev and release must agree on container props: dev={:#?}\nswift={:#?}\nkotlin={:#?}",
        report.dev,
        report.swift,
        report.kotlin,
    );
}

/// FLUX-F24 regression: a Flux program whose preserved string value (here, a
/// condition) contains a `$` or a `\\` must reduce to the same canonical
/// form on dev, Swift, and Kotlin. Before the fix the recognizer left the
/// release-side escape (`\\$`, `\\\\`) in place while the dev side held the
/// logical character, so `check_parity` diverged.
#[test]
fn string_escapes_in_condition_do_not_break_parity() {
    // Flux source: a `$` in a condition string (no escape in Flux), and a
    // backslash that the Flux source escapes as `\\`.
    let src = "compo C\n  state n: String = \"x\"\n  if n == \"a$b\" { Text(\"y\") }\n  if n == \"c\\\\d\" { Text(\"z\") }";
    let report = check_parity(src, 9985).expect("compiles");
    assert!(
        report.is_equivalent(),
        "escaped-string parity diverged: dev={:#?}\nswift={:#?}\nkotlin={:#?}",
        report.dev,
        report.swift,
        report.kotlin,
    );
}

/// FLUX-F24 regression (composite-expression form): a condition that embeds a
/// string literal with a `$` or backslash must canonicalize the embedded
/// literal on the release side, matching the dev side's already-unescaped
/// AST text. The recognizer feeds `canonicalize_release_expr` a composite
/// fragment like `( n == "a$b" )`, not a bare literal, so this test pins the
/// character-level walker (round 86 follow-up).
#[test]
fn composite_condition_with_embedded_string_escapes() {
    let src = "compo C\n  state n: String = \"x\"\n  if n == \"a$b\" { Text(\"y\") }\n  if n == \"c\\\\d\" { Text(\"z\") }";
    let report = check_parity(src, 9987).expect("compiles");
    assert!(
        report.is_equivalent(),
        "composite condition parity diverged: dev={:#?}\nswift={:#?}\nkotlin={:#?}",
        report.dev,
        report.swift,
        report.kotlin,
    );

/// FLUX-F25 (documented gap): a `match` node in a component's view body is
/// lowered to `ViewNode::Match` by the dev reducer, but neither release
/// recognizer has a `switch`/`when` dispatcher, so both flatten the arms'
/// view children into the enclosing level. This test is `#[ignore]`d — it
/// currently *asserts* the divergence (both release trees lose the Match
/// node). When the recognizers gain switch/when support, un-ignore and
/// flip the assertion to `report.is_equivalent()`.
#[test]
#[ignore = "FLUX-F25: recognizers cannot recover a match node in the view body"]
fn match_node_in_view_body_is_a_known_divergence() {
    let src = "compo C\n  state score: Int = 0\n  match score {\n    s if s > 90 => Text(\"A\")\n    s if s > 80 => Text(\"B\")\n    _ => Text(\"F\")\n  }";
    let report = check_parity(src, 9988).expect("compiles");
    // Dev side keeps a Match node…
    let dev_has_match = format!("{:?}", report.dev).contains("Match {");
    assert!(dev_has_match, "dev reducer must lower a match into ViewNode::Match");
    // …release side does not (the known gap).
    let swift_has_match = format!("{:?}", report.swift).contains("Match {");
    let kotlin_has_match = format!("{:?}", report.kotlin).contains("Match {");
    assert!(
        !swift_has_match && !kotlin_has_match,
        "if either recognizer now recovers Match, remove the `#[ignore]` and flip the assertion"
    );
}

/// FLUX-F26 (documented gap): a handler containing `batch { … }` or
/// `untrack { … }` currently fails at IR lowering with "unsupported handler
/// expression". This test is `#[ignore]`d; it documents the current failure.
/// When the IR lowerer + codegen gain `ExprKind::Lifecycle` arms, un-ignore
/// and assert `report.is_equivalent()`.
#[test]
#[ignore = "FLUX-F26: batch/untrack in a handler are rejected by IR lowering"]
fn batch_inside_handler_is_a_known_gap() {
    let src = "compo C\n  state n: Int = 0\n  Button(text: \"x\", onPress: { batch { n = n + 1 } })";
    let err = check_parity(src, 9989).expect_err("must currently fail at lowering");
    let msg = format!("{err:?}");
    assert!(
        msg.contains("unsupported handler expression"),
        "expected the F26 lowering gap, got: {msg}"
    );
}

/// FLUX-F28 (documented gap): a `match` arm that binds a payload field
/// and uses it in the arm body currently fails at IR lowering with
/// "`x` is not a state signal in scope". This test is `#[ignore]`d; it
/// documents the current failure. When `compile_match` binds pattern
/// fields into locals, un-ignore and assert parity.
#[test]
#[ignore = "FLUX-F28: match-arm pattern bindings not bound as locals"]
fn match_arm_binding_is_a_known_gap() {
    let src = "type Op =\n  | Add(Int)\n  | Sub(Int)\n\ncompo C\n  state n: Int = 0\n  Button(text: \"b\", onPress: {\n    let op = Add(1)\n    match op {\n      Add(x) => n = n + x\n      Sub(x) => n = n - x\n    }\n  })";
    let err = check_parity(src, 9990).expect_err("must currently fail at lowering");
    let msg = format!("{err:?}");
    assert!(
        msg.contains("not a state signal"),
        "expected the F28 lowering gap, got: {msg}"
    );
}

/// FLUX-F29 (documented gap): assigning to a lambda parameter (or any
/// non-signal binding) reaches IR lowering and fails with the misleading
/// "`x` is not a state signal in scope" — even though the type checker
/// accepted the same expression and the read path (`x + 1`) works.
/// This test is `#[ignore]`d; it documents the current failure. When the
/// semantics are decided (see the issue) un-ignore and assert either
/// successful compilation or the new diagnostic.
#[test]
#[ignore = "FLUX-F29: assignment to a lambda parameter rejected at lowering"]
fn assign_to_lambda_param_is_a_known_gap() {
    let src = "compo C\n  Button(text: \"b\", onPress: fn(x) { x = 1 })";
    let err = check_parity(src, 9991).expect_err("must currently fail at lowering");
    let msg = format!("{err:?}");
    assert!(
        msg.contains("not a state signal"),
        "expected the F29 lowering gap, got: {msg}"
    );
}

/// FLUX-F30 (documented gap): a component declaring `[T: Foo]` where `Foo`
/// is not a declared trait is accepted today — the bound is parsed into
/// the AST but no pass ever consults it. This test is `#[ignore]`d and
/// asserts the current (silently-OK) behavior. When `Checker` registers
/// bounds, un-ignore and assert an "unknown trait" error instead.
#[test]
#[ignore = "FLUX-F30: generic trait bounds are parsed but never enforced"]
fn undeclared_trait_bound_is_a_known_gap() {
    let src = "compo Counter[T: Foo]\n  state n: Int = 0\n  Text(\"x\")";
    let r = check_parity(src, 9992);
    assert!(
        r.is_ok(),
        "documented current behavior: undeclared trait bound is accepted; got {r:?}"
    );
}

/// FLUX-F30 (documented gap): a `[T: Numeric]` component instantiated with
/// `Bool` is accepted today. This test is `#[ignore]`d and asserts the
/// current (silently-OK) behavior. When instantiation checks bounds,
/// un-ignore and assert a trait-satisfaction error instead.
#[test]
#[ignore = "FLUX-F30: generic trait bounds are parsed but never enforced"]
fn bound_violated_at_instantiation_is_a_known_gap() {
    let src = "trait Numeric[T] {\n  fn zero() -> T\n}\n\ncompo Counter[T: Numeric]\n  state count: T = Numeric.zero()\n  Text(\"x\")\n\ncompo App\n  Counter[Bool]()";
    let r = check_parity(src, 9993);
    assert!(
        r.is_ok(),
        "documented current behavior: bound-violating instantiation is accepted; got {r:?}"
    );
}

/// FLUX-F31 (documented gap): `xs[0]` in a state initializer is silently
/// parsed as two separate items — `state first = xs` (truncated at the
/// `[`) plus a stray list literal. The type checker then rejects the
/// truncated expression with a misleading "expected Int, got List[Int]"
/// error. This test is `#[ignore]`d; it documents the current failure.
/// When the parser either supports subscripts or rejects them with a
/// clear error, un-ignore and flip the assertion.
#[test]
#[ignore = "FLUX-F31: subscript syntax is silently split into two expressions"]
fn subscript_index_is_a_known_gap() {
    let src = "compo C\n  state xs: List[Int] = [1, 2]\n  state first: Int = xs[0]\n  Text(\"x\")";
    let err = check_parity(src, 9994).expect_err("must currently fail with a type error");
    let msg = format!("{err:?}");
    assert!(
        msg.contains("expected `Int`, got `List[Int]`"),
        "expected the F31 subscript-split error, got: {msg}"
    );
}

/// FLUX-F32 regression: Flux form-control primitives whose emitted names
/// differ across the two codegen backends must round-trip through
/// `normalize_view_name` so the reduced trees compare equal. Before the
/// fix, `Checkbox` diverged (Swift emits `Toggle`), and `TextArea`
/// diverged (Swift emits `TextEditor`, Kotlin emits `OutlinedTextField`).
#[test]
fn form_control_primitive_names_round_trip() {
    // `Checkbox`: Swift folds it to `Toggle`, Kotlin keeps `Checkbox`.
    let checkbox = "compo C\n  state on: Bool = false\n  Checkbox(value: on, onChange: { on = !on })";
    let r = check_parity(checkbox, 9995).expect("compiles");
    assert!(
        r.is_equivalent(),
        "Checkbox parity diverged: dev={:#?}\nswift={:#?}\nkotlin={:#?}",
        r.dev,
        r.swift,
        r.kotlin,
    );

    // `TextArea`: Swift emits `TextEditor`, Kotlin emits `OutlinedTextField`.
    let text_area = "compo C\n  state text: String = \"\"\n  TextArea(value: text, onChange: { text = \"x\" })";
    let r = check_parity(text_area, 9996).expect("compiles");
    assert!(
        r.is_equivalent(),
        "TextArea parity diverged: dev={:#?}\nswift={:#?}\nkotlin={:#?}",
        r.dev,
        r.swift,
        r.kotlin,
    );
}

/// FLUX-F33 (documented gap): `Theme { … }` is elided by both codegen
/// backends, so the recognizer never sees a `Theme` node while the dev
/// reducer keeps one. This test is `#[ignore]`d; it documents the
/// divergence. When the emitters either keep the wrapper or the dev
/// reducer elides it (via `equivalence::elide_wrappers`), un-ignore and
/// assert parity.
#[test]
#[ignore = "FLUX-F33: Theme wrapper is flattened by both codegen backends"]
fn theme_wrapper_is_a_known_gap() {
    let src = "compo C\n  Theme {\n    Text(\"x\")\n  }";
    let r = check_parity(src, 9997).expect("compiles");
    assert!(
        !r.is_equivalent(),
        "documented current behavior: Theme parity diverges; if this now passes, un-ignore"
    );
}

/// FLUX-F34 (documented gap): `Gesture { … }` yields `Gesture` on dev,
/// `Column` on Swift, `Stack` on Kotlin — three different view names, so
/// `check_parity` reports a divergence. This test is `#[ignore]`d; it
/// documents the current failure. When both backends emit the same
/// wrapper name and the recognizer folds it back, un-ignore and assert
/// parity.
#[test]
#[ignore = "FLUX-F34: Gesture wrapper shape differs on Swift and Kotlin"]
fn gesture_wrapper_is_a_known_gap() {
    let src = "compo C\n  Gesture(onTap: { }) {\n    Text(\"x\")\n  }";
    let r = check_parity(src, 9998).expect("compiles");
    assert!(
        !r.is_equivalent(),
        "documented current behavior: Gesture parity diverges; if this now passes, un-ignore"
    );
}

}
