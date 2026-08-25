---
title: Format Wire
description: Le protocole wire binaire normatif (Appendice D) — encodages de frame, patch, node, value et prop-diff avec décalages d'octet.
---

Tous les entiers multi-octets sont en **little-endian**. Cette référence est
normative et tirée directement de l'Appendice D de la spécification. Le FrameInspector
du terrain de jeu décode une vraie trame `Init` contre cette disposition.

## D.1 Structure de la trame

| Décalage | Taille | Champ | Description |
|---|---|---|---|
| 0 | 4 | magic | `0x465C5558` (« FLUX ») |
| 4 | 1 | version | Version du protocole (actuellement 1) |
| 5 | 4 | seq | Numéro de séquence monotone |
| 9 | 1 | flags | bit 0 full_tree · 1 error · 2 heartbeat · 3 has_state_delta · 4 has_src_map_delta · 5 has_string_table_delta |
| 10 | 2 | patch_count | Nombre d'entrées Patch |
| 12 | 2 | handler_count | Nombre d'entrées HandlerDef |
| 14 | 2 | string_count | Nombre de nouvelles chaînes (0 si pas de delta) |
| 16 | … | patches | `[Patch; patch_count]` |
| … | … | handlers | `[HandlerDef; handler_count]` |
| … | … | strings | `[StringEntry; string_count]` (si delta) |
| … | … | state_delta | StateDelta (si flag set) |
| … | … | src_map_delta | SourceMapDelta (si flag set) |

## D.2 Encodage des patches

Chaque patch commence par un tag d'1 octet :

| Tag | Type | Charge utile |
|---|---|---|
| `0x01` | Replace | u32 id, Node |
| `0x02` | Update | u32 id, PropDiff |
| `0x03` | Insert | u32 parent_id, u16 index, Node |
| `0x04` | Remove | u32 id |
| `0x05` | Reorder | u32 parent_id, u16 key_count, [u32; key_count] |
| `0x06` | Handler | u32 id, ClosureRef |

## D.3 Encodage des nodes

| Décalage | Taille | Champ | Description |
|---|---|---|---|
| 0 | 4 | id | NodeId |
| 4 | 1 | kind | NodeKind (0=Component, 1=Primitive, …) |
| 5 | 4 | component_id | ID de nom de composant interné |
| 9 | 2 | prop_count | Nombre de props |
| 11 | … | props | `[(u16 prop_idx, Value); prop_count]` |
| … | 2 | child_count | Nombre d'enfants |
| … | … | children | `[Child; child_count]` |
| … | 2 | handler_count | Nombre de handlers |
| … | … | handlers | `[u32 HandlerId; handler_count]` |
| … | 4 | span_file | FileId |
| … | 4 | span_start | Offset d'octet |
| … | 4 | span_end | Offset d'octet |

## D.5 Encodage des values

| Tag | Type | Charge utile (après tag) |
|---|---|---|
| `0x00` | Null | (aucune) |
| `0x01` | Int | i64 (8 octets) |
| `0x02` | Float | f64 (8 octets) |
| `0x03` | Bool | u8 (0 ou 1) |
| `0x04` | Str | u32 string_id (interné) |
| `0x05` | HandlerRef | u32 handler_id |
| `0x06` | List | u16 count, [Value; count] |
| `0x07` | Record | u16 count, [(u16 prop_idx, Value); count] |

## D.6 Encodage des PropDiff

| Décalage | Taille | Champ | Description |
|---|---|---|---|
| 0 | 2 | change_count | Nombre de props changées |
| 2 | … | changes | `[(u16 prop_idx, Value); change_count]` |
| … | 2 | removal_count | Nombre de props supprimées |
| … | … | removals | `[u16 prop_idx; removal_count]` |

## Adressage par contenu

Les props, closures et nodes sont adressés par contenu via un hash BLAKE3 (D.14).
Pour chaque item d'une trame : si le hash est dans le cache hôte, n'envoyez que le
hash de 8 octets ; sinon envoyez les données complètes et mettez-les en cache. Après
l'`Init` initial, 90 %+ des items sont des cache hits.
