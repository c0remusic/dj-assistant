# Relevé live #2 — l'annulation demande encore ~2 clics (après le fix requireEl)

Le fix de la classe requireEl était RÉEL (la console n'a plus d'exception, 3 clics → 2). Donc
requireEl était UNE cause, pas la seule. Il en reste (au moins) une. **On réinstrumente** ; on ne
retirera les logs que quand le bug sera prouvé mort. **Aucun fix dans ce commit.**

## Théorie à tester (churn DOM)

`progress-zone.render()` fait `zone.innerHTML = …` à CHAQUE `setTask` → il **détruit et recrée le
bouton Stop** à chaque redraw. Or pendant le filing dans un dossier surveillé :
- chaque `file:progress` → `pushFileProgress` → `setTask("file")` → render ;
- **et** le watcher s'emballe (`watch batch: N event(s)`) → `analysis:changed` → `pushAnalyzeProgress`
  → `setTask("analyze")` → render **complet** (la ligne file et son bouton Stop sont reconstruits
  AUSSI, même si seul "analyze" change).

Si un clic (mousedown→mouseup) **chevauche** un render, le bouton sous le curseur est remplacé → le
`click` ne se déclenche pas → clic « perdu », **sans exception** → l'utilisateur reclique.

## Instrumentation posée (logs only)

### Frontend (console DevTools)
- `onFileStop` ([sift-live.ts:201](frontend/sift-live.ts:201)) :
  `[cancel] STOP CLICKED — fileStopping(before)=…` / `[cancel] fileStopping(after)=…` /
  `[cancel] invoking file_cancel…` / `[cancel] file_cancel resolved` (ou `FAILED`).
- `progress-zone.render()` ([progress-zone.ts](frontend/progress-zone.ts)) :
  `[pz] render zone #N tasks=…` à chaque redraw (compteur N).
- `progress-zone.row()` : `[pz] stop button (re)created kind=file render#N` à chaque (re)génération du
  bouton Stop de la ligne file.
- `pushFileProgress` ([sift-live.ts:178](frontend/sift-live.ts:178)) : `[ev] file:progress done=…/…`.
- `onAnalysisChanged` (watcher) ([sift-live.ts:866](frontend/sift-live.ts:866)) :
  `[ev] analysis:changed (watcher) → pushAnalyzeProgress`.

### Backend (terminal `npm run tauri dev`)
- `file_cancel CALLED, storing true` ([ipc_filing.rs:124](src-tauri/src/ipc_filing.rs:124)).
- `run_file_batch loop check: id=X cancel=<true|false>` à chaque fichier
  ([ipc_filing.rs:163](src-tauri/src/ipc_filing.rs:163)).

## CE QUE L'UTILISATEUR DOIT FAIRE

1. `npm run tauri dev`. Console DevTools + terminal visibles.
2. Revue → Batch, lot **avec conversions** rangé dans un **dossier surveillé** (pour provoquer la
   rafale watcher), cliquer **File selection**.
3. Cliquer **Stop UNE SEULE FOIS**, **attendre sans recliquer**. (Si le 1er clic ne fait rien, en
   faire un 2e — mais NOTER précisément ce que chaque clic produit.)
4. Copier console + terminal autour du/des clic(s).

## Arbre de décision (H1/H2/H3)

| Observation | Cause |
|---|---|
| Le 1er clic ne produit **PAS** `[cancel] STOP CLICKED`, et juste avant/après on voit une rafale de `[pz] render zone` / `[pz] stop button (re)created` | **H1 + H3 — churn DOM** : le clic tombe sur un bouton remplacé par un re-render → `click` perdu. (Cause la plus probable.) |
| `[cancel] STOP CLICKED` sort au 1er clic **et** `file_cancel CALLED` côté Rust, mais `loop check cancel=false` continue / beaucoup de fichiers passent | **H2 — autre** : flag posé mais pas vu / latence / 2e batch (à re-creuser, churn écarté). |
| `[pz] stop button (re)created` se répète **en rafale** (plusieurs par seconde) pendant que l'utilisateur vise le bouton | **H3 — churn confirmé** : corrèle les rafales `[ev] file:progress` / `[ev] analysis:changed` avec les renders. |

**Discriminant clé** : présence/absence de `[cancel] STOP CLICKED` au **1er** clic.
- Absent + rafale de renders autour ⇒ **churn DOM** (H1/H3) → le fix visera à **ne pas reconstruire le
  bouton à chaque render** (ex. redraw ciblé par ligne, ou ne pas re-render la ligne file sur un
  setTask d'une AUTRE tâche, ou délégation stable) — *à décider après preuve*.
- Présent ⇒ le clic marche ; le problème est plus loin (H2) → on suit `file_cancel CALLED` +
  `loop check`.

Compter aussi : combien de `[pz] render zone` par seconde pendant le filing (mesure de la rafale).

═══ STOP ═══ Instrumentation seulement. 1 clic Stop, attendre, copier les logs → trancher H1/H2/H3.
Aucun fix tant que les logs n'ont pas désigné la cause ; on garde les logs jusqu'à preuve que le bug
est mort. NE PAS toucher : le moteur annulation/filing/revert, les fixes requireEl, TRASH_PURGE_DAYS, P-6.
