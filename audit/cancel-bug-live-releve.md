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

═══ STOP ═══ Instrumentation seulement. On lit les logs réels (1 clic, sans recliquer), on désigne
T1/T2/T3/T4, PUIS on corrige la vraie cause. Aucune correction tant que les logs n'ont pas tranché.
NE PAS toucher (rappel) : la logique de filing (plan/execute/commit), le moteur revert, la
confirmation « Filed ↩ », TRASH_PURGE_DAYS, P-6.
