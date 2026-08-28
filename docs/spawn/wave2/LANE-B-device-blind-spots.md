# LANE-B — Device-only blind-spot verification (router route prop + CALL_CAP ids)

**PREREQ:** Wave 1 GREEN. **Dispatch:** Wave 2. Do NOT delegate; Louis runs this.
**Owned directories (exclusive):**
- `crates/flux-parity/tests/parity.rs` (strengthen the router gate)
- `crates/flux-parity/src/**` (reduce.rs / model.rs if the gate needs a new helper)
- `runtimes/ios/Tests/RenderMountTests.swift` (add on-device router smoke)
- `runtimes/android/host/src/test/kotlin/dev/flux/host/RuntimeFixesTest.kt` (add positional-Screen trap test)
- `examples/router/main.flux` (ensure NAMED `Screen(route:)` form)
**Consumed (read-only):** `prop_index_for_name` (flux-ir), `ROUTE_PROP_INDEX` (hosts), the
`router_example_emits_route_prop_and_navigate_call` test. No VM dispatch edits.

## Context (grounded — the documented trap)
Parity can be GREEN while a `Screen("x")` *positional* arg lowers to `PropIdx(0)` (not
`FNV-1a("route")`), so the host reconciler's `route` lookup finds NO `route` prop and
navigation silently never swaps. `flux-parity/src/reduce.rs::screen_route_from_args`
reconstructs `route` from the *positional* arg syntactically, masking the bug. The true gate
inspects the ACTUAL lowered prop index. The iOS `RouterAdapter` + Android `ShadowTree`
(activeRouteFromSignal, fixed 9840ab5) now read signal 97 as both `.str`/`.record`, but the
*route-prop-index* trap is still unguarded on device. CALL_CAP id derivation is also only
exercised by the parity test (registry round-trips call literal ids).

## Tasks (TDD)
1. **Strengthen parity gate.** Extend `router_example_emits_route_prop_and_navigate_call`
   (parity.rs) to assert, for BOTH a `Screen(route: "home")` NAMED form and a
   `Screen("home")` POSITIONAL form, that the lowered IR carries a `route` prop keyed by
   `prop_index_for_name("route")` on the NAMED form, and that the POSITIONAL form is
   REJECTED (lower error OR a parity-mismatch assertion). This makes the device-only trap
   visible in Rust.
2. **On-device router smoke (both hosts).** In `RenderMountTests.swift` (iOS) and
   `RuntimeFixesTest.kt` (Android), add a test that builds the `examples/router` tree,
   dispatches `Router.navigate("settings")` through the REAL executor to a real screen, and
   asserts the active screen swaps (uses the now-fixed signal-97 reader). Add a SECOND test
   using a POSITIONAL `Screen("x")` that asserts navigation does NOT swap (documents the trap
   so it can never regress silently) — keep `examples/router/main.flux` on the NAMED form.
3. **CALL_CAP id regression on real VM.** Add a `FluxBytecodeVM.run` of a handler that calls
   `Router.navigate` on BOTH hosts; assert it lowers to `CALL_CAP(3,1)` and the registry
   resolves it (the `capability_call_lowers_to_manifest_ids` Rust test already covers the
   lower; this adds the executed-host leg).

## Acceptance gates (DoD)
- Rust: `cargo nextest -p flux-parity` green with the strengthened gate; `cargo clippy -D`.
- iOS: `xcodebuild -scheme FluxApp test` — router smoke passes.
- Android: `./gradlew :runtimes:android:host:test` — router smoke passes.
- `examples/router/main.flux` uses NAMED `Screen(route:)`.
- `git commit --only <your files>` — no `git add -A`.

## Pitfalls
- Do NOT "fix" the parity reducer to synthesize a `route` for positional args — that hides
  the trap. Assert the lowered index; let the source author use the NAMED form.
- The `call_cap_basic` oracle vector (cap1,method1→sig99) must stay green.
