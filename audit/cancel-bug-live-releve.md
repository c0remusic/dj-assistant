# Relevé — annulation du filing qui ne s'arrête pas (instrumentation live)

Le relevé précédent (cancel-latency-releve.md) concluait « hypothèse A : feedback manquant, le 1er
clic marche ». **INFIRMÉ par l'observation live** : le message « Stop requested… » n'apparaît pas au
1er clic, ça ne s'arrête jamais sans recliquer, et le compteur AVANCE encore entre les clics (de
NOUVEAUX fichiers sont filés après le clic). Donc le 1er clic ne pose pas le flag, ou il n'est pas lu.
**VRAI BUG.** La lecture statique nous a trompés une fois — cette fois on INSTRUMENTE pour voir
l'exécution réelle. **Aucune correction dans ce commit (logs only).**

> Note lecture statique : `FilingCancel(AtomicBool)` est `app.manage()` **une seule fois**
> ([lib.rs:55](src-tauri/src/lib.rs:55)) ; `file_cancel`, `file_batch` (reset) et `run_file_batch`
> (lecture) le résolvent tous via `app.state::<FilingCancel>()`, qui DEVRAIT être la même instance.
> Mais comme le statique nous a déjà menti, on **confirme par l'adresse `{:p}`** loguée aux trois
> sites : si elles divergent → flag non partagé (T2).

---

## Instrumentation posée

### Frontend — `onFileStop` ([sift-live.ts:201](frontend/sift-live.ts:201))
`console.log` (console DevTools) :
- `[cancel] STOP CLICKED — fileStopping(before)=…` — au tout début du handler.
- `[cancel] fileStopping(after)=true` — après avoir posé le flag local.
- `[cancel] invoking file_cancel…` — juste AVANT l'appel.
- `[cancel] file_cancel resolved` (ou `… FAILED <err>`) — APRÈS l'await (then/catch).

### Backend — logs `tauri_plugin_log` (stdout du terminal `npm run tauri dev`)
- `file_batch reset cancel=false (FilingCancel flag @ 0x…)` — au clic **File**, démarrage du batch
  ([ipc_filing.rs:107](src-tauri/src/ipc_filing.rs:107)).
- `run_file_batch START: total=N (FilingCancel flag @ 0x…)` — au début du thread de filing
  ([ipc_filing.rs:155](src-tauri/src/ipc_filing.rs:155)).
- `run_file_batch loop check: id=X cancel=<true|false>` — **à CHAQUE fichier** (tête de boucle)
  ([ipc_filing.rs:163](src-tauri/src/ipc_filing.rs:163)).
- `file_cancel CALLED, storing true (FilingCancel flag @ 0x…)` — au clic **Stop**
  ([ipc_filing.rs:124](src-tauri/src/ipc_filing.rs:124)).

Les **trois `@ 0x…`** (reset, START, file_cancel) doivent être IDENTIQUES si le flag est partagé.

---

## CE QUE L'UTILISATEUR DOIT FAIRE ET OBSERVER

1. `npm run tauri dev`. Ouvrir la **console DevTools** (frontend) ET garder le **terminal** visible
   (logs Rust).
2. Aller en **Revue → Batch**, choisir un lot **contenant des conversions** (hi-res/FLAC, encode lent),
   cliquer **File selection**.
   - Attendu terminal : `file_batch reset … @ 0xA`, puis `run_file_batch START … @ 0xB`, puis des
     `loop check: id=… cancel=false` qui défilent.
3. Cliquer **Stop UNE SEULE FOIS**, puis **ATTENDRE sans recliquer**. Noter :
   - Console : voit-on `[cancel] STOP CLICKED` ? avec `fileStopping(before)=` quoi ?
   - Console : voit-on `[cancel] invoking file_cancel…` puis `[cancel] file_cancel resolved` ?
   - Terminal : voit-on `file_cancel CALLED, storing true @ 0xC` ?
   - Terminal : les `loop check` d'APRÈS le clic disent `cancel=false` ou `cancel=true` ? combien de
     fichiers passent encore ?
4. Copier ces lignes (console + terminal) — elles tranchent la cause.

---

## Arbre de décision (T1/T2/T3/T4)

| Observation dans les logs | Cause |
|---|---|
| **Pas** de `[cancel] STOP CLICKED` au 1er clic | **T1** — le clic n'atteint pas le handler (bouton/listener). Le bouton est dans la zone de progression ; le listener est délégué sur la zone. À creuser : le clic touche-t-il bien `[data-pz-cancel]` ? |
| `STOP CLICKED — fileStopping(before)=true` | **T4** — `fileStopping` est resté à `true` d'un run précédent → `if (fileStopping) return` court-circuite (ni note, ni `file_cancel`). Reset manquant qq part (runBatchFile/onFileBatchDone). |
| `STOP CLICKED (before=false)` + `invoking file_cancel…` mais **PAS** de `file_cancel CALLED` côté Rust | **T1-bis** — l'invoke n'atteint pas la commande (binding `file_cancel` / routage IPC). Regarder un `file_cancel FAILED` éventuel en console. |
| `file_cancel CALLED … @ 0xC` **ET** `loop check … cancel=false` continue, avec **`0xC ≠ 0xB`** | **T2 — flag NON partagé** (suspect n°1) : la boucle lit un AtomicBool différent de celui que `file_cancel` modifie. |
| `file_cancel CALLED … @ 0xC` avec **`0xC == 0xB`** mais la boucle lit toujours `cancel=false` | Anomalie de partage/visibilité malgré la même adresse (à approfondir — ordering, double-thread). |
| `loop check … cancel=true` mais le filing continue (pas de `break`) ou un 2e `run_file_batch START` apparaît | **T3** — le flag est lu vrai mais on ne sort pas / un autre batch tourne en parallèle. |

**Discriminant clé n°1** : comparer `0xC` (file_cancel) vs `0xB` (run_file_batch START) vs `0xA`
(reset). Égaux ⇒ partagé (écarte T2) ; différents ⇒ T2 prouvé.
**Discriminant clé n°2** : la présence/absence de `[cancel] STOP CLICKED` et de `file_cancel CALLED`
sépare T1 (rien ne part) de T2/T3 (ça part mais la boucle ne réagit pas).

---

═══ (instrumentation — section conservée pour mémoire) ═══

---

## CAUSE RÉELLE PROUVÉE (trace live)

Le moteur d'annulation est **CORRECT** (aucun T1/T2/T3/T4) : au 1er clic, tout part bien —
```
[cancel] STOP CLICKED — fileStopping(before)= false
[cancel] fileStopping(after)= true
[cancel] invoking file_cancel… → file_cancel resolved
```
Le flag passe, `file_cancel` répond. Le bug est AILLEURS, dans la FIN de cycle, révélé par :
```
Uncaught (in promise) Error: requireEl: élément introuvable ".home-left" (renderHomeSources)
    at refresh (sift-live.ts:528)
    at onFileBatchDone (sift-live.ts:504)
```

### Le chemin exact (cité)
1. La boucle voit le flag → `break` → émet `file:done` → **`onFileBatchDone`** ([sift-live.ts:482](frontend/sift-live.ts:482)).
2. `onFileBatchDone` vide la zone (branche `cancelled`, 484-494) **puis** `try { await refresh() }`
   ([sift-live.ts:504](frontend/sift-live.ts:504)).
3. **`refresh()` rend TOUTES les vues à l'aveugle**, sans regarder la vue active
   ([sift-live.ts:527-531](frontend/sift-live.ts:527)) :
   ```
   await renderHomeSources();  // 528
   await renderQueue();        // 529
   await updateRevueBadge();   // 530
   ```
4. **`renderHomeSources`** fait `requireEl(".home-left", …)` ([home-sources.ts:59](frontend/home-sources.ts:59)).
   `.home-left` n'existe que quand la **Home** est montée dans `#content`. Or on est en **Review** →
   `requireEl` **throw** (P-4 fail-fast). (Le `requireEl("#content")` ligne 17 passe — `#content` est
   toujours là ; c'est `.home-left`, propre à la Home, qui manque.)
5. L'exception remonte hors de `await refresh()` → **`renderQueue()` et `updateRevueBadge()` ne
   s'exécutent JAMAIS** → la vue Review n'est PAS rafraîchie (les morceaux filés ne sont pas retirés
   du lot) → l'écran semble inchangé → **impression que « ça n'a pas marché »** → l'utilisateur
   reclique. L'annulation, elle, a bien eu lieu au 1er clic.

### Réponses aux 4 points
1. **refresh()** rend Home + Queue + badge Revue **inconditionnellement**, sans tester la vue active
   ([sift-live.ts:527](frontend/sift-live.ts:527)). C'est l'héritier du « tout rafraîchir » de
   `queue:changed`. `renderHomeSources` est donc appelé même en Review.
2. **renderHomeSources** exige `.home-left` car il réécrit le bloc « Watched folders » dans la colonne
   gauche de la Home. Cet élément n'existe **que** sur la Home. Il **devrait être SKIPPÉ** quand la
   Home n'est pas montée — exactement comme `renderQueue` qui fait déjà `if (!ql) return`
   ([sift-live.ts:91-92](frontend/sift-live.ts:91)).
3. **Le try/finally du BUG 1** ([sift-live.ts:503-512](frontend/sift-live.ts:503)) **n'a pas empêché**
   la casse : un `finally` n'**attrape pas** l'exception — il exécute le `fileNote(résumé)` PUIS
   **re-propage** l'erreur. Donc le résumé est bien posté, mais l'exception ressort quand même
   (« Uncaught in promise ») **et** `renderQueue`/`updateRevueBadge` (à l'intérieur de `refresh`,
   APRÈS le throw) ne tournent pas. Le try/finally était un pansement sur le **symptôme** (la note),
   pas sur la **racine** (refresh qui throw).
4. **Autres fragilités du chemin refresh()** :
   - `renderHomeSources` → `requireEl(".home-left")` **throw hors-Home** (la racine du bug). NON gardé.
   - `renderQueue` → `if (!ql) return` ([:92](frontend/sift-live.ts:92)) le protège hors-Review ; ses
     autres `requireEl("#fldz")`/`requireEl("#mid")` ne sont atteints que si `#ql` existe (donc on est
     en Review). **Auto-gardé**, faible risque.
   - `updateRevueBadge` → `requireEl('.nav-badge[data-badge="revue"]')` cible le **nav** (toujours
     monté) ([:537](frontend/sift-live.ts:537)). **Sûr.**
   → Seul `renderHomeSources` est la vue-renderer **non auto-gardée**. C'est l'asymétrie à corriger.

## VERDICT
- **Cause** : `refresh()` rend une vue (Home) **non montée** → `renderHomeSources` fait
  `requireEl(".home-left")` qui **throw** → la fin du cycle (`renderQueue`, `updateRevueBadge`, la
  suite) est cassée et l'erreur sort en « Uncaught in promise ». Le moteur d'annulation est correct.
- **Correction RACINE recommandée (option B, la plus cohérente)** : rendre **chaque view-renderer
  auto-gardé** — `renderHomeSources` doit **no-op proprement** (early `return`) quand sa racine
  (`.home-left`/la Home) n'est pas montée, **exactement** comme `renderQueue` fait déjà
  `if (!ql) return`. « Une seule façon de faire » = tout renderer baille si sa vue n'est pas à
  l'écran ; `refresh()` peut alors appeler les trois sans risque. Changement minimal, symétrique au
  pattern existant, ne touche pas au moteur.
  - *Alternative (option A, plus lourde)* : `refresh()` ne rend QUE la vue active (il doit alors
    connaître la vue active). Plus de surface, et `renderQueue` est déjà auto-gardé → B est plus
    simple et plus fidèle à l'archi.
- **Impact BUG 1 (texte « Filing in the background… » résiduel)** : **même cause racine.** Avant le
  pansement fed9590, le throw de `refresh()` sautait le `fileNote(résumé)` → la note restait. Le
  try/finally a forcé la note, mais a laissé la racine. Une fois `renderHomeSources` auto-gardé,
  `refresh()` se termine normalement → le résumé se poste dans le flux normal **et** la vue se
  rafraîchit. Le pansement try/finally devient **redondant** (inoffensif ; à simplifier plus tard).

═══ STOP ═══ Cause RÉELLE prouvée (exception `requireEl(".home-left")` dans `refresh()` hors-Home →
fin de cycle cassée). Reco racine = auto-garder `renderHomeSources` (pattern `if (!ql) return` de
`renderQueue`). AUCUNE correction tant que tu n'as pas validé l'approche.
NE PAS toucher (avant validation) : le moteur d'annulation (PROUVÉ correct), filing/revert,
TRASH_PURGE_DAYS, P-6.
