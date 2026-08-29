// dialog.flux — `Dialog` adapter compo (FLUX-038).
//
// Modal dialog with a dimmed scrim, presented above the current scene. Overlay
// container: `content` is its children. The presentation is data the host
// consumes (iOS `Alert` / Compose `AlertDialog`); never a wire animation frame
// (ADR-0047 + FLUX-038).
//
// Props follow the PRD-N overlay-family contract:
//   onDismiss  Handler        invoked when the host presentation is dismissed
//
// Host adapter wiring (native surface + transition) is gated on the ADR-0048
// iOS dev-tier convergence decision; the type-checker and codegen can name and
// emit the primitive today.

compo Dialog(
  onDismiss: Handler = fn() {},
)
  // Overlay container — `content` children supplied by callers.
