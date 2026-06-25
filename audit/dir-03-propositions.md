# dir-03 — Propositions (uniquement sur le PROUVÉ)

> Rappel contrainte : dev **solo non-pro assisté IA**, temps = ressource rare, app gratuite,
> simplicité > complexité. Toute « meilleure façon » qui coûte une grosse réécriture pour un gain
> marginal est **explicitement déconseillée**. Rien n'est proposé sur un point RÉFUTÉ (A1, A2,
> B1, C1, C2, D1, D3).

Coûts en « j-IA » = jours de travail à ton rythme avec l'assistant.

---

## P-1 — Échelle typo + espacement tokenisée  ⟵ H-D2 (PROUVÉ ⭐), recouvre H-D4
**Problème prouvé** : 144 `font-size:` inline, zéro échelle type/space ; l'incohérence « pas pro »
vient de là (pas des couleurs, qui sont tokenisées à 198 usages).
- **Option A (recommandée)** : ajouter dans `styles.css` une échelle minimale — `--text-xs…xl`
  (reprendre les tailles déjà constatées 9/10/11/12.5/16/26) + `--space-1…6` (4/8/12/16/24/32, qui
  existe déjà comme **règle** dans la skill mais pas comme tokens) — puis **migrer au fil de l'eau**
  (chaque écran touché passe ses inline → tokens, à commencer par Revue). Pas de big-bang.
- **Option concurrente** : librairie de composants (design-system complet, classes BEM par
  élément). → **PAS LE COUP** pour un solo : sur-ingénierie, fige trop tôt, ralentit l'itération UI.
- **Coût** : 0,5 j poser l'échelle + helpers ; migration incrémentale ~0,25 j/écran (5 écrans).
  Risque casse **faible** (CSS additif, migration visuelle vérifiable à l'œil).
- **Bénéfice** : **D2** direct — cohérence verticale immédiate, fin des nombres magiques, base
  pour que tout futur écran soit « pro par défaut ».
- **Verdict : VAUT LE COUP MAINTENANT** (meilleur ROI visible/effort de tout l'audit ; tu viens
  justement de passer du temps à deviner des tailles au cas par cas).

## P-2 — Réconcilier README + CLAUDE.md avec le code  ⟵ H-M1 (PROUVÉ ⭐)
**Problème prouvé** : README dit « M2 à venir » (M6a livré) ; CLAUDE.md dit Symphonia non fait
(câblé). Cette mission a hérité d'une prémisse fausse à cause de ça.
- **Option A (recommandée)** : mettre à jour le tableau d'état README (M0→M6a faits, M6b en cours),
  la section Structure (ajouter sift-live/filing/report-view), et la note Symphonia de CLAUDE.md.
  `plan-implementation.md` est déjà bon → s'en servir de source.
- **Option concurrente** : générer l'état depuis le code (script). → **PLUS TARD** (gadget ; le
  coût d'un script > la mise à jour manuelle à cette échelle).
- **Coût** : **1–2 h**. Risque casse **nul** (docs).
- **Bénéfice** : **D1 indirect** (clarté de direction) — supprime la source de confusion qui fait
  prendre de mauvaises décisions (y compris « refaire M2 »).
- **Verdict : VAUT LE COUP MAINTENANT** (le moins cher, évite des erreurs de cap répétées).

## P-3 — Découper `sift-live.ts` (1144 l), extraction SÛRE seulement  ⟵ H-B5 (PROUVÉ, aggrave)
**Problème prouvé** : god-module doublé depuis le dernier audit ; 3 responsabilités séparables déjà
nommées par l'audit.
- **Option A (recommandée)** : extraire **uniquement** les blocs autonomes que les audits ont déjà
  ciblés — `ecartes-view.ts`, `chrome.ts` (titlebar + lean style + drag-drop), `home-sources.ts` —
  en gardant `sift-live` comme **installeur fin**. Déplacement mécanique, comportement identique.
- **Option concurrente** : réécrire le front en framework (Svelte/Lit) avec un store. → **PAS LE
  COUP** : réécriture massive d'un front qui marche, contre l'ADN « vanilla, chirurgical ».
- **Coût** : **0,5–1 j**, risque casse **moyen** (déplacements d'imports + ids partagés) → à faire
  avec tsc + un smoke run, **pas à l'aveugle**. Faire **avant** que le batch/M6b le regrossissent.
- **Bénéfice** : **D1** maintenabilité ; chaque futur ajout (M6b/M7) atterrit dans un module ciblé.
- **Verdict : VAUT LE COUP MAINTENANT** mais **borné** (les 3 extractions, rien de plus).

## P-4 — Garde-fou du contrat d'augmentation DOM  ⟵ H-B4 (PROUVÉ)
**Problème prouvé** : `sift-live` dépend d'ids créés par `app.js`, sans assertion → casse
silencieuse possible (anti-pattern vs « fail-fast »).
- **Option A (recommandée)** : à `installLiveWiring`, **asserter** la présence des ids/`window.__sift*`
  requis → **erreur explicite** si un id porteur manque (respecte fail-fast). + commenter les ids
  « porteurs » dans `app.js`.
- **Option concurrente** : **supprimer la couche `app.js`** et rendre tout le shell en TS (une seule
  source de rendu). → **PLUS TARD** : c'est la bonne cible de fond (élimine H-B3, H-B4, H-D4 d'un
  coup) mais c'est un chantier ; ne pas l'attaquer dans la même passe que P-3.
- **Coût** : Option A **2–3 h**, risque **faible**. Option concurrente ~2–3 j.
- **Bénéfice** : **D1** — transforme une classe de bug invisible en échec immédiat lisible.
- **Verdict : Option A VAUT LE COUP MAINTENANT** ; collapse de `app.js` = **PLUS TARD** (à
  reconsidérer quand la maquette navigateur cessera de servir).

## P-5 — Retirer les `#![allow(dead_code)]` des modules câblés  ⟵ H-B7 (PROUVÉ)
- **Option A** : retirer l'attribut fichier par fichier, traiter ce que clippy révèle (supprimer le
  vraiment mort, `pub`/`#[cfg(test)]` le légitime). **Option concurrente** : laisser. → la dette
  reste invisible.
- **Coût** : **2–4 h** (10 fichiers), risque **faible** (compilateur guidé). 
- **Bénéfice** : **D1** — fait ressortir le code mort (hygiène avant M6b/M7).
- **Verdict : VAUT LE COUP MAINTENANT** (cheap, mais après P-3 pour ne pas se croiser).

## P-6 — Pool de connexions DB (r2d2)  ⟵ H-B6 (PROUVÉ structurel / magnitude INDÉTERMINÉE)
- **Option A** : pool r2d2 + transactions/retry (WAL déjà posé). **Option concurrente (recommandée
  d'abord)** : **mesurer avant de fixer** — un test de charge scan+analyse sur un gros dossier réel,
  compter les `SQLITE_BUSY` loggués (le log existe déjà). Décider ensuite.
- **Coût** : pool **1–2 j** + risque **moyen** (touche toute la couche DB). Mesure **2–3 h**.
- **Bénéfice** : **D1** — mais **non prouvé nécessaire** aujourd'hui (mitigé). 
- **Verdict : PLUS TARD** — **ne pas faire le pool tant que la douleur n'est pas mesurée**. Faire
  d'abord la mesure (peu chère). La règle « pas de grosse réécriture pour gain marginal » s'applique.

## P-7 — Table erreur-code → message humain  ⟵ H-D5 (PROUVÉ, sévérité basse)
- **Option A** : une petite map front `code → message` (réutilise les codes stables `NO_TOKEN`,
  `RATE_LIMITED:<s>`…). **Option concurrente** : humaniser au cas par cas inline (ce qui est fait
  partiellement) → diverge.
- **Coût** : **2–3 h**, risque **nul**.
- **Bénéfice** : **D2** — finit le ressenti pro sur les erreurs.
- **Verdict : PLUS TARD** (à grouper avec la passe UX M6b Lot 5 déjà prévue dans la veille).

## P-8 — (Non technique) Mode « auto par règles »  ⟵ H-A3 (scope non bâti)
Pas une proposition de fix : un **rappel de séquencement**. Le mode *par défaut* annoncé n'existe
pas. **Arbitrage produit** (cf. RAPPORT) : soit le bâtir avant M7, soit assumer publiquement que la
V1 est « revue + batch » et repousser l'auto-règles. **Ne rien coder à l'aveugle ici.**

---

### Ce que je déconseille explicitement (anti-recommandations)
- **Réécrire le front en framework** : non. Le vanilla tient, M2/M3 (le plus dur) sont passés.
- **Codegen IPC (ts-rs/specta)** maintenant : non. Surface bornée (~40 cmd), tsc + discipline
  suffisent ; le coût build/CI Win+Mac n'est pas justifié par un bug réel. Revisiter si la surface
  explose. → **PAS LE COUP MAINTENANT**.
- **Pool DB** avant mesure : non (P-6).
- **Design-system / lib de composants** : non (P-1 option concurrente).
