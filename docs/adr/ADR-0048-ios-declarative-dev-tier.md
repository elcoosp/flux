# ADR-0048: Converge the iOS dev tier onto declarative SwiftUI

**Status:** Proposed (no code landed; supersedes nothing until implemented)
**Date:** 2026-08-28
**Resolves:** the "doc-reconciliation ADR is pending" note in AGENTS.md §0.2.
**Decision Drivers:** AGENTS.md §0.2 declares that dev and release render through the
same declarative components and that there is **no imperative dev tier**. Android
satisfies this; iOS does not. The gap is not a doc defect — the iOS comments
accurately describe a UIKit implementation that really is what ships — so it cannot
be closed by editing prose. It needs a port.

## Context and Problem Statement

The two platforms diverge on **one** axis, and a previous reading of this repo
conflated it with a second, unrelated one. Both are recorded here so the mistake
is not repeated.

### Axis 1 — in-place prop observation (NOT a divergence)

Both platforms already satisfy §0.2's requirement that the VM re-materializes a
node's props in place and the UI reacts. The mechanism differs by necessity:

* **Android** needs an explicit observable. `ShadowNode.props` lives in a Compose
  `MutableState`, injected via `propsStateFactory` from `FluxSession`. Before
  `6b61fa4` it was a plain `var`, invisible to Compose's snapshot tracking, so the
  UI froze after the first frame and interpolated text appeared to vanish once a
  signal changed.
* **iOS** observes in-place tree mutation natively and needs no state wrapper. The
  `6b61fa4` commit message says so explicitly: *"(Android-only; iOS SwiftUI
  observes in-place tree mutation natively)"*.

`propsStateFactory` deliberately lives in `:host` as a platform-neutral injection
point (no Compose-UI / Android-framework dependency), which is why the JVM suites
still run without an emulator. **Android is not "ahead" here and iOS is not missing
a feature.** Anyone treating `MutableState` as the doctrine iOS must copy has
misread a Compose constraint as an architectural decision.

### Axis 2 — the rendering tier (the real divergence)

| Platform | Dev rendering tier |
|---|---|
| Android | Declarative. `ShadowTreeRenderer` composes from the shadow tree; `DirtyReconciler.reconcileDirty` touches exactly `dependents[S]`. |
| iOS | Imperative. A UIKit adapter kit plus a reconciler that owns live view objects. |

Concretely, on iOS today:

* `adapters/ui-swift/Sources/FluxUIKit` is a UIKit kit: `Text`→`UILabel`,
  `Image`→`UIImageView`, `Button`→`UIButton`, `Column`/`Row`→`UIStackView`,
  `TextField`→`UITextField`, `Router`→`UINavigationController`,
  `Screen`→`UIViewController`, `Component`→plain `UIView`.
* `ShadowTree.swift` stores `let props: [Prop]` — a plain immutable array. There is
  no observable and nothing subscribes to it.
* `ShadowTreeReconciler.swift` therefore keeps a **parallel tree of live view
  objects** (`let adapter: AnyFluxAdapter`, `let view: AnyObject`) which it owns and
  mutates, calling `adapter.create()` for new nodes and `adapter.destroy()` for
  removed ones.
* `FluxHostController` / `FluxAppMain` mount that root `UIView`.
* 20 files import UIKit; effectively none import SwiftUI.

This is precisely the model AGENTS.md §0.2 calls **superseded**: *"dev
implementation: imperative, drives UIView/View directly."* It is still live on iOS.

Note the irony motivating this ADR: the codebase already *claims* iOS is SwiftUI
(the `6b61fa4` message above), while the iOS code is UIKit. The intended
architecture and the shipped architecture disagree, and until now nothing recorded
which was which.

## Decision

Port the iOS dev tier to declarative SwiftUI, so that one component vocabulary is
fed by two feeders (dev by the VM, release by codegen) on both platforms.

1. Replace the nine UIKit adapters with SwiftUI views reading the same props
   contract. Prop access stays name-derived (§3.2, FNV-1a-32 masked to `u16` via
   `Props.propIndex`) — **no positional indices**, and missing/renamed fields keep
   degrading to `null`/default rather than throwing (§3.5).
2. Retire the reconciler's view-ownership model. `ShadowTreeReconciler` stops
   owning `AnyObject` views and `adapter.create()`/`destroy()`; SwiftUI derives the
   view tree from the shadow tree instead.
3. Keep node-id-keyed identity semantics. Keyed reconciliation must still reorder
   rather than recreate, preserving scroll position, text and screen state across
   diffs and router push/pop (§3.5).
4. Interop, not rewrite: genuinely UIKit-only components (and third-party views)
   enter through `UIViewRepresentable`, per §0.2.
5. Hold adapter **contract version 1**. This port changes the platform lowering,
   not the wire or props contract, so no protocol version bump and no new opcodes.

## Consequences

**Positive.** §0.2 becomes true on both platforms; the component vocabulary
converges so a primitive is defined once per platform rather than once per platform
*per tier*; iOS gains free in-place observation instead of manual view mutation;
the "superseded Appendix F" caveat can finally be deleted.

**Negative / risks.** This touches every file in `adapters/ui-swift` plus the iOS
host mount path — a large, high-collision diff. `UINavigationController`-based
routing has no one-line SwiftUI equivalent, so `Router`/`Screen` need real design
work. Per-node adapter factories and `WeakReference`-held executors (§3.5, the
FLUX-007 leak history) must be preserved under a value-type view model.

**Verification gate.** Per the maintainer's standing requirement, green tests are
not sufficient: this must be proven with an actual run on the physical Poco
(Android parity check) and the iPhone 17 Pro simulator, with rendered-state
evidence — tap increments re-render, hot-reload of an interpolated string
re-renders, scroll position and router state survive a delta. Any temporary
instrumentation must be stripped before landing.

## Sequencing (why this ADR lands before the code)

At the time of writing the tree is not in a state to accept this port:

* `cargo build --workspace` is **red** — an untracked
  `crates/flux-codegen-core/src/view_tree.rs` has a non-exhaustive `match` on the
  `#[non_exhaustive]` `BinOp` enum (missing `_` arm). `cargo doc` passes because
  rustdoc is lenient, which is how it went unnoticed.
* Multiple agents hold uncommitted work in `runtimes/ios`, `runtimes/android`,
  `adapters/ui-swift` and `adapters/ui-kotlin` — exactly the files this port
  rewrites. `ButtonAdapter.swift` additionally carries live DEBUG instrumentation
  (`NSLog`, plus tap coordinates written to `UserDefaults` and
  `NSTemporaryDirectory()`) that must be stripped first (§1.2).

Accept this ADR now to fix the *record* (AGENTS.md §0.2 has been updated to
describe both axes accurately), then schedule the port once the build is green and
those lanes have landed. Until then the iOS doc comments stay as they are: they
describe the code that exists, which is the property that makes documentation worth
reading.
