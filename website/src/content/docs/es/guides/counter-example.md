---
title: Ejemplo de Contador
description: Recorre el contador Flux canónico — state, un Text ligado y un tap de Button — y cómo se reconcilia en un tap.
---

Este es el programa Flux más pequeño interesante, y la forma sobre la que se
construye el escenario golden `counter_1000` del playground.

```flux
compo Counter
  $count: Int = 0

  Column gap: 12.0
    Text text: "Count: {count}"
    Button text: "Increment", onClick: || {
      count = count + 1
    }
```

## Qué significa cada línea

- `state count: Int = 0` — una celda de señal mutable. El host posee `count`; el
  servidor solo ve su tipo y valor inicial.
- `Text("Count: {count}")` — interpola la señal en un string. Los `signal_deps`
  de este nodo son exactamente `[1]` (el id de `count`).
- `Button(text: "Increment", onClick: { ... })` — registra el manejador id 7
  (en el fixture del playground) cuyo closure escribe `count`.

## Qué pasa en un tap

1. El closure `onClick` corre en la VM del host, ejecutando `WRITE_SIGNAL count, +1`.
2. La VM reporta `signals: [1]` (el id de señal escrito, ascendente).
3. El host intersecta `{1}` con los `signal_deps` de cada nodo: solo el `Text`
   lo lee, así `dirty: [57]` (el id de nodo del Text), en orden
   `(depth asc, id asc)`.
4. El Text re-materializa sus props y dispara `update` — **una** actualización,
   **cero** construcciones, ≤ 2 materializaciones. El `Column` y el `Button` no
   se tocan (`skip_unchanged` en una re-aplicación completa).

Recorre esta traza exacta en el playground de la portada.

## Los presupuestos

De `reconcile-counters-and-budgets.md`: un dispatch `counter_1000` debe producir
≤ 1 update, 0 builds, ≤ 2 prop materializations — **independiente del tamaño del
árbol**. La regla general: cada contador tras un dispatch está acotado por
`|dependents[S]|` + tamaño del diff estructural, nunca por el tamaño del árbol.
