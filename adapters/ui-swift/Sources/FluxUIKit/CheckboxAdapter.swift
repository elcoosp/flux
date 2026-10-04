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
    /// Stable identifier for the UIAction this adapter registers
    /// in `bindHandler`. `UIControl` has no `removeAllActions()`;
    /// the correct pattern is to remove by identifier before
    /// re-adding, so rebinds replace (not accumulate) the handler.
    private static let fluxHandlerAction =
        UIAction.Identifier("flux.handler")

    public typealias View = UIButton
    weak var executor: (any FluxExecutor)?

    public init(executor: (any FluxExecutor)? = nil) { self.executor = executor }

    public func create() -> UIButton {
        let button = UIButton(type: .system)
        button.titleLabel?.font = .systemFont(ofSize: 17)
        return button
    }

    public func update(_ view: UIButton, from old: Props, to new: Props) {
        // Audit D14: absent prop retains previous value (no reset).
        if let value = new.getBool(named: "value"), view.isSelected != value {
            view.isSelected = value
        }
        applyGlyph(view)
        if let label = new.getString(named: "label") { view.setTitle(label, for: .normal) }
        view.isEnabled = new.getBool(named: "enabled") ?? true
    }

    public func setChildren(_ children: [AnyObject], on view: UIButton) {}

    public func bindHandler(_ handlerId: FluxHandlerId, to view: UIButton, nodeId: FluxNodeId) {
        // Audit fix: UIKit does NOT auto-toggle `isSelected` on
        // `touchUpInside`, so the old payload closure read the PRE-tap
        // boolean — every handler saw a stale value. The action below toggles
        // first (matching every sibling form adapter's contract) and only
        // then fires, so this payload reads the post-tap state.
        let target = HandlerTarget(executor: executor, handlerId: handlerId, nodeId: nodeId) { [weak view] in
            .bool(view?.isSelected ?? false)
        }
        // Audit fix: remove any previously-registered UIAction on this view
        // before adding a new one. The dev runtime re-binds handlers on
        // hot-swap and on prop change; without this each rebind accumulates
        // another action and a single user tap dispatches N times.
        view.removeAction(identifiedBy: Self.fluxHandlerAction, for: .touchUpInside)
        view.addAction(UIAction { [weak view, weak self] _ in
            guard let view else { return }
            // Toggle the selected state, refresh the glyph, THEN fire — so the
            // payload closure above observes the new boolean.
            view.isSelected.toggle()
            self?.applyGlyph(view)
            target.fire()
        }, for: .touchUpInside)
    }

    public func destroy(_ view: UIButton) {
        // Audit fix: `removeTarget(_:action:for:)` removes target/action pairs,
        // NOT `UIAction` registrations (those need `removeAllActions()`). The
        // previous call left stale actions alive and firing on recycled views.
        view.removeAction(identifiedBy: Self.fluxHandlerAction, for: .touchUpInside)
    }

    /// Renders the checkbox glyph from the current selected state.
    private func applyGlyph(_ view: UIButton) {
        let title = view.isSelected ? "☑︎" : "☐"
        view.setTitle(title + (view.title(for: .normal).map { " \($0)" } ?? ""), for: .normal)
    }
}
