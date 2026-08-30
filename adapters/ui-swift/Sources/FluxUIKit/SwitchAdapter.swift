//  SwitchAdapter.swift
//  FluxUIKit — `Switch` → `UISwitch` (FLUX-040, Appendix F form family).
//
//  Declarative adapter mapping a Flux `Switch` node to a `UISwitch`
//  (unified tier; AGENTS.md §3.5).
//
//  Props are read by name; the index is the FNV-1a-32 digest of the name
//  masked to `u16` (`Props.propIndex`), derived identically on server and
//  client (AGENTS.md §3.2) — never a hardcoded positional index. Fields:
//  - `value: Bool = false` (controlled state)
//  - `onChange: Handler`
//  - `enabled: Bool = true`
//
//  Flipping the switch dispatches `onChange` with the new boolean as the
//  payload so the runtime's handler can write the bound signal. The action
//  target is retained by the `UIAction` closure, which keeps the executor
//  `weak`, so there is no retain cycle back to the runtime.

import UIKit

public final class SwitchAdapter: FluxAdapter {
    public typealias View = UISwitch
    weak var executor: (any FluxExecutor)?

    public init(executor: (any FluxExecutor)? = nil) { self.executor = executor }

    public func create() -> UISwitch { UISwitch() }

    public func update(_ view: UISwitch, from old: Props, to new: Props) {
        let value = new.getBool(named: "value") ?? false
        if view.isOn != value { view.isOn = value }
        view.isEnabled = new.getBool(named: "enabled") ?? true
    }

    public func setChildren(_ children: [AnyObject], on view: UISwitch) {}

    public func bindHandler(_ handlerId: FluxHandlerId, to view: UISwitch, nodeId: FluxNodeId) {
        let target = HandlerTarget(executor: executor, handlerId: handlerId, nodeId: nodeId) { .bool(view.isOn) }
        view.addAction(UIAction { _ in target.fire() }, for: .valueChanged)
    }

    public func destroy(_ view: UISwitch) {
        view.removeTarget(nil, action: nil, for: .allEvents)
    }
}
