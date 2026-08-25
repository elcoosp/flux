---
title: Wire Format
description: The normative binary wire protocol (Appendix D) — frame, patch, node, value, and prop-diff encodings with byte offsets.
---

All multi-byte integers are **little-endian**. This reference is normative and is
taken directly from Appendix D of the specification. The playground's
FrameInspector decodes a real `Init` frame against this layout.

## D.1 Frame Structure

| Offset | Size | Field | Description |
|---|---|---|---|
| 0 | 4 | magic | `0x465C5558` ("FLUX") |
| 4 | 1 | version | Protocol version (currently 1) |
| 5 | 4 | seq | Monotonic sequence number |
| 9 | 1 | flags | bit 0 full_tree · 1 error · 2 heartbeat · 3 has_state_delta · 4 has_src_map_delta · 5 has_string_table_delta |
| 10 | 2 | patch_count | Number of Patch entries |
| 12 | 2 | handler_count | Number of HandlerDef entries |
| 14 | 2 | string_count | Number of new strings (0 if no delta) |
| 16 | … | patches | `[Patch; patch_count]` |
| … | … | handlers | `[HandlerDef; handler_count]` |
| … | … | strings | `[StringEntry; string_count]` (if delta) |
| … | … | state_delta | StateDelta (if flag set) |
| … | … | src_map_delta | SourceMapDelta (if flag set) |

## D.2 Patch Encoding

Each patch starts with a 1-byte tag:

| Tag | Type | Payload |
|---|---|---|
| `0x01` | Replace | u32 id, Node |
| `0x02` | Update | u32 id, PropDiff |
| `0x03` | Insert | u32 parent_id, u16 index, Node |
| `0x04` | Remove | u32 id |
| `0x05` | Reorder | u32 parent_id, u16 key_count, [u32; key_count] |
| `0x06` | Handler | u32 id, ClosureRef |

## D.3 Node Encoding

| Offset | Size | Field | Description |
|---|---|---|---|
| 0 | 4 | id | NodeId |
| 4 | 1 | kind | NodeKind (0=Component, 1=Primitive, …) |
| 5 | 4 | component_id | Interned component name ID |
| 9 | 2 | prop_count | Number of props |
| 11 | … | props | `[(u16 prop_idx, Value); prop_count]` |
| … | 2 | child_count | Number of children |
| … | … | children | `[Child; child_count]` |
| … | 2 | handler_count | Number of handlers |
| … | … | handlers | `[u32 HandlerId; handler_count]` |
| … | 4 | span_file | FileId |
| … | 4 | span_start | Byte offset |
| … | 4 | span_end | Byte offset |

## D.5 Value Encoding

| Tag | Type | Payload (after tag) |
|---|---|---|
| `0x00` | Null | (none) |
| `0x01` | Int | i64 (8 bytes) |
| `0x02` | Float | f64 (8 bytes) |
| `0x03` | Bool | u8 (0 or 1) |
| `0x04` | Str | u32 string_id (interned) |
| `0x05` | HandlerRef | u32 handler_id |
| `0x06` | List | u16 count, [Value; count] |
| `0x07` | Record | u16 count, [(u16 prop_idx, Value); count] |

## D.6 PropDiff Encoding

| Offset | Size | Field | Description |
|---|---|---|---|
| 0 | 2 | change_count | Number of changed props |
| 2 | … | changes | `[(u16 prop_idx, Value); change_count]` |
| … | 2 | removal_count | Number of removed props |
| … | … | removals | `[u16 prop_idx; removal_count]` |

## Content addressing

Props, closures, and nodes are content-addressed by BLAKE3 hash (D.14). For each
item in a frame: if the hash is in the host cache, send only the 8-byte hash;
otherwise send full data and cache it. After the initial `Init`, 90%+ of items
are cache hits.
