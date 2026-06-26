# Relevé — doublon `.aif` + `.aiff` après revert d'un AIFF lossless

Enquête lecture seule (méthode détective, fail-fast). Aucune modification, aucun commit.
Bug : après revert d'un AIFF lossless, deux fichiers même nom de base, extensions `.aif` ET `.aiff`,
dans le même dossier. On prouve la cause avant toute correction (opération destructive).

---

## 1. Conversion systématique ? (un AIFF conforme est-il reconverti ?)

**Non pour un AIFF déjà 16-bit/44.1 ; oui pour un AIFF non conforme (ex. 24-bit hi-res).**
- `is_conformant(path, Aiff1644)` accepte `.aif` ET `.aiff`, et exige que lofty lise
  `sample_rate==44100 && bit_depth==16` ([encode.rs:91-109](src-tauri/src/encode.rs:91), check ligne [:100](src-tauri/src/encode.rs:100) ; extensions `["aif","aiff"]` ligne [:107](src-tauri/src/encode.rs:107)).
- Qu'un AIFF 16/44.1 soit bien reconnu conforme est **prouvé par le test** `encodes_flac_to_conformant_aiff` ([encode.rs:230-241](src-tauri/src/encode.rs:230), assert [:241](src-tauri/src/encode.rs:241)).
- Donc : AIFF 16/44.1 → **conformant=true → DÉPLACÉ** (branche conformant [filing.rs:252-260](src-tauri/src/filing.rs:252)). AIFF non-16/44.1 → **conformant=false → CONVERTI** (branche [filing.rs:261-283](src-tauri/src/filing.rs:261)).

→ La reconversion n'est PAS systématique. Mais **le changement d'extension `.aif`→`.aiff` l'est** (voir §2), dans **les deux** branches.

## 2. Extension de sortie — LE POINT DE DIVERGENCE (prouvé)

Le nom du fichier de destination utilise **TOUJOURS `target.ext()`**, jamais l'extension de la source :
- `Target::Aiff1644.ext()` = **`"aiff"`** (4 lettres) ([encode.rs:27-33](src-tauri/src/encode.rs:27)).
- `render_filename(template, canonical, target.ext())` ([filing.rs:221](src-tauri/src/filing.rs:221)) →
  `format!("{}.{}", sanitize(stem), ext)` ([naming.rs:185](src-tauri/src/naming.rs:185)).
- `dest = ensure_unique(dest_dir.join(filename))` ([filing.rs:222](src-tauri/src/filing.rs:222)) — `ensure_unique` conserve l'extension ([library.rs:282-300](src-tauri/src/library.rs:282)).
- La source `.aif` route bien vers cette cible : `rail_from_ext("aif")=Lossless` ([tags.rs:24](src-tauri/src/analysis/tags.rs:24)) → `target_for(Lossless)=Aiff1644` ([encode.rs:66](src-tauri/src/encode.rs:66)).

→ **PROUVÉ** : une source nommée `X.aif` (3 lettres) produit une destination `…/X.aiff` (4 lettres),
que le filing **convertisse** (encode vers `.aiff`) **ou simplement déplace** (rename `.aif`→`.aiff`).
C'est ici que `.aif` et `.aiff` divergent : **même audio, nom de fichier forcé à `.aiff`**. (`.aif` et
`.aiff` sont le même format AIFF ; seul le nom change.)

## 3. Ce qui est journalisé (prouvé)

`execute_file` pousse des `FsLog` que `commit_file` écrit dans `actions` ([filing.rs:309-316](src-tauri/src/filing.rs:309)) :
- **conformant (déplacé)** : `move(from = src/X.aif, to = bin/X.aiff)` ([filing.rs:260](src-tauri/src/filing.rs:260)).
- **non conformant (converti)** : `convert(from = src/X.aif, to = bin/X.aiff)` ([filing.rs:275](src-tauri/src/filing.rs:275))
  **puis** `trash(from = src/X.aif, to = .sift-trash/<id>__X.aif)` ([filing.rs:277](src-tauri/src/filing.rs:277)).
  `trash_file_fs` garde l'extension d'origine `.aif` dans le nom du fichier corbeille ([filing.rs:118-124](src-tauri/src/filing.rs:118)).

→ Les extensions journalisées **correspondent aux fichiers réels** : `from`/trash portent `.aif` (l'original),
`to` (convert/move) porte `.aiff` (le résultat). Après un filing RÉUSSI, **un seul** fichier subsiste
dans le bin (`X.aiff`) — l'original `.aif` est soit renommé (move), soit en corbeille (convert).

## 4. Au revert — le moteur DEVRAIT éliminer le doublon (prouvé), donc pourquoi persiste-t-il ?

`revert_batch` lit les lignes du batch **récent→ancien** et inverse chaque effet ([actions.rs:100-134](src-tauri/src/actions.rs:100)) :
- **move-revert** : `rename(to → from)` = `bin/X.aiff → src/X.aif` ([actions.rs:64-78](src-tauri/src/actions.rs:64)). Gardes : `to` doit exister, `from` ne doit PAS exister.
- **convert-revert** : `remove_file(to)` = supprime `bin/X.aiff` ([actions.rs:80-88](src-tauri/src/actions.rs:80)).
- **trash-revert** (traité AVANT convert car id plus grand) : `rename(.sift-trash/…X.aif → src/X.aif)` restaure l'original.

Sur **succès propre**, il reste **exactement un fichier** : `X.aif` (move : renommé en `.aif` ; convert : original restauré + converti `.aiff` supprimé). Le test `revert_batch_conversion_restores_original_and_deletes_converted` ([actions.rs:~395](src-tauri/src/actions.rs:395)) prouve ce nettoyage pour le cas convert.

→ **Donc un doublon `.aif`+`.aiff` PERSISTANT prouve qu'une étape FS du revert n'a PAS abouti.** Le
seul chemin que le code permet pour laisser les DEUX :

**Un `revert_one_fs` retourne `Blocked` (erreur FS), le `?` avorte ([actions.rs:133](src-tauri/src/actions.rs:133)) — et depuis le durcissement par-ligne (commit 611c358) la restauration `trash` de `X.aif` a DÉJÀ été commitée (elle est traitée en premier), donc `X.aif` est revenu mais `X.aiff` n'a pas été supprimé.** Résultat : `X.aif` (restauré) **+** `X.aiff` (converti non supprimé) dans le même dossier, ligne `convert` restée vivante (track coincé `filed`).

Déclencheurs FS plausibles (à **confirmer par reproduction**, non prouvés ici) :
- **a. Le `.aiff` est ouvert/verrouillé sous Windows** au moment du revert → `remove_file` échoue
  (« Access denied »). Candidat fort : si le bin de la bibliothèque est une **source surveillée**, le
  nouveau `.aiff` est ré-enfilé et **le worker d'analyse** (Symphonia/lofty) le tient ouvert ; ou un
  antivirus/indexeur Windows. (Le lecteur, lui, ne tient PAS le `.aiff` : pour un AIFF il joue un WAV
  temporaire via `playback_url` [ipc.rs:280-308](src-tauri/src/ipc.rs:280).)
- **b. Branche move** : si `X.aif` a **réapparu** à `from_path` entre filing et revert (re-scan, etc.),
  le move-revert est `Blocked("destination occupied")` ([actions.rs:70-72](src-tauri/src/actions.rs:70)) → `X.aiff` reste, `X.aif` présent → doublon.

L'ordre du revert (trash→convert) et les gardes sont **corrects** (prouvé §4 + tests) — ce n'est PAS un
bug de logique d'inversion ; c'est une **opération FS qui échoue** et laisse le système à mi-chemin.

### Tests existants couvrant ce cas ?
Le cas **conversion réussie** est couvert (actions.rs). Le cas **`.aif`→`.aiff` spécifiquement** et le
cas **échec de suppression du converti pendant le revert** d'un vrai AIFF ne le sont pas. Le test
`revert_batch_resumes_after_partial_fs_failure` (commit 611c358) prouve la re-tentabilité mais avec un
`convert` synthétique — il ne modélise pas le doublon `.aif`/`.aiff` vu par l'utilisateur.

---

## VERDICT

1. **Cause de la DIVERGENCE `.aif`/`.aiff` (prouvée)** : le filing force l'extension de sortie à `.aiff`
   (`target.ext()`), sans préserver l'extension `.aif` de la source ([encode.rs:30](src-tauri/src/encode.rs:30) + [naming.rs:185](src-tauri/src/naming.rs:185) + [filing.rs:221](src-tauri/src/filing.rs:221)). Même format, nom différent. Inoffensif en soi.
2. **Cause de la PERSISTANCE du doublon (mécanisme prouvé, déclencheur à reproduire)** : une étape FS du
   revert échoue (`Blocked`) ; le durcissement par-ligne ayant déjà restauré `X.aif`, le `X.aiff` non
   supprimé reste → les deux coexistent. Ce n'est PAS la logique du revert qui est fausse, c'est une
   opération FS qui ne passe pas (fichier verrouillé le plus probable). **À PROUVER par repro** avant
   tout correctif — ne pas deviner.

## Options de correction (coût / risque) — AUCUNE destructive proposée

- **A. (racine, recommandée) Préserver l'extension de la source quand le conteneur correspond déjà.**
  Pour une source AIFF, produire la destination avec **la même extension** que la source (`.aif`→`.aif`,
  `.aiff`→`.aiff`) au lieu de forcer `.aiff`. Supprime la divergence `.aif`/`.aiff` (et le rename inutile
  dans la branche conformant). Coût : **petit-moyen** (passer l'ext source au calcul du nom de dest dans
  `plan_file`/`render_filename` ; décider de l'ext pour la branche convert). Risque : **faible-moyen**
  (touche naming/encode ; ré-exécuter les tests de filing). Ne corrige pas à lui seul un revert qui
  échoue, mais élimine la confusion à la racine.
- **B. (filet de sécurité) Reproduire + durcir l'observabilité.** Ajouter un test qui FILE un vrai `.aif`
  AIFF puis revert, en asseyant « un seul fichier » ; et faire remonter l'échec FS du revert (le moteur
  renvoie déjà `Blocked` ; le front l'affiche depuis la sous-étape 2). Optionnel : log explicite du
  chemin qui a échoué. Coût : **petit**. Risque : **faible**. **C'est l'étape qui PROUVE le déclencheur**
  (verrou worker d'analyse vs autre) avant de toucher au reste.
- **C. (NON recommandée, destructive) Au revert, supprimer aussi le frère même-stem d'extension AIFF
  différente.** Risque de **perte de données** (effacer un fichier que l'utilisateur a légitimement).
  À écarter tant que le déclencheur n'est pas prouvé.

**Ordre recommandé** : d'abord **B** (reproduire pour PROUVER le déclencheur de l'échec FS — soupçon n°1 :
le worker d'analyse tient le `.aiff` quand le bin est une source surveillée), puis **A** (supprimer la
divergence d'extension à la racine). Jamais **C**.

---

## ÉTAPE B — DÉCLENCHEUR PROUVÉ (reproduction, 2026-06-26)

Instrumentation + reproduction livrées (backend/tests uniquement, AUCUNE correction racine).

### Instrumentation (commit `f04c4ca`)
`revert_batch` logue désormais l'échec FS au point unique d'inversion, avec l'erreur OS brute
([actions.rs:135](src-tauri/src/actions.rs:135)) :
`log::error!("revert_batch {batch_id}: FS step '{kind}' failed (from=… to=…): {e}")`. Comportement
inchangé (même `Blocked`, même marquage par-ligne) ; l'erreur n'est plus seulement dans le toast
frontend mais aussi dans le log applicatif (`tauri_plugin_log`). En production, la PROCHAINE
occurrence imprimera l'erreur OS réelle → on saura si c'est 32 (verrou), 5 (permissions) ou autre.

### Reproduction (tests `actions.rs`)
Géométrie fidèle : **MÊME dossier** (le filing garde le morceau dans son répertoire) → `Track.aif`
converti en `Track.aiff` + original en `.sift-trash`, ordre réel `convert` puis `trash`.

| Test | Mécanisme du verrou | Résultat |
|---|---|---|
| `cold_revert_of_aif_filing_leaves_single_file` (2a) | revert à froid, rien ne tient le fichier | **1 seul fichier** (`Track.aif` restauré, `.aiff` supprimé). Logique correcte. |
| `windows_std_reader_does_not_block_revert` (2b-i) | handle tenu via `std::fs::File::open`, **exactement comme le worker** ([decode.rs:31](src-tauri/src/analysis/decode.rs:31) + lofty `Probe::open`) | revert **réussit**, 1 seul fichier. **std ouvre avec FILE_SHARE_DELETE → un lecteur std NE bloque PAS.** |
| `windows_held_handle_reproduces_aif_aiff_duplicate` (2b-ii) | handle SANS partage-suppression (`share_mode(FILE_SHARE_READ)`) | revert **Blocked**, **doublon reproduit** : `Track.aif` + `Track.aiff` coexistent. Handle relâché → re-run finit en **1 fichier**. |

### Erreur OS réellement observée (variante 2b-ii)
```
remove converted: Le processus ne peut pas accéder au fichier car ce fichier est utilisé par un
autre processus. (os error 32)
```
= **ERROR_SHARING_VIOLATION (os error 32)** — PAS `os error 5` (Access denied) supposé au §4. 32 = un
autre handle tient le fichier sans autoriser la suppression. (5 indiquerait une cause DIFFÉRENTE :
ACL / lecture seule — à distinguer via le log d'instrumentation si ça se produit en vrai.)

### VERDICT sur le soupçon « verrou-worker » (n°1) : **INFIRMÉ tel quel**
- Le worker d'analyse ouvre les fichiers via **std** (`File::open` [decode.rs:31](src-tauri/src/analysis/decode.rs:31) ;
  lofty `Probe::open`). La variante 2b-i **prouve** qu'un handle std n'empêche pas `remove_file`
  (std inclut FILE_SHARE_DELETE). Donc le worker de Sift, **tel qu'il est codé, ne peut pas** être la
  cause du doublon. Le watcher (`notify`/ReadDirectoryChangesW) surveille le DOSSIER, pas le fichier
  individuel → ne bloque pas non plus.
- Le doublon n'est reproductible **que** par un handle ouvert **sans** partage-suppression (2b-ii).
  Sur Windows, ce sont les processus **externes** qui ouvrent ainsi, surtout sur un fichier qui vient
  d'apparaître dans un dossier surveillé : **indexeur Windows Search, antivirus, aperçu/miniature de
  l'Explorateur**. C'est le déclencheur prouvé : un **verrou externe (os error 32)**, pas le worker.

### Conséquence pour la suite
- Mécanisme PROUVÉ ; l'identité exacte du verrou externe sera confirmée en production par le log
  d'instrumentation (`f04c4ca`) à la prochaine occurrence réelle (lire le code OS : 32 vs 5 vs autre).
- **Étape A** (préserver l'extension source pour supprimer la divergence `.aif`/`.aiff` à la racine)
  reste la correction de fond et la plus sûre : si la sortie d'un AIFF conforme reste `.aif`, le move
  ne crée plus de second nom, donc **même un échec FS du revert ne peut plus laisser deux extensions**.
  À décider ensuite, hors de cette étape B.

═══ STOP ═══ Étape B faite : déclencheur PROUVÉ (verrou externe sans partage-suppression, os error 32 ;
soupçon worker infirmé). Aucune correction racine ici. Décision suivante = étape A.
NE PAS toucher (rappel) : le code, TRASH_PURGE_DAYS, P-6.
