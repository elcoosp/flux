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
        #if DEBUG
        NSLog("[FluxRT] ButtonAdapter.bindHandler handlerId=\(handlerId) nodeId=\(nodeId) executor=\(executor != nil ? "set" : "NIL")")
        // Deep-dive probe: record the button's real window-space hit frame and
        // what `hitTest` at its center actually returns, so we can tell whether
        // a real tap reaches the button or is intercepted by an ancestor.
        DispatchQueue.main.asyncAfter(deadline: .now() + 1.0) {
            let f = view.frame
            let abs = view.convert(view.bounds, to: nil)
            let center = CGPoint(x: abs.midX, y: abs.midY)
            let hit = view.window?.hitTest(center, with: nil)
            let hitDesc = String(describing: hit.map { type(of: $0) })
            let selfIsHit = hit === view
            let line = "[frame] handlerId=\(handlerId) nodeId=\(nodeId) frame=\(f) absWin=\(abs) enabled=\(view.isEnabled) userInt=\(view.isUserInteractionEnabled) superview=\(type(of: view.superview)) hitAtCenter=\(hitDesc) selfIsHit=\(selfIsHit) at \(Date())\n"
            NSLog("[FluxRT] ButtonAdapter frame: \(line)")
            UserDefaults.standard.set(line, forKey: "flux_frame")
            let tmp = NSTemporaryDirectory() + "flux_frame.log"
            try? line.write(to: URL(fileURLWithPath: tmp), atomically: true, encoding: .utf8)
        }
        #endif
        view.addAction(UIAction { _ in target.fire() }, for: .touchUpInside)
    }

    public func destroy(_ view: UIButton) {
        view.removeTarget(nil, action: nil, for: .allEvents)
    }
}
