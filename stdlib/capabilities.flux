// capabilities.flux — stdlib capabilities (mlp-spec §24.1).
//
// Capabilities expose vetted platform APIs to the VM. The VM has no
// `CALL_NATIVE`; the only mechanism for platform calls is `CALL_CAP` with a
// capability id (mlp-spec §24). These declarations are declarations only — the
// bodies are bound per-platform (dev mode runs in-memory stand-ins; release
// mode calls native directly, per §24.2/§24.3). No method bodies are
// provided here.
//
// `Data` is the opaque binary payload type shared with these capabilities; it
// is declared in prelude.flux and is in scope via the implicit prelude.
//
// IDs are stable and match the native `CapabilityRegistry` tables (cap 1 =
// Camera, cap 2 = Storage, cap 3 = Router, cap 4 = Clipboard, cap 5 =
// Geolocation). Sync vs async is a binding detail declared by `fn` vs
// `async fn` in this IDL (NOT a method-name suffix): sync methods return
// immediately; async methods (most platform calls — camera, permissions,
// network) resolve through the VM's await machinery (ADR-0044 / ADR-0045) and
// return a `Result` on failure. Method names mirror the Expo Modules verb
// surface (e.g. `takePicture`, `setString`) so RN/Expo developers feel at home.

capability Camera {
  // requires: .camera — every Camera method needs the OS camera grant; a denied
  // grant returns a `Capability` error, never a crash.
  fn takePicture() -> Data
  fn startPreview() -> Unit
  fn stopPreview() -> Unit
}

capability Storage {
  // requires: .storage — every Storage method needs the OS storage grant; a
  // denied grant returns a `Capability` error, never a crash.
  fn setItem(key: String, value: Data) -> Unit
  fn getItem(key: String) -> Option[Data]
  fn removeItem(key: String) -> Unit
  fn devReferenceAsync() -> Data
}

capability Router {
  // requires: .none — navigation is always permitted; no OS grant gates it.
  fn navigate(target: String) -> Unit
}

capability Clipboard {
  // requires: .clipboard — every Clipboard method needs the OS pasteboard
  // grant; a denied grant returns a `Capability` error, never a crash.
  fn setString(value: Data) -> Unit
  fn getString() -> Option[Data]
}

capability Geolocation {
  // requires: .location — `get` needs the OS location grant; a denied grant
  // returns a `Capability` error, never a crash.
  fn getCurrentPosition() -> Option[Data]
}

// --- FLUX-045: six concrete native capabilities (PRD-Q deferred set) ---
// IDs continue the stable sequence (cap 6..=11) and must match
// `CAPABILITY_IDL` in crates/flux-types/src/capabilities.rs. The bodies are
// bound per-platform in the native `CapabilityRegistry` (runtimes/android/host
// + runtimes/ios); dev mode runs in-memory stand-ins. Method names mirror the
// Expo Modules surface.

capability Push {
  // requires: .notification — posting local/remote notifications needs the OS
  // notification grant; resolves async via the VM's await machinery (ADR-0045).
  fn registerForNotifications() -> Unit
  fn scheduleNotification(payload: Data) -> Unit
}

capability Biometric {
  // requires: .biometric — local device authentication via LAContext / BiometricPrompt.
  fn authenticate() -> Result[Bool, Data]
}

capability Background {
  // requires: .background — scheduling background work (BGTaskScheduler / WorkManager).
  fn schedule(task: Data) -> Unit
}

capability FileSystem {
  // requires: .filesystem — read/write/delete the app's sandboxed file system.
  fn readAsString(path: String) -> Option[Data]
  fn writeAsString(path: String, value: Data) -> Unit
  fn delete(path: String) -> Unit
}

capability DeepLink {
  // requires: .none — opening external URLs / universal links is always permitted.
  fn openURL(url: String) -> Unit
}

capability Sensors {
  // requires: .sensors — reading device motion / ambient sensors (CMMotionManager / SensorManager).
  fn read(kind: String) -> Option[Data]
}

// --- FLUX-048: WebView escape-hatch capability (the release valve) ---
// Maps to WKWebView (iOS) / WebView (Android). `load` sets the `src` (a URL or
// html string); `evaluate` runs JS in the page; `sendMessage` posts to the
// host↔web message bridge. Always permitted (it is app-authored content), but
// the host must serve only app-controlled `src` and sandbox the view (FLUX-049
// threat model).
capability WebView {
  // requires: .none — embedding web content is always permitted; the risk is
  // contained by sandboxing, not an OS grant prompt.
  fn load(src: String) -> Unit
  fn evaluate(script: String) -> Option[Data]
  fn sendMessage(message: Data) -> Unit
}

// --- FLUX-046: native-module escape hatch (wrap any native SDK) ---
// The user-facing path to bind an SDK the framework does not ship. A wrapper is
// a capability whose id is derived deterministically
// (`derive_capability_id(name)`) so server + both hosts agree (AGENTS.md §3.4);
// the host allow-lists which module names resolve (LANE-I), so a malicious
// `.flux` patch cannot invoke an undeclared module — there is no open
// `CALL_NATIVE`. `invoke` calls a method on the wrapped module with positional args.
capability NativeModule {
  // requires: .native — every escape-hatch invoke needs the host's native-module
  // grant; a module the app did not register is denied (red banner, no crash).
  fn invoke(name: String, method: String, args: Data) -> Option[Data]
}

// --- FLUX-047: HTTP fetch/JSON + structured persistence (the data layer) ---
// `Http` performs outbound network requests (async; resolves through the VM's
// await machinery, ADR-0044/0045 — a denied/empty grant returns a `Capability`
// error, never a crash). `Persist` is structured, queryable local persistence
// beyond key-value `Storage` (a thin record store keyed by id + optionally
// queryable by field). Host bodies (`URLSession` / `OkHttp`; `UserDefaults` /
// `Room`-style store) land in `runtimes/*/host` (parallel-owned), mirroring the
// other capabilities; this file is the single-source declaration.
capability Http {
  // requires: .network — outbound requests need the network grant.
  // (async: resolved through the VM's await machinery, ADR-0044/0045 — a denied
  // grant returns a `Capability` error, never a crash.)
  fn fetch(url: String, options: Data) -> Data
  fn getJson(url: String) -> Data
  fn postJson(url: String, body: Data) -> Data
}

capability Persist {
  // requires: .storage — structured local persistence reuses the storage grant
  // (sandboxed app data, same OS gate as key-value Storage).
  fn put(key: String, value: Data) -> Unit
  fn get(key: String) -> Option[Data]
  fn query(where: String) -> List[Data]
  fn delete(key: String) -> Unit
}
