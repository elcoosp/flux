---
title: État Autoritatif de l'Hôte
description: Pourquoi l'état Flux vit dans le graphe de signaux de l'hôte, et non dans le serveur, et ce que cela implique pour le patchage (ADR-0002).
---

L'état Flux est **autoritatif côté hôte**. Le serveur ne possède jamais les valeurs
de signaux à l'exécution ; il envoie la structure (l'IR) et les deltas (les patches).
L'hôte possède le graphe de signaux, évalue les handlers et réconcilie la vue.

## L'invariant

> Après qu'un dispatch a écrit l'ensemble de signaux `S`, les seuls nœuds dont la
> sortie rendue peut changer sont (a) les nœuds dont les expressions de prop/contrôle
> lisent un `s ∈ S`, et (b) les nœuds construits/détruits/réordonnés par des diffs
> structurels par clé déclenchés par (a). Tout le reste doit rester intact.

Ceci est normatif (ADR-0027). C'est ce qui fait qu'un arbre de 1 000 nœuds coûte
**une mise à jour** quand on tape un compteur lié à un seul signal — indépendamment
de la taille de l'arbre. Le terrain de jeu de la page d'accueil rejoue exactement ce
scénario.

## Conséquences

- **Les patches sont minimaux.** Un tap `count = count + 1` produit un patch `Update`
  adressé uniquement au(x) nœud(s) sale(s), pas un renvoi de tout l'arbre.
- **L'aller-retour serveur est supprimable.** En Phase 3 (ADR-0027) les prop-thunks
  sont envoyés à l'hôte, donc l'aller-retour serveur par tap et la reconstruction
  `currentFrame()` sont supprimés entièrement — l'hôte recalcule à partir du dirty set.
- **La parité des traces est observable.** Comme l'état est possédé par l'hôte et
  déterministe, la même frame + script de dispatch exécutés contre Swift et Kotlin
  produisent des traces octet-pour-octet identiques (`reconcile-trace-format.md`).

## Ce que cela exclut

- Le réconciliateur ne doit **pas** s'abonner au graphe de signaux comme un observateur
  — cela déclenche en double. Il consomme la *sortie* de la VM (ADR-0027, hors périmètre).
- Un handler qui écrit un signal que rien ne lit produit `dirty: []` et zéro
  événement update/build/detach (golden `noop_dispatch`).
