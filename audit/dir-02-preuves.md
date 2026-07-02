# dir-02 — Preuves (PROUVÉ / RÉFUTÉ / INDÉTERMINÉ)

> Règle dure : une hypothèse **RÉFUTÉE ferme la porte** à toute proposition sur ce point.
> Chaque verdict est adossé à un extrait `fichier:ligne` ou un fait doc.

## Étage A — Produit

### H-A1 (dispersion / pièges de scope) — **RÉFUTÉ**
Rekordbox `master.db` natif = **M8 gelée** (« Feature gelée : on ne fixe pas le design tant que
des tests réels… », [plan-implementation.md:236-242](docs/plan-implementation.md)). Formatage clé
= **M7, non bâti**. Les deux sont **masqués dans l'app** : le lean style cache
`nv[data-view="rkb"]`, `nv[data-view="cle"]` et le groupe Export (`injectLeanStyle`,
[sift-live.ts](frontend/sift-live.ts)). → features **séquencées et gardées**, pas un éparpillement
actif. Le périmètre est large mais **discipliné**.

### H-A2 (geste reine dilué) — **RÉFUTÉ (en grande partie)**
Revue est le **seul** écran avec un flux d'action complet (audition→verdict→File/Discard,
`filing.ts`+`report-view.ts`) et porte un **badge de file** (`.nav-badge[data-badge="revue"]`,
ajouté cette session). Reste vrai à la marge : au-delà du badge, les items nav ont le **même
poids visuel** — pas de hiérarchie typographique qui crie « c'est ICI que ça se passe ». Faible.

### H-A3 (3 modes promis, moteur de règles absent) — **PROUVÉ (scope non bâti, pas un défaut)**
Le plan promet « **auto par règles (défaut)** » + règles configurables
([plan-implementation.md:259-300](docs/plan-implementation.md)). **Aucun module de règles** dans
les ~35 modules Rust (pas de `rules.rs`, pas de routage par confiance auto). Le mode « par
défaut » du produit **n'existe pas encore**. Ce n'est pas de la dette d'archi — c'est du **scope
à venir** — mais c'est le mode censé être *par défaut*, donc à ne pas oublier dans le séquencement.

## Étage B — Architecture

### H-B1 (état éparpillé ne tiendra pas) — **RÉFUTÉ (sur le « ne tiendra pas »)**
La pièce d'état la plus lourde — **lecture + position + verdict/piste + waveform** — est **déjà
livrée** (M2/M3) et **fonctionne** : `report-view.ts` gère le player avec jeton monotone `openSeq`
et invalidation de cache (cf. corrections round 2,
[full-audit.md:104-111](docs/superpowers/reviews/2026-06-13-full-audit.md)). L'état éparpillé **a
déjà encaissé le plus dur** sans réécriture. La menace « réécriture forcée » est réfutée par le
fait accompli. (Le coût de **maintenabilité** est réel mais c'est H-B5, pas celui-ci.)

### H-B2 (contrat IPC à la main → désync) — **PROUVÉ (risque structurel, sévérité basse)**
Pas de codegen (`grep ts-rs|specta` Cargo.toml = **NONE**). `shared/contracts.ts` (205 l) est un
miroir **manuel** des structs Rust. **Preuve vécue cette session** : ajouter `rail`/`artist`/
`title` à `QueueItem` a exigé d'éditer à la main `queue.rs` **et** `contracts.ts` **et** `ipc.ts`
(rituel répété, dont le GateGuard témoigne). Aucun bug de désync **shippé** constaté (tsc + la
discipline rattrapent) → surface bornée (~40 commandes). Risque réel, friction réelle, sévérité
actuelle basse.

### H-B3 (`app.js` hors type-check) — **PROUVÉ (sévérité basse)**
[tsconfig.json:8-9](tsconfig.json) `allowJs:true, checkJs:false`. `app.js` = 318 l de **JS non
typé**. Mais c'est le **shell maquette** ; dans Tauri le TS live remplace les données. Surface non
typée = la démo + le squelette DOM, bornée.

### H-B4 (couplage front↔front via ids DOM) — **PROUVÉ**
`sift-live.ts` dépend d'ids **créés par `app.js`** (`#ql`, `#qcol`, `#mid`, `.dest`, `#fldz`,
`#filfoot`) sans aucune assertion. Déjà flaggé (« Contrat d'augmentation implicite »,
[full-audit.md:64-65](docs/superpowers/reviews/2026-06-13-full-audit.md) #7). **Preuve vécue** :
cette session `ensureReviewSeg` insère le toggle Detail|Batch dans `#qcol` que `renderRevue`
(app.js) crée. Un renommage d'id côté app.js casse le live **en silence** (pas de fail-fast).

### H-B5 (god-module qui grossit) — **PROUVÉ + EN AGGRAVATION**
`sift-live.ts` = **1144 lignes** aujourd'hui ; les deux audits le donnaient à **~520** et
recommandaient déjà le split ([full-audit.md:51-53](docs/superpowers/reviews/2026-06-13-full-audit.md),
round 2 #2). **Il a doublé** (en partie le batch/identify de cette session). `filing.ts` 899 l,
`report-view.ts` 643 l. Le différé « split » n'a pas été fait et le coût monte.

### H-B6 (`Mutex<Connection>` unique = goulot) — **PROUVÉ (structurel) / INDÉTERMINÉ (magnitude)**
[lib.rs:54](src-tauri/src/lib.rs) `app.manage(Mutex::new(conn))` : une seule connexion partagée.
Déjà **top différé** des deux audits ([full-audit.md:46-49 + round 2 #1]). Mitigations posées
(scan hors-verrou, WAL, busy_timeout) mais **worker-écriture et IPC peuvent encore se télescoper
sur `SQLITE_BUSY`**. Magnitude réelle à l'échelle 15 000 fichiers = **non mesurée** → indéterminé.

### H-B7 (`#![allow(dead_code)]` masque du mort) — **PROUVÉ**
**10 fichiers** portent `#![allow(dead_code)]` (`grep -rl`). Déjà identifié comme différé. Empêche
le compilateur de signaler le code réellement mort (ex. `file_batch` l'était avant cette session).

## Étage C — Technique

### H-C1 (décisions non réalisées) — **RÉFUTÉ**
Symphonia **est câblé** ([decode.rs:1,37](src-tauri/src/analysis/decode.rs) : « In-process audio
decode via Symphonia », `get_probe()`). FFmpeg reste pour l'encode + check container
([analysis/structure.rs]). `rusty-chromaprint` en deps **et** utilisé (`dedup`/`fingerprint`). Les
décisions de `ressources-externes.md` sont **réalisées**. Seuls les **docs** mentent (→ H-M1).

### H-C2 (décodage/build ne tiendra pas le scan) — **RÉFUTÉ (en grande partie)**
Choix Symphonia = **pas de spawn par fichier** (vs FFmpeg), bench mesuré 0,2–0,7 s/morceau, non
bloquant ([ressources-externes.md:88-118](docs/ressources-externes.md)). `profile.dev` optimise
les deps à fond pour le DSP ([Cargo.toml:44-48](src-tauri/Cargo.toml)). Concurrence plein-biblio
non profilée (→ recouvre H-B6) mais le **choix de décodage est sain**.

## Étage D — UI / UX

### H-D1 (pas de tokens) — **RÉFUTÉ**
`styles.css` définit `--color-*` et ils sont **massivement utilisés** : **198** `var(--color…)`
dans les TS. Le système de tokens **couleur** existe et vit.

### H-D2 (typo/espacement en dur = vraie cause du « pas pro ») — **PROUVÉ ⭐**
**144** `font-size:` **inline** dans les `.ts` (vs `styles.css` = **126 lignes au total**). Aucune
échelle typographique ni d'espacement tokenisée : tailles et marges sont des **nombres magiques**
disséminés. C'est **la** source prouvée de l'incohérence ressentie — pas les couleurs. (Le spec
`penpot-detail-spec.md` documente d'ailleurs des tailles « 9/12.5/11/16/26 » au cas par cas, signe
qu'il n'y a pas d'échelle.)

### H-D3 (7 vues diluent) — **RÉFUTÉ (en partie)**
Tauri n'en montre que **5** (lean cache rkb/cle). Revue badgée. Dilution résiduelle = faible
hiérarchie visuelle, recouvre H-A2.

### H-D4 (rendu inline multi-fichiers diverge) — **PROUVÉ**
5 surfaces de rendu (`app.js`, `sift-live.ts`, `filing.ts`, `report-view.ts`, `library-detail.ts`)
génèrent chacune du HTML à coups de styles inline. La cohérence inter-écrans **dépend de la
discipline manuelle**, pas d'un composant partagé. Corrobore H-D2.

### H-D5 (erreurs brutes) — **PROUVÉ (sévérité basse)**
Codes techniques exposables (`NO_TOKEN`, `RATE_LIMITED:<s>`, `NoLibraryRoot`). Déjà noté dans la
veille UX ([ressources-externes.md:204-206](docs/ressources-externes.md)). Partiellement traité
(le cas no-token est humanisé cette session dans `runBatchIdentify`), reste épars ailleurs.

## Méta

### H-M1 (docs divergés) — **PROUVÉ ⭐**
[README.md:16](README.md) « M2 à venir » alors que M6a est livré ; structure README ne liste même
pas `sift-live.ts`/`filing.ts`/`report-view.ts`. CLAUDE.md/skill dit « Symphonia pas encore fait »
alors que `decode.rs` l'utilise. **Cette mission elle-même a hérité d'une prémisse fausse** (« M2
prochaine pièce ») de ces docs. Preuve directe que la divergence doc↔code coûte déjà.
