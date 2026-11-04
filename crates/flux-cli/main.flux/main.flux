// main.flux — Flux app entry point.
//
// `Hello` is the root component; `flux dev` serves it over WebSocket and
// `flux build` lowers it for iOS / Android.

component Hello {
    state count: Int = 0

    Column(gap: 8.0) {
        Text(text: "tapped ${count} times")
        Button(text: "Increment", onClick: fn() { count = count + 1 })
    }
}
