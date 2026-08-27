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
                "Router.navigate → CALL_CAP(3,1)"
            );
            navigate_calls += 1;
        }
    }
    assert_eq!(navigate_calls, 2, "both buttons must call Router.navigate");
}
