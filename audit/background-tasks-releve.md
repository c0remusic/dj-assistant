# Relevé — vers un système générique de tâches de fond + progression globale

Enquête lecture seule (méthode détective, fail-fast). Aucune modification.
But : savoir si les autres tâches longues (analyse-à-l'ajout, filing, conversion) sont déjà
détachables comme l'identification, pour dimensionner un système de progression **global**
(visible quel que soit l'onglet) **sans sur-ingénierie** — juste ce que ces tâches réelles
justifient.

---

## 1. IDENTIFICATION batch — modèle de référence (déjà fait, étape 1)

- Détachée : commande sync qui lit le token puis **spawn un thread OS** et rend la main
  ([ipc_identify.rs:91](src-tauri/src/ipc_identify.rs:91), spawn [:110-112](src-tauri/src/ipc_identify.rs:110)).
- Event de fin : **`identify:done`** avec payload `IdentifyBatchResult { identified, no_match, failed }`
  ([ipc_identify.rs:182](src-tauri/src/ipc_identify.rs:182)).
- Front : `runBatchIdentify` lance + note « Identifying… » ; abonnement `onIdentifyDone` refresh + résumé
  ([sift-live.ts](frontend/sift-live.ts), [ipc.ts:onIdentifyDone](frontend/ipc.ts)).
- **Manque** (= étape 2) : aucun event **par morceau** ; `pick_batch` n'a aucun hook par item.

→ Contrat actuel : `lancement (rend la main) → event terminal avec résumé`. Pas de courant/total.

## 2. « IDENTIFICATION à l'ajout au dossier racine » = en réalité l'ANALYSE (déjà détachée)

⚠️ Clarification par preuve : **il n'y a AUCUNE auto-identification Discogs à l'ajout.** `identify`
n'apparaît que dans `ipc_identify.rs` (commandes déclenchées par l'utilisateur) — rien dans
`worker.rs`, `ipc.rs`, `watcher.rs`. Ce qui tourne quand des fichiers arrivent, c'est :

- **Scan** (découverte → queue) : `spawn_scan` = **thread détaché**
  ([ipc.rs:323](src-tauri/src/ipc.rs:323)), appelé par add_source / rescan / import
  ([ipc.rs:64](src-tauri/src/ipc.rs:64), [:130](src-tauri/src/ipc.rs:130), [:199](src-tauri/src/ipc.rs:199)),
  émet `queue:changed` à la fin ([ipc.rs:356](src-tauri/src/ipc.rs:356)).
- **Analyse** (détection faux-lossless, la vraie tâche longue) : **pool de 1-4 threads** démarré au
  boot ([worker.rs:112-137](src-tauri/src/worker.rs:112)), `refill` ré-enfile les pending après chaque
  `queue:changed` ([worker.rs:141](src-tauri/src/worker.rs:141)), `worker_loop` analyse **hors lock**
  et émet **`analysis:changed` par morceau** ([worker.rs:220-228](src-tauri/src/worker.rs:220)).
  **Progression DÉJÀ existante** : commande de poll `analysis_progress` (done/total)
  ([ipc.rs:214](src-tauri/src/ipc.rs:214)) → barre dans l'en-tête Review (`renderQueue`).

→ L'analyse est **déjà détachée ET déjà dotée d'une progression** — mais en modèle **poll**
(event ping `analysis:changed` + requête `analysis_progress`), pas en payload comme identify.
Et sa barre vit dans #content/Review → elle disparaît au changement d'onglet (le problème à régler).

## 3. FILING — BLOQUANT (sur le thread principal)

- `file_track` (mono) : commande **sync** ([ipc_filing.rs:51](src-tauri/src/ipc_filing.rs:51)).
  Bon point : l'encode ffmpeg (phase 2) se fait **hors lock** ([ipc_filing.rs:68-69](src-tauri/src/ipc_filing.rs:68)).
  Mais sync ⇒ tourne sur le thread principal ⇒ **gèle pendant l'encode** (mêmes symptômes qu'identify avant étape 1).
- `file_batch` (batch) : commande **sync** ([ipc_filing.rs:81](src-tauri/src/ipc_filing.rs:81)) qui
  appelle `filing::file_batch(&conn, …)` **en tenant le lock DB sur TOUT le batch**
  ([ipc_filing.rs:88-93](src-tauri/src/ipc_filing.rs:88)). Pire qu'identify : non seulement ça gèle
  l'UI, mais ça **bloque le worker d'analyse** (lock tenu pendant tous les encodes). Le doc du module
  l'admet explicitement ([ipc_filing.rs:9-11](src-tauri/src/ipc_filing.rs:9)).
- Events : **seulement `queue:changed` à la fin** ([:75](src-tauri/src/ipc_filing.rs:75), [:94](src-tauri/src/ipc_filing.rs:94)). Aucun courant/total.

→ Filing = la tâche qui demande **d'abord un détachement façon étape 1** (et, pour le batch, le
passage au lock par-morceau qu'identify a déjà).

## 4. CONVERSION — pas une tâche autonome : sous-étape du filing

- `encode::encode` lance ffmpeg ([encode.rs:127](src-tauri/src/encode.rs:127)) et **bloque** le thread
  appelant jusqu'à la fin. Ce n'est PAS une commande IPC séparée : elle est appelée **dans** le filing
  (`filing::execute_file` / `file_batch`). Aucun event propre.

→ La conversion devient « en fond » **gratuitement** quand le filing l'est. La granularité de
progression du filing = **par fichier** (chaque fichier inclut sa conversion).

---

## 5. POINT COMMUN → contrat de progression générique MINIMAL

Ce que ces tâches réelles ont en commun (et donc ce qu'il faut, ni plus ni moins) :

| Tâche | label | courant/total ? | terminal/résumé ? | concurrente ? |
|---|---|---|---|---|
| Analyse | « Analyzing » | oui (done/total déjà calculés) | implicite (done==total) | **oui, en continu** |
| Identify | « Identifying » | à émettre (étape 2) | oui (`identify:done`) | oui |
| Filing | « Filing » | à émettre (après détachement) | oui (résumé batch) | oui |

**Contrat minimal justifié :** un event unique, ex.
```
task:progress { kind: "analyze" | "identify" | "file", done: number, total: number,
                state: "running" | "done" | "error", detail?: string }
```
- `kind` = le label affiché **ET** la clé (voir concurrence).
- `done`/`total` = la barre.
- `state` = pour effacer/clore + colorer une erreur.

**Pas besoin** (ce serait de la sur-ingénierie ici) : pas d'**id de job** (chaque `kind` n'a qu'un
run actif à la fois — on ne lance pas deux batches identify en parallèle, et le pool d'analyse est
UN job logique même à N threads) ; pas de **payload par item** (les counts suffisent) ; pas de file
de jobs générique. Keyed-by-`kind` couvre tout.

**Le seul vrai point de design = la CONCURRENCE.** L'analyse tourne en continu pendant qu'on peut
lancer identify puis filing. Donc la zone globale doit pouvoir afficher **plusieurs tâches à la
fois** (1 ligne par `kind` actif) — d'où la clé `kind`. C'est modeste côté front (une
`Map<kind, {done,total,state}>`), pas un ordonnanceur.

Note d'uniformisation : l'analyse a déjà `analysis:changed` + `analysis_progress` (poll). Deux choix
(à décider) : (a) la laisser en poll et que le front la fasse rentrer dans la zone globale telle
quelle, ou (b) lui faire émettre aussi le `task:progress` générique pour un seul chemin de code
côté front. (b) est plus propre, (a) est zéro-risque sur l'analyse qui marche déjà.

## 6. EMPLACEMENT FRONT pour la zone globale

Éléments **persistants** (hors `#content`, survivent au changement de vue) :
- **La sidebar `.sb #nav`** ([index.html:16-28](index.html:16)) — toujours dans le DOM ; `reglages` est
  poussé en bas via `margin-top:auto` ([index.html:27](index.html:27)). **Candidat recommandé : un bloc
  en bas de `#nav`**, sous (ou au-dessus de) Settings. De la place, et c'est « au-dessus des onglets »
  conceptuellement (c'est le chrome, pas le contenu).
- `#sift-titlebar` (injecté Tauri-only par `chrome.ts`) — persistant aussi mais fin (30px) → une ligne
  de progression mince possible, mais à l'étroit pour label + barre + counts.

À l'inverse, l'emplacement actuel du moulin (`#filfoot`, dans `#content`/Review) est **non
persistant** → c'est précisément pourquoi il disparaît. La barre d'analyse (en-tête Review) a le même
défaut. La zone globale = **bas de la sidebar `#nav`**.

---

## VERDICT — ampleur du chantier, tâche par tâche

Fondation commune (à faire une fois) : **contrat `task:progress` + zone globale dans la sidebar**
(une `Map<kind,…>` qui rend 1 ligne par tâche active). Modeste.

| Tâche | détachée ? | effort pour la brancher | pourquoi |
|---|---|---|---|
| **Analyse** | ✅ déjà | **TRIVIAL** | déjà détachée + done/total déjà calculés ; juste l'afficher dans la zone globale (réutiliser `analysis_progress`, ou émettre le `task:progress` générique). L'essentiel est un **déplacement front** (barre Review → sidebar). |
| **Identify** | ✅ déjà (étape 1) | **PETIT** = étape 2 | détaché ; reste à émettre la progression par item depuis `run_identify_batch` — mais `pick_batch` n'a **aucun hook par item** → ajouter un callback, ou remonter la boucle dans `identify_batch`. |
| **Filing** | ❌ bloquant | **MOYEN** | demande **d'abord un détachement façon étape 1** (spawn thread, rendre la main, event terminal). Pour `file_batch` : adopter en plus le **lock par-morceau** (aujourd'hui lock tenu sur tout le batch) — sinon le fond bloquerait quand même l'analyse. Puis émettre `task:progress` par fichier. |
| **Conversion** | — | **GRATUIT** | sous-étape du filing ; devient « en fond » avec lui, granularité par fichier. |

Découpage logique suggéré (à valider) : (1) fondation `task:progress` + zone sidebar ; (2) brancher
l'**analyse** (trivial, valide la zone tout de suite, gros gain visible) ; (3) **identify** progression
(= étape 2 du chantier batch, réutilise la fondation) ; (4) **filing** détachement + progression (le
plus lourd, à traiter comme un « étape 1 bis » dédié). L'annulation (étape 3 batch) se branchera sur
le même contrat plus tard (`task:cancel { kind }`).

AUCUNE correction ici. On lira le verdict et on décidera du découpage.
NE PAS toucher (rappel) : code, TRASH_PURGE_DAYS, P-6, fallback rate-limit — intégrés au chantier batch.
