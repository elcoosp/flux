//  TextAdapter.swift
//  FluxUIKit — `Text` → `UILabel` (Appendix F.1).

import UIKit

/// Dev adapter mapping a Flux `Text` node to a `UILabel`.
///
/// Prop fields and their `PropIdx` (Appendix F.1 contract):
/// - `0 text: String` (required)
/// - `1 font: Option[Font]`
/// - `2 size: Option[Float]`
/// - `3 color: Option[Color]`
/// - `4 alignment: Option[Alignment]`
/// - `5 max_lines: Option[Int]`
/// - `6 overflow: Option[Overflow]`
public final class TextAdapter: FluxAdapter {
    public typealias View = UILabel
    weak var executor: (any FluxExecutor)?

    public init(executor: (any FluxExecutor)? = nil) { self.executor = executor }

    public func create() -> UILabel {
        let label = UILabel()
        label.numberOfLines = 0
        return label
    }

    public func update(_ view: UILabel, from old: Props, to new: Props) {
        view.text = new.getString(named: "text") ?? old.getString(named: "text")
        applyFont(to: view, props: new)
        if let color = new.getColor(named: "color") { view.textColor = color.uiColor }
        if let align = new.getRecord(named: "alignment").flatMap(FluxAlignment.init(record:)) {
            view.textAlignment = align.textAlignment
        }
        if let maxLines = new.getInt(named: "max_lines") { view.numberOfLines = Int(maxLines) }
    }

    public func setChildren(_ children: [AnyObject], on view: UILabel) {
        // `Text` is a leaf; the runtime never sends children.
    }

    public func bindHandler(_ handlerId: FluxHandlerId, to view: UILabel, nodeId: FluxNodeId) {
        // `Text` has no handlers.
    }

    public func destroy(_ view: UILabel) {}

    private func applyFont(to view: UILabel, props: Props) {
        if let font = props.getFont(1) {
            view.font = font.uiFont
        } else if let size = props.getFloat(2) {
            view.font = UIFont.systemFont(ofSize: CGFloat(size))
        } else if view.font == nil {
            view.font = UIFont.systemFont(ofSize: 14)
        }
    }
}
