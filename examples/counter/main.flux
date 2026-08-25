// main.flux — Flux counter app entry point.
//
// `Counter` is the root component; `flux dev` serves it over WebSocket and
// `flux build` lowers it for iOS / Android. It is the canonical vertical
// slice used by the headless full-pipeline e2e test (see
// `crates/flux-devserver/tests/full_pipeline.rs`) and by the
// `runtime-packaging-gap` ADR as the recommended integration path.

component Counter {
    state count: Int = 0

    Column(gap: 8.0) {
        Text(text: "tapped ${count} times")
        Button(text: "Increment", onClick: fn() { count = count + 1 })
    }
}
