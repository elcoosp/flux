# Wire fixtures (FLUX-083, ADR-0059)

Binary wire frames shared across the three host decoders (Rust `flux-ir-serde`,
Kotlin `FrameDeserializer`, Swift `FrameDeserializer`), so they stay in
lockstep on both the **`PROTOCOL_VERSION` fail-closed** path (FLUX-050 /
ADR-0056) and the **v2 → v3 decoder fallback** path (ADR-0059).

## Fixture set

| File | Protocol | Purpose |
|---|---|---|
| `init_v2.bin`             | 2 | Legacy full-tree Init; decoder v2 fallback coverage |
| `delta_v2.bin`            | 2 | Legacy patch delta; decoder v2 fallback coverage |
| `init_v3.bin`             | 3 | Current full-tree Init (u32 length prefixes) |
| `delta_v3.bin`            | 3 | Current patch delta |
| `unsupported-version.bin` | 4 | Fail-closed version rejection |

The v2 and v3 pairs share **identical structural content** — the only
differences are the header version byte and the widened `u32` length prefixes
introduced by ADR-0059. Any unexpected byte divergence between v2 and v3
pairs indicates encoder drift.

## Regenerating

```sh
cargo run -p flux-ir-serde --example dump_fixtures
