//  SliderAdapter.swift
//  FluxUIKit — `Slider` → `UISlider` (FLUX-040, Appendix F form family).
//
//  Declarative adapter mapping a Flux `Slider` node to a `UISlider`
//  (unified tier; AGENTS.md §3.5).
//
//  Props are read by name; the index is the FNV-1a-32 digest of the name
//  masked to `u16` (`Props.propIndex`), derived identically on server and
//  client (AGENTS.md §3.2). Fields:
//  - `value: Float = 0.0` (controlled state)
//  - `onChange: Handler`
//  - `min: Float = 0.0`, `max: Float = 1.0`, `step: Float = 0.0`
//  - `enabled: Bool = true`
//
//  Dragging the thumb dispatches `onChange` with the new float as payload.

import UIKit

public final class SliderAdapter: FluxAdapter {
    public typealias View = UISlider
    weak var executor: (any FluxExecutor)?

    public init(executor: (any FluxExecutor)? = nil) { self.executor = executor }

    public func create() -> UISlider { UISlider() }

    public func update(_ view: UISlider, from old: Props, to new: Props) {
        // Audit D14: absent props retain previous values (no reset).
        if let min = new.getFloat(named: "min"), view.minimumValue != Float(min) {
            view.minimumValue = Float(min)
        }
        if let max = new.getFloat(named: "max"), view.maximumValue != Float(max) {
            view.maximumValue = Float(max)
        }
        // UIKit has no native step; record the requested step but keep value continuous.
        if let step = new.getFloat(named: "step") {
            view.fluxRecord(FluxRecordedProp.step, step)
        }
        if let value = new.getFloat(named: "value"), view.value != Float(value) {
            view.value = Float(value)
        }
        view.isEnabled = new.getBool(named: "enabled") ?? true
    }

    public func setChildren(_ children: [AnyObject], on view: UISlider) {}

    public func bindHandler(_ handlerId: FluxHandlerId, to view: UISlider, nodeId: FluxNodeId) {
        let target = HandlerTarget(executor: executor, handlerId: handlerId, nodeId: nodeId) { .float(Double(view.value)) }
        // Audit fix: remove any previously-registered UIAction on this view
        // before adding a new one. The dev runtime re-binds handlers on
        // hot-swap and on prop change; without this each rebind accumulates
        // another action and a single user tap dispatches N times.
        view.removeAllActions()
        view.addAction(UIAction { _ in target.fire() }, for: .valueChanged)
    }

    public func destroy(_ view: UISlider) {
        // Audit fix: `removeTarget(_:action:for:)` removes target/action pairs,
        // NOT `UIAction` registrations (those need `removeAllActions()`). The
        // previous call left stale actions alive and firing on recycled views.
        view.removeAllActions()
    }
}
