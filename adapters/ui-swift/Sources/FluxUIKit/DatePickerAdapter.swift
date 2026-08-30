//  DatePickerAdapter.swift
//  FluxUIKit — `DatePicker` → `UIDatePicker` (.date) (FLUX-040, Appendix F form family).
//
//  Declarative adapter mapping a Flux `DatePicker` node to a `UIDatePicker`
//  (unified tier; AGENTS.md §3.5).
//
//  Props are read by name; the index is the FNV-1a-32 digest of the name
//  masked to `u16` (`Props.propIndex`), derived identically on server and
//  client (AGENTS.md §3.2). Fields:
//  - `value: Int = 0` (controlled epoch-millis)
//  - `onChange: Handler`
//  - `min: Int = 0`, `max: Int = 0` (epoch-millis bounds; 0 = unset)
//  - `enabled: Bool = true`
//
//  Confirming a date dispatches `onChange` with the new epoch-millis as payload.

import UIKit

public final class DatePickerAdapter: FluxAdapter {
    public typealias View = UIDatePicker
    weak var executor: (any FluxExecutor)?

    public init(executor: (any FluxExecutor)? = nil) { self.executor = executor }

    public func create() -> UIDatePicker {
        let picker = UIDatePicker()
        picker.datePickerMode = .date
        if #available(iOS 13.4, *) { picker.preferredDatePickerStyle = .wheels }
        return picker
    }

    public func update(_ view: UIDatePicker, from old: Props, to new: Props) {
        if let value = new.getInt(named: "value") {
            let date = Date(timeIntervalSince1970: TimeInterval(value) / 1000)
            if view.date != date { view.date = date }
        }
        if let min = new.getInt(named: "min"), min > 0 {
            view.minimumDate = Date(timeIntervalSince1970: TimeInterval(min) / 1000)
        }
        if let max = new.getInt(named: "max"), max > 0 {
            view.maximumDate = Date(timeIntervalSince1970: TimeInterval(max) / 1000)
        }
        view.isEnabled = new.getBool(named: "enabled") ?? true
    }

    public func setChildren(_ children: [AnyObject], on view: UIDatePicker) {}

    public func bindHandler(_ handlerId: FluxHandlerId, to view: UIDatePicker, nodeId: FluxNodeId) {
        let target = HandlerTarget(executor: executor, handlerId: handlerId, nodeId: nodeId) {
            .int(Int64(view.date.timeIntervalSince1970 * 1000))
        }
        view.addAction(UIAction { _ in target.fire() }, for: .valueChanged)
    }

    public func destroy(_ view: UIDatePicker) {
        view.removeTarget(nil, action: nil, for: .allEvents)
    }
}
