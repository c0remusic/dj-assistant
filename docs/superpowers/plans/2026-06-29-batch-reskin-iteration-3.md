# Batch re-skin — itération 3 Implementation Plan

> **For agentic workers:** front-pur (sauf décision #3). Pas de runner de test unitaire front
> dans ce repo : vérification = `./node_modules/.bin/tsc --noEmit` + test live par Antoine.
> Standing rules : NE PAS COMMITER, tokens only, changements chirurgicaux, fail-fast.

**Goal:** corriger le bug "Aucun (clear)", rendre les groupes batch repliables, montrer le MOTIF
de nommage (pas 1752 noms), fondre "Excluded" dans "Selection", remonter "Destination" en pilule.

**Architecture:** tout en `frontend/sift-live.ts` (module god du mode batch) + un export DRY de
`targetExt` depuis `frontend/filing.ts`. État de sélection/repli en variables-module persistantes
(comme `batchBin`/`batchSel`). Aucun backend (voir décision #3 ci-dessous).

**Tech Stack:** Vite vanilla TS, classes `.sift-bgrp-*`/`.sift-fil-prev`, icônes Tabler (`ti`).

---

## Décision #3 — provenance du template (STOP avant backend)

Le filer Rust nomme via `naming::render_filename(template, …)` où `template` = setting
`FILENAME_TEMPLATE` **configurable** (defaut `"{artist} - {title}{version}"`, `settings.rs:17`),
lu par `template(&conn)` (`ipc_filing.rs:45`). Le FRONT n'a **aucune** IPC pour ce setting ; il
reconstruit déjà la convention par défaut en dur dans `previewName()` (`filing.ts:363`).

- **Option A (retenue, front-pur) :** rendre le motif depuis la convention par défaut déjà
  embarquée (`{artist} - {title}` → `Artiste - Titre.<ext>`), une ligne par extension distincte
  choisie. Aussi fidèle que l'aperçu "Final name" du détail déjà livré. Limite : un
  `FILENAME_TEMPLATE` personnalisé rendrait ce motif (et l'aperçu détail existant) périmé.
- **Option B (hop backend, sur demande) :** IPC lecture seule `get_filename_template` →
  motif vrai même customisé. Non codée tant qu'Antoine n'a pas tranché.

Ce plan implémente l'Option A. Bascule Option B = un seul `#[tauri::command]` + binding ipc.

---

### Task 1: Export `targetExt` (DRY pour le motif)

**Files:** Modify `frontend/filing.ts:356`

- [ ] `function targetExt(t: Target): string` → `export function targetExt(t: Target): string`
- [ ] Vérif : `./node_modules/.bin/tsc --noEmit` → PASS.

### Task 2: État + import (init guard, repli, targetExt)

**Files:** Modify `frontend/sift-live.ts` (import filing ~ligne 31-42 ; état ~ligne 70-85)

- [ ] Ajouter `targetExt` à l'import depuis `"./filing"`.
- [ ] Après `const batchSel = new Set<number>();` ajouter
      `let batchSelInit = false;` (commenté : auto-fill une seule fois, sinon "Aucun" est annulé).
- [ ] Ajouter `const batchCollapsed = new Set<string>();` (clés `kind:railKey`, persistant).

### Task 3: BUG #1 — "Aucun (clear)" opérant

**Files:** Modify `frontend/sift-live.ts` (renderBatch ~ligne 287-288 ; handler batchall ~ligne 1073-1078)

- [ ] renderBatch : remplacer
      `if (batchSel.size === 0) for (const it of ready) batchSel.add(it.id);`
      par `if (!batchSelInit && ready.length) { batchSelInit = true; for (const it of ready) batchSel.add(it.id); }`
- [ ] handler `batchall` : sur l'état "tout coché", vider AUSSI `batchFakeSel` :
      `if (batchSel.size === ready.length) { batchSel.clear(); batchFakeSel.clear(); }`

### Task 4: #2 — groupes repliables (chevron)

**Files:** Modify `frontend/sift-live.ts` (groupHeaderHtml ~437 ; railGroup ~379 ; fakes ~412 ; handler ~1101)

- [ ] `groupHeaderHtml` : calculer `gkey = kind:railKey`, `collapsed = batchCollapsed.has(gkey)`,
      insérer AVANT `box` un chevron `data-sift="batchcollapse" data-gkey` avec
      `ti-chevron-${collapsed ? "right" : "down"}`.
- [ ] `railGroup` : masquer les lignes si `batchCollapsed.has('file:'+rail)`.
- [ ] bloc Fakes : masquer `fakes.map(fakeRow)` si `batchCollapsed.has('fake:fake')`.
- [ ] Nouveau handler `batchcollapse` : toggle `batchCollapsed`, `renderBatch()`. La sélection
      n'est jamais touchée (cases tri-état comptent toujours via les ids).

### Task 5: #3/#4/#5 — motif, fusion exclus, destination en pilule

**Files:** Modify `frontend/sift-live.ts` (batchPreview→motif ~499 ; renderBatchRail ~543-574)

- [ ] Remplacer `batchPreview()` par `batchNameMotifHtml()` : `—` si rien ; sinon une ligne
      `${esc(dest)}/Artiste - Titre.${ext}` par extension distincte parmi les groupes sélectionnés
      (`targetExt(railTarget(rail))`), jointes par `<br>`. Convention = template par défaut.
- [ ] renderBatchRail : `destBlock` en pilule (`--color-background-secondary`,
      `--border-radius-md`, padding) ; ordre = Destination → Selection → Final name → tracks → action.
- [ ] Selection fond "Excluded" : `${n} à filer${jeter}${reviewN ? ' · <span tertiary>'+reviewN+' exclus (en review)</span>' : ''}` ; supprimer le bloc `Excluded` séparé.
- [ ] Final name : injecter `batchNameMotifHtml()` SANS `esc()` (déjà sûr).

### Task 6: Vérification

- [ ] `./node_modules/.bin/tsc --noEmit` → PASS (aucune erreur).
- [ ] REBUILD non requis (front pur). Test live Antoine (a–e du brief).
