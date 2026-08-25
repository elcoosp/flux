---
title: Le Wire
description: Le protocole de trame binaire (Appendice D) entre le serveur de dev et l'app hôte — magic, flags, patches, et comment le diffing garde les patches minuscules.
---

Flux communique avec l'hôte via un **protocole wire binaire** (Appendice D), pas du
JSON. Tous les entiers multi-octets sont en little-endian. L'adressage par contenu
(BLAKE3) signifie qu'un patch typique après l'`Init` initial est composé à 90 % de
références de hash.

## Structure de la trame

Chaque trame commence par un en-tête (D.1) :

| Décalage | Taille | Champ | Description |
|---|---|---|---|
| 0 | 4 | magic | `0x465C5558` (« FLUX ») |
| 4 | 1 | version | Version du protocole (1) |
| 5 | 4 | seq | Numéro de séquence monotone |
| 9 | 1 | flags | `full_tree`, `error`, `heartbeat`, … |
| 10 | 2 | patch_count | Nombre d'entrées Patch |
| 12 | 2 | handler_count | Nombre d'entrées HandlerDef |
| 14 | 2 | string_count | Nouvelles chaînes internées (0 si pas de delta) |

## Patches

Chaque patch commence par un tag d'1 octet (D.2) :

| Tag | Type | Charge utile |
|---|---|---|
| `0x01` | Replace | u32 id, Node |
| `0x02` | Update | u32 id, PropDiff |
| `0x03` | Insert | u32 parent_id, u16 index, Node |
| `0x04` | Remove | u32 id |
| `0x05` | Reorder | u32 parent_id, u16 key_count, [u32] |
| `0x06` | Handler | u32 id, ClosureRef |

## Pourquoi cela compte

Le serveur de dev calcule `dirty = ⋃ dependents[S]` (ADR-0027 Phase 2) et émet des
patches **Update** adressés uniquement aux nœuds sales, plus des patches structurels
là où les props de contrôle ont changé. Après la trame `Init` initiale, 90 %+ des
props/closures sont des cache hits — un changement de corps de handler fait
généralement < 500 octets.

Le FrameInspector du terrain de jeu décode l'en-tête et les flags d'une vraie trame
`Init` de l'Appendice D pour que vous puissiez voir la disposition directement.
