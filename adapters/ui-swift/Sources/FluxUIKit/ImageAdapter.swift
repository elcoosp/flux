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

    /// Generation token for in-flight loads. Incremented on every update (and
    /// on destroy) so a stale load that completes after a newer update can
    /// discard its result instead of overwriting the current image. The old
    /// code spawned an unstructured `Task` per update with no cancellation,
    /// so rapid prop changes could leave an older fetch as the final image.
    private var generation: Int = 0
    /// The two optional Auto Layout size constraints installed for the
    /// `width`/`height` props. Stored so update mutates `constant` on change
    /// rather than accumulating a fresh constraint per call.
    private var widthConstraint: NSLayoutConstraint?
    private var heightConstraint: NSLayoutConstraint?

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
        // Audit fix: previously wrote `view.frame.size`, which is a no-op
        // under Auto Layout (every adapter sets `translatesAutoresizingMaskIntoConstraints = false`).
        // Store two real Auto Layout constraints and mutate their constants so
        // the props are honest and no constraints accumulate.
        if let width = new.getFloat(named: "width") {
            if let c = widthConstraint {
                c.constant = CGFloat(width)
            } else {
                let c = view.widthAnchor.constraint(equalToConstant: CGFloat(width))
                c.isActive = true
                widthConstraint = c
            }
        }
        if let height = new.getFloat(named: "height") {
            if let c = heightConstraint {
                c.constant = CGFloat(height)
            } else {
                let c = view.heightAnchor.constraint(equalToConstant: CGFloat(height))
                c.isActive = true
                heightConstraint = c
            }
        }
        if let mode = new.getString(named: "resizeMode") {
            view.contentMode = Self.contentMode(for: mode)
        }
        // Invalidate any in-flight load first, then either clear (no src) or
        // start a fresh load carrying the new generation.
        generation &+= 1
        guard let src = new.getString(named: "source"), !src.isEmpty else {
            // Missing/empty `source` is treated as a load failure up front: show
            // the placeholder. This is the graceful-degrade path for BR-003.
            view.image = Self.placeholder
            return
        }
        let gen = generation
        Task { @MainActor [weak self, weak view] in
            guard let self, let view else { return }
            await self.load(src, onto: view, generation: gen)
        }
    }

    public func setChildren(_ children: [AnyObject], on view: UIImageView) {
        // `Image` is a leaf; the runtime never sends children.
    }

    public func bindHandler(_ handlerId: FluxHandlerId, to view: UIImageView, nodeId: FluxNodeId) {
        // `Image` has no handlers.
    }

    public func destroy(_ view: UIImageView) {
        // Invalidate any in-flight load so a completion after destroy cannot
        // touch a recycled view. Deactivate stored size constraints so a
        // reused view does not keep stale dimensions.
        generation &+= 1
        widthConstraint?.isActive = false
        heightConstraint?.isActive = false
        widthConstraint = nil
        heightConstraint = nil
        view.image = Self.placeholder
    }

    /// Fetches `src` through the shared [ImageCache] (disk + memory, single-flight)
    /// and swaps the decoded bitmap onto `view`, falling back to the placeholder on
    /// any failure. The cache coalesces concurrent same-URL loads and serves
    /// repeats from `URLCache` without a network round-trip. The cache actor is
    /// `await`ed off the main actor; the completion re-dispatches to the main
    /// actor before touching the view (UIKit requirement).
    private func load(_ src: String, onto view: UIImageView, generation gen: Int) async {
        let url: URL
        do {
            url = try await Self.cache.resolveURL(src, assetBase: Self.assetBaseURL.absoluteString)
        } catch {
            if gen == generation { view.image = Self.placeholder }
            return
        }
        let result = await Self.cache.get(url)
        // Audit fix: gate on the generation captured at dispatch time. A newer
        // update between the fetch start and completion invalidates this load,
        // so the stale bitmap never overwrites the newer image.
        guard gen == generation else { return }
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
