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

    /// Shared host-side image cache (FLUX-039): `URLCache` (disk + memory) plus
    /// single-flight fetch. One instance per adapter is unnecessary — a single
    /// app-wide cache keeps every `Image` node from re-fetching the same asset.
    static let cache = ImageCache.shared

    /// Placeholder shown until the bitmap arrives or when loading fails. A
    /// system symbol is used so it is always available and never `nil`.
    private static let placeholder: UIImage = {
        UIImage(systemName: "photo") ?? UIImage()
    }()

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
            // the placeholder. This is the graceful-degrade path for BR-003.
            view.image = Self.placeholder
            return
        }
        Task { @MainActor in
            await self.load(src, onto: view)
        }
    }

    public func setChildren(_ children: [AnyObject], on view: UIImageView) {
        // `Image` is a leaf; the runtime never sends children.
    }

    public func bindHandler(_ handlerId: FluxHandlerId, to view: UIImageView, nodeId: FluxNodeId) {
        // `Image` has no handlers.
    }

    public func destroy(_ view: UIImageView) {
        view.image = Self.placeholder
    }

    /// Fetches `src` through the shared [ImageCache] (disk + memory, single-flight)
    /// and swaps the decoded bitmap onto `view`, falling back to the placeholder on
    /// any failure. The cache coalesces concurrent same-URL loads and serves
    /// repeats from `URLCache` without a network round-trip. The cache actor is
    /// `await`ed off the main actor; the completion re-dispatches to the main
    /// actor before touching the view (UIKit requirement).
    private func load(_ src: String, onto view: UIImageView) async {
        let url: URL
        do {
            url = try await Self.cache.resolveURL(src, assetBase: Self.assetBaseURL.absoluteString)
        } catch {
            view.image = Self.placeholder
            return
        }
        let result = await Self.cache.get(url)
        // Re-check in-flight identity is unnecessary: the cache is authoritative
        // and a recycled view simply shows the latest resolved image.
        switch result {
        case .success(let data):
            guard let image = UIImage(data: data) else {
                view.image = Self.placeholder
                return
            }
            view.image = image
        case .failure:
            view.image = Self.placeholder
        }
    }

    /// Maps the `contentMode` prop string to a `UIView.ContentMode`.
    private static func contentMode(for mode: String) -> UIView.ContentMode {
        switch mode {
        case "fit": .scaleAspectFit
        case "stretch": .scaleToFill
        default: .scaleAspectFill
        }
    }
}
