# LANE-C — Real native capability bodies (replace in-memory stubs)

**Dispatch:** Wave 1 (independent). Do NOT delegate; Louis runs this in his own session.
**Owned directories (exclusive — write ONLY here):**
- `runtimes/ios/FluxHost/Sources/FluxHost/Registry.swift`
- `runtimes/ios/FluxHost/Sources/FluxHost/HelloFrame.swift`
- `runtimes/ios/Tests/RuntimeGapTests.swift` (add round-trip tests)
- `runtimes/android/host/src/main/kotlin/dev/flux/host/vm/CapabilityRegistry.kt`
- `runtimes/android/host/src/main/kotlin/dev/flux/host/vm/wire/HelloFrame.kt`
- `runtimes/android/host/src/test/kotlin/dev/flux/host/RuntimeFixesTest.kt` (add round-trip tests)
- `stdlib/capabilities.flux`
**Consumed (read-only):** `flux-syntax::Value`/`VMValue` shapes; `stdlib/capabilities.flux`
cap/method id table. Do NOT edit any VM dispatch site (`FluxBytecodeVM.swift`,
`StepResult.kt`, `Opcode.kt`, `flux-vm-ref/src/vm.rs`) — capability registry/Hello/stdlib
are YOUR lane; VM lowering/dispatch is NOT (per flux-capabilities skill, R-unsafe list).

## Context (grounded)
The capability *wiring* is complete: `CALL_CAP` lowers to manifest ids
(`Router.navigate` → cap 3, method 1), the `CapabilityRegistry` tables are data-driven,
and `HelloFrame` advertises Camera/Storage/Router. BUT the registered *bodies* are
in-memory stubs: `Camera.take` (1,1) echoes arg field 0 into signal 99 (oracle-parity
echo only — no real capture); `Camera.startPreview`/`stopPreview` just flip signal 96;
`Storage.set/get/delete` (2,1/2/2/2/3) read/write an in-memory `CapabilityStore`;
`Router.navigate` (3,1) writes signal 97 (correct — reconciler-driven). For production,
Camera/Storage must hit real OS APIs.

## Tasks (TDD — RED test before GREEN)
1. **Storage → real persistence.**
   - iOS: `store.putStorage`/`getStorage` back onto `UserDefaults.standard` (namespaced
     key, e.g. `flux.storage.<keyId>`), `delete` removes it. Keep the `CapabilityStore`
     wrapper as the injection seam so tests can pass an in-memory store.
   - Android: back `CapabilityStore` onto `androidx.datastore.preferences` OR a file under
     `context.filesDir` (JVM-test uses an in-memory store; the real impl reads
     `BuildConfig`-gated path). Keep `CapabilityStore` class as the seam.
   - Test: set a value, drop the registry instance, recreate, get returns the same value
     (persisted, not in-memory).
2. **Camera → real capture (dev-safe).**
   - iOS: `Camera.take` (1,1) wraps `UIImagePickerController`/`PHPhotoLibrary` behind an
     `@MainActor` bridge; in a headless/test build it MAY fall back to the deterministic
     `List[Int]` payload (keep `call_cap_basic` oracle parity: echo arg field0 → sig99).
     `startPreview`/`stopPreview` manage an `AVCaptureSession` guard (no-op in tests).
   - Android: `Camera.take` bridges to `CameraX` `ImageCapture`; in JVM tests fall back to
     the synthetic payload. Document the permission requirement.
   - Test: a capability round-trip with a fake/photo-library-backed store asserts the
     returned cell id + that signal 99 (iOS echo) is preserved for oracle parity.
3. **New capabilities (optional, same table pattern):** `Clipboard` (set/get),
   `Geolocation` (get). Add to `stdlib/capabilities.flux` with stable ids, to both
   `advertisedCapabilities` (Hello frames), and both registries. Keep ids deterministic.
4. **Router** stays as-is (signal 97 write) — do NOT change.

## Acceptance gates (DoD)
- `cargo fmt --check` / `cargo clippy -D warnings` clean on any Rust you touch (none here;
  Rust only reads stdlib).
- iOS: `xcodebuild -scheme FluxApp test` — your `RuntimeGapTests` round-trips pass;
  `call_cap_basic` oracle vector stays green (signal 99 echo).
- Android: `./gradlew :host:test` round-trip tests pass (root wrapper present). Run it
  for real; do NOT claim "types verified, runs in CI" — it runs here.
- NO `unreachable!`/`!!`/`try!` in production code; every public item documented.
- `git commit --only <your 7 files>` — do NOT `git add -A` (shared index hazard).

## Pitfalls
- Keep `Camera.take` (1,1) writing arg field0 into signal 99 — the oracle
  `call_cap_basic` vector depends on it. Breaking it fails `flux-vm-ref` conformance.
- Do NOT reintroduce blake3 name-hash for cap/method ids — they come from
  `CAPABILITY_IDL` (flux-types). Registry entries use the literal manifest ids.
- The Hello `advertisedCapabilities` GENERATED blocks must stay byte-compatible with the
  server's `capability_idl` (a known-red `kotlin_registry_matches_idl` test is
  pre-existing drift — don't "fix" it by reformatting; leave it for the Android lane).
