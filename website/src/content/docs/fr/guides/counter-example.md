---
title: Exemple de Compteur
description: Parcourez le compteur Flux canonique — state, un Text lié, et un tap de Button — et comment il se réconcilie sur un tap.
---

C'est le plus petit programme Flux intéressant, et la forme sur laquelle le scénario
golden `counter_1000` du terrain de jeu est construit.

```flux
component Counter {
  state count: Int = 0

  Column(gap: 12) {
    Text("Count: {count}")
    Button(text: "Increment", onClick: {
      count = count + 1
    })
  }
}
```

## Ce que signifie chaque ligne

- `state count: Int = 0` — une cellule de signal mutable. L'hôte possède `count` ;
  le serveur ne voit jamais que son type et sa valeur initiale.
- `Text("Count: {count}")` — interpole le signal dans une chaîne. Le `signal_deps`
  de ce nœud est exactement `[1]` (l'id de `count`).
- `Button(text: "Increment", onClick: { ... })` — enregistre le handler id 7
  (dans le fixture du terrain de jeu) dont la closure écrit `count`.

## Ce qui se passe sur un tap

1. La closure `onClick` s'exécute dans la VM hôte, exécutant `WRITE_SIGNAL count, +1`.
2. La VM rapporte `signals: [1]` (l'id de signal écrit, ascendant).
3. L'hôte intersecte `{1}` avec le `signal_deps` de chaque nœud : seul le `Text`
   le lit, donc `dirty: [57]` (l'id de nœud du Text), dans l'ordre
   `(depth asc, id asc)`.
4. Le Text re-matérialise ses props et déclenche `update` — **une** mise à jour,
   **zéro** construction, ≤ 2 matérialisations de prop. Le `Column` et le `Button`
   restent intacts (`skip_unchanged` dans une ré-application complète).

Parcourez cette trace exacte sur le terrain de jeu de la page d'accueil.

## Les budgets

D'après `reconcile-counters-and-budgets.md` : un dispatch `counter_1000` doit produire
≤ 1 update, 0 construction, ≤ 2 matérialisations de prop — **indépendamment de la
taille de l'arbre**. La règle générale : chaque compteur après un dispatch est borné
par `|dependents[S]|` + la taille du diff structurel, jamais par la taille de l'arbre.
