# Relevé — render storm de la zone de progression (feedback noyé → « plusieurs clics »)

Lecture seule. La preuve live a INFIRMÉ « churn-DOM-qui-perd-le-clic » : `[cancel] STOP CLICKED`
sort du 1er coup, `fileStopping` false→true, `file_cancel resolved`, le filing s'arrête (file:progress
stoppe à 1425, `render zone tasks=(none)`). **Le moteur d'annulation est CORRECT — on n'y touche pas.**

Cause réelle à confirmer : **render storm**. Le log montre `render zone #7912` pour 1425 fichiers
(~5,6 renders/fichier, ~14 renders/s), chacun reconstruisant tout le DOM de la zone alors que SEULS
deux nombres changent → thread UI sollicité en rafale → le feedback visuel du clic Stop est noyé →
l'utilisateur croit que ça n'a pas marché et reclique. **Problème de FEEDBACK, pas d'annulation.**

## 1. render() reconstruit-il tout à chaque appel ? → OUI

[progress-zone.ts:107-117](frontend/progress-zone.ts:107). À chaque appel :
```js
zone.innerHTML = [...tasks.entries()].map(([kind, p]) => row(kind, p)).join("");  // L116
```
`zone.innerHTML = …` **détruit et recrée TOUS les nœuds** de la zone (chaque ligne : icône, label,
compteur, **bouton Stop**, track, barre fill) — et pour **toutes** les tâches, pas seulement celle
qui a changé. `render()` est appelé par `setTask` ([:122](frontend/progress-zone.ts:122)) et
`clearTask` ([:127](frontend/progress-zone.ts:127)), donc à **chaque** `file:progress`
(`pushFileProgress`) **et** à chaque tick `analyze` (`pushAnalyzeProgress`, déclenché par les rafales
du watcher). Conséquence clé : un tick **analyze** reconstruit AUSSI la ligne **file** et son bouton
Stop, même si seul « analyze » a bougé.

## 2. Qu'est-ce qui change vraiment entre deux file:progress ? → 2 valeurs

Entre deux ticks d'une même tâche `running` (state/stopping inchangés), seuls changent :
- le **compteur** `.sift-pz-count` : `${p.done}/${p.total}` ([:94](frontend/progress-zone.ts:94)) ;
- la **largeur de barre** `.sift-pz-fill` : `width:${pct}%` ([:96](frontend/progress-zone.ts:96)).

Tout le reste est **identique** : `rowClass`, `label`, l'icône, le nom, et le **bouton Stop**
(structure produite par `row()` [:90-98](frontend/progress-zone.ts:90)). Ils ne varient QUE si
`state`/`stopping`/`showStop` changent (début, clic Stop, fin) — pas à chaque tick.

## 3. Raison de recréer structure + bouton à chaque tick ? → AUCUNE

C'est le pattern naïf « re-render tout depuis l'état ». Aucune nécessité : la structure d'une ligne
`running` est stable entre deux ticks. Recréer le bouton Stop 14×/s (et le faire reconstruire par les
ticks d'une AUTRE tâche) est du pur gaspillage — et fait clignoter/réinitialiser le retour visuel
(état « Stopping… », bouton) que l'utilisateur regarde après son clic.

## Correction minimale recommandée — « create once, update in place »

Séparer **création** (structure + bouton, une seule fois par ligne tant que sa signature structurelle
ne change pas) de **mise à jour** (écrire les 2 valeurs sur les nœuds existants à chaque tick).

- **Cache par-tâche** : `Map<TaskKind, HTMLElement>` (la ligne) + une **signature structurelle**
  = `{state, stopping, showStop, label}` (ce qui détermine `rowClass`, le texte du label, et la
  présence du bouton Stop).
- **`render()` réconcilie** : ajoute les lignes des nouveaux `kind`, retire celles des `kind`
  disparus, respecte l'ordre d'insertion.
- **Pour chaque `kind`** :
  - signature **inchangée** → écrire seulement `countEl.textContent = \`${done}/${total}\`` et
    `fillEl.style.width = \`${pct}%\`` sur les nœuds existants (les retrouver une fois et les
    mémoriser, ou `row.querySelector`). **Pas de innerHTML.**
  - signature **nouvelle/changée** → (re)construire CETTE ligne uniquement (début, clic Stop →
    `stopping`, fin → `done`).
- Le **listener délégué reste sur la zone** ([:66](frontend/progress-zone.ts:66)) — déjà stable ;
  en prime le **nœud bouton n'est plus jamais recréé**.

### Effet
- **14 reconstructions DOM/s → 14 écritures de texte/s.** Un tick `analyze` ne touche QUE la ligne
  analyze → la ligne **file** et son bouton Stop ne sont plus reconstruits par les rafales du watcher.
- Bouton Stop **stable** (nœud + retour visuel « Stopping… » qui ne clignote plus) → feedback
  instantané au clic.
- Cohérent avec la responsabilité du module (store + rendu) : création/maj séparées.

### Honnêteté sur le périmètre
La zone de progression est **un** contributeur. Les rafales watcher déclenchent aussi
`renderQueue(false)` (debounced 300 ms) qui reconstruit `#ql`, et `pushAnalyzeProgress` fait un IPC
`analysis_progress` — hors périmètre de CE relevé (ciblé `progress-zone.ts`). Le fix create-once de la
zone est correct et nécessaire indépendamment : il supprime la reconstruction du **bouton que
l'utilisateur clique** et la part de churn DOM de la zone. Si le feedback reste noyé après, on
instrumentera/optimisera les autres contributeurs séparément.

## CONCLUSION — fix validé en live (commit create-once `3130d49`)

Implémenté en `progress-zone.ts` (cache `RowCache {rowEl,countEl,fillEl,sig}` par `kind`, signature
`{state,stopping,showStop,label}` ; sig stable → 2 écritures de texte, sig changée → reconstruction
de la seule ligne). **Test terrain de l'utilisateur :**
- 3 clics Stop sur tout le log, chacun = **1 seul clic**, `STOP CLICKED` immédiat, `file_cancel
  resolved`, filing arrêté net (`tasks=(none)`). Le ressenti « plusieurs clics » a disparu.
- `stop button (re)created` = **3 fois sur tout le log (≈1/batch)** au lieu de ~14/s avant → le
  render storm du bouton est **éliminé**. Feedback instantané confirmé.

→ Bug « annulation = plusieurs clics » **PROUVÉ MORT**. Le correctif est bien le create-once
(séparation création/mise-à-jour), pas les logs.

**Nettoyage** (commit suivant) : toute l'instrumentation d'enquête retirée — front `[cancel]`
(onFileStop), `[pz]` (progress-zone : render zone + stop button + le compteur `renderSeq`), `[ev]`
(sift-live : file:progress + analysis:changed) ; Rust `file_cancel CALLED` + `loop check`. Grep
`[cancel]`/`[pz]`/`[ev]` hors `audit/` = 0. `tsc` + `cargo check` verts. Le fix reste ; seuls les
`console.log`/`log::info!` partent.

**Reste hors périmètre (non requis, à mesurer SI besoin)** : les autres contributeurs du churn
(`renderQueue(false)` qui reconstruit `#ql`, l'IPC `analysis_progress`) — non touchés ; le feedback
est désormais instantané, donc rien à faire pour l'instant.

═══ STOP ═══ Fix create-once livré ET validé en live (storm bouton ~14/s → ~1/batch, feedback
instantané, clic pris du 1er coup). Instrumentation retirée. NE PAS toucher : le moteur
annulation/filing/revert (prouvé correct), les fixes requireEl, le cache de lignes de progress-zone,
TRASH_PURGE_DAYS, P-6.
