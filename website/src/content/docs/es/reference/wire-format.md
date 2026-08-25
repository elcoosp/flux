---
title: Formato Wire
description: El protocolo wire binario normativo (Apéndice D) — frame, patch, node, value y codificaciones de prop-diff con offsets de byte.
---

Todos los enteros multi-byte son **little-endian**. Esta referencia es normativa
y proviene directamente del Apéndice D de la especificación. El FrameInspector
del playground decodifica un frame `Init` real contra este layout.

## D.1 Estructura del Frame

| Offset | Size | Campo | Descripción |
|---|---|---|---|
| 0 | 4 | magic | `0x465C5558` ("FLUX") |
| 4 | 1 | version | Versión del protocolo (actualmente 1) |
| 5 | 4 | seq | Número de secuencia monótono |
| 9 | 1 | flags | bit 0 full_tree · 1 error · 2 heartbeat · 3 has_state_delta · 4 has_src_map_delta · 5 has_string_table_delta |
| 10 | 2 | patch_count | Número de entradas Patch |
| 12 | 2 | handler_count | Número de entradas HandlerDef |
| 14 | 2 | string_count | Número de strings nuevos (0 si no hay delta) |
| 16 | … | patches | `[Patch; patch_count]` |
| … | … | handlers | `[HandlerDef; handler_count]` |
| … | … | strings | `[StringEntry; string_count]` (si delta) |
| … | … | state_delta | StateDelta (si flag set) |
| … | … | src_map_delta | SourceMapDelta (si flag set) |

## D.2 Codificación de Patch

Cada patch empieza con un tag de 1 byte:

| Tag | Tipo | Payload |
|---|---|---|
| `0x01` | Replace | u32 id, Node |
| `0x02` | Update | u32 id, PropDiff |
| `0x03` | Insert | u32 parent_id, u16 index, Node |
| `0x04` | Remove | u32 id |
| `0x05` | Reorder | u32 parent_id, u16 key_count, [u32; key_count] |
| `0x06` | Handler | u32 id, ClosureRef |

## D.3 Codificación de Node

| Offset | Size | Campo | Descripción |
|---|---|---|---|
| 0 | 4 | id | NodeId |
| 4 | 1 | kind | NodeKind (0=Component, 1=Primitive, …) |
| 5 | 4 | component_id | Interned component name ID |
| 9 | 2 | prop_count | Número de props |
| 11 | … | props | `[(u16 prop_idx, Value); prop_count]` |
| … | 2 | child_count | Número de hijos |
| … | … | children | `[Child; child_count]` |
| … | 2 | handler_count | Número de manejadores |
| … | … | handlers | `[u32 HandlerId; handler_count]` |
| … | 4 | span_file | FileId |
| … | 4 | span_start | Byte offset |
| … | 4 | span_end | Byte offset |

## D.5 Codificación de Value

| Tag | Tipo | Payload (tras tag) |
|---|---|---|
| `0x00` | Null | (ninguno) |
| `0x01` | Int | i64 (8 bytes) |
| `0x02` | Float | f64 (8 bytes) |
| `0x03` | Bool | u8 (0 o 1) |
| `0x04` | Str | u32 string_id (internado) |
| `0x05` | HandlerRef | u32 handler_id |
| `0x06` | List | u16 count, [Value; count] |
| `0x07` | Record | u16 count, [(u16 prop_idx, Value); count] |

## D.6 Codificación de PropDiff

| Offset | Size | Campo | Descripción |
|---|---|---|---|
| 0 | 2 | change_count | Número de props cambiadas |
| 2 | … | changes | `[(u16 prop_idx, Value); change_count]` |
| … | 2 | removal_count | Número de props eliminadas |
| … | … | removals | `[u16 prop_idx; removal_count]` |

## Direccionamiento por contenido

Props, closures y nodes se direccionan por contenido con hash BLAKE3 (D.14). Para
cada item en un frame: si el hash está en caché del host, envía solo el hash de 8
bytes; si no, envía los datos completos y lo cachea. Tras el `Init` inicial, 90%+
de los items son cache hits.
