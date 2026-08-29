---
title: Getting started
description: Build your first Flux app — scaffold a project, run the dev server, hot-reload, and ship a native release.
---

This guide takes you from a clean checkout to a working Flux UI on a real device
or simulator, then shows the daily hot-reload loop and how to ship a native
release. It mirrors the real `examples/counter/` and `examples/todo/` apps in
the repo.

## 1. Build the CLI

```bash
cargo build --bin flux
```

This produces `target/debug/flux`. (Use `--release` for a faster binary.)

## 2. Scaffold a project

```bash
flux init myapp
cd myapp
```

`flux init` writes `main.flux` (a stateful `Counter` component), `flux.toml`
(project config + dev ports `7331`/`7332`), and `.fluxignore`.

You can skip the scaffold and use `examples/counter/` instead — the rest of
these docs reference it.

## 3. Start the dev server

```bash
flux dev
# Listening on ws://127.0.0.1:7331
```

The dev server parses + type-checks every `.flux` file, lowers to the reactive
IR, and binds a WebSocket on `:7331` (patch channel) plus an HTTP asset server
on `:7332` (with a `/health` probe). Verify it's up:

```bash
curl http://127.0.0.1:7332/health   # -> ok
```

Edit `main.flux` and save — the server diffs the tree and ships a binary patch.
No restart required.

> **Physical device?** Pass `--ws-host 0.0.0.0` and point the host app at your
> LAN IP, e.g. `ws://192.168.1.42:7331`.

## 4. Run the host app

The host app is precompiled native code that connects to the dev server and
renders the IR. Pick your platform:

### iOS

```bash
xcodegen generate            # writes runtimes/ios/FluxApp.xcworkspace
open runtimes/ios/FluxApp.xcworkspace
```

Pick a simulator (or tethered device) in Xcode and press **Run**. It connects to
`ws://127.0.0.1:7331` and renders your root component. Tap **Increment** — the
label updates live.

### Android

```bash
./gradlew :runtimes:android:app:installDebug
```

Launch **Flux** from the launcher. It connects to `ws://127.0.0.1:7331` and
renders the IR. Tap **Increment** to see hot reload.

## 5. Make a change (hot reload)

Open `main.flux` and change a `Column` gap or a `Text` label, then save. The dev
server ships a patch and the host updates in place — you never leave the running
app. A handler-body edit is typically a few hundred bytes on the wire, because
props and closures are content-addressed (BLAKE3) and cached after the first
`Init` frame.

## 6. Build a native release

```bash
flux build ios       # -> platforms/ios/Generated/main.swift
flux build android   # -> platforms/android/Generated/main.kt
```

`flux build` lowers your `.flux` to idiomatic native source (SwiftUI /
Jetpack Compose) with no VM. If `xcodebuild`/`gradle` is present it suggests the
next build step; otherwise it emits the generated sources for you to compile.

## Next steps

- Walk the [Counter example](/guides/counter-example/) end to end, including release codegen.
- Browse the [Cookbook](/guides/cookbook/) — one page per stdlib primitive.
- Understand [Dev vs Release](/concepts/dev-vs-release/) and why Flux has two execution modes.
- Learn [State management](/guides/state-management/) for real apps beyond the counter.
