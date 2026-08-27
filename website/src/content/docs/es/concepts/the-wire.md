---
title: El Wire
description: El protocolo de frames binario (Apéndice D) entre el servidor de desarrollo y la app host — magic, flags, parches y cómo el diff mantiene los parches diminutos.
---

Flux habla con el host sobre un **protocolo wire binario** (Apéndice D), no JSON.
Todos los enteros multi-byte son little-endian. El direccionamiento por contenido
(BLAKE3) significa que un parche típico tras el `Init` inicial es 90%+ referencias
hash.

## Estructura del frame

Cada frame empieza con una cabecera (D.1):

| Offset | Size | Campo | Descripción |
|---|---|---|---|
| 0 | 4 | magic | `0x465C5558` ("FLUX") |
| 4 | 1 | version | Versión del protocolo (1) |
| 5 | 4 | seq | Número de secuencia monótono |
| 9 | 1 | flags | `full_tree`, `error`, `heartbeat`, … |
| 10 | 2 | patch_count | Número de entradas Patch |
| 12 | 2 | handler_count | Número de entradas HandlerDef |
| 14 | 2 | string_count | Strings internados nuevos (0 si no hay delta) |

## Parches

Cada parche empieza con un tag de 1 byte (D.2):

| Tag | Tipo | Payload |
|---|---|---|
| `0x01` | Replace | u32 id, Node |
| `0x02` | Update | u32 id, PropDiff |
| `0x03` | Insert | u32 parent_id, u16 index, Node |
| `0x04` | Remove | u32 id |
| `0x05` | Reorder | u32 parent_id, u16 key_count, [u32] |
| `0x06` | Handler | u32 id, ClosureRef |

## Por qué importa

El servidor de desarrollo calcula `dirty = ⋃ dependents[S]` (ADR-0027 Fase 2) y
emite parches **Update** dirigidos solo a los nodos sucios, más parches
estructurales donde cambiaron props de control. Tras el `Init` inicial, 90%+ de
props/closures son cache hits — un cambio de cuerpo de manejador suele ser < 500
bytes.

El FrameInspector del playground decodifica la cabecera y los flags de un frame
`Init` real del Apéndice D para que veas el layout directamente.
