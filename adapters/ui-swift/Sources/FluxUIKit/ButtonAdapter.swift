//  ButtonAdapter.swift
//  FluxUIKit — `Button` → `UIButton` (Appendix F.2).

import UIKit

/// Dev adapter mapping a Flux `Button` node to a `UIButton`.
///
/// Prop fields and their `PropIdx` (Appendix F.2 contract):
/// - `0 text: String` (required)
/// - `1 onClick: Handler` (required)
/// - `2 enabled: Bool = true`
/// - `3 color: Option[Color]`
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
        let title = new.getString(0) ?? old.getString(0) ?? ""
        view.setTitle(title, for: .normal)
        view.isEnabled = new.getBool(2) ?? true
        if let color = new.getColor(3) { view.setTitleColor(color.uiColor, for: .normal) }
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
