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
        let min = Float(new.getFloat(named: "min") ?? 0.0)
        let max = Float(new.getFloat(named: "max") ?? 1.0)
        if view.minimumValue != min { view.minimumValue = min }
        if view.maximumValue != max { view.maximumValue = max }
        let step = new.getFloat(named: "step") ?? 0.0
        // UIKit has no native step; we keep the value continuous and record the
        // requested step so a future quantized binding can snap. `value` is the
        // controlled position and is only pushed when it differs to avoid loops.
        _ = step
        if let value = new.getFloat(named: "value"), view.value != Float(value) {
            view.value = Float(value)
        }
        view.isEnabled = new.getBool(named: "enabled") ?? true
    }

    public func setChildren(_ children: [AnyObject], on view: UISlider) {}

    public func bindHandler(_ handlerId: FluxHandlerId, to view: UISlider, nodeId: FluxNodeId) {
        let target = HandlerTarget(executor: executor, handlerId: handlerId, nodeId: nodeId) { .float(Double(view.value)) }
        view.addAction(UIAction { _ in target.fire() }, for: .valueChanged)
    }

    public func destroy(_ view: UISlider) {
        view.removeTarget(nil, action: nil, for: .allEvents)
    }
}
