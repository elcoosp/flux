---
title: Adding a Primitive
description: How to add a new adapter primitive (e.g. a Slider) to both hosts and keep the adapter contract and parity tests in sync.
---

A *primitive* is a leaf UI component backed by a native view on both platforms
(see Appendix F of the spec). Adding one is a cross-cutting change: the adapter
contract, the dev implementation, the release implementation, and a parity test
must all move together.

## 1. Extend the adapter contract (Appendix F)

Add the primitive's props to the contract. Every prop is part of the contract: a
new prop must be added in **both** dev and release, with the same name and type.

```flux
// slider.flux — `Slider` adapter component (Appendix F.N).
component Slider(
  value: Float,
  min: Float = 0.0,
  max: Float = 1.0,
  onValueChange: Handler,
) {
  // Adapter leaf — native rendering defined by Appendix F.N.
}
```

## 2. Implement it on both hosts

- **Dev (iOS/Android):** drive `UISlider` / `android.widget.SeekBar` (or Compose
  `Slider`) imperatively in the adapter's `update`.
- **Release (SwiftUI/Compose):** emit `@State` + `Slider(value:)` /
  `Slider(value = …)`.

Both consume the **same props** — the props are the contract.

## 3. Wire signal deps + handlers

If the primitive writes a signal (`onValueChange` fires on drag), the host must
register the handler and the node's `signal_deps` must include whatever the handler
closure reads. This is what keeps the dirty-set reconcile correct
(see [Host-Authoritative State](/concepts/host-authoritative-state/)).

## 4. Add a parity test

Add a golden scenario to `reconcile-trace-format.md` (e.g. `slider_drag`) and a
trace in `/tests/trace-goldens/`. The parity tool (`flux-parity trace diff`) proves
the Swift and Kotlin hosts produce byte-identical traces.

## Checklist

- [ ] Props added to Appendix F (dev + release).
- [ ] Dev adapter implemented on both platforms.
- [ ] Release codegen implemented on both platforms.
- [ ] `signal_deps` registered for any written signals.
- [ ] Parity golden + trace added.
