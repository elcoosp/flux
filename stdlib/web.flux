// web.flux — `WebHost` adapter compo (FLUX-0xx).
//
// Embeds an external web URL. The `src` prop carries the URL string.
//
// Native rendering: SwiftUI `WebView` (WKWebView via UIViewRepresentable) /
// Compose `WebView` (release); the dev host maps the same node kind through
// `WebHostAdapter`.

compo WebHost(
  src: String,
)
  // Adapter leaf — native rendering defined by FLUX-0xx.
