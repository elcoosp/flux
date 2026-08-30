---
title: Exemple de Compteur
description: Parcourez le compteur Flux canonique — signal, un Text lié, et un tap de Button — et comment il se réconcilie sur un tap.
---

C'est le plus petit programme Flux intéressant, et la forme sur laquelle le scénario
golden `counter_1000` du terrain de jeu est construit.

```flux
compo Counter
  $count: Int = 0

  Column gap: 12.0
    Text text: "Count: {count}"
    Button text: "Increment", onPress: || { count = count + 1 }
```

## Ce que signifie chaque ligne

- `$count: Int = 0` — une cellule de signal mutable (le sigile `$` la marque
  comme signal). L'hôte possède `count` ; le serveur ne voit que son type et sa
  valeur initiale.
- `Text(text: "Count: {count}")` — interpole le signal dans une chaîne. Le
  `signal_deps` de ce nœud est exactement `[1]` (l'id de `count`).
- `Button(text: "Increment", onPress: || { ... })` — enregistre un handler
  `onPress` (un `Handler` sans argument) dont la closure écrit `count`.

## Ce qui se passe sur un tap

1. La closure `onPress` s'exécute dans la VM hôte, exécutant
   `READ_SIGNAL count` → `LOAD_INT_CONST` → `ADD_I64` → `WRITE_SIGNAL count`.
2. La VM rapporte `signals: [1]` (l'id de signal écrit, ascendant).
3. L'hôte intersecte `{1}` avec le `signal_deps` de chaque nœud : seul le `Text`
   le lit, donc `dirty: [57]` (l'id de nœud du Text), dans l'ordre
   `(depth asc, id asc)`.
4. Le Text re-matérialise ses props et déclenche `update` — **une** mise à jour,
   **zéro** construction, ≤ 2 matérialisations de prop. Le `Column` et le `Button`
   restent intacts (`skip_unchanged` dans une ré-application complète).

Parcourez cette trace exacte sur le terrain de jeu de la page d'accueil.
