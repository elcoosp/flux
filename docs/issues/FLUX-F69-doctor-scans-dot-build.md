# FLUX-F69 — `flux doctor` scans Swift Package Manager's `.build/` directory

**Status:** OPEN (fix landed in same commit)
**Found:** round-115 (`flux doctor` output)
**Severity:** low (advisory-only; a user who runs `flux doctor --strict`
gets a false-positive advisory finding on a generated file, and the
noisy output hides real violations.)

## Summary

`crates/flux-cli/src/doctor/agents_rules.rs::is_skipped_dir`:

    fn is_skipped_dir(name: &str) -> bool {
        matches!(
            name,
            "target" | "Generated" | ".git" | "node_modules" | "build"
        )
    }

The list skips `target` (Cargo) and `build` (Gradle / general), but not
`.build` — Swift Package Manager's build directory. The `scan_sources`
walker therefore descends into `runtimes/ios/FluxHost/.build/` and
reports violations on the generated SwiftPM artifacts:

    runtimes/ios/FluxHost/.build/arm64-apple-macosx/debug/FluxHostPackageTests.derived/runner.swift:1 — file is 547 lines (exceeds 300)

That file is regenerated on every `swift build` and is not source the
project owns. It is counted among the 344 advisory findings the
`flux doctor` run produced.

## Impact

Cosmetic: the doctor's advisory-findings count is inflated by generated
SwiftPM output. A developer running `flux doctor --strict` to gate a
pre-commit hook sees a false positive on a file they cannot edit.

## Fix

Add `.build` to the skipped-directory set:

    matches!(
        name,
        "target" | "Generated" | ".git" | "node_modules" | "build" | ".build"
    )

Also worth considering (but not fixed here): `DerivedData` (Xcode),
`.gradle` (Gradle), `Pods` (CocoaPods), and `dist` (frontend bundles) —
all are tool output and none are project source. Adding `.build` is the
immediate fix; the others can be added as future cases prove them
noisy.

## Regression test

`crates/flux-cli/src/doctor/tests.rs::dot_build_directory_is_skipped` —
added in this commit. Writes a fixture with a `.build/foo.swift`
containing a violation, asserts `scan_sources` ignores it.

## Related

`docs/issues/FLUX-F48-doctor-test-mod-prefix-overbroad.md` — another
false-positive class in the doctor's walker.
