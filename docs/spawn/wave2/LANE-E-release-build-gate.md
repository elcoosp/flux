# LANE-E — `flux build` actually compiles the generated app (release gate)

**PREREQ:** Wave 1 GREEN. **Dispatch:** Wave 2. Do NOT delegate; Louis runs this.
**Owned directories (exclusive):** `crates/flux-cli/src/**` (only `build.rs` +
`build.rs`'s test module; do NOT touch other cli files unless a boundary accessor is needed).
**Consumed (read-only):** `flux_devserver::Pipeline`, `flux_codegen_swift`/
`flux_codegen_kotlin`, `Platform` enum in this crate. No manifest edits (R2).

## Context (grounded)
`crates/flux-cli/src/build.rs::invoke_native_toolchain` (lines 82–110) does NOT run the
native toolchain. It calls `which("xcodebuild")` / `which("gradle")` and only *logs*
"invoke manually" — then `run()` returns `Ok(())` regardless. So `flux build` is GREEN even
when the generated sources do not compile. The spec requires the release path to produce a
buildable app; a silent no-op violates the production-readiness contract (a broken generated
app must fail the command).

## Tasks (TDD — RED: a failing toolchain must fail the command)
1. **Make the gate real.** Replace `invoke_native_toolchain` with an actual spawn:
   - iOS: when `xcodebuild` present, run `xcodebuild -scheme FluxApp -configuration Release
     -destination 'generic/platform=iOS' build` (or the generated-app scheme under
     `platforms/ios`) and capture its exit status; on non-zero, `bail!` with the tail of the
     build log + the path to the generated sources for triage.
   - Android: when `./gradlew` (repo root) present, run `./gradlew :runtimes:android:app:assembleDebug`
     (or `:app:build`) and `bail!` on non-zero.
   - When the toolchain is ABSENT, keep the current warn-and-return-0 behavior (CI without
     Xcode/Gradle should still emit sources, not hard-fail) — but log which verification was
     SKIPPED.
2. **Test (cargo).** Add a test in `build.rs` (or `tests/`) that drives `run()` against a
   fixture project under `crates/flux-cli/tests/fixtures/` with a stubbed `xcodebuild`/
   `gradlew` that exits 1, and asserts `run()` returns `Err` (not `Ok`). Then a second test
   with a stub that exits 0 asserts `Ok`. Use an injected command resolver (do NOT shell out
   to the real toolchain in unit tests).
3. **Asset-server parity:** ensure `flux build` still emits the `Generated/` sources BEFORE
   attempting the native build (current order is correct — keep it).

## Acceptance gates (DoD)
- `cargo fmt --check`, `cargo clippy -D warnings`, `cargo nextest -p flux-cli` green with the
  new gate tests.
- Documented: a generated app that fails to compile makes `flux build` exit non-zero with an
  actionable message (what/where/how per AGENTS.md §3.11).
- No `unwrap()`/`expect()` on the spawned process result; capture stderr.
- `git commit --only crates/flux-cli/src/build.rs crates/flux-cli/...test...` — no `git add -A`.

## Pitfalls
- Do NOT modify `Cargo.toml` (R2) to add deps — use `std::process::Command` (already in std).
- Keep the "toolchain absent → emit-only" path; CI without Xcode must not hard-fail.
- The generated-app scheme for iOS may need `platforms/ios` to exist with a consumer app
  wrapping `Generated/` — if it doesn't, that's LANE-F's deliverable; coordinate, don't
  invent a scheme here. Until then, gate the iOS spawn on the scheme existing.
