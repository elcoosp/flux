---
title: Añadir un Primitivo
description: Cómo añadir un primitivo adapter nuevo (p. ej. un Slider) a ambos hosts y mantener sincronizados el contrato adapter y los tests de paridad.
---

Un *primitivo* es un componente leaf UI respaldado por una vista nativa en ambas
plataformas (ver Apéndice F). Añadir uno es un cambio transversal: el contrato
adapter en la spec, la implementación dev, la implementación release y un test de
paridad deben moverse juntos.

## 1. Extiende el contrato adapter (Apéndice F)

Añade las props del primitivo al contrato. Cada prop es parte del contrato: una
prop nueva debe añadirse en **dev y release**, con el mismo nombre y tipo.

```flux
compo Slider(
  value: Float,
  min: Float = 0.0,
  max: Float = 1.0,
  onValueChange: Handler,
)
  // Adapter leaf — native rendering defined by Appendix F.N.
```


## 2. Impleméntalo en ambos hosts

- **Dev (iOS/Android):** conduce `UISlider` / `android.widget.SeekBar` (o
  `Slider` de Compose) imperativamente en el `update` del adapter.
- **Release (SwiftUI/Compose):** emite `@State` + `Slider(value:)` /
  `Slider(value = …)`.

Ambos consumen las **mismas props** — las props son el contrato.

## 3. Cables de signal deps + manejadores

Si el primitivo escribe una señal (aquí `onValueChange` dispara al arrastrar), el
host debe registrar el manejador y los `signal_deps` del nodo deben incluir
cualquier señal que lea el closure. Esto mantiene correcta la reconciliación del
conjunto sucio.

## 4. Añade un test de paridad

Añade un escenario golden a `reconcile-trace-format.md` (p. ej. `slider_drag`) y
una traza en `/tests/trace-goldens/`. La herramienta de paridad
(`flux-parity trace diff`) prueba que los hosts Swift y Kotlin producen trazas
idénticas byte a byte.

## Checklist

- [ ] Props añadidas a Appendix F (dev + release).
- [ ] Adapter dev implementado en ambas plataformas.
- [ ] Codegen release implementado en ambas plataformas.
- [ ] `signal_deps` registrado para señales escritas.
- [ ] Golden de paridad + traza añadidos.
