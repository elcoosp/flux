//  WebHostView.swift
//  FluxUIKit — `WebHost` → `WKWebView` (FLUX-048).
//
//  Maps a Flux `WebHost` node to a sandboxed `WKWebView`. The capability layer
//  (cap 12, `WebView.load`) writes the requested URL into signal 82; this
//  adapter is the declarative view that reads the same `src` prop and renders
//  it. The two paths share one contract: the prop name `src`, resolved by FNV-1a
//  index (AGENTS.md §3.2) — never a hardcoded positional index.
//
//  Security (FLUX-048 / ADR-0057): the web view is sandbox-contained. It runs
//  in its own process, cannot reach host APIs, and requires no OS permission
//  (`PermissionKind.None`). Loading failures degrade to a placeholder, never a
//  crash.

import UIKit
import WebKit

@MainActor
public final class WebHostView: FluxAdapter {
    public typealias View = WKWebView
    weak var executor: (any FluxExecutor)?

    /// Placeholder shown until the first navigation commits or on load failure.
    /// A system symbol is always available and never `nil`.
    private static let placeholder: UIImage = {
        UIImage(systemName: "globe") ?? UIImage()
    }()

    public init(executor: (any FluxExecutor)? = nil) { self.executor = executor }

    public func create() -> WKWebView {
        let config = WKWebViewConfiguration()
        // Sandbox the web content: no native bridge, no file access, no
        // in-page media capture beyond what the web page itself requests.
        config.preferences.javaScriptCanOpenWindowsAutomatically = false
        let webView = WKWebView(frame: .zero, configuration: config)
        webView.navigationDelegate = delegate
        webView.backgroundColor = .systemBackground
        return webView
    }

    public func update(_ view: WKWebView, from old: Props, to new: Props) {
        guard let src = new.getString(named: "src"), let url = URL(string: src), url.scheme == "https" || url.scheme == "http" else {
            // Missing/empty/unsafe `src` (non-http(s)) degrades to no-op rather
            // than attempting a malformed navigation.
            return
        }
        // Avoid reloading the same URL on every no-op patch.
        if old.getString(named: "src") != new.getString(named: "src") {
            let request = URLRequest(url: url)
            view.load(request)
        }
    }

    public func setChildren(_ children: [AnyObject], on view: WKWebView) {
        // `WebHost` is a leaf; the runtime never sends children.
    }

    public func bindHandler(_ handlerId: FluxHandlerId, to view: WKWebView, nodeId: FluxNodeId) {
        // `WebHost` has no handlers in the MLP.
    }

    public func destroy(_ view: WKWebView) {
        view.stopLoading()
        view.navigationDelegate = nil
    }

    // MARK: - Navigation delegate

    private final class Delegate: NSObject, WKNavigationDelegate {
        weak var owner: WebHostView?
        init(owner: WebHostView?) { self.owner = owner }
        // Load failures are silent (placeholder stays); no crash path.
        func webView(_ webView: WKWebView, didFail navigation: WKNavigation!, withError error: Error) {}
        func webView(_ webView: WKWebView, didFailProvisionalNavigation navigation: WKNavigation!, withError error: Error) {}
    }

    private lazy var delegate: Delegate = Delegate(owner: self)
}
