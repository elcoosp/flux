//  TextAreaAdapter.swift
//  FluxUIKit — `TextArea` → `UITextView` (FLUX-040, Appendix F form family).
//
//  Declarative adapter mapping a Flux `TextArea` node to a `UITextView`
//  (unified tier; AGENTS.md §3.5).
//
//  Props are read by name; the index is the FNV-1a-32 digest of the name
//  masked to `u16` (`Props.propIndex`), derived identically on server and
//  client (AGENTS.md §3.2). Fields:
//  - `value: String = ""` (controlled text)
//  - `onChange: Handler`
//  - `placeholder: Option[String]` (shown as a fading hint when empty)
//  - `maxLines: Option[Int]` (soft cap on scrollable height)
//  - `enabled: Bool = true`
//
//  Editing dispatches `onChange` with the new string as payload, mirroring the
//  `TextInput` contract. The delegate is retained via object association
//  (like `TextInputAdapter`) because `UITextView.delegate` is `weak`.

import UIKit

public final class TextAreaAdapter: FluxAdapter {
    public typealias View = UITextView
    weak var executor: (any FluxExecutor)?
    private var textDelegate: Delegate?

    public init(executor: (any FluxExecutor)? = nil) { self.executor = executor }

    public func create() -> UITextView {
        let view = UITextView()
        view.font = .systemFont(ofSize: 17)
        view.layer.borderWidth = 1
        view.layer.borderColor = UIColor.separator.cgColor
        view.layer.cornerRadius = 6
        let delegate = Delegate()
        delegate.adapter = self
        view.delegate = delegate
        textDelegate = delegate
        objc_setAssociatedObject(view, &Self.associationKey, self, .OBJC_ASSOCIATION_RETAIN_NONATOMIC)
        return view
    }

    public func update(_ view: UITextView, from old: Props, to new: Props) {
        if let text = new.getString(named: "value"), view.text != text { view.text = text }
        if let placeholder = new.getString(named: "placeholder") { view.text = view.text.isEmpty ? placeholder : view.text }
        if let maxLines = new.getInt(named: "maxLines"), maxLines > 0 {
            // Soft cap: bound the intrinsic height to ~`maxLines` of text.
            let lineHeight = view.font?.lineHeight ?? 20
            view.heightAnchor.constraint(lessThanOrEqualToConstant: CGFloat(maxLines) * lineHeight).isActive = true
        }
        view.isEditable = new.getBool(named: "enabled") ?? true
    }

    public func setChildren(_ children: [AnyObject], on view: UITextView) {}

    public func bindHandler(_ handlerId: FluxHandlerId, to view: UITextView, nodeId: FluxNodeId) {
        (view.delegate as? Delegate)?.bind(handlerId: handlerId, nodeId: nodeId)
    }

    public func destroy(_ view: UITextView) {
        view.delegate = nil
        objc_setAssociatedObject(view, &Self.associationKey, nil, .OBJC_ASSOCIATION_RETAIN_NONATOMIC)
    }

    private static var associationKey: UInt8 = 0

    /// Forwards edits to the bound `onChange` handler via the weak executor.
    @MainActor
    final class Delegate: NSObject, UITextViewDelegate {
        weak var adapter: TextAreaAdapter?
        var handlerId: FluxHandlerId?
        var nodeId: FluxNodeId?

        func bind(handlerId: FluxHandlerId, nodeId: FluxNodeId) {
            self.handlerId = handlerId
            self.nodeId = nodeId
        }

        func textViewDidChange(_ textView: UITextView) {
            guard let handlerId, let nodeId, let text = textView.text else { return }
            MainActor.assumeIsolated {
                adapter?.executor?.dispatch(FluxEvent(handlerId: handlerId, nodeId: nodeId, payload: .str(text)))
            }
        }
    }
}
