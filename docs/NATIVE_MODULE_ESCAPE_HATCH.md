# Native-module escape hatch (FLUX-046)

Wrap any third-party SDK or platform API as a Flux **capability**, call it from
`.flux` through `CALL_CAP`, and bind the implementation on each host. No fork,
no new opcode, no wire-format change.

This is the sanctioned pressure-release valve for "Flux doesn't have a built-in
for X." Instead of waiting for a stdlib capability, you register your own.

## 1. Declare it in `.flux`

Capabilities are declared in `stdlib/capabilities.flux`. Add a block:

```flux
capability NativeModule {
    // Derive ids deterministically (FNV-1a over name) — never hand-assign.
    invoke(name: String, args: String): String
}
```

The server lowers `CALL_CAP` to your `capId` / `methodId` using
`derive_capability_id("NativeModule")` and `derive_method_id("invoke")` — the
**same derivation runs on both hosts**, so the ids cannot drift between server
and clients.

## 2. Bind the implementation on each host

### Android (`runtimes/android/host`, pure-JVM core)

Register in `CapabilityRegistry` (the `.dev` registry used by the dev executor):

```kotlin
// cap 13 = NativeModule, method 1 = invoke
put(13u, 1u.toUShort()) { args, signals ->
    // args: record { name: str, args: str }
    val name = args.get(0)?.asString() ?: ""
    val payload = args.get(1)?.asString() ?: ""
    // Call your SDK here (off the reactive dispatcher where appropriate).
    val result = MySdk.call(name, payload)
    // Surface the result into the result-cell signal id the VM reserved.
    signals.write(83u, FluxValue.StrVal(intern(result)))
    83u
}
```

### iOS (`runtimes/ios/FluxHost`, `Registry.swift`)

```swift
(13, 1, { args, _, _, signals in
    let name = args.recordValue(at: 0)?.stringValue ?? ""
    let payload = args.recordValue(at: 1)?.stringValue ?? ""
    let result = MySdk.call(name, payload)
    signals.write(83, .str(try InternString.resolve(result)))
    return 83
})
```

## 3. The permission gate (FLUX-049 / ADR-0057)

A wrapped SDK runs **arbitrary native code**, so by default `NativeModule` is
gated by `PermissionKind.NativeModule` — an explicit allow-list grant the
consumer must approve (mirrored on both hosts in `Permission.kt` /
`Permission.swift`). A denied grant settles a typed `CAPABILITY_DENIED` error
(red banner, branchable via `Result[T, E]`), never a crash.

If your wrapper is sandbox-contained and needs no OS grant, map it to
`PermissionKind.None` (as `WebView` does, cap 12) by editing the mirrored
`required_permission` table on all three sites (Rust `capabilities.rs`,
Android, iOS). Keep the three tables in lock-step — a missing gate entry is a
security hole.

## 4. The result cell (ADR-0044 / ADR-0045)

`CALL_CAP` returns a **result-cell signal id** (here `83u`). Synchronous caps
settle it before returning; async caps leave it `Pending` and the executor
resolves it. Read it in `.flux`:

```flux
let cell = NativeModule.invoke("payments", json)
match cell {
  Ready(v) => Text(v)
  Pending    => Text("loading…")
  Error(e)   => Text("denied: {e}")
}
```

## 5. Denied-grant test (must exist on both hosts)

Every escape-hatch capability needs a test asserting a denied grant settles the
typed error and the VM returns normally (no panic). See
`RuntimeFixesTest.testDeniedPermissionFaultsCallCapAsCapabilityDenied` (Android)
and `CapabilityRoundTripTests.testDeniedPermissionFaultsCallCapAsCapabilityDenied`
(iOS).

## Security summary

- Capability ids are **deterministic** (FNV-1a), never hand-assigned → no
  collision with stdlib caps.
- The gate is **enforced on-device**, not just server-side.
- Unknown cap id → **denied**, not resolved.
- Denied grant → **typed error**, never a crash.
