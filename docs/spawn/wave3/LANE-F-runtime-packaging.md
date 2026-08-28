# LANE-F — Runtime packaging as an importable engine (ADR-0036)

**PREREQ:** Wave 2 GREEN (the async resolver + build gate land first). **Dispatch:** Wave 3.
Do NOT delegate; Louis runs this.
**Owned directories (exclusive):**
- `runtimes/ios/**` (extract engine so a consumer `import FluxHost` + `import FluxUIKit`
  drives it; keep `FluxApp` as the demo host — `FluxAppMain.swift` stays `@main` demo)
- `runtimes/android/**` (ensure `:runtimes:android:host` is a library module a consumer app
  can `implementation(project(":runtimes:android:host"))`; verify `:app` consumes it)
**Consumed (read-only):** the engine public surface (`FluxExecutor`/`FluxExecutor`,
`AdapterRegistry`, `SignalGraph`) already in `FluxHost` (SwiftPM lib, `Package.swift:23`).
**R2 (frozen manifests):** `settings.gradle.kts` + `gradle/wrapper` ALREADY exist at repo
root (incl. `:runtimes:android:host` + `:app`) — do NOT recreate them. The wrapper is present,
so `./gradlew :runtimes:android:host:test` and `./gradlew :runtimes:android:app:assembleDebug`
RUN HERE. Your task is the engine/library split, not the wrapper.

## Context (grounded)
- iOS: `FluxHost` is ALREADY a SwiftPM `.library` target (`runtimes/ios/FluxHost/Package.swift`),
  and `FluxAppMain.swift` `@main` is a thin SwiftUI `App` that imports `FluxHost`. So the
  engine is already importable; the remaining gap (ADR-0036) is (a) a clear "consumer embeds
  FluxHost" sample and (b) ensuring NO app-shell singleton leaks into `FluxHost` (the engine
  must be drivable headlessly, which `FluxExecutor(graph:registry:)` already allows).
- Android: `:host` (pure-JVM reactive core) and `:app` (Compose shell) are separate modules;
  `FluxSession.kt:55` builds `FluxExecutor` via the primary ctor. A consumer app can already
  `implementation(project(":runtimes:android:host"))`. Verify this compiles; close any leak
  of `:app`-only Android context into `:host`.
- The real "monolith" risk is that `:host` depends on `:app` or vice-versa in a way that
  blocks a third-party consumer. Assert the dependency direction is `:app → :host` only.

## Tasks (TDD — assert importability)
1. **iOS:** add a `FluxEmbedSample` (or document in ADR-0036) target that `import FluxHost` +
   `import FluxUIKit` and renders a hand-built tree WITHOUT `@main` app glue. Keep `FluxApp`
   as the dev demo. Assert `FluxHost` builds with no `UIApplication`/`@main` symbols in the lib.
2. **Android:** confirm `:runtimes:android:host` has NO `androidx.activity`/`compose` runtime
   deps that would force a consumer to pull the app; if `:host` imports `:app`, break it.
   Verify `./gradlew :runtimes:android:app:assembleDebug` succeeds (SDK present).
3. **ADR-0036 update:** mark it Implemented with the realized module graph; add the
   "embed Flux in an existing app" skeleton (create-only ADR carve-out if a NEW adr, or append
   to ADR-0036 — appending is fine since it's your lane).

## Acceptance gates (DoD)
- iOS: `xcodebuild -scheme FluxHost build` (library) succeeds with no app symbols; demo app
  still builds.
- Android: `./gradlew :runtimes:android:host:test` + `:runtimes:android:app:assembleDebug`
  both succeed HERE.
- Engine public surface documented; no `UIApplication`/`@main` in `FluxHost` lib.
- `git commit --only runtimes/ios/... runtimes/android/...` — no `git add -A`.

## Pitfalls
- Do NOT recreate `settings.gradle.kts`/`gradle/wrapper` (R2 — they exist at repo root).
- Keep `FluxAppMain.swift` `@main` (it's the demo host); the engine split is additive.
- Pre-existing ktlint violations in `ShadowTree.kt`/`HelloFrame.kt` are the android-runtime
  agent's WIP — do not sweep; keep your new files ktlint-clean.
