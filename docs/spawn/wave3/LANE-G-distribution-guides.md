# LANE-G — Distribution artifacts, version pinning & embed guide

**PREREQ:** Wave 3 LANE-F (packaging) GREEN. **Dispatch:** Wave 3 (can run in parallel with
F after F lands). Do NOT delegate; Louis runs this.
**Owned directories (exclusive):**
- `/docs/**` (orchestrator carve-out: write the embed guide + a versioning note; ADRs are
  create-only — new file `adr-00XX-runtime-versioning.md` if a protocol-version bump is needed)
- `/.github/workflows/**` (add artifact-publish + host-side handshake-check jobs — `ci` lane)
**Consumed (read-only):** `PROTOCOL_VERSION` (flux-devserver/session.rs), `FluxHost`/`FluxUIKit`
packaging (LANE-F), `:host` AAR output (LANE-F). No manifest edits (R2).

## Context (grounded)
- `PROTOCOL_VERSION` is advertised in the Hello handshake; the dev server rejects a mismatched
  frame type with a version-mismatch Error frame (`crates/flux-devserver/src/server/session.rs`),
  but the HOSTS do not yet fail-closed on a protocol-version mismatch (a host on an old runtime
  could boot against a newer server and silently misdecode). ADR-0036/0034 note the node-ID
  bridge is version-sensitive.
- There is NO published artifact: no `FluxHost.xcframework`, no `:host` AAR. A consumer cannot
  pull a versioned engine; they must build from source.
- No "Embed Flux in an existing app" guide exists (ADR-0036 open item).

## Tasks (TDD — assert host rejects a mismatched version)
1. **Host-side handshake fail-closed.** In `runtimes/ios` (`FluxWebSocketTransport.swift`/
   `HelloFrame.swift`) and `runtimes/android` (`OkHttpTransport.kt`/`HelloFrame.kt`), after
   decoding the server Hello, compare the server's `PROTOCOL_VERSION` to the host's compiled
   `PROTOCOL_VERSION`; on mismatch, surface a RED error overlay / banner (never a crash,
   Appendix E §E.6) and refuse to apply further frames. Add a test on each host that feeds a
   Hello with a wrong version and asserts the connection enters an error state.
2. **Artifacts.** Add CI jobs that produce `FluxHost.xcframework`
   (`xcodebuild -create-xcframework`) and `:runtimes:android:host` AAR
   (`./gradlew :runtimes:android:host:bundleReleaseAar`), and upload them as release artifacts.
   Pin the artifact version to the runtime release tag.
3. **Embed guide.** Write `docs/embed-flux.md`: a consumer app `import FluxHost` (iOS) /
   `implementation(project(":runtimes:android:host"))` (Android), wires `FluxExecutor` to a
   `FluxRootView` / `FluxTreeView`, and connects to a dev server or loads `Generated/`.
4. **Versioning ADR** (create-only): ratify that `PROTOCOL_VERSION` is bumped on any wire/IR
   change and that hosts MUST fail-closed (item 1).

## Acceptance gates (DoD)
- iOS: `xcodebuild -scheme FluxApp test` — wrong-version Hello test fails-closed.
- Android: `./gradlew :runtimes:android:host:test` — same.
- CI produces xcframework + AAR artifacts (linux runner can't build xcframework — run that job
  on macos; AAR on linux is fine).
- `docs/embed-flux.md` reviewed for accuracy against the realized `FluxHost` surface.
- `git commit --only docs/... .github/...` — no `git add -A`.

## Pitfalls
- Do NOT weaken the server-side version-mismatch Error frame — extend, don't replace.
- xcframework build needs macOS runner (Xcode); AAR can be linux. Split the CI jobs by runner.
- Keep the node-ID bridge (ADR-0034) intact when pinning versions.
