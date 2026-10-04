//  ToggleAdapter.swift
//  FluxUIKit — `Toggle` → `UISwitch` (FLUX-077, data-driven surface FLUX-072).
//
//  Declarative adapter mapping a Flux `Toggle` node to a `UISwitch`
//  (unified tier; AGENTS.md §3.5).
//
//  `Toggle` is the two-state boolean control the data-driven surface uses —
//  `examples/todo` renders each `TaskRow` with
//  `Toggle value: task.done, onValueChange: fn(v) { … }`. It shares the same
//  prop contract as `Switch` (`value` + `enabled`) but emits the change
//  callback under `onValueChange` (see `examples/todo` + the codegen bridge in
//  `flux-codegen-core/src/primitives.rs`).
//
//  Props are read by name; the index is the FNV-1a-32 digest of the name
//  masked to `u16` (`Props.propIndex`), derived identically on server and
//  client (AGENTS.md §3.2) — never a hardcoded positional index. Fields:
//  - `value: Bool = false` (controlled state)
//  - `onValueChange: Handler`
//  - `enabled: Bool = true`
//
//  Flipping the switch dispatches `onValueChange` with the new boolean as the
//  payload so the runtime's handler can write the bound signal. The action
//  target is retained by the `UIAction` closure, which keeps the executor
//  `weak`, so there is no retain cycle back to the runtime.

import UIKit

/// Declarative adapter (unified tier; AGENTS.md §3.5) mapping a Flux `Toggle`
/// node to a `UISwitch`.
public final class ToggleAdapter: FluxAdapter {
    /// Stable identifier for the UIAction this adapter registers
    /// in `bindHandler`. `UIControl` has no `removeAllActions()`;
    /// the correct pattern is to remove by identifier before
    /// re-adding, so rebinds replace (not accumulate) the handler.
    private static let fluxHandlerAction =
        UIAction.Identifier("flux.handler")

    public typealias View = UISwitch
    weak var executor: (any FluxExecutor)?

    public init(executor: (any FluxExecutor)? = nil) { self.executor = executor }

    public func create() -> UISwitch { UISwitch() }

    public func update(_ view: UISwitch, from old: Props, to new: Props) {
        // Audit D14: absent prop retains previous value (no reset).
        if let value = new.getBool(named: "value"), view.isOn != value {
            view.isOn = value
        }
        view.isEnabled = new.getBool(named: "enabled") ?? true
    }

    public func setChildren(_ children: [AnyObject], on view: UISwitch) {}

    public func bindHandler(_ handlerId: FluxHandlerId, to view: UISwitch, nodeId: FluxNodeId) {
        let target = HandlerTarget(executor: executor, handlerId: handlerId, nodeId: nodeId) { .bool(view.isOn) }
        // Audit fix: remove any previously-registered UIAction on this view
        // before adding a new one. The dev runtime re-binds handlers on
        // hot-swap and on prop change; without this each rebind accumulates
        // another action and a single user tap dispatches N times.
        view.removeAction(identifiedBy: Self.fluxHandlerAction, for: .valueChanged)
        view.addAction(UIAction(identifier: Self.fluxHandlerAction) { _ in target.fire() }, for: .valueChanged)
    }

    public func destroy(_ view: UISwitch) {
        // Audit fix: `removeTarget(_:action:for:)` removes target/action pairs,
        // NOT `UIAction` registrations (those need `removeAllActions()`). The
        // previous call left stale actions alive and firing on recycled views.
        view.removeAction(identifiedBy: Self.fluxHandlerAction, for: .valueChanged)
    }
}
