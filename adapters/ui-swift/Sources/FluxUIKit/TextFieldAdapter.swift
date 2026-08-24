//  TextFieldAdapter.swift
//  FluxUIKit — `TextField` → `UITextField` (Appendix F.5).

import UIKit

/// Dev adapter mapping a Flux `TextField` node to a `UITextField`.
///
/// Prop fields and their `PropIdx` (Appendix F.5 contract):
/// - `0 text: String = ""` (controlled value)
/// - `1 onChange: Handler`
/// - `2 placeholder: Option[String]`
/// - `3 ref: Option[Ref]` (unused in dev; the view itself is the ref)
/// - `4 enabled: Bool = true`
/// - `5 secure: Bool = false`
///
/// Editing changes dispatch `onChange` with the new text as the payload, so the
/// runtime's handler can write the bound signal. A `UITextFieldDelegate` relays
/// the edit. The field retains this adapter via object association so the
/// adapter outlives `create()`; the delegate holds the executor `weak`, so there
/// is no retain cycle back to the runtime.
public final class TextFieldAdapter: FluxAdapter {
    public typealias View = UITextField
    weak var executor: (any FluxExecutor)?
    /// Strongly retains the delegate; `UITextField.delegate` is weak, so without
    /// this the delegate would be released the moment `create()` returns.
    private var textDelegate: Delegate?

    public init(executor: (any FluxExecutor)? = nil) { self.executor = executor }

    public func create() -> UITextField {
        let field = UITextField()
        field.borderStyle = .roundedRect
        let delegate = Delegate()
        delegate.adapter = self
        field.delegate = delegate
        textDelegate = delegate
        // Keep this adapter alive as long as the field is alive.
        objc_setAssociatedObject(field, &Self.associationKey, self, .OBJC_ASSOCIATION_RETAIN_NONATOMIC)
        return field
    }

    public func update(_ view: UITextField, from old: Props, to new: Props) {
        if let text = new.getString(0), view.text != text { view.text = text }
        view.placeholder = new.getString(2)
        view.isEnabled = new.getBool(4) ?? true
        view.isSecureTextEntry = new.getBool(5) ?? false
    }

    public func setChildren(_ children: [AnyObject], on view: UITextField) {}

    public func bindHandler(_ handlerId: FluxHandlerId, to view: UITextField, nodeId: FluxNodeId) {
        (view.delegate as? Delegate)?.bind(handlerId: handlerId, nodeId: nodeId)
    }

    public func destroy(_ view: UITextField) {
        view.delegate = nil
        objc_setAssociatedObject(view, &Self.associationKey, nil, .OBJC_ASSOCIATION_RETAIN_NONATOMIC)
    }

    private static var associationKey: UInt8 = 0

    /// Forwards edits to the bound `onChange` handler via the weak executor.
    @MainActor
    final class Delegate: NSObject, UITextFieldDelegate {
        weak var adapter: TextFieldAdapter?
        var handlerId: FluxHandlerId?
        var nodeId: FluxNodeId?

        func bind(handlerId: FluxHandlerId, nodeId: FluxNodeId) {
            self.handlerId = handlerId
            self.nodeId = nodeId
        }

        func textFieldDidChangeSelection(_ textField: UITextField) {
            guard let handlerId, let nodeId, let text = textField.text else { return }
            MainActor.assumeIsolated {
                adapter?.executor?.dispatch(FluxEvent(handlerId: handlerId, nodeId: nodeId, payload: .str(text)))
            }
        }
    }
}
