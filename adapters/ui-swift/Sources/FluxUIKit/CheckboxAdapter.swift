//  CheckboxAdapter.swift
//  FluxUIKit — `Checkbox` → `UIButton` (checkmark) (FLUX-040, Appendix F form family).
//
//  Declarative adapter mapping a Flux `Checkbox` node to a `UIButton` rendered
//  as a checkbox (unified tier; AGENTS.md §3.5). UIKit has no native checkbox
//  control, so the conventional faithful mapping is a `UIButton` whose
//  `isSelected` drives a "✓"/empty glyph (the iOS idiom for a checkbox).
//
//  Props are read by name; the index is the FNV-1a-32 digest of the name
//  masked to `u16` (`Props.propIndex`), derived identically on server and
//  client (AGENTS.md §3.2). Fields:
//  - `value: Bool = false` (controlled state)
//  - `onChange: Handler`
//  - `label: Option[String]` (title rendered beside the box)
//  - `enabled: Bool = true`
//
//  Tapping toggles `isSelected` and dispatches `onChange` with the new boolean.

import UIKit

public final class CheckboxAdapter: FluxAdapter {
    public typealias View = UIButton
    weak var executor: (any FluxExecutor)?

    public init(executor: (any FluxExecutor)? = nil) { self.executor = executor }

    public func create() -> UIButton {
        let button = UIButton(type: .system)
        button.titleLabel?.font = .systemFont(ofSize: 17)
        return button
    }

    public func update(_ view: UIButton, from old: Props, to new: Props) {
        let value = new.getBool(named: "value") ?? false
        if view.isSelected != value { view.isSelected = value }
        applyGlyph(view)
        if let label = new.getString(named: "label") { view.setTitle(label, for: .normal) }
        view.isEnabled = new.getBool(named: "enabled") ?? true
    }

    public func setChildren(_ children: [AnyObject], on view: UIButton) {}

    public func bindHandler(_ handlerId: FluxHandlerId, to view: UIButton, nodeId: FluxNodeId) {
        let target = HandlerTarget(executor: executor, handlerId: handlerId, nodeId: nodeId) { .bool(view.isSelected) }
        view.addAction(UIAction { _ in target.fire() }, for: .touchUpInside)
    }

    public func destroy(_ view: UIButton) {
        view.removeTarget(nil, action: nil, for: .allEvents)
    }

    /// Renders the checkbox glyph from the current selected state.
    private func applyGlyph(_ view: UIButton) {
        let title = view.isSelected ? "☑︎" : "☐"
        view.setTitle(title + (view.title(for: .normal).map { " \($0)" } ?? ""), for: .normal)
    }
}
