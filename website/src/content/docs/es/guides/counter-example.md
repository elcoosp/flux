---
title: Ejemplo de Contador
description: Recorre el contador Flux canónico — señal, un Text ligado y un tap de Button — y cómo se reconcilia en un tap.
---

Este es el programa Flux más pequeño interesante, y la forma sobre la que se
construye el escenario golden `counter_1000` del playground.

```flux
compo Counter
  $count: Int = 0

  Column gap: 12.0
    Text text: "Count: {count}"
    Button text: "Increment", onPress: || { count = count + 1 }
```

## Qué significa cada línea

- `$count: Int = 0` — una celda de señal mutable (el sigilo `$` la marca como
  señal). El host posee `count`; el servidor solo ve su tipo y valor inicial.
- `Text(text: "Count: {count}")` — interpola la señal en un string. Los
  `signal_deps` de este nodo son exactamente `[1]` (el id de `count`).
- `Button(text: "Increment", onPress: || { ... })` — registra un handler `onPress`
  (un `Handler` sin argumentos) cuyo closure escribe `count`.

## Qué pasa en un tap

1. El closure `onPress` corre en la VM del host, ejecutando
   `READ_SIGNAL count` → `LOAD_INT_CONST` → `ADD_I64` → `WRITE_SIGNAL count`.
2. La VM reporta `signals: [1]` (el id de señal escrito, ascendente).
3. El host intersecta `{1}` con los `signal_deps` de cada nodo: solo el `Text`
   lo lee, así `dirty: [57]` (el id de nodo del Text), en orden
   `(depth asc, id asc)`.
4. El Text re-materializa sus props y dispara `update` — **una** actualización,
   **cero** construcciones, ≤ 2 materializaciones. El `Column` y el `Button` no
   se tocan (`skip_unchanged` en una re-aplicación completa).

Recorre esta traza exacta en el playground de la portada.
