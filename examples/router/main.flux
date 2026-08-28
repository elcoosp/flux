// main.flux — Flux router example (standalone vertical slice).
//
// Demonstrates a capability-driven navigation stack: `Router.navigate(target)`
// writes its argument record to signal 97 (ADR-0045), and both host reconcilers
// (iOS / Android) present only the child `Screen` whose `route` prop equals the
// active route. Each `Screen` is addressed by its `route` prop (Appendix F.7) —
// the named `route:` form is required so the lowered IR carries a `route` prop
// the host's FNV-1a lookup can read; the positional `Screen("home")` form lowers
// to a positional prop index and would not match.
//
// `flux dev` serves this app over WebSocket; `flux build --platform ios|android`
// lowers it for release. It is the canonical Router example exercised by the
// `examples/router` full-pipeline and parity tests.

compo App
    Column {
        // Root navigation container; the visible screen is driven by signal 97.
        Router {
            Screen(route: "home") {
                Column(gap: 16) {
                    Text(text: "Home")
                    Button(text: "Go to Settings", onClick: {
                        Router.navigate("settings")
                    })
                }
            }
            Screen(route: "settings") {
                Column(gap: 16) {
                    Text(text: "Settings")
                    Button(text: "Go to Home", onClick: {
                        Router.navigate("home")
                    })
                }
            }
        }
    }
