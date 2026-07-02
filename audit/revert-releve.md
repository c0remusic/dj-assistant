# Relevé — REVERT d'un rangement (annuler le filing)

Enquête lecture seule (méthode détective, fail-fast). Aucune modification, aucun commit.
Opération DESTRUCTIVE (re-déplace des fichiers) → on relève tout, surtout le cas **conversion**.

---

## A. SÛRETÉ DU MOTEUR

### 1. Que font `revert_batch` et `undo_last`, étape par étape

**`revert_batch(conn, batch_id)`** ([actions.rs:98](src-tauri/src/actions.rs:98)) :
1. Charge les lignes **vivantes** du batch, **plus récentes d'abord** :
   `SELECT … FROM actions WHERE batch_id=?1 AND undone=0 ORDER BY id DESC` ([actions.rs:100-108](src-tauri/src/actions.rs:100)).
2. Si aucune ligne vivante → `Blocked("no live actions…")` ([actions.rs:109-111](src-tauri/src/actions.rs:109)).
3. **Garde LIFO** : `track_id = rows.iter().find_map(|r| r.1)` (= **le PREMIER track_id trouvé**, [actions.rs:114](src-tauri/src/actions.rs:114)) ; refuse si une action plus récente, hors ce batch, touche **ce** track : `count(*) … WHERE track_id=?1 AND undone=0 AND batch_id<>?2 AND id>?3` ([actions.rs:117-129](src-tauri/src/actions.rs:117)).
4. **Inverse chaque effet FS**, dans l'ordre chargé (récent→ancien) : `revert_one_fs(kind, from, to)?` ([actions.rs:132-134](src-tauri/src/actions.rs:132)). Le `?` **avorte au premier échec**.
5. Restaure le track : `UPDATE tracks SET status='pending', folder=NULL, target_format=NULL, confidence=NULL` + `DELETE FROM metadata` — **pour le SEUL `track_id` trouvé en 3** ([actions.rs:141-148](src-tauri/src/actions.rs:141)).
6. Marque les lignes : `UPDATE actions SET undone=1 WHERE batch_id=?1 AND undone=0` ([actions.rs:149-152](src-tauri/src/actions.rs:149)).

**`undo_last(conn)`** ([actions.rs:158](src-tauri/src/actions.rs:158)) : prend le `batch_id` de la ligne vivante **la plus récente** (`ORDER BY id DESC LIMIT 1`, [actions.rs:159-166](src-tauri/src/actions.rs:159)) puis appelle `revert_batch` dessus. Donc undo_last = « annule le DERNIER rangement ».

### 2. Types d'actions journalisées + ordre inverse

Types : **`convert` | `move` | `trash` | `reject`** (colonne `type`, [db.rs:47](src-tauri/src/db.rs:47)). `revert_one_fs` ([actions.rs:56](src-tauri/src/actions.rs:56)) les défait **TOUS** :
- **`move` | `trash`** ([actions.rs:64-78](src-tauri/src/actions.rs:64)) : `rename(to → from)` (remet le fichier à sa source). Gardes : `to` doit exister (sinon `Blocked("source gone")`), `from` ne doit PAS exister (sinon `Blocked("destination occupied")`), recrée le dossier parent.
- **`convert`** ([actions.rs:80-88](src-tauri/src/actions.rs:80)) : `remove_file(to)` (supprime le fichier converti). **Idempotent** si déjà absent.
- **`reject`** ([actions.rs:90](src-tauri/src/actions.rs:90)) : no-op (statut seul).
- inconnu → `Blocked("unknown action type")`.

L'ordre inverse est garanti par `ORDER BY id DESC` + l'ordre d'écriture dans `commit_file` ([filing.rs:309-316](src-tauri/src/filing.rs:309)), qui enregistre les `FsLog` dans l'ordre où `execute_file` les a produits.

### 3. Cas limites — comportement PROUVÉ

**Ce que le filing journalise réellement** ([filing.rs execute_file:250-285](src-tauri/src/filing.rs:250)) :
- **conformant** (pas de conversion) : tag en place → `rename(source → dest)` → **un seul** `FsLog "move"` (from=source, to=dest) ([filing.rs:252-260](src-tauri/src/filing.rs:252)).
- **non conformant** (CONVERSION) : `encode` vers dest → tag dest → trash de l'original → **DEUX** `FsLog` : `"convert"` (from=original, **to=dest converti**, [filing.rs:275](src-tauri/src/filing.rs:275)) **puis** `"trash"` (from=original, **to=chemin .sift-trash**, [filing.rs:277](src-tauri/src/filing.rs:277)). `trash_file_fs` déplace l'original dans `.sift-trash` ([filing.rs:118-124](src-tauri/src/filing.rs:118)).

→ ordre des `id` : `convert` < `trash`. Donc `revert_batch` (DESC) inverse **`trash` d'abord, `convert` ensuite**.

**3a — `from_path` (origine) n'existe plus** : `revert_one_fs` recrée le **parent** (`create_dir_all`, [actions.rs:73-76](src-tauri/src/actions.rs:73)) puis `rename`. Si la création échoue → `Blocked`. **Propre, pas de perte.**

**3b — collision (un fichier occupe déjà la source)** : `from` existe → `Blocked("destination occupied")` ([actions.rs:70-72](src-tauri/src/actions.rs:70)), **rien n'est déplacé**. **Échec clair, pas de perte.**

**3c — CONVERSION (le nœud) — risque de perte de données ?**
Séquence de revert d'un fichier converti :
1. **`trash` d'abord** : `rename(trash_path → original_source)` → **l'original revient à sa source**. Garde : trash_path doit exister, source ne doit pas exister.
2. **`convert` ensuite** : `remove_file(dest_converti)` → **le converti est supprimé du bin**.
Résultat : **original restauré, converti supprimé**. ✅ **Pas de perte.**

**Pourquoi c'est sûr** : l'ordre **`trash`→`convert`** restaure l'original AVANT de supprimer le converti. Et si le trash est inaccessible (voir ci-dessous), le `?` **avorte AVANT** la suppression du converti → l'utilisateur garde au moins le converti.
- **Si `.sift-trash` a été vidé** (`purge_trash` [ipc_filing.rs:255](src-tauri/src/ipc_filing.rs:255), ou `TRASH_PURGE_DAYS` un jour) : l'original est perdu **indépendamment du revert** ; le revert `trash` → `to` (trash) absent → `Blocked("source gone")` ([actions.rs:67-69](src-tauri/src/actions.rs:67)), avorte → **le converti reste dans le bin**. L'utilisateur ne perd PAS le converti. **Refus sûr, pas de perte causée par le revert.**
- **Aucun chemin où le revert laisse NI l'original NI le converti** : impossible tant que l'ordre est trash-d'abord + `?` avorte au 1er échec. La suppression du converti n'arrive **qu'après** la restauration réussie de l'original.

⚠️ **MAIS — incohérence en cas d'échec PARTIEL** : si `trash` réussit (original restauré) puis `convert` échoue (`remove_file` sur un fichier verrouillé), le `?` retourne `Err` **avant** l'étape 5/6 → les lignes restent `undone=0`, le track reste `status='filed'`, mais l'original est **physiquement déjà restauré**. Re-lancer le revert : `trash` → `to` (trash) n'existe plus → `Blocked`. **État bloqué (DB dit filed, fichier restauré, converti orphelin dans le bin)** — pas de perte, mais incohérent et non auto-réparable. Cas rare (verrou FS) mais réel.

**3d — `to_path` déplacé/renommé/supprimé entre filing et revert** : `move`/`trash` → `to` absent → `Blocked("source gone")` ([actions.rs:67](src-tauri/src/actions.rs:67)). `convert` → `to` absent → `remove_file` sauté (idempotent). **Échec clair (move/trash) ou no-op sûr (convert), jamais silencieux destructeur.**

### 4. Tests existants — couvrent-ils la conversion ?

Tests dans [actions.rs:222-388](src-tauri/src/actions.rs:222) :
- `revert_move_puts_file_back`, `revert_move_blocked_when_origin_occupied` (move) ([:251](src-tauri/src/actions.rs:251), [:261](src-tauri/src/actions.rs:261)).
- `revert_convert_deletes_converted_file` (**convert isolé**) ([:274](src-tauri/src/actions.rs:274)).
- `revert_reject_is_noop` ([:283](src-tauri/src/actions.rs:283)).
- `revert_batch_restores_file_and_status…` ([:306](src-tauri/src/actions.rs:306)) — mais son `seed_filed` ([:288](src-tauri/src/actions.rs:288)) journalise **`convert` + `move`** (même from/to), un mélange **synthétique qui ne correspond À AUCUN filing réel** (réel = `move` seul, ou `convert`+`trash` avec des `to` DIFFÉRENTS).
- `…blocked_when_newer_action`, `…unknown_is_blocked`, `undo_last_*`, `journal_lists_batches` ([:334](src-tauri/src/actions.rs:334)+).

→ **GAP CRITIQUE** : **aucun test ne couvre le vrai scénario de conversion** (converti dans le bin + original dans `.sift-trash`, puis revert = restaurer l'original depuis la corbeille **avant** de supprimer le converti). La sûreté du 3c est déduite par lecture du code, **non prouvée par un test**. Le path `"trash"` partage le code de `"move"` (testé), mais la **combinaison `convert`+`trash` de bout en bout** n'est pas testée, ni l'ordre trash-d'abord. **À couvrir AVANT d'exposer un bouton revert** sur les fichiers convertis.

### VERDICT SÛRETÉ

Le moteur est **sûr quant aux DONNÉES** : aucun chemin connu où le revert perd à la fois l'original et le converti (garanti par l'ordre `trash`→`convert` + `?` qui avorte + gardes qui bloquent au lieu de forcer). Les cas limites échouent **clair** (`Blocked`), jamais en silence destructeur.

**Réserves à traiter AVANT d'exposer le bouton** :
1. **Test manquant** du vrai cas conversion (`convert`+`trash`) et de l'ordre de restauration — **bloquant** (on n'expose pas un geste destructif non testé sur son cas le plus risqué).
2. **Échec partiel** (trash ok, convert ko) → état DB/FS incohérent non auto-réparable. À rendre robuste (ex. tolérer un convert manquant après un trash réussi, ou marquer undone malgré l'orphelin) — **important**.
3. **Dépendance à `.sift-trash`** : revert d'un converti impossible si la corbeille a été purgée (refus sûr). À **signaler dans l'UI** (« revert indisponible : original purgé »).
4. **Multi-track non supporté** (voir §6) — bloquant pour le revert « lot entier ».

---

## B. FAISABILITÉ DES DEUX GESTES UX

### 5. MODE DÉTAIL — « Filed ↩ »

**Ça existe déjà en partie.** Après `fileTrack`, `doRanger` ([filing.ts:665](frontend/filing.ts:665), appel [:679](frontend/filing.ts:679)) affiche `toast("Filed → …", true)` ([:680](frontend/filing.ts:680)) — le 2ᵉ argument `true` ajoute un **bouton « Undo »** dans le toast ([filing.ts:630-646](frontend/filing.ts:630)) qui appelle **`undoLast()`** ([:638](frontend/filing.ts:638)). Toast visible **6 s** ([:646](frontend/filing.ts:646)). Il existe aussi un **Ctrl+Z global** → `undoLast` (`installUndoShortcut` [filing.ts:852](frontend/filing.ts:852), monté [sift-live.ts:710](frontend/sift-live.ts:710)).

**Limites de l'existant vs la vision** :
- Le toast appelle `undoLast()` = annule le **dernier** batch, pas un batch ciblé. OK tant qu'un seul toast vit à la fois (l'ancien est retiré, [filing.ts:625](frontend/filing.ts:625)), mais fragile si on enchaîne.
- `fileTrack` **retourne `FileResult { path, batch_id }`** ([filing.rs:46](src-tauri/src/filing.rs:46), binding [ipc.ts:74](frontend/ipc.ts:74)) — mais `doRanger` **ignore** ce `batch_id`.
- **Le geste précis = capturer ce `batch_id` et appeler `revertBatch(batch_id)`** ([ipc.ts:126](frontend/ipc.ts:126)) au lieu de `undoLast`. Cela cible CE fichier, indépendamment de l'ordre.
- **Durée / après navigation** : le déplacement reste **revertable indéfiniment** via le journal tant que les lignes sont vivantes (`undone=0`) et que les gardes passent — `revertBatch(batch_id)` marche bien après avoir changé de fichier. Le toast 6 s n'est qu'une commodité ; un « Filed ↩ » persistant (sur la ligne du fichier rangé, ou via le journal) est faisable sans rien ajouter au moteur.

**Ampleur Détail : PETITE.** Capturer `batch_id` dans `doRanger`, remplacer `undoLast` par `revertBatch(batch_id)`, présenter « Filed ↩ » (persistant plutôt que 6 s). Back inchangé (commande `revert_batch` déjà exposée).

### 6. MODE BATCH — barre persistante + « Revert »

Deux pré-requis, dont **un nœud moteur** :

**(a) `batch_id` partagé — où, et impact**
Aujourd'hui `new_batch_id(track_id)` est appelé **dans `plan_file`** ([filing.rs:242](src-tauri/src/filing.rs:242), fn [:92](src-tauri/src/filing.rs:92)) → **un `batch_id` par fichier**. Pour un id partagé par lancement, il faudrait le générer **une fois dans `run_file_batch`** ([ipc_filing.rs:123](src-tauri/src/ipc_filing.rs:123)) et le pousser dans chaque `FilePlan` (param de `plan_file`, ou setter — `FilePlan.batch_id` est privé). **Schéma : aucun changement** (colonne `batch_id TEXT` déjà là, [db.rs:46](src-tauri/src/db.rs:46)).
**⚠️ MAIS** : `revert_batch` **ne gère qu'UN track par batch** — il prend `find_map` le **premier** `track_id` ([actions.rs:114](src-tauri/src/actions.rs:114)), ne reset le statut **que** de ce track ([actions.rs:141](src-tauri/src/actions.rs:141)), et la garde LIFO ne vérifie **que** ce track. Un `batch_id` partagé sur N tracks ⇒ revert qui inverse bien les FS de tous, **mais ne remet en `pending` qu'UN seul track** et journal/LIFO incohérents. **Donc le `batch_id` partagé EXIGE de généraliser `revert_batch` au multi-track** (boucle sur tous les `track_id` distincts du batch). Coût moyen + retests.

**(b) Alternative recommandée — PAS de batch_id partagé**
Garder un `batch_id` par fichier (revert unitaire **déjà testé**), et faire **collecter par `run_file_batch` la liste des `batch_id` réellement filés** (il appelle `commit_file` qui **retourne `FileResult{batch_id}`** [filing.rs:329](src-tauri/src/filing.rs:329) — aujourd'hui ignoré). Inclure cette liste (ou les track_ids) dans le payload `file:done`. « Revert le lot » = **boucler `revertBatch` sur ces ids, du plus récent au plus ancien** (pour respecter la garde LIFO). Réutilise le revert unitaire **tel quel**, **zéro changement de schéma, zéro réécriture de `revert_batch`**. (Option : une commande back `revert_batches(ids)` qui fait la boucle sous un seul lock.)

**(c) Zone de progression — état « annulé, revertable » persistant**
Aujourd'hui la zone ([progress-zone.ts](frontend/progress-zone.ts)) a `state: "running" | "done" | "error"` ([:14](frontend/progress-zone.ts:14)) et **se masque** : à la fin normale `pushFileProgress` flashe `done` puis `clearTask` après 1,2 s ; à l'annulation `onFileBatchDone` flashe le partiel puis `clearTask` ([sift-live.ts onFileBatchDone](frontend/sift-live.ts)). Le module sait **déjà** rendre un bouton d'action sur une ligne (le **Stop** de la sous-étape 3 : `cancelHandlers` + bouton délégué `[data-pz-cancel]`). Pour un état **persistant revertable**, il faut : un nouvel état terminal (ex. `"cancelled"`) qui **ne déclenche pas le clearTask**, + un **bouton Revert** (même mécanique que le Stop : un registre `revertHandlers` + bouton `[data-pz-revert]`), + côté `sift-live`, sur `file:done(cancelled)` : **ne pas masquer**, poser la ligne en `cancelled` avec le résumé partiel et brancher le Revert sur la liste de `batch_id` collectée. **Ampleur : MOYENNE** (réutilise le pattern Stop existant ; ajoute un état + un registre + le câblage).

**Ampleur Batch : MOYENNE** (option recommandée), **MOYENNE-GRANDE** si on choisit le `batch_id` partagé (réécriture multi-track de `revert_batch` + retests).

### 7. UI revert/undo existante

**Oui, partielle** : toast « Filed → … [Undo] » (6 s, [filing.ts:630](frontend/filing.ts:630)) + Ctrl+Z global ([filing.ts:852](frontend/filing.ts:852)), tous deux via `undoLast`. Bindings `undoLast`/`revertBatch`/`listJournal` exposés ([ipc.ts:123-130](frontend/ipc.ts:123)). **Pas** de bouton Revert persistant, **pas** d'usage de `revertBatch(batch_id)` ciblé, **pas** d'UI de journal.

---

## C. ORDRE DE DÉCOUPAGE RECOMMANDÉ (sous-étapes commitées)

1. **Filet de sécurité moteur (AVANT toute UI)** — ajouter les tests manquants du **vrai cas conversion** (`convert`+`trash` : converti au bin + original en `.sift-trash` → revert restaure l'original puis supprime le converti, dans cet ordre ; + cas trash purgé → `Blocked` sans perte) ; rendre l'**échec partiel** robuste (tolérer un converti déjà supprimé après un trash réussi). **Pur backend + tests. Rien d'exposé.** ← le plus important, c'est le cas 3c.
2. **Détail « Filed ↩ »** — capturer le `batch_id` de `FileResult` dans `doRanger`, exposer un revert **ciblé** `revertBatch(batch_id)` (persistant, pas seulement le toast 6 s). Petit, s'appuie sur (1).
3. **Collecte des `batch_id` du lot** dans `run_file_batch` + payload `file:done` (liste des filés). Petit backend, sans changement de schéma.
4. **Batch persistant + Revert** — état `cancelled` persistant dans `progress-zone` + bouton Revert (pattern du Stop) câblé sur la liste de (3) (boucle `revertBatch` récent→ancien). Moyen, front surtout.
5. *(Optionnel/différé)* — `batch_id` partagé + `revert_batch` multi-track, **seulement si** on veut un revert atomique « tout le lot en une ligne actions » plutôt que la boucle. Plus lourd ; non nécessaire pour l'UX visée.

═══ STOP ═══ Aucune correction ici. On lit et on décide — surtout sur le cas conversion (3c) et le
filet de tests (étape 1) avant d'exposer quoi que ce soit.
NE PAS toucher (rappel) : le code, TRASH_PURGE_DAYS, P-6.
