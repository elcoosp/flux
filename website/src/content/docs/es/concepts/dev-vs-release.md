---
title: Dev vs Release
description: Cómo Flux se ejecuta en dev (interpretado, parcheado) frente a release (generado a SwiftUI / Compose nativos).
---

import { Card, CardGrid } from '@astrojs/starlight/components';

Flux tiene dos modos de ejecución. Ambos consumen el **mismo IR Reactive Tree**
producido por la pasada de lowering del servidor de desarrollo; difieren solo en
cómo el host convierte ese IR en píxeles.

## Modo dev — interpretado y parcheado

En dev, una **app host** precompilada se envía al dispositivo. El servidor de
desarrollo parsea `.flux`, lo baja al IR Reactive Tree, lo diffea y envía
**parches binarios** (Apéndice D) por WebSocket. La VM de bytecode basada en
registros y el grafo de señales estilo SolidJS del host aplican el parche y
mutan un árbol sombra de vistas nativas.

- Iteración rápida: guardar → diff → parche, sin recompilar la app.
- Introspección completa: el frame wire, el resultado de la VM y la traza de
  reconciliación son observables (ver el playground en la portada).
- El sink `trace` es **gratis en producción** (ADR-0027 INV-2): sin coste cuando
  no hay driver conectado.

## Modo release — generado a nativo

En release, el mismo IR se **genera** a Swift/SwiftUI y Kotlin/Jetpack Compose
idiomáticos. No hay VM, no hay protocolo de parches, no hay árbol sombra — la
salida es una app nativa normal.

<CardGrid>
  <Card title="SwiftUI" icon="seti:swift">
    `compo Counter` → `struct Counter: View`. `state` → `@State`.
    `Column(gap:)` → `VStack(spacing:)`.
  </Card>
  <Card title="Jetpack Compose" icon="seti:kotlin">
    `compo Counter` → `@Composable fun Counter()`. `state` →
    `remember { mutableStateOf(...) }`. `Column(gap:)` →
    `Column(verticalArrangement = spacedBy(...))`.
  </Card>
</CardGrid>

## ¿Por qué dos modos?

El bucle dev necesita vivacidad (parche, no recompiles); el producto enviado
necesita cero coste de runtime (codegen nativo). Mantener un único IR como
contrato significa que la experiencia dev y el binario release son
demostrablemente el mismo programa — que es justo lo que la herramienta de traza
de paridad (FLUX-023) prueba.
