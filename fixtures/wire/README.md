# Wire fixtures (FLUX-083)

Binary wire frames shared across the three decoders (Rust `flux-ir-serde`,
Kotlin `FrameDeserializer`, Swift `FrameDeserializer`) so they stay in
lockstep on the **`PROTOCOL_VERSION` fail-closed** path (FLUX-050 / ADR-0056).

## `unsupported-version.bin`

A valid v2 `Init` (full-tree) frame produced by `Frame::init` in
`crates/flux-ir-serde`, with the **version byte (header offset 4) set to `0x03`**
— a protocol version no host decoder supports:

- Rust decoder: accepts `{2}` → rejects.
- Swift decoder: accepts `{2}` → rejects.
- Kotlin decoder: accepts `{1, 2}` (its `FrameBuilder` test helper emits v1) → rejects.

Layout (Appendix D §D.1 / §D.12.2):

```
offset 0..4  magic  0x465C5558 ("FLUX")
offset 4     version 0x03  ← unsupported, the whole point
offset 5     kind   0x02    (FRAME_INIT)
offset 6..   Init payload (root node, descendant count, signal seed,
              source map, string table, component names, handler section)
```

Any decoder reaching this file **must** surface a typed `WireError` (Rust
`WireError::InvalidTag { context: "frame.version" }`, Kotlin `WireError`,
Swift `WireError.unsupportedVersion`) **before** decoding any field, never a
partial/best-effort decode. The Rust test
`unsupported_protocol_version_fixture_matches_and_is_rejected` regenerates
these exact bytes from its encoder when the file is absent, then asserts the
committed bytes are byte-identical and rejected — so the fixture can never
silently drift from the encoder.

### Regenerating

```sh
cargo test -p flux-ir-serde --test round_trip unsupported_protocol_version
```
