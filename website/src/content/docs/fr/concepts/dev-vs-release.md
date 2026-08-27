---
title: Dev vs Release
description: Comment Flux s'exécute en dev (interprété, patché) par rapport à la release (SwiftUI / Compose natif généré).
---

import { Card, CardGrid } from '@astrojs/starlight/components';

Flux possède deux modes d'exécution. Les deux consomment le **même IR Reactive Tree**
produit par la passe d'abaissement du serveur de dev — ils diffèrent uniquement
par la façon dont l'hôte transforme cet IR en pixels.

## Mode dev — interprété et patché

En dev, une **app hôte précompilée** est déployée sur l'appareil. Le serveur de dev
parse le `.flux`, l'abaisse en IR Reactive Tree, le diffère par rapport à l'arbre
précédent, et envoie des **patches binaires** (Appendice D) via un WebSocket. La VM
en bytecode register-based et le graphe de signaux de style SolidJS embarqués dans
l'hôte appliquent le patch et mutent un shadow tree de vues natives.

- Itération rapide : sauvegarder → diff → patch, sans recompiler l'app.
- Introspection complète : la trame wire, le résultat de la VM et la trace de
  réconciliation sont tous observables (voir le terrain de jeu sur la page d'accueil).
- Le sink `trace` est **gratuit en production** (ADR-0027 INV-2) : aucun coût quand
  aucun driver n'est attaché.

## Mode release — natif généré

En release, le même IR est **généré** en Swift/SwiftUI et Kotlin/Jetpack Compose
idiomatiques. Il n'y a pas de VM, pas de protocole de patch, pas de shadow tree —
la sortie est une app native normale.

<CardGrid>
  <Card title="SwiftUI" icon="seti:swift">
    `component Counter` → `struct Counter: View`. `state` → `@State`.
    `Column(gap:)` → `VStack(spacing:)`.
  </Card>
  <Card title="Jetpack Compose" icon="seti:kotlin">
    `component Counter` → `@Composable fun Counter()`. `state` →
    `remember { mutableStateOf(...) }`. `Column(gap:)` →
    `Column(verticalArrangement = spacedBy(...))`.
  </Card>
</CardGrid>

## Pourquoi deux modes ?

La boucle dev a besoin de vivacité (patcher, ne pas recompiler) ; le produit livré a
besoin d'un coût d'exécution nul (codegen natif). Garder un seul IR comme contrat
signifie que l'expérience dev et le binaire release sont prouvablement le même
programme — ce que l'outil de trace de parité (FLUX-023) prouve exactement.
