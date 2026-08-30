---
title: Set de Instrucciones de la VM
description: Referencia de opcodes de la VM host de Flux (Apéndice E) — ops de señal, aritmética monomorfizada, control de flujo y closures.
---

El host embebe una **VM de bytecode basada en registros**. Las instrucciones son
monomorfizadas: no hay `ADD` genérico con tag dispatch — hay `ADD_I64`, `ADD_F64`,
etc. Esto mantiene el hot path sin ramas y la traza idéntica entre hosts.

> Los opcodes y las codificaciones de operandos son normativos y provienen del
> Apéndice E de la especificación. No añadas opcodes sin un ADR y un bump de
> versión de protocolo.

## Operaciones de señal

| Opcode | Mnemonic | Args | Descripción |
|---|---|---|---|
| `0x10` | `READ_SIGNAL` | reg_dst(u8), signal_id(u32) | Lee una señal en un registro |
| `0x11` | `WRITE_SIGNAL` | signal_id(u32), reg_src(u8) | Escribe el valor de un registro a una señal |

Un closure manejador termina escribiendo las señales que consumió el dispatch.
Esos ids escritos se vuelven el evento de traza `signals` (orden ascendente).

## Aritmética entera (monomorfizada)

| Opcode | Mnemonic | Args | Descripción |
|---|---|---|---|
| `0x20` | `ADD_I64` | dst, a, b (u8) | `dst = a + b` (i64) |
| `0x21` | `SUB_I64` | dst, a, b | `dst = a - b` |
| `0x22` | `MUL_I64` | dst, a, b | `dst = a * b` |
| `0x23` | `DIV_I64` | dst, a, b | `dst = a / b` |
| `0x24` | `MOD_I64` | dst, a, b | `dst = a % b` |

Las variantes de punto flotante (`ADD_F64`, …) existen en la misma banda `0x2x`
con el sufijo `F64`. El `count = count + 1` del contador compila a `READ_SIGNAL`,
`LOAD_INT_CONST`, `ADD_I64`, `WRITE_SIGNAL`.

## Control de flujo y closures

Los closures de manejador y prop-thunk comparten la codificación `ClosureRef`
(D.7): un hash BLAKE3 de 8 bytes (dirección de contenido), un offset/longitud de
bytecode en el blob compartido, las señales capturadas y un span de fuente. Las
prop thunks Fase 3 corren localmente desde el conjunto sucio — `r0` se reserva
para el contexto de nodo, `r1` contiene el `ALLOC_RECORD` en `HALT`, y
`prop_layout` mapea campos del record a índices de prop.

## Política de fallo

Un fallo de thunk o manejador ⇒ el nodo conserva sus props previas (renderiza
obsoleto, nunca en blanco); el error sale por el overlay existente y se registra
un evento `error` en la traza. La vista nunca se destruye.
