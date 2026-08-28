//  ButtonAdapter.swift
//  FluxUIKit — `Button` → `UIButton` (Appendix F.2).

import UIKit

/// Declarative adapter mapping a Flux `Button` node to a `UIButton`
/// (unified tier; AGENTS.md §3.5).
///
/// Props are read by name; the index is the FNV-1a-32 digest of the name
/// masked to `u16` (`Props.propIndex`), derived identically on server and
/// client (AGENTS.md §3.2) — never a hardcoded positional index. Fields:
/// - `text: String` (required)
/// - `onClick: Handler` (required)
/// - `enabled: Bool = true`
/// - `color: Option[Color]`
///
/// Tapping the button dispatches `onClick` via the weak executor. The target
/// is held only by the button's action, which cannot resurrect a deallocated
/// runtime because it keeps the executor `weak`.
public final class ButtonAdapter: FluxAdapter {
    public typealias View = UIButton
    weak var executor: (any FluxExecutor)?

    public init(executor: (any FluxExecutor)? = nil) { self.executor = executor }

    public func create() -> UIButton {
        UIButton(type: .system)
    }

    public func update(_ view: UIButton, from old: Props, to new: Props) {
        let title = new.getString(named: "text") ?? old.getString(named: "text") ?? ""
        view.setTitle(title, for: .normal)
        view.isEnabled = new.getBool(named: "enabled") ?? true
        if let color = new.getColor(named: "color") { view.setTitleColor(color.uiColor, for: .normal) }
    }

    public func setChildren(_ children: [AnyObject], on view: UIButton) {}

    public func bindHandler(_ handlerId: FluxHandlerId, to view: UIButton, nodeId: FluxNodeId) {
        let target = HandlerTarget(executor: executor, handlerId: handlerId, nodeId: nodeId) { nil }
        view.addAction(UIAction { _ in target.fire() }, for: .touchUpInside)
    }

    public func destroy(_ view: UIButton) {
        view.removeTarget(nil, action: nil, for: .allEvents)
    }
}
