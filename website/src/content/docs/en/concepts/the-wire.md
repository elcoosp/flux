---
title: The Wire
description: The binary frame protocol (Appendix D) between the dev server and the host app — magic, flags, patches, and how diffing keeps patches tiny.
---

Flux talks to the host over a **binary wire protocol** (Appendix D), not JSON.
All multi-byte integers are little-endian. Content addressing (BLAKE3) means a
typical patch after the initial `Init` is 90%+ hash references.

## Frame structure

Every frame starts with a header (D.1):

| Offset | Size | Field | Description |
|---|---|---|---|
| 0 | 4 | magic | `0x465C5558` ("FLUX") |
| 4 | 1 | version | Protocol version (1) |
| 5 | 4 | seq | Monotonic sequence number |
| 9 | 1 | flags | `full_tree`, `error`, `heartbeat`, … |
| 10 | 2 | patch_count | Number of Patch entries |
| 12 | 2 | handler_count | Number of HandlerDef entries |
| 14 | 2 | string_count | New interned strings (0 if no delta) |

## Patches

Each patch begins with a 1-byte tag (D.2):

| Tag | Type | Payload |
|---|---|---|
| `0x01` | Replace | u32 id, Node |
| `0x02` | Update | u32 id, PropDiff |
| `0x03` | Insert | u32 parent_id, u16 index, Node |
| `0x04` | Remove | u32 id |
| `0x05` | Reorder | u32 parent_id, u16 key_count, [u32] |
| `0x06` | Handler | u32 id, ClosureRef |

## Why it matters

The dev server computes `dirty = ⋃ dependents[S]` (ADR-0027 Phase 2) and emits
**Update** patches addressed only to dirty nodes plus structural patches where
control props changed. After the initial `Init` frame, 90%+ of props/closures are
cache hits — a handler-body change is typically < 500 bytes.

The playground's FrameInspector decodes a real Appendix-D `Init` frame's header
and flags so you can see the layout directly.
