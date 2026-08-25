---
title: Estado Autoritativo del Host
description: Por qué el estado de Flux vive en el grafo de señales del host, no en el servidor, y qué implica para los parches (ADR-0002).
---

El estado de Flux es **autoritativo en el host**. El servidor nunca posee los
valores de señal en runtime; envía estructura (el IR) y deltas (parches). El host
posee el grafo de señales, evalúa los manejadores y reconcilia la vista.

## El invariante

> Tras un dispatch que escribe el conjunto de señales `S`, los únicos nodos cuyo
> resultado renderizado puede cambiar son (a) los nodos cuyas expresiones de prop/
> control leen algún `s ∈ S`, y (b) los nodos construidos/destruidos/reordenados
> por diffs estructurales claveados disparados por (a). Todo lo demás debe quedar
> intacto.

Esto es normativo (ADR-0027). Es lo que hace que un árbol de 1.000 nodos cueste
**una** actualización cuando pulsas un contador ligado a una sola señal —
independientemente del tamaño del árbol. El playground de la portada reproduce
exactamente este escenario.

## Consecuencias

- **Los parches son mínimos.** Un tap `count = count + 1` produce un parche
  `Update` dirigido solo a los nodos sucios, no un reenvío de todo el árbol.
- **El round-trip con el servidor es eliminable.** En Fase 3 (ADR-0027) las prop
  thunks llegan al host, así que el round-trip por tap y la reconstrucción de
  `currentFrame()` se eliminan — el host recalcula desde el conjunto sucio.
- **La paridad de traza es observable.** Como el estado es del host y
  determinista, el mismo frame + script de dispatch contra Swift y Kotlin da
  trazas idénticas byte a byte (`reconcile-trace-format.md`).

## Qué descarta

- El reconciliador **no** debe suscribirse al grafo de señales como observer —
  eso dispara dos veces. Consume el *outcome* de la VM (ADR-0027, fuera de
  alcance explícitamente).
- Un manejador que escribe una señal que nadie lee produce `dirty: []` y cero
  eventos update/build/detach (golden `noop_dispatch`).
