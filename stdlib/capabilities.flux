// capabilities.flux — stdlib capabilities (mlp-spec §24.1).
//
// Capabilities expose vetted platform APIs to the VM. The VM has no
// `CALL_NATIVE`; the only mechanism for platform calls is `CALL_CAP` with a
// capability id (mlp-spec §24). These declarations are declarations only —
// the bodies are bound per-platform (dev mode forwards over WS; release mode
// calls native directly, per §24.2/§24.3). No method bodies are provided
// here.
//
// `Data` is the opaque binary payload type shared with these capabilities;
// it is declared in prelude.flux and is in scope via the implicit prelude.

capability Camera {
  fn capture() -> Data
  fn startPreview() -> Unit
  fn stopPreview() -> Unit
}

capability Storage {
  fn set(key: String, value: Data) -> Unit
  fn get(key: String) -> Option[Data]
  fn delete(key: String) -> Unit
}
