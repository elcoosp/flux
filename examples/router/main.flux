// main.flux — Flux router example (standalone vertical slice).
//
// Demonstrates a capability-driven navigation stack: `Router.navigate(target)`
// writes its argument record to signal 97 (ADR-0045), and both host reconcilers
// (iOS / Android) present only the child `Screen` whose `route` prop equals the
// active route. Each `Screen` is addressed by its `route` prop (Appendix F.7),
// which the host reads via `FNV-1a("route")` — so the prop name MUST be `route`.
//
// `flux dev` serves this app over WebSocket; `flux build --platform ios|android`
// lowers it for release. The new indentation-delimited syntax requires a view
// call to carry at least one named prop to own an indented child block, which is
// why `Router` declares an `initial` route and each `Screen` carries `route`.

compo App
    Column gap: 8.0
        // Root navigation container; the visible screen is driven by signal 97.
        Router initialRouteName: "home"
            Screen route: "home"
                Column gap: 16.0
                    Text text: "Home"
                    Button text: "Go to Settings", onPress: || {
                        Router.navigate("settings")
                    }
            Screen route: "settings"
                Column gap: 16.0
                    Text text: "Settings"
                    Button text: "Go to Home", onPress: || {
                        Router.navigate("home")
                    }
