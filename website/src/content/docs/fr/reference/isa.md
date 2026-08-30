---
title: Jeu d'Instructions de la VM
description: Référence des opcodes de la VM hôte Flux (Appendice E) — ops de signal, arithmétique monomorphisée, flux de contrôle et closures.
---

L'hôte embarque une **VM bytecode register-based**. Les instructions sont
monomorphisées : il n'y a pas d'`ADD` générique avec dispatch par tag — il y a
`ADD_I64`, `ADD_F64`, etc. Cela garde le chemin chaud sans branche et la trace
octet-pour-octet identique entre les hôtes.

> Les opcodes et encodages d'opérandes ci-dessous sont normatifs et tirés de
> l'Appendice E de la spécification. N'ajoutez pas d'opcode sans un ADR et un bump
> de version de protocole.

## Opérations de signal

| Opcode | Mnémonique | Args | Description |
|---|---|---|---|
| `0x10` | `READ_SIGNAL` | reg_dst(u8), signal_id(u32) | Lit un signal dans un registre |
| `0x11` | `WRITE_SIGNAL` | signal_id(u32), reg_src(u8) | Écrit la valeur d'un registre dans un signal |

Une closure de handler se termine en écrivant les signaux que le dispatch a consommés.
Ces ids écrits deviennent l'événement de trace `signals` (triés ascendant).

## Arithmétique entière (monomorphisée)

| Opcode | Mnémonique | Args | Description |
|---|---|---|---|
| `0x20` | `ADD_I64` | dst, a, b (u8 chacun) | `dst = a + b` (i64) |
| `0x21` | `SUB_I64` | dst, a, b | `dst = a - b` |
| `0x22` | `MUL_I64` | dst, a, b | `dst = a * b` |
| `0x23` | `DIV_I64` | dst, a, b | `dst = a / b` |
| `0x24` | `MOD_I64` | dst, a, b | `dst = a % b` |

Les variantes flottantes (`ADD_F64`, …) existent dans la même bande `0x2x` avec le
suffixe `F64`. Le `count = count + 1` du compteur compile en `READ_SIGNAL`, `LOAD_INT_CONST`,
`ADD_I64`, `WRITE_SIGNAL`.

## Flux de contrôle & closures

Les closures de handler et de prop-thunk partagent l'encodage `ClosureRef` (D.7) : un
hash BLAKE3 de 8 octets (adresse de contenu), un offset/longueur de bytecode dans le
blob partagé, les ids de signaux capturés, et un span source. Les prop-thunks Phase 3
s'exécutent localement à partir du dirty set — `r0` est réservé au contexte de nœud,
`r1` contient le résultat `ALLOC_RECORD` sur `HALT`, et `prop_layout` mappe les champs
du record aux indices de prop.

## Politique de faute

Une faute de thunk ou de handler ⇒ le nœud garde ses props précédentes (rendu
périmé, jamais vide) ; l'erreur remonte via le chemin d'overlay existant et un
événement de trace `error` est enregistré. La vue n'est jamais détruite.
