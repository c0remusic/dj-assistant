# Relevé — détacher le filing en arrière-plan (ÉTAPE 3)

Enquête lecture seule (méthode détective, fail-fast). Aucune modification, aucun commit.
But : préparer le détachement du filing + progression globale + annulation stop-net + journal des
déplacements (fondation revert, NON implémenté maintenant). On lit la carte, on décide du découpage.

---

## 1. `file_track` (mono) et `file_batch` (multi) : structure réelle

### `file_track` — chemin interactif, UNE piste ([ipc_filing.rs:51-77](src-tauri/src/ipc_filing.rs:51))
Signature : `file_track(app, conn, track_id, bin_rel, target?, edited?) -> Result<FileResult, String>`.
Déjà découpé en **3 phases** avec le lock relâché autour de l'encode :
1. **Phase 1 (SOUS lock)** — `plan_file` ([filing.rs:195](src-tauri/src/filing.rs:195)) : résout source, canonical
   (édité ou `reconcile_track`), target + garde no-upscale, crée le dossier dest + nom dest sans
   collision, charge `TagExtras` (label/year/genres/cover). **Lectures DB rapides uniquement** → `FilePlan`.
2. **Phase 2 (HORS lock)** — `execute_file` ([filing.rs:250](src-tauri/src/filing.rs:250)) : le travail lent.
   - **conformant** : `write_tags_full` en place → `std::fs::rename` dans le bin → `FsLog "move"`.
   - **non conformant** : `encode::encode` (transcode vers dest) → `write_tags_full` sur dest →
     `trash_file_fs` de l'original → `FsLog "convert"` + `"trash"`. Nettoie l'orphelin si échec.
3. **Phase 3 (SOUS lock)** — `commit_file` ([filing.rs:304](src-tauri/src/filing.rs:304)) : journalise chaque
   `FsLog` dans `actions` + `UPDATE tracks SET status='filed', folder, target_format, confidence` +
   `save_metadata`. En cas d'erreur DB : `rollback_fs` + purge du journal partiel (rien de demi-rangé).

Émet `queue:changed` une fois à la fin ([ipc_filing.rs:75](src-tauri/src/ipc_filing.rs:75)).
**Ordre** : (convertir si besoin) → taguer → déplacer → écrire DB. Mono-location garantie.

⚠️ MAIS `file_track` est une **commande sync** → tourne sur le **thread principal** → **gèle l'UI
pendant l'encode**, même pour une seule piste (un seul ffmpeg). Le lock est bien relâché en phase 2,
donc le worker d'analyse n'est PAS bloqué — seul le thread UI l'est.

### `file_batch` — chemin batch, N pistes ([ipc_filing.rs:81-96](src-tauri/src/ipc_filing.rs:81))
Signature : `file_batch(app, conn, track_ids, bin_rel) -> Result<BatchResult, String>`.
**Tient le lock DB sur TOUT le batch** ([ipc_filing.rs:88-93](src-tauri/src/ipc_filing.rs:88)) : il
appelle `filing::file_batch(&conn, …)` ([filing.rs:379](src-tauri/src/filing.rs:379)) qui **boucle** sur
les ids en appelant la variante **in-process** `filing::file_track(&conn, …)` ([filing.rs:336](src-tauri/src/filing.rs:336)) =
`plan_file` + `execute_file` + `commit_file` **tous avec le même `&Connection` tenu**. Donc **chaque
encode du batch tourne SOUS le lock**.
Sélection par piste : `canonical_from_metadata` (identité Discogs) sinon `reconcile_track` (doit sortir
**Green**) sinon → `needs_validation`. Émet `queue:changed` une fois à la fin ([ipc_filing.rs:94](src-tauri/src/ipc_filing.rs:94)).

→ **Le défaut est ici** : sync + lock tenu sur tout le batch ⇒ gèle l'UI **et** bloque le worker
d'analyse pendant toute la durée (tous les encodes). Pire qu'identify avant l'étape 1.

## 2. Le LOCK DB — VERDICT

| Chemin | Portée du lock | Encode | État |
|---|---|---|---|
| `file_track` (IPC mono) | 3 fenêtres courtes (phase 1, phase 3) | **hors lock** (phase 2) | bon modèle |
| `file_batch` (IPC) | **un seul lock sur tout le batch** | **sous lock** | défaut à corriger |

**VERDICT : OUI, passer le batch au lock par-fichier**, exactement comme l'identification est passée
de batch-global à par-item. Ce n'est PAS une nécessité technique que `file_batch` tienne le lock — c'est
un raccourci (`filing::file_batch` prend `&Connection`). La preuve qu'on peut faire mieux est **déjà
dans le code** : le chemin mono IPC relâche le lock autour de l'encode via les 3 primitives publiques
`plan_file` / `execute_file` / `commit_file`. Le corps détaché du batch n'a qu'à **boucler ces 3
phases par fichier** : `{lock: plan_file}` → `{hors lock: execute_file}` → `{lock: commit_file}`.
Aucune nouvelle logique métier ; on réutilise les phases existantes. `filing::file_batch(&Connection)`
devient inutile pour l'IPC (peut rester pour les tests). La sélection canonical/needs_validation se
fait dans la fenêtre lock de la phase 1 (lecture rapide).

## 3. Le déplacement réel — source et dest connus ?

- Déplacement = `std::fs::rename(&plan.source, &plan.dest)` ([filing.rs:259](src-tauri/src/filing.rs:259),
  conformant) et trash via `trash_file_fs` → `std::fs::rename(source, dest)` ([filing.rs:122](src-tauri/src/filing.rs:122)).
- **Source ET dest sont connus dès la phase 1** (`FilePlan.source` / `FilePlan.dest`, [filing.rs:164-175](src-tauri/src/filing.rs:164)).
  Journaliser « A → B » est donc trivial — **et c'est déjà fait** (voir §6).

## 4. La conversion (encode.rs) — avant/pendant le déplacement ? sous lock ?

- `encode::encode(src, dst, target)` ([encode.rs:115](src-tauri/src/encode.rs:115)) **spawn ffmpeg et
  BLOQUE le thread appelant** jusqu'à `child.wait()` ([encode.rs:149](src-tauri/src/encode.rs:149)). Synchrone.
- Appelée dans `execute_file` ([filing.rs:263](src-tauri/src/filing.rs:263)) = **phase 2**, **AVANT** le
  trash de l'original et après le choix de dest. Donc : convertir → taguer → trash original.
- **Sous lock ?** Mono IPC : **NON** (phase 2 hors lock). Batch IPC : **OUI** (execute_file via
  `filing::file_track` sous le lock tenu). C'est exactement le contraste avec l'identification (réseau
  hors lock) : ici la conversion est hors lock en mono mais **sous lock en batch** — le point à corriger.
- On **déplace** ce moteur (on le fait tourner sur un thread de fond, lock relâché), on ne le réécrit pas.

## 5. Contrat front↔back actuel

- **Mono (Detail)** : `await fileTrack(...)` ([filing.ts:679](frontend/filing.ts:679)) — le front **attend
  tout le filing** (bouton « Filing… » spinner, toast au retour, `clearPane`).
- **Batch** : `runBatchFile` → `await fileBatch(ids, batchBin)` ([sift-live.ts:383-387](frontend/sift-live.ts:383)) —
  attend tout le batch, puis met à jour `batchSel` + affiche « N filed · M need validation » dans `#filfoot`.
- **Events aujourd'hui** : seulement `queue:changed` à la fin (les deux chemins). **Aucune progression
  par fichier, aucune annulation.**

→ Pour détacher : le front **cesse d'attendre le résultat** et fait `lancement → rend la main →
s'abonne` (progression `kind="file"` dans la zone globale + event terminal avec résumé). **Miroir exact
du détachement d'identify (étape 1)** : un event `file:done` portant `BatchResult`, le front affiche le
résumé si le footer est présent.

## 6. Table/log des opérations — EXISTE DÉJÀ

- **`actions`** ([db.rs:44-51](src-tauri/src/db.rs:44) + colonnes `undone`/`batch_id` ajoutées
  [db.rs:84-85](src-tauri/src/db.rs:84)) :
  `id, track_id, type (convert|move|trash|reject), from_path, to_path, ts DEFAULT datetime('now'), undone, batch_id`.
- `actions::record(conn, batch_id, track_id, kind, from, to)` ([actions.rs:38](src-tauri/src/actions.rs:38))
  insère une ligne. `commit_file` **journalise déjà chaque `FsLog` (from→to) par fichier** ([filing.rs:309-316](src-tauri/src/filing.rs:309)).
- **Moteur de revert complet déjà présent et testé** : `revert_batch` / `undo_last` / `list_journal`
  ([actions.rs:98](src-tauri/src/actions.rs:98), [:158](src-tauri/src/actions.rs:158), [:193](src-tauri/src/actions.rs:193)),
  câblé IPC (`undo_last`, `revert_batch`).

→ **VERDICT : aucune nouvelle table à créer.** La « journalisation légère des déplacements (fondation
revert) » demandée **existe déjà** — et supporte même le revert. Si on réutilise plan/execute/commit
par fichier, le journal source→dest s'écrit **gratuitement, au fil de l'eau**, par fichier.
- *Nuance (future, hors scope)* : aujourd'hui chaque fichier d'un batch reçoit son **propre `batch_id`**
  (`new_batch_id` par `plan_file`, [filing.rs:92](src-tauri/src/filing.rs:92)) → l'undo est **par fichier**,
  pas « tout le batch en un ». Pour un « undo du batch entier » il faudrait un `batch_id` partagé. Petit,
  optionnel, **NON implémenté maintenant**.

## 7. Point d'insertion de l'annulation stop-net

- Le corps détaché bouclera sur `track_ids` **séquentiellement** (un seul `Mutex<Connection>`, et on veut
  des fenêtres de lock par fichier). Le point « entre deux fichiers » est **propre** : chaque fichier est
  atomique côté FS (rollback en cas d'échec), donc rien n'est à demi-fait à la frontière.
- **Mécanique** : un `Arc<AtomicBool>` (flag d'annulation) en managed state, vérifié **en tête de chaque
  itération, AVANT de planifier/encoder le fichier suivant** (`if cancel.load() { break; }`). Une commande
  `file_cancel` le met à `true`. Un fichier déjà en cours d'encode se termine (tuer ffmpeg en plein vol =
  hors scope) ; aucun **nouveau** fichier ne démarre. Résumé final = combien rangés avant l'arrêt.

---

## Où s'insère chaque morceau (récapitulatif)

| Morceau | Insertion | Réutilise |
|---|---|---|
| **Détachement (thread)** | `file_batch` IPC : lire root+template sous un lock court, puis `std::thread::Builder::spawn(run_file_batch)`, rendre la main (fail-fast si le spawn échoue). Corps `run_file_batch` tire `app.state::<Mutex<Connection>>()`. | **Miroir de `identify_batch` étape 1** |
| **Lock par-fichier** | dans `run_file_batch`, boucler `{lock: plan_file}` → `{hors lock: execute_file}` → `{lock: commit_file}` par id. | `plan_file`/`execute_file`/`commit_file` **déjà publics** |
| **Progression (event/fichier)** | émettre un event par fichier (done/total) depuis la boucle ; le front alimente la ligne `kind="file"` de la zone globale. | **`progress-zone.ts` (`setTask`/`clearTask`)** + miroir du wiring analyze |
| **Annulation (flag entre fichiers)** | `Arc<AtomicBool>` en state + commande `file_cancel` + `break` en tête de boucle + résumé partiel. | — (neuf, petit) |
| **Journalisation source→dest** | **rien à faire** : `commit_file` écrit déjà dans `actions` par fichier. | **`actions` + moteur revert (déjà là)** |

## Ampleur

- **Détachement batch + lock par-fichier** : **PETIT** (backend seul ; copie de la forme étape 1 + réemploi
  des 3 phases déjà publiques). C'est ce qui **supprime le gel** UI ET le blocage du worker.
- **Progression par fichier** : **PETIT** (event backend dans la boucle + wiring front sur la zone qui
  existe déjà).
- **Annulation stop-net** : **PETIT-MOYEN** (flag en state + commande `file_cancel` + bouton Stop dans
  l'UI batch + résumé partiel).
- **Journalisation** : **GRATUITE** (déjà en place et testée). *Undo-du-batch-entier* (batch_id partagé) =
  petit, différé.
- *(Optionnel, plus tard)* : détacher aussi `file_track` mono (aujourd'hui sync → gèle 1 fichier). Même
  patron de thread ; priorité basse (un seul encode).

## Ordre de découpage recommandé (commits séparés)

1. **Détacher `file_batch` + lock par-fichier** — le cœur. `run_file_batch` sur un thread, boucle des 3
   phases par fichier, le front cesse d'`await` et s'abonne à un `file:done` (résumé `BatchResult`). Le
   journal s'écrit déjà tout seul. **À lui seul, ce commit corrige le gel et le blocage de l'analyse.**
2. **Progression par fichier dans la zone globale** (`kind="file"`) — émettre un event de progression par
   fichier depuis la boucle ; câbler le front sur la zone (réutilise `setTask`/`clearTask`). S'appuie sur 1.
3. **Annulation stop-net** — `Arc<AtomicBool>` + commande `file_cancel` + bouton Stop + résumé partiel.
   S'appuie sur 1.
4. *(Différé / optionnel)* — undo du batch entier (batch_id partagé) ; et/ou détachement de `file_track`
   mono.

═══ STOP ═══ Aucune correction ici. On lit le verdict et on décide du découpage avant de toucher au code.
NE PAS toucher (rappel) : le code, TRASH_PURGE_DAYS, P-6, le moteur de conversion (on le déplace, pas
on ne le réécrit).
