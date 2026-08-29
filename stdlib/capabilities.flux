// capabilities.flux — stdlib capabilities (mlp-spec §24.1).
//
// Capabilities expose vetted platform APIs to the VM. The VM has no
// `CALL_NATIVE`; the only mechanism for platform calls is `CALL_CAP` with a
// capability id (mlp-spec §24). These declarations are declarations only —
// the bodies are bound per-platform (dev mode runs in-memory stand-ins;
// release mode calls native directly, per §24.2/§24.3). No method bodies are
// provided here.
//
// `Data` is the opaque binary payload type shared with these capabilities;
// it is declared in prelude.flux and is in scope via the implicit prelude.
//
// IDs are stable and match the native `CapabilityRegistry` tables (cap 1 =
// Camera, cap 2 = Storage, cap 3 = Router, cap 4 = Clipboard, cap 5 =
// Geolocation). Sync vs async is a binding detail: sync methods return
// immediately; async methods (most platform calls — camera, permissions,
// network) resolve through the VM's await machinery (ADR-0044 / ADR-0045) and
// return a `Result` on failure.

capability Camera {
  // requires: .camera — every Camera method (take/startPreview/stopPreview) needs
  // the OS camera grant; a denied grant returns a `Capability` error, never a crash.
  fn take() -> Data
  fn startPreview() -> Unit
  fn stopPreview() -> Unit
}

capability Storage {
  // requires: .storage — every Storage method (set/get/delete) needs the OS
  // storage grant; a denied grant returns a `Capability` error, never a crash.
  fn set(key: String, value: Data) -> Unit
  fn get(key: String) -> Option[Data]
  fn delete(key: String) -> Unit
}

capability Router {
  // requires: .none — navigation is always permitted; no OS grant gates it.
  fn navigate(target: String) -> Unit
}

capability Clipboard {
  // requires: .clipboard — every Clipboard method (set/get) needs the OS
  // pasteboard grant; a denied grant returns a `Capability` error, never a crash.
  fn set(value: Data) -> Unit
  fn get() -> Option[Data]
}

capability Geolocation {
  // requires: .location — `get` needs the OS location grant; a denied grant
  // returns a `Capability` error, never a crash.
  fn get() -> Option[Data]
}

// --- FLUX-045: six concrete native capabilities (PRD-Q deferred set) ---
// IDs continue the stable sequence (cap 6..=11) and must match
// `CAPABILITY_IDL` in crates/flux-types/src/capabilities.rs. The bodies are
// bound per-platform in the native `CapabilityRegistry` (runtimes/android/host
// + runtimes/ios); dev mode runs in-memory stand-ins.

capability Push {
  // requires: .notification — posting local/remote notifications needs the OS
  // notification grant; resolves async via the VM's await machinery (ADR-0045).
  fn register() -> Unit
  fn notify(payload: Data) -> Unit
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
  fn read(path: String) -> Option[Data]
  fn write(path: String, value: Data) -> Unit
  fn delete(path: String) -> Unit
}

capability DeepLink {
  // requires: .none — opening external URLs / universal links is always permitted.
  fn open(url: String) -> Unit
}

capability Sensors {
  // requires: .sensors — reading device motion / ambient sensors (CMMotionManager / SensorManager).
  fn read(kind: String) -> Option[Data]
}
