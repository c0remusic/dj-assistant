# dir-00 — Cartographie des 4 étages de décision (lecture seule)

> Reconstitué depuis le **code réel** + docs le 2026-06-25, branche `m6a-discogs`.
> Aucune supposition : chaque ligne est traçable. Les écarts doc↔code sont signalés ⚠️.

## ⚠️ Écart majeur, à lire avant tout : l'état réel n'est PAS celui de la mission

La mission pose « M0+M1 faits, M2 (analyseur) = prochaine grosse pièce ». **C'est faux dans le
code.** Cette phrase vient du **README périmé** ([README.md:16](README.md) : « M2 — Analyseur …
à venir »). L'état réel, prouvé par `plan-implementation.md` (à jour) + le code :

| Jalon | README (périmé) | Réalité code |
|---|---|---|
| M2 Analyseur | « à venir » | **fait** — `analysis/{decode,spectrum,verdict,dynamics,peaks,phase,structure,tags}.rs` + `worker.rs` |
| M3 Player/tempo | « wavesurfer M3 » | **fait** — `report-view.ts` (WaveSurfer v7, key-lock `preservesPitch`, fader tempo) |
| M4 Encode+filing | non listé | **fait** — `encode/naming/tagging/filing/actions.rs`, `filing.ts` |
| M4b Écartés | non listé | **fait** — `ecartes.rs`, `renderEcartes` |
| M5 Dédup empreinte | « Chromaprint M5 » | **fait (flux entrant)** — `dedup.rs`, `fingerprint.rs` |
| M6a Discogs | non listé | **fait** — `metadata/discogs.rs`, `ipc_identify.rs` |
| M6b Bibliothèque | non listé | **en cours** — `library.rs`, `ipc_library.rs`, `library-detail.ts` |

**Conséquence de direction** : la douleur n°1 (« l'archi tiendra-t-elle pour la suite ? ») ne se
teste pas contre M2 (livré, qui tourne) mais contre **M6b/M7** et les **3 modes de traitement**
(auto-règles / batch / détail) encore à bâtir. Tout l'audit est recadré là-dessus.

---

## A. Stratégie produit

- **Périmètre** ([plan-implementation.md:10-19](docs/plan-implementation.md)) : backlog ~15 000
  fichiers **et** flux Soulseek hebdo ; nettoyage **actif** de la biblio en V1 ; diffusion
  gratuite avec signing+site en V1.
- **Geste reine** : « écouter → ranger / écarter », un geste par morceau. Dans les faits il vit
  dans l'écran **Revue** (`filing.ts` + `report-view.ts`), c'est le seul écran avec un flux
  d'action complet (audition → verdict → File/Discard). Surfacé par un badge nav cette session.
- **Ordre des jalons** : M0→M1→M2→M3→**M4 (1re boucle utile)**→M5 (MVP)→M6 (confort/Discogs)→
  M7 (Rekordbox/USB, finalité)→M8 (écriture Rekordbox, **gelée** jusqu'à tests réels).
- **Direction « suite »** explicite ([next-steps-brainstorm.md:58-67](docs/superpowers/specs/2026-06-14-next-steps-brainstorm.md)) :
  **M6b → AcoustID → M7**, polish en continu.
- **8 features** : périmètre stable, mais **séquencé et gardé** — Rekordbox `master.db` natif et
  formatage clé sont en M7/M8, **non bâtis** et masqués dans l'app Tauri (cf. étage D, lean style).

## B. Architecture

- **Découpage Rust** : lib `sift_lib` ([Cargo.toml:13-15](src-tauri/Cargo.toml)) ; bin fin
  `main.rs` → `lib::run`. ~35 modules, séparés par responsabilité (analysis/, metadata/,
  filing, queue, dedup, encode, ffmpeg, db, scanner, watcher, worker, ipc*).
- **Surface IPC** : `lib.rs` `invoke_handler![…]` (≈40 commandes) ; un fichier `ipc_*.rs` par
  domaine (filing/identify/library + `ipc.rs` core).
- **État backend** : **une seule** `Mutex<Connection>` SQLite partagée
  ([lib.rs:54](src-tauri/src/lib.rs) `app.manage(Mutex::new(conn))`) ; WAL + busy_timeout posés ;
  le scan a sa propre connexion (sorti du verrou en round 2 d'audit).
- **Contrat IPC** : `shared/contracts.ts` (205 l) = **miroir manuel** des structs Rust. **Pas de
  codegen** (ni `ts-rs` ni `specta` dans Cargo.toml). Synchronisation à la main.
- **Front** : deux couches. `app.js` (318 l, **JS non typé**) rend le **shell + maquette** (DOM
  avec ids porteurs : `#ql`, `#qcol`, `#mid`, `.dest`, `#fldz`, `#filfoot`). `sift-live.ts`
  (1144 l) **remplace les données** par du réel **uniquement dans Tauri** (`installLiveWiring`,
  câblé via `window.__sift*` + manipulation des ids créés par app.js). `filing.ts` (899 l) et
  `report-view.ts` (643 l) portent la Revue ; `library-detail.ts` la biblio.
- **Frontière front↔back** : le front ne fait jamais d'I/O fichier (tout via IPC `ipc.ts`).
  Boot : [main.ts:13-17](frontend/main.ts) garde tout le live derrière `__TAURI_INTERNALS__`.
- **Où vit l'état UI** : éparpillé — vars de module dans `sift-live.ts` (`currentItems`,
  `reviewMode`, `batchSel`, `batchBin`, `batchBins`, `bibState`), objet `state` dans `filing.ts`,
  `currentWs` dans `report-view.ts`, état maquette dans `app.js` (`cur`, `T`, `FOLDERS`…), **plus
  le DOM lui-même** (`.qi.cur`, classes). Aucun store central (vanilla assumé).

## C. Choix techniques (tous documentés ET réalisés)

- **Décodage hybride** : **Symphonia** pur Rust pour l'analyse
  ([decode.rs:37](src-tauri/src/analysis/decode.rs) `symphonia::default::get_probe()`) ; **FFmpeg
  sidecar** pour l'encodage CDJ + check container/codec ([analysis/structure.rs](src-tauri/src/analysis/structure.rs)).
  ⚠️ **Écart doc** : CLAUDE.md/skill dit « adoption Symphonia à implémenter, pas encore faite » —
  **périmé**, c'est fait.
- **Empreinte** : `rusty-chromaprint 0.2` ([Cargo.toml:35](src-tauri/Cargo.toml)). Migration
  `chromaprint-next` étudiée puis **écartée** sauf si AcoustID en ligne (ressources-externes.md).
- **SQLite** `rusqlite` bundled, migrations `PRAGMA user_version`.
- **DSL de masks** (renommage) : inspiré MediaMonkey, version réduite — `naming.rs` (template
  `{artist} - {title}`), pas encore le DSL complet (`$If`, `<Artist@3>`…).
- **Écartés explicites** : Qdrant, SoundTouch.js, détection de key — tous écartés avec rationale
  ([ressources-externes.md:216-238](docs/ressources-externes.md)).

## D. UI / UX / design

- **Navigation** : 7 vues déclarées dans `index.html` (Home, Review, Discarded, Library,
  Rekordbox, USB, Settings) ; mais le **lean style** ([sift-live.ts](frontend/sift-live.ts) §
  `injectLeanStyle`) **masque Rekordbox + USB + groupe Export dans Tauri** → **5 vues effectives**.
  Nav groupée Process/Organize/Export + marque Sift (refait cette session).
- **Flux Revue** : son-d'abord — hero → bande audition (WaveSurfer) → panneau verdict → carte
  Discogs → footer hints ; rail de validation à droite (`.dest`/`#filfoot`). Mode **Batch** ajouté
  cette session (toggle Detail|Batch, groupes lossless/lossy, identify en lot).
- **Système visuel** : **tokens couleur** `--color-*` dans `styles.css` (126 l) **bien utilisés**
  (198 `var(--color…)` dans les TS). **MAIS** typo/espacement **non tokenisés** : **144
  `font-size:` inline** + tailles/marges en dur, répartis sur `sift-live.ts`/`filing.ts`/
  `report-view.ts`. Police Outfit + JetBrains Mono self-hosted ([main.ts:3-5](frontend/main.ts)).
- **Cohérence inter-écrans** : chaque écran génère son HTML avec **styles inline** dans un fichier
  différent → la cohérence dépend de la discipline, pas d'un système. C'est le terreau du
  ressenti « pas pro » (étage à prouver en phase 2).

## Deux audits multi-agents préexistants (à NE PAS dupliquer)

[full-audit 2026-06-13](docs/superpowers/reviews/2026-06-13-full-audit.md) (rounds 1+2) +
[m6a-audit](docs/superpowers/reviews/2026-06-14-m6a-audit.md) ont déjà couvert sécu/archi/
correctness/UX. **Différés encore ouverts** repris en phase 2 : pool de connexions DB, split
`sift-live.ts`, `name_key` indexé, undo des tags, contrat d'augmentation DOM implicite, retrait
des `#![allow(dead_code)]` (10 fichiers).
