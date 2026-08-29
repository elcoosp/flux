//  ErrorOverlay.swift
//  FLUX-028 (LANE-O, Phase 3) — native on-device error overlay (PRD-K FluxError + Span).
//
//  On a `FluxError` in DEV mode, renders a native (non-webview) screen with the
//  message, the highlighted `.flux` source span (file:line via the SourceMap),
//  and a formatted dispatch stack. Per AGENTS.md Appendix E §E.6 it is a native
//  UIView, never a webview, and never a crash. Guarded by `#if DEBUG` so there is
//  zero release impact.
//
//  ADR-0049 does not apply (these are new iOS-native types).

import UIKit

#if DEBUG
/// A native dev-mode error screen. Presented over the current window when the
/// host catches a `FluxError` (VM/Wire/Runtime variant) during a dev session.
@MainActor
public final class ErrorOverlayView: UIView {
    private let messageLabel = UILabel()
    private let spanLabel = UILabel()
    private let stackView = UITextView()

    public override init(frame: CGRect) {
        super.init(frame: frame)
        configure()
    }

    public required init?(coder: NSCoder) {
        super.init(coder: coder)
        configure()
    }

    private func configure() {
        backgroundColor = UIColor.systemRed.withAlphaComponent(0.92)
        layer.cornerRadius = 12
        clipsToBounds = true

        messageLabel.textColor = .white
        messageLabel.font = .boldSystemFont(ofSize: 16)
        messageLabel.numberOfLines = 0

        spanLabel.textColor = .white
        spanLabel.font = .monospacedSystemFont(ofSize: 12, weight: .regular)
        spanLabel.numberOfLines = 0

        stackView.textColor = .white
        stackView.font = .monospacedSystemFont(ofSize: 11, weight: .regular)
        stackView.backgroundColor = .clear
        stackView.isEditable = false

        let stack = UIStackView(arrangedSubviews: [messageLabel, spanLabel, stackView])
        stack.axis = .vertical
        stack.spacing = 8
        stack.translatesAutoresizingMaskIntoConstraints = false
        addSubview(stack)
        NSLayoutConstraint.activate([
            stack.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 16),
            stack.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -16),
            stack.topAnchor.constraint(equalTo: topAnchor, constant: 16),
            stack.bottomAnchor.constraint(equalTo: bottomAnchor, constant: -16),
        ])
    }

    /// Presents the overlay for a `FluxError`, highlighting the source span and
    /// rendering the dispatch stack. Safe to call repeatedly; it only updates
    /// content (never throws, never crashes).
    public func show(_ error: FluxError, fileResolver: (UInt32) -> String) {
        messageLabel.text = error.message
        if let span = error.span {
            let file = fileResolver(span.fileID)
            spanLabel.text = "\(file):\(span.line):\(span.column)"
        } else {
            spanLabel.text = "span: <unknown>"
        }
        stackView.text = error.callSites.joined(separator: "\n")
    }
}
#endif

#if DEBUG
/// Present `error` on the key window's overlay layer (dev only). No-op in release.
@MainActor
public func presentFluxError(_ error: FluxError, fileResolver: @escaping (UInt32) -> String) {
    guard let window = UIApplication.shared
        .connectedScenes
        .compactMap({ $0 as? UIWindowScene })
        .first?.windows
        .first(where: { $0.isKeyWindow }) else { return }
    let overlay = ErrorOverlayView(frame: window.bounds.insetBy(dx: 24, dy: 80))
    overlay.show(error, fileResolver: fileResolver)
    window.addSubview(overlay)
}
#endif
