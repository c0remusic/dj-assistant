# Relevé — Batch d'identification Discogs : pourquoi l'UI gèle

Enquête lecture seule (méthode détective, fail-fast). Aucune modification.
Question : comment le batch d'identification tourne aujourd'hui, et pourquoi il gèle la
fenêtre sur ~2700 morceaux. On établit l'état réel par preuve avant de décider quoi changer.

---

## 1. Lancement côté frontend + appel IPC

- Clic « Identify (N) » du mode Batch → handler délégué `#pa`, case `batchidentify` →
  `void runBatchIdentify()` — [sift-live.ts:775-777](frontend/sift-live.ts:775).
- `runBatchIdentify` ([sift-live.ts:399-434](frontend/sift-live.ts:399)) :
  - prend `[...batchSel]` (tous les ids cochés — donc potentiellement 2700),
  - affiche une note statique « Identifying… » ([sift-live.ts:411](frontend/sift-live.ts:411)),
  - **un seul `await identifyBatch(ids)`** ([sift-live.ts:413](frontend/sift-live.ts:413)),
  - puis affiche un résumé une fois TOUT fini.
- `identifyBatch` = un seul `invoke` :
  `invoke("identify_batch", { trackIds })` — [ipc.ts:191-192](frontend/ipc.ts:191).

**Donc : un appel IPC unique, le front attend la fin de TOUT le batch en une seule promesse.**

## 2. La commande Rust : bloquante, monolithique

`identify_batch` — [ipc_identify.rs:87-146](src-tauri/src/ipc_identify.rs:87). Une seule
invocation traite les 2700 d'un coup, en deux phases séquentielles :

- **Phase 0 (1 lock DB)** : reconcile chaque id en `Query` ([ipc_identify.rs:101-109](src-tauri/src/ipc_identify.rs:101)).
- **Phase 1 — réseau, tout d'un bloc** : `metadata::pick_batch(&provider, &queries, sleep)`
  ([ipc_identify.rs:117-119](src-tauri/src/ipc_identify.rs:117)).
  `pick_batch` est `queries.iter().map(|(id,q)| provider.search(q) …).collect()`
  ([mod.rs:171-187](src-tauri/src/metadata/mod.rs:171)) — **une passe synchrone sur les 2700**,
  bloquant sur chaque `provider.search` (HTTP **ureq bloquant** :
  [discogs.rs:250](src-tauri/src/metadata/discogs.rs:250) ; et `search` peut faire 2-3 requêtes
  HTTP par morceau — requête principale + retry titre + tracklist).
- **Phase 2 — par morceau** : boucle sur les picks
  ([ipc_identify.rs:123-142](src-tauri/src/ipc_identify.rs:123)) : pour chaque `Picked`,
  download cover (réseau, [ipc_identify.rs:127-132](src-tauri/src/ipc_identify.rs:127)) puis
  `apply_identity` (écriture DB).

**Aucun découpage en plusieurs appels IPC : tout est dans un seul appel, deux boucles
séquentielles de 2700.**

## 3. Progression : quasi inexistante

- `pick_batch` n'a **aucun callback de progression** — sa signature ne prend que `provider`,
  `queries`, `sleep` ([mod.rs:159](src-tauri/src/metadata/mod.rs:159)).
- `identify_batch` n'émet **rien pendant** le traitement. Le seul event est
  `app.emit("queue:changed", ())` **à la toute fin** ([ipc_identify.rs:144](src-tauri/src/ipc_identify.rs:144)).
- Le front ne montre donc qu'un « Identifying… » figé jusqu'au retour de la promesse.

**Il n'y a aucun mécanisme de progression par morceau à afficher.** Et même s'il y en avait
un, il ne pourrait pas s'afficher — voir §4.

## 4. Thread d'exécution = LE cœur du gel

`identify_batch` est une commande **synchrone** : `#[tauri::command] pub fn identify_batch(…)`
([ipc_identify.rs:87-88](src-tauri/src/ipc_identify.rs:87)) — pas de `async`, pas de
`#[tauri::command(async)]`.

> Règle Tauri (doc « Async Commands ») : *« Commands without the async keyword are executed on
> the main thread. »* Une commande sync tourne sur le **thread principal**, celui qui fait
> tourner la boucle d'événements de la fenêtre native.

Conséquence : la boucle des 2700 (HTTP bloquant + `std::thread::sleep` sur rate-limit,
[ipc_identify.rs:117-119](src-tauri/src/ipc_identify.rs:117) / [mod.rs:177](src-tauri/src/metadata/mod.rs:177))
**monopolise le thread principal** pendant des dizaines de minutes. Pendant ce temps la fenêtre
ne pompe plus ses messages OS → Windows la marque « Ne répond pas », plus de repaint/clic/resize.
Les logs continuent de défiler dans le terminal **parce que c'est précisément ce thread-là qui
fait le travail** — il bosse au lieu de servir la fenêtre.

## 5. Annulation : aucune

Aucun flag, aucun canal, aucun token. La signature et le corps de `identify_batch` /
`pick_batch` n'ont ni `AtomicBool`, ni `Receiver`, ni check d'interruption
([ipc_identify.rs:87-146](src-tauri/src/ipc_identify.rs:87), [mod.rs:159-188](src-tauri/src/metadata/mod.rs:159)).
Une fois lancé, **rien ne peut l'arrêter** sauf tuer l'app. (Et tuer l'app au milieu = la
fenêtre étant gelée, il faut passer par le gestionnaire des tâches.)

## 6. Écritures DB : par morceau, en phase 2

- Les recherches réseau (phase 1) se font **toutes d'abord** ; les écritures DB se font ensuite,
  **une par morceau** dans la phase 2 : `apply_identity(&conn, id, …)` par id
  ([ipc_identify.rs:133-134](src-tauri/src/ipc_identify.rs:133)), avec le lock **pris et relâché
  à chaque itération** (lock à [ipc_identify.rs:133](src-tauri/src/ipc_identify.rs:133), dans la
  boucle).
- Le réseau (search, cover) se fait **hors lock**. Donc le `Mutex<Connection>` n'est jamais tenu
  pendant une requête HTTP.

**Bon point pour une future annulation : chaque identité appliquée est persistée au fil de
l'eau (et réversible — metadata only, rien n'est déplacé/encodé). Un arrêt en cours laisse un
état propre : les N premiers identifiés, le reste intact.**

---

## VERDICT — pourquoi l'UI gèle (cause prouvée)

**La commande `identify_batch` est synchrone, donc elle s'exécute sur le thread principal de
Tauri ; sa boucle de 2700 recherches Discogs (HTTP bloquant + sleeps rate-limit) accapare ce
thread pendant tout le batch, ce qui empêche la fenêtre de traiter ses événements → gel
« Ne répond pas ».**

Ce n'est PAS un problème d'affichage de progression manquant : même un beau % ne s'afficherait
pas, car le thread qui devrait rafraîchir l'UI est celui qui est occupé à boucler. La cause est
le **lieu d'exécution** (main thread), pas la présence/absence d'events.

Facteurs aggravants secondaires (pas la cause racine) :
- 1 seul appel IPC monolithique (le front ne reprend la main qu'à la toute fin).
- `search` fait 2-3 requêtes HTTP par morceau → le temps total explose (2700 × plusieurs
  requêtes + sleeps), ce qui allonge d'autant la durée du gel.

## Ampleur du chantier pour passer en arrière-plan

**Moyenne — pas une réécriture, mais le traitement n'est PAS du tout détaché aujourd'hui.**
Ce n'est donc pas « il manque juste la remontée de progression » : **il faut déplacer le
traitement hors du thread principal** (c'est le fix central). La bonne nouvelle : la logique
est déjà bien factorisée, ce qui limite le chantier.

Déjà en place (favorable) :
- `pick_batch` est **pur sur le provider** + prend un `sleep` injecté → déplaçable tel quel sur
  une tâche de fond, testé unitairement ([mod.rs:159](src-tauri/src/metadata/mod.rs:159)).
- Écritures DB **par morceau, lock pris/relâché à chaque item**, réseau hors lock
  ([ipc_identify.rs:127-134](src-tauri/src/ipc_identify.rs:127)) → compatible fond : d'autres
  commandes peuvent s'intercaler entre deux items (lié à P-6, mais ici déjà non-bloquant entre items).
- Annulation « propre » triviale côté données : chaque identité est déjà persistée au fil de l'eau.

À ajouter (le vrai travail) :
1. **Détacher l'exécution** : commande qui lance le job sur une tâche de fond
   (`tauri::async_runtime::spawn` / thread dédié) et **rend la main tout de suite** (le contrat
   IPC change : on ne peut plus renvoyer `IdentifyBatchResult` en synchrone — il faut streamer).
2. **Progression par event** : émettre `identify:progress {done,total,id,outcome}` à chaque item.
   `pick_batch` n'a **aucun hook par item** aujourd'hui → soit lui ajouter un callback, soit
   remonter la boucle dans `identify_batch` (appeler une fonction « pick 1 morceau » et émettre).
3. **Annulation** : un token partagé (`AtomicBool` / `CancellationToken`) vérifié à chaque
   itération + une commande `identify_cancel`. Inexistant aujourd'hui.
4. **Front** : `runBatchIdentify` doit s'abonner à progress/done/cancel au lieu d'`await` une
   promesse unique ([sift-live.ts:399-434](frontend/sift-live.ts:399)) ; barre de progression +
   bouton Annuler à câbler.

Estimation : le **cœur** (détacher + 1 event de progression + 1 flag d'annulation) est contenu
parce que `pick_batch`/`apply_identity` ne bougent quasi pas ; l'essentiel de l'effort est le
**changement de contrat IPC** (job lancé → events) et son pendant frontend (abonnement + UI
progression/annulation). Pas de refonte du moteur d'identification.

NE PAS faire dans cette passe (rappel) : aucune correction, fallback rate-limit traité à part,
TRASH_PURGE_DAYS / P-6 hors sujet.
