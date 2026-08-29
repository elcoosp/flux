# ADR-0053: Public surface naming aligns to React Native / Expo

- **Status:** Accepted
- **Supersedes:** —
- **Superseded by:** —
- **Related issues:** FLUX-038, FLUX-049
- **Related ADRs:** ADR-0047 (unified data-driven codegen), ADR-0049
  (cross-host naming convergence), ADR-0048 (iOS dev-tier convergence)

## Context

Flux is a write-once UI language: one `.flux` source lowers to a VM-fed dev tier
on both platforms and to idiomatic Swift/Kotlin codegen in release. RN/Expo
developers are the primary audience. The *runtime architecture* (VM, diffing
reconciler, signal graph, CALL_CAP capability bridge) is deliberately NOT React
Native — there is no JS thread, no React reconciler, no Yoga layout, no
ViewManager lifecycle. But the *naming ergonomics* — component names, prop
names, and capability/method names — were partly aligned and partly ad-hoc.

We surveyed the existing surface (see investigation behind this ADR) and found
the alignment was already ~80% there at the naming layer: components are
PascalCase (`Text`, `Button`, `Screen`, `Router`), props are mostly camelCase,
and capabilities use PascalCase names (`Camera`, `Storage`, `Router`) that
already echo Expo Modules. The divergence was concentrated in three places:

1. **Snake_case props** that RN uses camelCase for (`max_lines` vs `maxLines`).
2. **Flux-idiomatic verb names** on capability methods where Expo uses
   explicit noun-bearing names (`take` vs `takePicture`, `set` vs `setItem`).
3. **`Async` suffix on async capability methods** — redundant, because the
   `fn` / `async fn` keyword in `CAPABILITY_IDL` already signals sync vs async;
   the suffix is pure noise that a RN/Expo dev does not expect.

Decisions made with the user (verbatim: "i don't want no Async suffix on method
the signature indicate if it's fn or not ... make sure everything green after and
that you changed all occurences even examples"). Two deliverables: this ADR (A)
and a low-risk rename landing across stdlib + both adapter kits + codegen +
parity fixtures + examples (B), with the full test matrix green.

## Decision

### 1. Naming mirrors RN/Expo ergonomics — NOT architecture

The public surface (component names, prop names, capability names + method
names) follows React Native component/prop vocabulary and Expo Modules
capability/method vocabulary so RN/Expo developers feel at home. The runtime
architecture is explicitly NOT React Native and is out of scope for this ADR.

- **Components:** PascalCase; names and prop spellings track RN where a
  first-class equivalent exists. `Text`, `Button`, `Image`, `Screen`, `Router`,
  `Column`, `Row`, `TextInput` mirror RN's `Text`, `Pressable`/`TouchableOpacity`,
  `Image`, `Screen`, `Navigator`, RN's flex `View` (see §2), `TextInput`.
- **Props:** camelCase, matching RN prop spellings where one exists
  (`onPress`, `onChangeText`, `secureTextEntry`, `keyboardType`, `maxLines`,
  `resizeMode`, `source`, `initialRouteName`).
- **Capabilities:** PascalCase names; methods use Expo-style explicit names
  (`Camera.takePicture`, `Storage.setItem`/`getItem`/`removeItem`,
  `Clipboard.setString`/`getString`, `Geolocation.getCurrentPosition`,
  `FileSystem.readAsString`/`writeAsString`/`deleteAsync`,
  `Push.registerForNotifications`/`scheduleNotification`, `DeepLink.openURL`).

### 2. Idiomatic divergence is allowed and documented

Where RN has no first-class primitive or the name would mislead, Flux keeps its
own name but the choice is recorded here so it is a decision, not drift:

- **`Column` / `Row`** (not RN's `<View style={{flexDirection}}>`). Flux's
  `Column`/`Row` map to SwiftUI `VStack`/`HStack` and Compose `Column`/`Row`,
  not to RN's flex `<View>`. No `View`+`direction` alias is added; RN's flex
  model is out of scope for the declarative component tier. This is the one
  place the naming deliberately does NOT mirror RN, because mirroring would
  pull in RN's layout primitive we explicitly avoid.
- **`route` prop is load-bearing and is NOT renamed** to RN's `name`. Both
  reconcilers pick the visible screen via `FNV-1a("route")`; renaming it would
  require coordinated reconciler + parity-gate edits for zero ergonomic gain
  (the signal-97 screen-swap model fits `route` better than `name`). RN's
  `name` is a static screen identifier; Flux's `route` is a live signal.
- **New primitives are allowed** as long as they ship on BOTH adapter kits
  (iOS + Android) before they are advertised. New names still follow RN/Expo
  spelling where a moral equivalent exists; otherwise they follow the PascalCase
  / camelCase house rules above.

### 3. No `Async` suffix on capability methods

Capability methods are synchronous or asynchronous based on the `fn` vs
`async fn` keyword in `CAPABILITY_IDL` — that IS the signal. Method names NEVER
carry an `Async` suffix (`takePicture`, not `takePictureAsync`; `getCurrentPosition`,
not `getCurrentPositionAsync`). The server, both host kits, and the codegen
bridge all key capabilities by **numeric** `(capId, methodId)`, so renaming a
method is purely cosmetic and touches only `stdlib/capabilities.flux`,
`CAPABILITY_IDL`, the `HelloFrame` wire fixtures, and parity/comments — never the
registry dispatch logic.

### 4. Wire contract is untouched by renames

- Prop indices are `FNV-1a(name)` (named args) / sequential (positional), derived
  identically on server and both host kits (AGENTS.md §3.2). Renaming a prop
  changes its wire index; the rename lands atomically on server + both kits so
  the index stays in lockstep. Adapters MUST read props by name (they derive the
  same `FNV-1a`), never by hardcoded positional index — a prior latent device
  fault (Swift adapters reading positional `0/1/2/3`) was corrected as part of
  this work.
- Capability ids/method ids are numeric and deterministic from `CAPABILITY_IDL`;
  names are human-readable metadata only.

## Consequences

- RN/Expo developers read Flux component/prop/capability names without a
  translation layer.
- The `fn`/`async fn` keyword is the single source of truth for sync/async; no
  duplicate suffix to keep in sync.
- Both adapter kits must implement any advertised primitive; divergence between
  the kits surfaces as a parity-gate failure, not as a silent runtime blank.
- `Column`/`Row` and `route` are explicit, documented exceptions to RN naming.
- The rename is verified green across: 309 Rust tests (flux-* crates), 36 Swift
  adapter tests (FluxUIKit on iOS Simulator), and 144 pure-JVM Android host
  tests, including a regenerated `counter_init_frame.bin` captured from the
  renamed source so the E2E path tracks the live prop vocabulary.

## Open Questions

1. Should form primitives (`Switch`, `Checkbox`, `Slider`, `Picker`, `DatePicker`,
   `TextArea`) adopt RN prop spellings (`value`/`onValueChange` vs the current
   `onChange`)? Left for a follow-up; their `handler_prop` is not yet consumed by
   the codegen emitter, so renaming now would be untested churn.
2. RN `Image` `resizeMode` values (`cover`/`contain`/`stretch`/`repeat`/`center`)
   vs the current `fit`/`stretch`/`fill` set — align the value vocabulary in a
   later pass if both kits agree.
