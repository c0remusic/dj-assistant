# Relevé — retrait de l'identification Discogs EN BATCH (garder l'unitaire)

Enquête lecture seule. Aucune modification, aucun commit.
Décision produit : supprimer le batch de masse (rate-limit + titres faux + identifications non
validées par l'humain) ; garder l'identification **à l'unité dans Review**, où l'utilisateur
choisit la bonne release. Ce relevé sépare ce qui PART, ce qui RESTE, et ce qui doit être
PRÉSERVÉ pour que l'unitaire continue de marcher.

---

## 1. Le flux BATCH (front → IPC → backend) = ce qui ne sert QU'au batch

**Front (déclenchement)**
- Bouton « Identify (N) » du mode Batch, construit dans `renderBatch` / `readyHead`
  ([sift-live.ts:276](frontend/sift-live.ts:276)).
- Handler délégué `#pa`, case `batchidentify` → `void runBatchIdentify()`
  ([sift-live.ts:790-792](frontend/sift-live.ts:790)).
- `runBatchIdentify` ([sift-live.ts:395-432](frontend/sift-live.ts:400)) → `await identifyBatch(ids)`
  ([sift-live.ts:411](frontend/sift-live.ts:411)).
- Helpers batch : `batchNote` (utilisé seulement par le batch) et `onIdentifyBatchDone`
  ([sift-live.ts:434-...](frontend/sift-live.ts:434), [:439](frontend/sift-live.ts:439)).
- Abonnement `void onIdentifyDone(onIdentifyBatchDone)` ([sift-live.ts:812](frontend/sift-live.ts:812)).
- Imports : `identifyBatch`, `onIdentifyDone` ([sift-live.ts:9-10](frontend/sift-live.ts:9)),
  type `IdentifyBatchResult` ([sift-live.ts:42](frontend/sift-live.ts:42)).

**IPC (bindings)**
- `identifyBatch` → `invoke("identify_batch")` ([ipc.ts:192-193](frontend/ipc.ts:192)).
- `onIdentifyDone` → `listen("identify:done")` ([ipc.ts:195-200](frontend/ipc.ts:195)).
- Import type `IdentifyBatchResult` ([ipc.ts:15](frontend/ipc.ts:15)).
- Contrats : `BatchFailure` + `IdentifyBatchResult` ([contracts.ts:130-141](shared/contracts.ts:130)).

**Backend (orchestration)**
- Commande `identify_batch` ([ipc_identify.rs:84-115](src-tauri/src/ipc_identify.rs:84)).
- Corps détaché `run_identify_batch` ([ipc_identify.rs:117-183](src-tauri/src/ipc_identify.rs:117)) :
  boucle, thread, event `identify:done` ([:182](src-tauri/src/ipc_identify.rs:182)).
- Structs `IdentifyBatchResult` + `BatchFailure` ([ipc_identify.rs:71-82](src-tauri/src/ipc_identify.rs:71)).
- Moteur d'orchestration `pick_batch` (boucle + auto-pick du top) + enum `BatchPick`
  ([metadata/mod.rs:146-188](src-tauri/src/metadata/mod.rs:146)).
- Test `pick_batch_picks_top…` + son provider `Fake` ([metadata/mod.rs:349-400](src-tauri/src/metadata/mod.rs:349)).
- Enregistrement `ipc_identify::identify_batch` ([lib.rs:97](src-tauri/src/lib.rs:97)).
- Import `BatchPick` dans le `use` ([ipc_identify.rs:6](src-tauri/src/ipc_identify.rs:6)).

→ **Tout ce bloc est batch-only.** Caractéristique commune : il prend une **liste** d'ids,
**auto-applique le top hit sans validation humaine**, et le fait en masse.

## 2. L'identification À L'UNITÉ — existe déjà, c'est le flux Review

Oui, elle existe et fonctionne indépendamment du batch :
- **Commande** `identify(track_id)` ([ipc_identify.rs:15-33](src-tauri/src/ipc_identify.rs:15)) :
  reconcile → `Query` → `provider.search(&query)` → renvoie les **candidats classés** (l'humain
  choisit). Puis **`apply_identity_cmd`** ([ipc_identify.rs:37-67](src-tauri/src/ipc_identify.rs:37))
  applique LA release choisie (download cover + écrit tags/DB).
- **Front Review** : `doIdentify` dans `filing.ts` (bouton « Identify » du footer) appelle
  `identify(trackId)` → `renderCandidates` → l'utilisateur clique une release → `applyIdentity`.
  Même chose dans `library-detail.ts` (panneau Bibliothèque).
- **Bindings** `identify` / `applyIdentity` ([ipc.ts](frontend/ipc.ts)), rendu partagé
  `renderCandidates` (`identify-shared.ts`).

→ L'unitaire **ne touche jamais** `identify_batch` / `pick_batch` / `run_identify_batch`. Il
n'y a **rien à recréer** : retirer le batch ne casse pas l'unitaire.

## 3. DISTINCTION : moteur (GARDER) vs orchestration batch (RETIRER)

| | Rôle | Verdict | Lieu |
|---|---|---|---|
| `Discogs::search` / `search_query` / `fetch_tracklist` / `best_track_match` / `rank_by_match` | **Moteur** : recherche, scoring, choix de release | **GARDER** | `metadata/discogs.rs` |
| `metadata::apply_identity`, `cover::download_cover`, `Query`, `Candidate`, `AppliedIdentity`, `ProviderError`, trait `MetadataProvider` | types/IO du moteur | **GARDER** | `metadata/mod.rs`, `metadata/cover.rs` |
| `identify` (cmd) + `apply_identity_cmd` (cmd) | **unitaire** (Review) | **GARDER** | `ipc_identify.rs:15`, `:37` |
| `pick_batch` + `BatchPick` | boucle + auto-pick top, **sans humain** | **RETIRER** | `metadata/mod.rs:146`, `:159` |
| `identify_batch` + `run_identify_batch` + `IdentifyBatchResult` + `BatchFailure` | orchestration batch (liste, thread, `identify:done`) | **RETIRER** | `ipc_identify.rs:71-183` |
| `runBatchIdentify` + `onIdentifyBatchDone` + `batchNote` + bouton + handler + abos | UI/wiring batch | **RETIRER** | `sift-live.ts` (voir §1) |

**Le clivage est net** : le moteur (1 morceau → candidats classés) est intégralement réutilisé
par l'unitaire via `identify`. Le batch n'ajoute qu'une **boucle + auto-pick + détachement**.
`pick_batch` **appelle** `provider.search` (le moteur) mais n'en fait PAS partie — c'est une
couche au-dessus.

## 4. Ce qui casserait si on retire le batch

Vérifié par grep sur tout le repo (hors `dist/`) :
- `identify_batch` n'est appelé QUE par `runBatchIdentify` ([sift-live.ts:411](frontend/sift-live.ts:411)). Aucun autre appelant.
- `identify:done` n'est écouté QUE par `onIdentifyDone`/`onIdentifyBatchDone`. Aucun autre.
- `pick_batch` / `BatchPick` ne sont utilisés QUE par `identify_batch` + leur propre test
  ([metadata/mod.rs:349](src-tauri/src/metadata/mod.rs:349)). Aucun orphelin une fois le test retiré avec.
- `IdentifyBatchResult` / `BatchFailure` ne sont utilisés QUE dans le set ci-dessus (ipc.ts, sift-live, contracts).
- **Faux positifs (commentaires seulement, PAS du code à casser)** : `filing.rs:352` et `:482`
  mentionnent « what lets identify_batch feed file_batch ». Le **mécanisme** (`canonical_from_metadata`,
  filing qui honore l'identité persistée) **reste valable via l'unitaire** (`apply_identity` persiste,
  le filing l'honore). Seul le **libellé** du commentaire devient daté → reformulation optionnelle,
  aucune casse.
- **Le mode Batch lui-même** (toggle Detail|Batch, sélection, **File** / **Discard** en masse) **n'est
  PAS concerné** : seul le bouton « Identify (N) » + son flux partent. `renderBatch` reste, on retire
  juste le bouton de `readyHead` (en gardant « Select all »). `fileBatch` (filing batch) est un autre
  chemin, intact.

→ Rien ne casse hors le set batch-identify. La séparation est propre.

## 5. Ampleur du retrait : FAIBLE et mécanique

Suppression d'une couche d'orchestration, **zéro** ligne du moteur ou de l'unitaire touchée.

Fichiers impactés :
- **Backend** (3) : `ipc_identify.rs` (retirer 2 structs + 2 fns + alléger 1 `use`),
  `metadata/mod.rs` (retirer 1 enum + 1 fn + 1 test/Fake), `lib.rs` (retirer 1 ligne d'enregistrement).
- **Frontend** (3) : `sift-live.ts` (imports, bouton, `runBatchIdentify`/`onIdentifyBatchDone`/`batchNote`,
  case handler, abonnement), `ipc.ts` (2 exports + 1 import), `contracts.ts` (2 interfaces).

Vérifs attendues post-retrait : `cargo check` vert (vérifier qu'aucun `use` ne devient inutilisé,
notamment garder `MetadataProvider` requis par `identify`), `cargo test` vert (1 test en moins),
`tsc --noEmit` vert. Réversible (c'est une suppression nette d'une feature, pas un refactor profond).

Points à TON arbitrage avant coupe :
1. **Le commentaire `filing.rs:352/482`** : reformuler « identify_batch feed file_batch » → « une identité
   appliquée (unitaire) feed file_batch », ou laisser tel quel ? (cosmétique).
2. **Le mode Batch** garde File/Discard de masse : OK, ou tu veux aussi reconsidérer le toggle
   Detail|Batch un autre jour ? (hors périmètre ici — je ne touche qu'à l'Identify).

═══ STOP ═══ On lit la carte, tu valides, PUIS on coupe. Rien retiré tant que tu n'as pas dit go.
NE PAS toucher (rappel) : code (phase lecture seule), zone progression (étape 2), filing (étape 3),
TRASH_PURGE_DAYS, P-6.
