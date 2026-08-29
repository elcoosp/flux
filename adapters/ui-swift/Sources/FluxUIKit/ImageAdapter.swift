//  ImageAdapter.swift
//  FluxUIKit — `Image` → `UIImageView` (Appendix F.8).

import UIKit

/// Declarative adapter mapping a Flux `Image` node to a `UIImageView`
/// (unified tier; AGENTS.md §3.5).
///
/// Props are read by name; the index is the FNV-1a-32 digest of the name
/// masked to `u16` (`Props.propIndex`), derived identically on server and
/// client (AGENTS.md §3.2) — never a hardcoded positional index. Fields:
/// - `source: String` (required) — asset path relative to the project root,
///   e.g. `"assets/logo.png"`.
/// - `width: Option[Float]`
/// - `height: Option[Float]`
/// - `resizeMode: Option[String]` — `"fill"` (default), `"fit"`, `"stretch"`.
///
/// In dev the bitmap is fetched over HTTP from the dev server's asset route
/// (`http://localhost:7332/assets/<src>`). Load failures (missing asset,
/// offline server, decode error) degrade to a `photo` system placeholder
/// rather than crashing the host — see BR-003. The image view is configured on
/// the main actor because UIKit views may only be touched from the main
/// thread; the network callback re-dispatches to the main actor before
/// mutating the view.
@MainActor
public final class ImageAdapter: FluxAdapter {
    public typealias View = UIImageView
    weak var executor: (any FluxExecutor)?

    /// The dev-server asset base URL (FLUX-019). `<src>` is appended verbatim,
    /// so a node with `src = "assets/logo.png"` resolves to
    /// `…/assets/assets/logo.png`, which the server joins onto the project
    /// root.
    static let assetBaseURL = URL(string: "http://localhost:7332/assets/")!

    /// Placeholder shown until the bitmap arrives or when loading fails. A
    /// system symbol is used so it is always available and never `nil`.
    private static let placeholder: UIImage = {
        UIImage(systemName: "photo") ?? UIImage()
    }()

    /// The in-flight data task for the current `src`, if any. Cancelled when a
    /// new `src` arrives or the view is destroyed so a stale response can never
    /// land on a recycled image view.
    private var loadTask: URLSessionDataTask?

    public init(executor: (any FluxExecutor)? = nil) { self.executor = executor }

    public func create() -> UIImageView {
        let imageView = UIImageView()
        imageView.contentMode = .scaleAspectFill
        imageView.clipsToBounds = true
        imageView.image = Self.placeholder
        return imageView
    }

    public func update(_ view: UIImageView, from old: Props, to new: Props) {
        if let width = new.getFloat(named: "width"), let height = new.getFloat(named: "height") {
            view.frame.size = CGSize(width: CGFloat(width), height: CGFloat(height))
        }
        if let mode = new.getString(named: "resizeMode") {
            view.contentMode = Self.contentMode(for: mode)
        }
        guard let src = new.getString(named: "source"), !src.isEmpty else {
            // Missing/empty `source` is treated as a load failure up front: show
            // the placeholder and clear any pending request. This is the
            // graceful-degrade path for BR-003.
            loadTask?.cancel()
            loadTask = nil
            view.image = Self.placeholder
            return
        }
        load(src, onto: view)
    }

    public func setChildren(_ children: [AnyObject], on view: UIImageView) {
        // `Image` is a leaf; the runtime never sends children.
    }

    public func bindHandler(_ handlerId: FluxHandlerId, to view: UIImageView, nodeId: FluxNodeId) {
        // `Image` has no handlers.
    }

    public func destroy(_ view: UIImageView) {
        loadTask?.cancel()
        loadTask = nil
    }

    /// Fetches `src` from the dev asset server and swaps it onto `view`,
    /// falling back to the placeholder on any failure. The data task runs off
    /// the main actor; the completion re-dispatches to the main actor before
    /// touching the view (UIKit requirement).
    private func load(_ src: String, onto view: UIImageView) {
        let url = Self.assetBaseURL.appending(path: src)
        var request = URLRequest(url: url)
        request.timeoutInterval = Self.loadTimeout
        loadTask?.cancel()
        let task = URLSession.shared.dataTask(with: request) { data, response, error in
            // The completion runs on a background session queue, NOT the main
            // actor. Hop to the main actor to mutate the view — `assumeIsolated`
            // would trap here because we are not isolated (it crashed the app
            // on any screen that renders an `Image`, e.g. the About screen).
            Task { @MainActor in
                defer { self.loadTask = nil }
                if error != nil {
                    view.image = Self.placeholder
                    return
                }
                guard let data, let image = UIImage(data: data) else {
                    view.image = Self.placeholder
                    return
                }
                view.image = image
            }
        }
        loadTask = task
        task.resume()
    }

    /// Maps the `contentMode` prop string to a `UIView.ContentMode`.
    private static func contentMode(for mode: String) -> UIView.ContentMode {
        switch mode {
        case "fit": .scaleAspectFit
        case "stretch": .scaleToFill
        default: .scaleAspectFill
        }
    }

    /// Network timeout for an asset fetch, in seconds. The dev server is local,
    /// so a slow response indicates a real problem rather than latent latency.
    private static let loadTimeout: TimeInterval = 5
}
