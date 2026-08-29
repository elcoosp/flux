// image.flux — `Image` adapter compo (Appendix F.8).
//
// Renders a bitmap from a source path. Props follow the Appendix F.8 contract
// exactly, aligned to the React Native `Image` surface:
//   source     String          required — asset path relative to the project
//                                  root, e.g. `"assets/logo.png"`.
//   width      Option[Float]   explicit width, defaults to None.
//   height     Option[Float]   explicit height, defaults to None.
//   resizeMode Option[String]  `"fill"` (default) | `"fit"` | `"stretch"`.
//
// The `= None` defaults encode Appendix F.8's optional props. `prop_decl`
// carries an optional `"=" expr` default (Appendix B.2).
//
// Native rendering is defined by Appendix F.8 (UIImageView / ImageView in dev
// mode; SwiftUI `Image` / Compose `Image` in release). In dev the bitmap is
// fetched over HTTP from the dev server's asset route.

compo Image(
  source: String,
  width: Option[Float] = None,
  height: Option[Float] = None,
  resizeMode: Option[String] = None,
)
  // Adapter leaf — native rendering defined by Appendix F.8.
