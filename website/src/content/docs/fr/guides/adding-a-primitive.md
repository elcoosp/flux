---
title: Ajouter un Primitif
description: Comment ajouter un nouveau primitif adapter (ex. un Slider) aux deux hôtes et garder le contrat adapter et les tests de parité synchronisés.
---

Un *primitif* est un composant UI feuille soutenu par une vue native sur les deux
plateformes (voir Appendice F). En ajouter un est un changement transversal : le
contrat adapter dans la spec, l'implémentation dev, l'implémentation release et un
test de parité doivent tous avancer ensemble.

## 1. Étendre le contrat adapter (Appendice F)

Ajoutez les props du primitif au contrat. Chaque prop fait partie du contrat : une
nouvelle prop doit être ajoutée dans le **dev** et la **release**, avec le même nom
et le même type.

```flux
// slider.flux — composant adapter `Slider` (Appendice F.N).
component Slider(
  value: Float,
  min: Float = 0.0,
  max: Float = 1.0,
  onValueChange: Handler,
) {
  // Feuille adapter — rendu natif défini par l'Appendice F.N.
}
```

## 2. Implémentez-le sur les deux hôtes

- **Dev (iOS/Android) :** pilotez `UISlider` / `android.widget.SeekBar` (ou
  le `Slider` Compose) impérativement dans l'`update` de l'adapter.
- **Release (SwiftUI/Compose) :** émettez `@State` + `Slider(value:)` /
  `Slider(value = …)`.

Les deux consomment les **mêmes props** — les props sont le contrat.

## 3. Câblez les signal_deps + handlers

Si le primitif écrit un signal (ici `onValueChange` se déclenche au drag), l'hôte
doit enregistrer le handler et le `signal_deps` du nœud doit inclure tout ce que la
closure du handler lit. C'est ce qui garde la réconciliation du dirty set correcte.

## 4. Ajoutez un test de parité

Ajoutez un scénario golden à `reconcile-trace-format.md` (ex. `slider_drag`) et une
trace dans `/tests/trace-goldens/`. L'outil de parité (`flux-parity trace diff`)
prouve que les hôtes Swift et Kotlin produisent des traces octet-pour-octet
identiques.

## Checklist

- [ ] Props ajoutées à l'Appendice F (dev + release).
- [ ] Adapter dev implémenté sur les deux plateformes.
- [ ] Codegen release implémenté sur les deux plateformes.
- [ ] `signal_deps` enregistré pour tout signal écrit.
- [ ] Golden de parité + trace ajoutés.
