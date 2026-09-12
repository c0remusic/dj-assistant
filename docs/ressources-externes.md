# Ressources externes & veille technique — Sift

> Veille des libs / outils / API utiles à Sift, classée par jalon, avec **statut**
> (mûr / jeune / référence-only) et liens. Inclut deux évaluations menées le
> 2026-06-24 : **test Symphonia vs FFmpeg** et **étude chromaprint-next vs
> rusty-chromaprint**.
>
> Rappel pile : Tauri v2 (Rust), FFmpeg via `ffmpeg-sidecar` (bundlé), SQLite
> (`rusqlite`), `rustfft`, `lofty`, `rusty-chromaprint`, `ureq`. MSRV projet =
> Rust 1.77.2.

---

## Sommaire

> Index de navigation (ligne + gist), pas une table des matières auto-générée —
> les numéros de ligne dérivent si le fichier bouge, revérifier au grep si un
> doute. Charger ce fichier entier n'est plus automatique (retiré de `CLAUDE.md`
> le 2026-07-09) : ouvrir la section visée via son numéro de ligne plutôt que
> tout lire.

- L14 — M2 — Analyseur / détection de faux lossless : Audio Fake Detector PRO (algo à lire), auCDtect, rustfft déjà en deps.
- L33 — M3 — Décodage / waveform / analyse : Symphonia adopté, bpm-finder à évaluer, key hors scope.
- L42 — M5 — Empreinte / dédoublonnage / identification : rusty-chromaprint (déjà en dep) vs chromaprint-next vs Chromaprint/AcoustID.
- L51 — Export Rekordbox : rbox candidat n°1, rekordcrate (heavy dev), pyrekordbox (doc vivante), rekordbox-repair (3 idées récupérables).
- L78 — Renommage Discogs : API Discogs (rate limits, flux de matching).
- L92 — Évaluation 1 — Symphonia vs FFmpeg (2026-06-24) : archi hybride adoptée (FFmpeg encode, Symphonia decode pour l'analyse).
- L144 — Évaluation 2 — chromaprint-next vs rusty-chromaprint (2026-06-24) : pas de migration maintenant, dépend du scope online/local.
- L179 — Évaluation 3 — workflow d'itération UI en direct (2026-07-03) : root cause 3 implémentations UI parallèles ; tauri dev+HMR retenu.
- L265 — Évaluation 4 — `/design-sync` et Open Design (2026-07-03) : les deux écartés, `design-system-states.md` reste la source de vérité.
- L316 — Évaluation 5 — spike lecture native `master.db` Rekordbox (2026-07-03) : lecture seule validée et sûre.
- L373 — Évaluation 6 — re-vérification `/design-sync` vs Open Design (2026-07-04) : statu quo confirmé, drift réel minime.
- L423 — Évaluation 7 — spike d'écriture `master.db` (2026-07-04) : écriture possible et sûre en principe, portage Rust restant.
- L493 — Évaluation 8 — outil de sync de tokens design↔code, construit (2026-07-04) : token-sync/ livré (générateurs, editor.html, dev-inspector).
- L556 — Évaluation 9 — token-sync tool v2 (2026-07-04) : format DTCG retenu sans module Resolver, Figma écarté.
- L610 — Évaluation 10 — token-sync tool v2 exécuté (2026-07-04) : migration DTCG livrée, gap rgba trouvé et corrigé.
- L692 — Évaluation 11 — chantier triple : token-sync v3 + spike CDP + spike M8 n°2 (2026-07-05) : `styles.css` canonique unique, CDP WebView2 validé.
- L777 — Évaluation 12 — pointeur visuel d'annotation, construit (2026-07-05) : `dev-inspector.ts`/`dev_locate.rs`/`dev_annotate.rs` livrés.
- L830 — Évaluation 13 — prompt externe « Figma local » (2026-07-05) : rejeté (faits faux sur le repo, esquive le mapping inverse).
- L888 — Évaluation 14 — spike M8 sur le fichier live Rekordbox (2026-07-05) : incident (backup déjà contaminé), 2 garde-fous actés.
- L945 — Veille concurrente — MediaMonkey (2026-06-24) : 3 enseignements (dédup Chromaprint, DSL masks, MusicBrainz vs Discogs).
- L979 — Veille UX — design d'interface (2026-06-24) : checklist UX (icon-only, microcopy, a11y, cohérence).
- L998 — Titlebar custom (2026-07-03) : 3/3 briques faites, macOS non vérifié visuellement.
- L1047 — Design system Sift — audit tokens hauteur/radius/typo (2026-07-03) : `--h-32`/`-44` supprimés, `--border-radius-sm`/`pill` ajoutés, `--text-hero` renommé.
- L1079 — Infra / Release — décisions en attente : tauri-plugin-updater et tauri-specta reportés post-RC.
- L1086 — Outillage Claude Code — purge plugins/skills cross-projet (2026-07-03) : désinstalls/désactivations, propagé au registre skills.
- L1139 — Dette technique — audit Project Cleaner (2026-07-09) : 10 tokens CSS orphelins, version désync, refs mortes.
- L1194 — Dette technique — double décryptage master.db (M8 Tier 1+3, 2026-07-09) : fix, index chargé une fois par `commit_file`.
- L1223 — Dette technique — nettoyage clippy `m7-rekordbox-xml` (2026-07-04) : `TRASH_PURGE_DAYS` mort supprimé, `needless_range_loop` fixé.
- L1247 — Écarté : vykee.co, SoundTouch.js, Qdrant, Graphify.
- L1290 — Évaluation 15 — spectrogramme trop clair (2026-07-06) : 2 bugs empilés (dB non normalisé + cache `report_json` obsolète).
- L1346 — Évaluation 16 — délégation Claude→Codex CLI (2026-07-06) : coût en tokens, override anti-routage dans le prompt de mission.
- L1436 — Évaluation 17 — corruption d'encodage mojibake (2026-07-07) : 2 fichiers réparés, pas d'outillage ajouté.
- L1481 — Évaluation 18 — test M8 Tier 1 contre copie réelle (2026-07-08) : bug WAL trouvé et corrigé.
- L1578 — Évaluation 19 — spike stack UI écarté (2026-07-08) : racine = absence de référence canonique, pool shadcn/uithing/coss/HIG adopté.
- L1700 — Évaluation 20 — M8 Tier 3 Test 1 infirmé (2026-07-08) : flag `TrackInfoUpdated` seul ne déclenche pas de reload auto.
- L1777 — Évaluation 21 — M8 Tier 3 diff exact « Relire le tag » (2026-07-09) : find-or-create documenté (chemin création).
- L1849 — Évaluation 22 — M8 Tier 3 reuse vs duplicate (2026-07-09) : REUSE confirmé pour un artiste déjà connu.
- L1880 — Évaluation 23 — M8 Tier 3 moteur Rust livré (2026-07-09) : `sync_track_metadata` + spike 8 pochette.

---

## M2 — Analyseur / détection de faux lossless ⭐

Feature signature. Le gros du gisement est dans l'algorithme de détection.

- **[Audio Fake Detector PRO](https://github.com/alessandrocomito/audiofakedetectorpro)**
  — open source, alternative gratuite à *Fakin' The Funk*. **Algo étudiable** et
  directement transposable : découpe en segments → inspection bitmap du
  spectrogramme par segment → **vote majoritaire** → + validation **auCDtect**
  (analyse PCM statistique) pour les lossless. _Statut : référence + code à lire
  avant d'écrire M2._
- **[auCDtect](https://thewelltemperedcomputer.com/SW/AudioTools/Detect.htm)** —
  validation statistique d'authenticité lossless (référence historique).
- **[Fakin' The Funk](https://fakinthefunk.net/en/)** /
  **[Spek / Fabl](https://www.fabl.app/tools/audio-quality-checker)** —
  concurrents/références pour calibrer le **verdict UX**. _Référence-only._
- Principe technique commun : repérer la **coupure de fréquence** (frequency
  cutoff) qui trahit un encodage lossy planqué dans un WAV/FLAC. Côté Sift, c'est
  `rustfft` (déjà en deps) sur des trames PCM.

## M3 — Décodage / waveform / analyse

- **[Symphonia](https://github.com/pdeljanov/Symphonia)** — décodage audio **pur
  Rust** (FLAC, MP3, AAC, ALAC, WAV, AIFF, OGG…). _Statut : mûr, évalué ci-dessous
  → **adopter pour le chemin d'analyse**._
- **[bpm-finder-tools](https://crates.io/crates/bpm-finder-tools)** — détection
  BPM en Rust. _Statut : à évaluer si le BPM entre dans le scope._
- _(Détection de tonalité / key : **hors scope**, décidé le 2026-06-24.)_

## M5 — Empreinte / dédoublonnage / identification

- **[rusty-chromaprint](https://crates.io/crates/rusty-chromaprint)** — **déjà en
  dépendance** (`0.2`). Pur Rust, sur crates.io. Marche pour l'algo par défaut.
- **[chromaprint-next](https://github.com/attilagyorffy/chromaprint-next)** —
  alternative pur Rust **bit-identique** à la lib C. _Voir étude ci-dessous._
- **[Chromaprint / AcoustID (réf C)](https://github.com/acoustid/chromaprint)** —
  implémentation de référence + service d'identification en ligne.

## Export Rekordbox (partie historiquement la plus pénible)

- **[rbox](https://crates.io/crates/rbox)** — Rust, **lit ET écrit** le XML
  Rekordbox + One Library + fichiers d'analyse. _Statut : candidat n°1 pour
  « pousser des playlists »._
- **[rekordcrate](https://github.com/Holzhaus/rekordcrate)** — Rust, parse les
  exports device CDJ/XDJ (PDB + ANLZ + XML). _Statut : solide mais ⚠️ « heavy
  development », API susceptible de casser._
- **[pyrekordbox](https://pypi.org/project/pyrekordbox/)** — Python ; précieux
  comme **doc vivante des formats** Rekordbox même si non utilisé. _Référence-only._
- **[rekordbox-repair](https://github.com/edkennard/rekordbox-repair)**
  (edkennard, Scala) — répare les références de fichiers cassées après
  déplacement sur disque (pistes marquées "missing"). **Travaille sur l'export
  XML, pas sur `master.db`** : aucun accès DB, pas de SQLCipher, pas de
  verrou/USN/`masterPlaylists6.xml` — donc c'est la **voie XML** que le spike M8
  (Éval 5/7/11) cherche justement à dépasser, pas une avancée sur l'écriture
  native. _Statut : référence-only (Scala hors stack)._ **3 idées récupérables**
  (pas le code) : (1) le cas d'usage "réparation de chemins cassés" est un vrai
  besoin DJ concret, proche du spike M8 Task 3 (réparation `FolderPath`), plus
  simple et moins risqué que la synchro playlists complète ; (2) garde-fou
  multi-match — **refuse d'agir quand plusieurs fichiers matchent** le même nom
  et liste les candidats pour décision manuelle (cohérent méthode Sift :
  fail-fast, pas de fallback silencieux, jamais deviner sur une action
  coûteuse) ; (3) cas limites à couvrir non encore listés côté Sift : chemins
  > 255 caractères, fichiers réellement supprimés à retirer, fichiers non
  importés qui traînent.

## Renommage Discogs

- **[API Discogs](https://www.discogs.com/developers/)**. À intégrer dès le design :
  - **60 req/min** (authentifié clé+secret) vs **25 req/min** (anonyme).
  - **User-agent unique obligatoire** pour obtenir le quota max.
  - Lire les headers `X-Discogs-Ratelimit`, `-Used`, `-Remaining` pour throttler
    (fenêtre glissante de 60 s).
  - Flux de matching : `/search` → `/master` (le plus canonique pour titre/année)
    → `/release` pour le détail.
  - Token utilisateur requis seulement pour les ressources privées (collection,
    inventaire) — inutile pour du simple lookup de metadata.

---

## Évaluation 1 — Symphonia vs FFmpeg (2026-06-24)

**Question** : remplacer/compléter le sidecar FFmpeg par Symphonia (pur Rust,
in-process) pour le **chemin d'analyse** (décodage → PCM → `rustfft` / peaks /
empreinte).

**Méthode** : projet jetable hors-repo (`~/Desktop/sift-symphonia-probe`,
`symphonia 0.5` features mp3/flac/wav/aiff/aac/alac, build `--release`). Décodage
intégral en `f32`, mesure du wall-time (run à chaud). Comparé à
`ffmpeg -v error -i <f> -f null -` (décode tout, jette la sortie), bundlé Sift.
Fichiers : 2 vrais morceaux + fixtures du repo.

| Fichier | Durée | Symphonia | FFmpeg (incl. spawn) |
|---|---|---|---|
| Vrai FLAC | 5:44 (344 s) | 276 ms | **163 ms** |
| Vrai MP3 | 8:29 (509 s) | 710 ms | **632 ms** |
| Fixture FLAC | 10 s | **5,3 ms** | 67 ms |
| Fixture MP3 | 10 s | **9,2 ms** | 73 ms |

**Constats**
- FFmpeg gagne ~1,2–1,7× en **débit brut** sur les longs fichiers, mais paie un
  **coût fixe ~60 ms de spawn process par fichier** (visible sur les fixtures
  courtes où Symphonia est 8–12× plus rapide).
- En valeur absolue, décoder un morceau de 5–8 min coûte 0,2–0,7 s aux deux : non
  bloquant.
- Symphonia a décodé **correctement** tous les fichiers (sample rate, canaux,
  durée, peak, somme des magnitudes plausibles).
- Symphonia sort des **`f32` directement en mémoire** → branchement direct sur
  `rustfft` (déjà en deps). La voie FFmpeg impose spawn + pipe PCM `f32le` sur
  stdout + parsing (le `-f null` du bench ne pipe même pas les données).
- Symphonia est **decode-only** : **pas d'encodage**. FFmpeg reste **obligatoire**
  pour la conversion au format CDJ (étape « ranger »).

**Recommandation : architecture hybride.**
- **Garder FFmpeg sidecar** pour la **conversion CDJ** (Symphonia ne sait pas
  encoder) — il est de toute façon déjà bundlé.
- **Adopter Symphonia pour le chemin de lecture/analyse** (décode → PCM →
  `rustfft` / peaks / alimentation empreinte) :
  - pas de coût de spawn répété sur une biblio de milliers de fichiers (scan) ;
  - `f32` direct, intégration propre avec `rustfft`, zéro pipe/parsing fragile ;
  - pur Rust, multiplateforme, pas d'IPC.
  Le déficit de débit brut (~100 ms sur un long fichier) est négligeable face au
  gain de spawn + à la simplicité d'intégration.

**À garder en tête** : Symphonia 0.6 est sorti (testé en 0.5 pour stabilité API) ;
FFmpeg reste plus robuste sur fichiers exotiques/cassés (fallback utile).

> Projet de test conservé à `~/Desktop/sift-symphonia-probe` (hors repo, jetable —
> supprimable). Code du probe : `src/main.rs`.

---

## Évaluation 2 — chromaprint-next vs rusty-chromaprint (2026-06-24)

**Contexte** : Sift dépend **déjà** de `rusty-chromaprint 0.2`. La vraie question
n'est donc pas « ajouter chromaprint-next » mais « **faut-il migrer ?** ».

**Findings (chromaprint-next)**
- Pur Rust, **bit-identique** à la lib C de référence sur **les 5 variantes**
  d'algo (vérifié côte-à-côte), **~4 % plus rapide** (269 vs 258 Melem/s @ 120 s).
- vs `rusty-chromaprint` : ce dernier marche pour l'algo par défaut **mais**
  utilise un resampler différent, **ne reproduit pas certains bugs C nécessaires à
  la compatibilité avec la base**, et a des **presets incomplets sur 3 des 5
  variantes**.
- ⚠️ **Distribution** : uniquement en **dépendance git + submodules**
  (`clone --recursive`), pas un simple crate versionné crates.io comme
  rusty-chromaprint → friction de build/CI Win+Mac.
- ⚠️ **Licence** : **MIT AND LGPL-2.1-or-later** (le resampler est un port LGPL de
  `av_resample` de FFmpeg). OK pour une app desktop, mais à noter.
- MSRV : non documentée (à vérifier vs 1.77.2 avant adoption).

**Décision pilotée par l'usage de l'empreinte**
- **Dédoublonnage strictement local** (comparer les fichiers de la biblio entre
  eux) → seule la **cohérence interne** compte, pas la compatibilité bit-à-bit
  avec la base C. → **Rester sur `rusty-chromaprint`** (déjà en place, crates.io,
  zéro friction).
- **Identification via le service AcoustID en ligne** → la base AcoustID a été
  construite avec la lib C (bugs compris). Le **bit-identique de chromaprint-next
  améliore le taux de match**. → Envisager la migration, **après un spike** qui
  valide MSRV + build git/submodules en CI Win+Mac + licence.

**Recommandation** : ne pas migrer maintenant. Verrouiller d'abord si M5 vise
l'AcoustID en ligne ou seulement le dédoublonnage local. Si online → spike
chromaprint-next ; sinon → statu quo.

---

## Évaluation 3 — workflow d'itération UI en direct (2026-07-03)

**Question** : comment itérer sur l'UI (couleur/spacing/cohérence) sans rebuild
Tauri complet à chaque changement, avec Claude capable de voir le rendu avant
d'éditer.

**Constat (root cause confirmé sur le repo)** : trois implémentations UI
parallèles coexistent, pas deux — `design_handoff_sift_refonte/Sift.dc.html`
(maquette figée, DSL propriétaire, son propre `README.md` dit explicitement
« pas du code de production à copier tel quel ») ; `frontend/app.js` (355
lignes, aucun import, data bidon en dur, ses propres fonctions
`renderHome()`/`renderRevue()` — un **mock navigateur autonome**) ; le vrai
code (`chrome.ts`, `home-sources.ts`, `report-view.ts`…). `frontend/main.ts`
ne charge le vrai câblage (`installLiveWiring()`) que si `__TAURI_INTERNALS__`
existe ; `app.js` a le garde-fou inverse par vue (`if(!('__TAURI_INTERNALS__'
in window)){...}`). **Vérifié en lançant le serveur Vite et en screenshotant** :
le navigateur affiche bien le mock `app.js` (data "Mr. Fingers" en dur), jamais
le vrai `chrome.ts`.

**Pistes écartées** : Storybook / Pattern Lab (recréent une double
implémentation via un langage de template séparé) ; Cursor Visual Editor
(optimisé React, fiabilité douteuse sur `var(--token)`) ; migration React (ne
traite pas la root cause, non chirurgical).

**Piste testée : Claude in Chrome** (`mcp__claude-in-chrome__*`, équivalent de
`claude --chrome`/`/chrome` — pas de flag CLI de ce nom dans ce harnais).
Fonctionne réellement (navigateur connecté, screenshot obtenu), mais regarde le
même onglet Vite que ci-dessus → **voit le mock, pas le vrai code**. Valide
uniquement pour `styles.css` (fichier de tokens partagé par les trois
implémentations, non gated par `inTauri`), pas pour le markup.

**Piste retenue — `tauri dev` + HMR + computer-use** : `src-tauri/tauri.conf.json`
(`devUrl: http://localhost:5173`, `beforeDevCommand: npm run dev`) fait pointer
la vraie fenêtre Tauri sur le même serveur Vite. Dans cette fenêtre,
`__TAURI_INTERNALS__` existe réellement → le mock `app.js` se tait, le vrai
code s'affiche avec les vraies données IPC, et le HMR Vite s'applique en direct
à tout fichier `frontend/*.ts`/`styles.css` **sans rebuild Rust** (seul
`src-tauri/*.rs` en nécessite un). C'est littéralement l'app elle-même,
éditable en direct — il n'y a plus besoin de maquette. Reste à valider : que
`computer-use` (fenêtre desktop native, pas un onglet Chrome) peut effectivement
voir/cliquer cette fenêtre — nécessite l'autorisation `request_access` de
l'utilisateur, pas encore testé en pratique.

**Recommandation** : `styles.css` reste itérable via Chrome DevTools
Workspaces/`preview_*`/Claude in Chrome sur le serveur Vite nu (couleur/
spacing/tokens uniquement). Tout changement de markup/structure (`chrome.ts`,
`report-view.ts`, `home-sources.ts`…) doit être vérifié dans la vraie fenêtre
`tauri dev` via `computer-use`, jamais via le navigateur seul.

**Suite testée le même jour — `computer-use` écarté, `mcp__claude_design__*`
confirmé réel.** Antoine a explicitement écarté `computer-use` comme défaut
("ça bouffe trop de tokens") — voir mémoire
`prefer-ask-user-to-test-over-computeruse`. Le défaut redevient donc : Antoine
regarde lui-même la fenêtre `tauri dev` (retour instantané, zéro coût), avec
un screenshot ponctuel via `claude-in-chrome` seulement si un point précis a
vraiment besoin d'être vu — jamais une session interactive complète.

En creusant l'alternative "demander à Claude Design de juger à ma place" :
- `mcp__claude_design__*` est un accès réel et authentifié au compte
  claude.ai/design (`list_projects` a listé les projets existants, dont
  "Refonte UI Sift", celui qui a produit `Sift.dc.html`). Rôle confirmé :
  outil d'**exploration/wireframe pour une nouvelle direction** (génère des
  options, donne un lien à ouvrir), jamais un mécanisme de sync live avec le
  code réel — le réutiliser comme "maquette toujours à jour" recréerait
  exactement le problème diagnostiqué plus haut.
- `render_preview(render:true)` (rendu headless serveur → screenshot direct)
  est **désactivé sur ce compte** ("Server-side rendering not enabled here")
  — pas de raccourci gratuit pour que Claude Design "voie" à la place de
  Claude ou d'Antoine.
- `write_files` n'accepte que du contenu inline complet (`local_path` non
  implémenté côté serveur) — pousser un fix, même d'une ligne, sur un gros
  `.dc.html` coûte de faire transiter le fichier entier. À réserver aux vrais
  bugs, pas aux micro-syncs cosmétiques.
- **Bug structurel trouvé et corrigé** : `Sift.dc.html` ouvert seul (`file://`,
  hors de l'éditeur Claude Design) affichait des `{{...}}` bruts, aucune
  erreur console. Cause : `support.js` (le runtime dc, vérifié identique sur
  l'ancienne version restaurée ET sur une version fraîchement régénérée via
  `create_support_js`) exige `window.React`/`window.ReactDOM` déjà présents
  sur la page — fournis silencieusement par l'éditeur hébergé Claude Design,
  jamais par le fichier exporté lui-même. Fix : ajouter les balises
  `<script>` React 18.3.1 UMD (version épinglée + hash SRI) avant
  `support.js` dans `<head>`. Appliqué au fichier local et à la copie cloud,
  vérifié sans erreur console et avec les vraies données interpolées.

---

## Évaluation 4 — `/design-sync` et Open Design pour le sync design↔code (2026-07-03)

**Question** : deux outils évalués pour automatiser la synchronisation entre
`Sift.dc.html` (mockup Claude Design) et `styles.css`/le code réel, en
continuation de l'Évaluation 3 ci-dessus.

**`/design-sync` (outil natif `DesignSync`, disponible dans cette install
CLI)** — écarté après lecture du schéma complet de l'outil et vérification
réelle du projet cible. L'outil synchronise une **bibliothèque de composants**
vers un projet Claude Design de type `PROJECT_TYPE_DESIGN_SYSTEM`
(marqueurs `@dsCard`, `register_assets`...) — pas un mockup applicatif
unique. Vérifié via `get_project` : le projet "Refonte UI Sift" (celui qui a
produit `Sift.dc.html`) est de type `PROJECT_TYPE_PROJECT`, pas
`DESIGN_SYSTEM`, et ce type est immuable à la création. `/design-sync` ne
s'applique donc pas à l'artefact actuel sans le reconstruire entièrement en
composants cardés.

**Open Design (`nexu-io/open-design`, GitHub, vérifié via `gh api`)** — projet
réel et actif (74 629 ⭐, créé 2026-04-28, push quotidien), écarté quand même.
Architecture : daemon Node/Express + SQLite (persistant, à faire tourner en
continu, Electron ou Docker) qui spawn le CLI d'un agent de code en
sous-processus. Import `/api/import/claude-design` : **plus permissif que
supposé** — lu le code source (`claude-design-import.ts`), c'est un parseur
ZIP maison qui cherche juste un fichier `.html` dedans, sans validation de
schéma Claude Design (`Sift.dc.html` + `support.js` zippés fonctionneraient
mécaniquement). Mais la fonctionnalité qui résoudrait vraiment le problème
(pipeline `code-migration` : `design-extract`/`token-map`, crosswalk de
tokens source→cible) est **non implémentée** — les deux atomes concernés sont
explicitement marqués `Status: Reserved id, prompt-only fragment in v1` dans
leur propre `SKILL.md`. Rejeté pour deux raisons cumulées : service
persistant à maintenir pour un outil solo desktop (contradiction avec le
principe déjà appliqué à `tauri-plugin-updater`/`tauri-specta`, reportés pour
la même raison), et la fonctionnalité de fond n'existe pas encore.

**Root cause du vrai problème (pas un outil manquant)** : l'infidélité
design→code venait de deux causes distinctes, pas d'une absence de sync
automatique — (1) absence de vérification par état (hover/sélectionné/thème)
avant de déclarer un portage fini (voir `sift-audit-fidelite-methode`), et
(2) `Sift.dc.html` encode de la vraie logique JS conditionnelle par état
(ex. `M={green,amber,neutral,muted}` dans le fichier), pas de simples valeurs
statiques — la lire correctement demande de simuler cette logique, pas de la
copier.

**Décision** : pas d'outil de sync adopté. `docs/design-system-states.md`
(catalogue d'états par composant réel, alimenté 2026-07-03) sert de source de
vérité pour le portage — extrait du vrai code une fois, réutilisé à chaque
nouveau design plutôt que re-dérivé de `Sift.dc.html` à chaque fois. `Sift.dc.html`
reste un artefact d'exploration figé, jamais resynchronisé en continu.

---

## Évaluation 5 — spike lecture native `master.db` Rekordbox (2026-07-03)

**Contexte** : en brainstormant M7 (export/suivi de playlists Rekordbox via XML),
question posée : peut-on sync Sift↔Rekordbox nativement plutôt que par
export/réimport XML manuel ? Ça revient à la question déjà tranchée pour M8
(écriture directe `master.db`, **gelée** — voir plus haut) : ce spike teste la
**moitié lecture seule**, low-risk, de cette question, sans lever le gel sur
l'écriture.

**Méthode** : probe jetable hors repo (`~/Desktop/sift-rekordbox-probe/`,
**jamais le fichier live** — `master.db` copié une fois en `master.db.copy`,
tout le spike travaille sur cette copie). `pip install pyrekordbox` (0.4.4,
inclut `sqlcipher3-wheels` — déchiffrement SQLCipher géré nativement par la
lib, pas de clé à extraire manuellement). Script `probe.py` : ouverture via
`Rekordbox6Database(path=...)`, `get_content()` (tracks), `get_playlist()`
(playlists + hiérarchie dossier/liste), puis `playlist.Songs` pour la
composition d'une playlist.

**Résultat : succès complet.** Sur la vraie bibliothèque de l'utilisateur (via
la copie) : **2828 tracks**, **24 playlists** (structure plate ici, mais l'API
expose `is_folder`/`ParentID` pour la hiérarchie dossier). Chemins de fichiers
(`FolderPath`) lisibles et corrects (`D:/MUSIQUE 2025/MP3/...`). Appartenance
track↔playlist vérifiée sur `BACKUP DD A TRIER` (698 morceaux, chemins réels
cohérents). Seul warning : `masterPlaylists6.xml` absent du dossier de la copie
(fichier de checksums d'intégrité que Rekordbox place à côté de `master.db` en
prod — non bloquant pour une lecture, juste absent ici car on n'a copié que
le `.db`).

**Implication pour l'architecture M7/M8** :
- **Lecture native `master.db` est un remplacement viable** pour l'étape
  d'import XML de la brique 1/2 (plus besoin que l'utilisateur exporte
  manuellement une collection XML depuis Rekordbox — Sift peut lire
  directement l'état courant des playlists/tracks, en lecture seule, sans
  toucher au fichier live).
- Ça ne change **rien** à la question de l'écriture (toujours gelée, M8) : ce
  spike n'a testé aucune écriture, `db.update()`/`db.commit()` existent dans
  l'API pyrekordbox mais n'ont pas été exercés — le risque de corruption sur
  écriture reste entier et non évalué.
- Portage Rust : `pyrekordbox` est Python — à réévaluer si on veut porter cette
  lecture en Rust pur (candidats déjà en veille : `rekordcrate`, mais son parsing
  cible plutôt les exports device PDB/ANLZ, pas `master.db` directement ; pas
  encore vérifié qu'il gère le SQLCipher de `master.db`). Alternative : appeler
  `pyrekordbox` via un sidecar Python packagé, à évaluer côté coût de
  distribution (Sift est un seul binaire Rust aujourd'hui, un sidecar Python
  serait une vraie rupture d'architecture — à peser).

**Décision** : lecture seule de `master.db` validée comme faisable et sûre.
Reste à décider si M7 doit l'adopter à la place du round-trip XML (élimine
l'export manuel initial, garde le gel sur l'écriture) — discussion en cours
avec l'utilisateur, décision pas encore actée au moment de la rédaction de
cette entrée.

Probe conservé à `~/Desktop/sift-rekordbox-probe/` (hors repo, jetable —
supprimable). Code : `probe.py`.

---

## Évaluation 6 — re-vérification `/design-sync` vs Open Design pour le drift `Sift.dc.html`↔`styles.css` (2026-07-04)

**Contexte** : suite de l'Évaluation 4. Investigation détective sur le mécanisme
exact du drift (pas seulement la faisabilité des outils) : où et comment
`Sift.dc.html` diverge de `styles.css` aujourd'hui.

**Mécanisme du drift, confirmé sur le code réel** : `Sift.dc.html` n'a pas de
`:root{}` statique — les custom properties sont sérialisées à la volée en
style inline depuis un objet JS `theme()` (`Sift.dc.html:836-846`, 17 clés,
deux variantes clair/sombre), injecté ligne `Sift.dc.html:1067`. Diff des 17
clés contre `:root` de `styles.css:11-52` : **16/17 correctement portées**
(avec renommage non trivial par endroits — `nav`→`--color-background-tertiary`,
`card`→`--color-background-secondary`), **1/17 (`disabled`) absente** au
premier passage. Investigation plus poussée (voir
`docs/design-system-states.md`, section "Token `disabled`") : ce n'est pas un
oubli mais un non-besoin confirmé — les 3 usages réels de `T.disabled` dans la
maquette ont chacun un équivalent déjà en place dans le vrai code (réutilisation
de `--color-text-tertiary`, atténuation par `opacity` sur `button:disabled`,
canvas de waveform volontairement fixe). **Verdict : le drift constaté est
minime (0 vraie divergence sur 17 clés) et de nature transcription manuelle
valeur-par-valeur avec renommage** — pas un problème de volume qui justifierait
un outil de sync.

**`/design-sync` (outil natif `DesignSync`) — schéma relu en détail** :
confirme et durcit le rejet de l'Évaluation 4. L'outil ne connaît que des
méthodes sur arbres de fichiers (`list_files`/`get_file`/`write_files`/
`delete_files`, plus `register_assets` pour des cards `@dsCard`) vers un projet
`type: PROJECT_TYPE_DESIGN_SYSTEM`, **immuable à la création** (le schéma de
`get_project` le dit explicitement). Aucune méthode d'extraction de valeurs
depuis un objet JS conditionnel — même en reconstruisant "Refonte UI Sift" en
design system, l'outil n'aurait aucune prise sur le mécanisme réel du drift
(objet `theme()` → chaîne de style inline). Mauvais niveau d'abstraction,
confirmé indépendamment de la question de type de projet.

**Open Design (`nexu-io/open-design`)** — aucune ré-vérification réseau
nécessaire : les deux blocages notés le 2026-07-03 (daemon persistant à
maintenir pour un outil desktop solo ; fonctionnalité crosswalk de tokens
`design-extract`/`token-map` marquée "Reserved id, prompt-only fragment in v1"
dans son propre `SKILL.md`) sont structurels, pas des questions de version —
rien n'indique qu'ils aient changé en un jour.

**Décision confirmée** : statu quo. `docs/design-system-states.md` reste la
source de vérité, alimentée manuellement lors des audits ponctuels — un
diff périodique des ~17 clés de `theme()` contre `:root` (comme fait ici) est
suffisant vu le faible volume et l'absence de vrai gain d'un outil externe.
Aucun code modifié suite à cette évaluation (le seul candidat, `disabled`,
s'est avéré un non-besoin après vérification des consommateurs réels).

---

## Évaluation 7 — spike d'écriture `master.db` (2026-07-04)

**Contexte** : suite de l'Évaluation 5 (lecture seule validée). M8 est
gelé (`docs/plan-implementation.md:236-243`) jusqu'à preuve qu'un
round-trip d'écriture ne corrompt pas `master.db`. Ce spike teste
exactement ça, sur une copie jetable (`~/Desktop/sift-masterdb-write-probe/`,
jamais le fichier live).

**Méthode** : `pyrekordbox` (déjà utilisé en lecture) pour 3 scénarios —
réparation de `FolderPath`, dédup d'une entrée de playlist dupliquée,
détection de verrou fichier. Chaque test relit avec une connexion fraîche
pour confirmer le round-trip (pas juste l'état en mémoire).

**Résultat** : les 4 tests passent, aucun échec.

- Baseline (Task 2) : `track_count=2828`, `playlist_count=24` — cohérent
  avec l'Évaluation 5 (même bibliothèque, lecture seule).
- Task 3 (réparation de chemin) — **PASS**. `db.commit()` persiste
  correctement une modification de `FolderPath` ; round-trip
  écriture→fermeture→réouverture fraîche→relecture fidèle. `track_count`
  inchangé (2828), autres champs (`Title`) intacts. Aucun signal de
  corruption SQLCipher/HMAC.
- Task 4 (dédup playlist) — **PASS**. Une entrée dupliquée injectée dans
  `djmdSongPlaylist` puis supprimée jusqu'à n'en garder qu'une, vérifié via
  connexion fraîche : aucune référence orpheline, le track existe toujours
  dans `djmdContent`. Note : l'API réelle de pyrekordbox 0.4.4 utilise des
  arguments positionnels (`add_to_playlist(playlist, content)` /
  `remove_from_playlist(playlist, song)`), pas les kwargs
  `playlist_id=`/`content_id=`/`song=` supposés dans le plan initial.
- Task 5 (verrou fichier) — **PASS**, avec nuance importante.
  `master.db.copy` est chiffré SQLCipher (pas du SQLite en clair) — le
  blocker de test a dû utiliser `sqlcipher3` plutôt que le module stdlib
  `sqlite3` pour même pouvoir ouvrir le fichier. Une transaction `BEGIN
  EXCLUSIVE` concurrente a bien fait lever
  `sqlalchemy.exc.OperationalError: database is locked` sur `db.commit()`
  côté pyrekordbox — donc SQLite protège nativement contre une corruption
  par écriture concurrente. Mais ceci n'est qu'un filet de rattrapage a
  posteriori : une vraie implémentation de prod doit faire sa propre
  vérification de verrou/process AVANT d'écrire (ex.
  `pyrekordbox.utils.get_rekordbox_pid()` pour détecter si Rekordbox
  tourne), pas se reposer uniquement sur le fait de catcher cette exception.

**Implication pour M8** : les 3 scénarios d'écriture critiques (réparation
de chemin, dédup de playlist, comportement sous verrou) passent tous sur
une copie de la vraie bibliothèque de l'utilisateur, via `pyrekordbox`
(Python). Ceci valide la **faisabilité et la sûreté fonctionnelle** de
l'approche d'écriture — mais ce spike reste en Python via une lib tierce,
pas le portage Rust pur (symétrique au lecteur SQLCipher M7) qui serait
nécessaire pour la prod. M8 peut donc passer d'un statut "gelé, design non
prouvé" à "prouvé faisable en Python, portage Rust restant à
spécifier/faire" — ce n'est **pas** encore un feu vert pour écrire du code
de production Rust sans une étape de portage supplémentaire, et le point de
verrou (Task 5) doit être traité explicitement dans ce portage (vérifier
qu'aucun process Rekordbox ne tourne, pas seulement catcher une exception
SQLite).

**Décision** : le gel de M8 (`docs/plan-implementation.md:236-243`) peut
être levé pour la partie "écriture est possible et sûre en principe" mais
reste conditionné à : (1) un futur portage Rust du write path (symétrique
au lecteur déjà écrit pour M7), (2) l'implémentation d'une vérification
explicite de process Rekordbox avant écriture (pas seulement une exception
catchée), avant tout code de prod. Documenter ceci comme prochaine étape,
ne pas encore lancer l'implémentation Rust dans le cadre de cette tâche.

Probe conservé à `~/Desktop/sift-masterdb-write-probe/` (hors repo,
jetable — supprimable). Scripts : `baseline.py`, `test_path_repair.py`,
`test_playlist_dedup.py`, `test_file_lock.py`.

---

## Évaluation 8 — outil de sync de tokens design↔code, construit (2026-07-04)

**Contexte** : suite des Évaluations 4/6 (drift `Sift.dc.html`↔`styles.css`, outils
externes rejetés). Plutôt qu'un outil externe, un petit outillage maison a été
construit et livré cette session : `design_handoff_sift_refonte/token-sync/`.

**Ce qui existe** :
- **Canonique** : `design-tokens.json` (clair/sombre) + `alias-map.json` (noms
  legacy `theme()` ↔ noms de prod `--color-*`).
- **3 générateurs** (`generate-styles-css.cjs`, `generate-theme-html.cjs`,
  `generate-design-md.cjs`) : canonique → `styles.css` / `Sift.dc.html` /
  `DESIGN.md`, dry-run par défaut, `--write` pour persister, no-op vérifié à
  chaque fois.
- **2 scripts `pull-*`** (sens inverse, remontée vers le canonique) :
  `pull-styles-css.cjs` (édit à la main sur `styles.css`) et
  `pull-theme-html.cjs` (édit dans Claude Design sur `Sift.dc.html`) —
  partagent la même baseline `last-sync.json`, avec détection de conflit
  explicite (jamais de résolution automatique silencieuse).
- **`apply-tokens.cjs`** : CLI que j'utilise moi-même (pas besoin de navigateur
  ni de serveur) — édite `design-tokens.json`, lance ce script avec `--write`.
- **`editor.html` + `editor-server.cjs`** : UI pensée pour un usage non-technique
  (groupes repliables, libellés en français, sélecteurs de couleur natifs,
  preview live incluant la maquette complète interactive), plus un aperçu des
  "consommateurs réels" (`locate.cjs`, grep+contexte sur `frontend/`) pour
  chaque token modifié.
- **`dev_locate.rs` + `dev-inspector.ts`** : inspecteur Alt+Clic dans la vraie
  app `tauri dev` (debug-only), pointe un élément cliqué vers son fichier/ligne
  réel.

**Workflow de sync avec Claude Design** (le vrai projet cloud "Refonte UI Sift",
`mcp__claude_design__*`) :
1. Antoine édite les couleurs dans Claude Design (interface web).
2. Fetch à la demande : `list_files` (compare l'`etag` du fichier à la dernière
   valeur connue, sans tout retélécharger) puis `read_file` si ça a changé —
   écrase la copie locale de `Sift.dc.html`.
3. `pull-theme-html.cjs` remonte les valeurs dans le canonique (conflit
   signalé si le canonique a aussi bougé depuis le dernier sync).
4. `apply-tokens.cjs --write` repropage vers `styles.css`/`DESIGN.md`.
5. Rien de plus à faire : l'app réelle se recharge via HMR `tauri dev`, et
   `editor.html` refait un `GET /tokens.json` à chaque chargement de page.

**Décision actée : pas de veille automatique (cron)**. `CronCreate` permettrait
de poller l'`etag` du projet Claude Design à intervalle régulier (ex. 15-30 min)
et déclencher fetch + pull + `PushNotification` sans computer-use (l'accès
Claude Design est déjà une API directe, jamais un navigateur piloté). Écarté
pour l'instant : le déclenchement manuel ("je te dis fetch") coûte zéro effort
supplémentaire pour un usage solo, alors qu'un cron ajoute une vraie surface
(job à surveiller, expiration à 7 jours, notifications à calibrer) pour un
gain marginal. À reconsidérer seulement si le besoin réel apparaît ("je veux
bosser une heure dans Design sans avoir à te le redire à chaque fois").

**Audité et corrigé le même jour** : 2 agents de revue parallèles (bugs +
nettoyage/conventions) + `/impeccable audit` sur `editor.html`, 8 correctifs
identifiés et livrés via `subagent-driven-development` (5 tâches, chacune
implémentée + revue par un agent frais, revue finale de branche "Ready to
merge: Yes"). Un suivi tracé, non bloquant : `generate-design-md.cjs`'s
`expectedDarkCount` dépend d'un "+2" lié à une section sans rapport
(`## Composants`) à cause d'une variable préexistante (`restFromDark`) non
bornée — faux positif possible si cette section change, pas corrigé (hors
scope, risque faible sur un outil interne).

---

## Évaluation 9 — token-sync tool v2 : 4 outils externes + spec DTCG officielle (2026-07-04)

**Contexte** : suite de l'Évaluation 8. 4 outils trouvés par Antoine
(`TrySound/engramma`, Tokens Studio, Style Dictionary, Magic Patterns) plus 2
sources DTCG faisant autorité (designtokens.org/tr/drafts/resolver/,
alwaystwisted.com — design tokens workflow part 7) évalués pour voir s'ils
remplacent notre outil ou apportent des idées à incorporer.

**Verdict par outil** :
- **Style Dictionary** : prior art le plus proche pour la partie push
  (canonique → N formats), mais ses formats ne couvrent aucune de nos 2
  cibles propriétaires (`Sift.dc.html`, `DESIGN.md`) — écarté comme
  dépendance.
- **Engramma** : éditeur web avec live-preview réel (comparable à notre
  `editor.html`), mais I/O JSON(DTCG)/CSS/SCSS seulement, pas de vraie
  reconciliation bidirectionnelle avec conflit — idée d'auto-refresh
  incorporée, pas l'outil lui-même.
- **Tokens Studio** : plugin Figma. Idée de format DTCG + cascade de token
  sets incorporée (voir Section A du design v2), outil lui-même écarté (on
  n'est pas sur Figma).
- **Magic Patterns** : hors sujet (génération de code UI par prompt).

**Figma comme remplacement de Claude Design — écarté** : creusé suite à la
question d'Antoine sur pourquoi tout l'écosystème pointe vers Figma plutôt
que Claude Design. Root cause confirmée : `Sift.dc.html` est un format
propriétaire que rien d'externe ne comprend (d'où `pull-theme-html.cjs`
maison). Migration vers Figma jugée non rentable : **l'API REST Variables de
Figma (lecture ET écriture) est réservée aux comptes Enterprise** (vérifié
sur developers.figma.com — "This API is available to full members of
Enterprise orgs"), donc même après migration le sync resterait manuel (export
JSON via le plugin Tokens Studio, pas d'automatisation), pour le coût de
reconstruire toute la maquette `Sift.dc.html` dans Figma. Pas de doc dédiée
pour l'instant, actée dans la conversation.

**Spec DTCG officielle vs conventions tierces (correction faite en session)** :
la première passe de design confondait la convention de theming de Tokens
Studio (cascade de "token sets", pas dans le spec DTCG) avec le vrai module
Resolver DTCG (`resolver.json` : `sets`/`modifiers`/`contexts`/`resolutionOrder`,
vérifié sur designtokens.org). Après lecture du spec officiel + d'un workflow
réel (alwaystwisted.com, qui n'utilise PAS de resolver formel pour un cas à 2
modes), décision : format de token réellement DTCG (`$type`/`$value`
structuré) mais **sans** le module Resolver formel ni Terrazzo comme
dépendance — jugés être de la machinerie disproportionnée pour 2 modes fixes.

**Décision finale + implémentation** : voir
`docs/superpowers/specs/2026-07-04-token-sync-tool-v2-design.md` (design complet,
4 sections : architecture DTCG, consolidation partielle des générateurs,
navigation barre latérale+recherche façon panneau Variables Figma, aperçu
auto-rafraîchi façon engramma) et
`docs/superpowers/plans/2026-07-04-token-sync-tool-v2.md` (plan à 8 tâches,
exécution laissée à une session ultérieure).

---

## Évaluation 10 — token-sync tool v2 exécuté + audit UX post-plan (2026-07-04)

**Contexte** : exécution du plan à 8 tâches de l'Évaluation 9
(`docs/superpowers/plans/2026-07-04-token-sync-tool-v2.md`) via
subagent-driven-development, suivie d'un audit UX/UI et de plusieurs
sessions de debug en direct sur `editor.html`.

**Migration DTCG (Tasks 1-8) : terminée, revue finale "Ready to merge".**
Un vrai gap trouvé pendant l'exécution (pas dans le design initial) : 13 des
33 tokens couleur réels sont des `rgba(r,g,b,a)`, pas du hex — le plan
original ne gérait que le hex. Addendum ajouté à
`docs/superpowers/specs/2026-07-04-token-sync-tool-v2-design.md` (fonctions
`parseColorValue`/`cssColorLiteral` dans `sync-core.cjs`), tous les
générateurs/pull-scripts migrés en conséquence. Task 5 (pull scripts)
contenait un vrai bug de perte de données : le code du plan comparait
`.$value.hex` pour la purge de `dark.json`, ce qui aurait effacé silencieusement
toute valeur rgba divergente en dark (`null === null` toujours vrai) — corrigé
par comparaison de la valeur littérale (`cssColorLiteral`), confirmé par la
revue finale comme un vrai risque de perte de données évité, pas hypothétique.

**Suivi immédiat — mode sombre permanent + fixes UX (hors plan initial,
demandé par Antoine après coup)** : chrome de l'éditeur passé en sombre
permanent (pas de toggle), lu dynamiquement depuis `/tokens.json` (pas un
snapshot figé — un rechargement de page reflète toujours l'état réel des
tokens dark de Sift). 2 bugs UX trouvés en auditant : le hint de groupe
("surfaces de l'app" etc., dont un documente la règle "seule 3e teinte
autorisée" du bouton Identifier) disparu silencieusement lors du refactor
sidebar+recherche (Task 7) — restauré ; highlight sidebar figé sur l'ancien
groupe actif pendant une recherche transversale — corrigé.

**Bug racine trouvé en investiguant "les éditions ne montrent aucun
changement"** : `frontend-styles.css`'s règle média sombre utilise le
sélecteur `:root:not([data-theme="light"])` (spécificité 0,2,0) — plus
élevée que le bloc `:root{}` de live-preview de l'éditeur (0,1,0,
`buildOverrideCss()`). `document.documentElement` n'avait jamais son
attribut `data-theme` initialisé avant le premier clic sur le toggle — donc
dans n'importe quel navigateur préférant le mode sombre, cette règle statique
gagnait toujours sur les éditions en direct, quel que soit le mode édité.
Fix : initialiser `data-theme="light"` au chargement, correspondant à l'état
initial déclaré du bouton toggle.

**Découverte plus large en creusant le rapport "Texte vert — OK ne change
rien"** : ce n'est pas un bug de rafraîchissement — `alias-map.json` ne
mappe que 16 des 33 tokens réels vers des clés legacy de `Sift.dc.html`.
**17 tokens n'ont donc aucune voie d'aperçu live nulle part**, ni Aperçu
rapide ni Maquette complète (tout "États vert/ambre" - 7, tout
"Survol/sélection" - 4, tout "Bouton Identifier" - 4, 2 des 4 "Bordures").
Le seul moyen de vérifier ces 17 tokens aujourd'hui : éditer, Valider, puis
regarder la vraie app Sift qui tourne. Scindé en deux, avec Antoine : Groupe
A (3 tokens texte de verdict, `semGreen()`/`semAmber()` déjà présents dans
`Sift.dc.html` mais non branchés sur `theme()` — pur rewiring bas risque,
design écrit dans
`docs/superpowers/specs/2026-07-04-mockup-verdict-text-color-rewire-design.md`,
implémentation laissée à une session dédiée) ; Groupe B (10 tokens restants,
fonds de verdict + survol + bouton Identifier — nécessite de la vraie
nouvelle UI dans la maquette, décision de scope séparée non actée).

**Groupe B — abandonné (2026-07-09)**, pas construit. Depuis, `Sift.dc.html`
est devenu explicitement un artefact d'exploration figé, jamais resynchronisé
(CLAUDE.md, token-sync v3 — `alias-map.json` ne pivote plus que vers
`generate-theme-html.cjs`, plus de JSON canonique) ; la vérification standard
du projet pour tout ce qui est gated `inTauri`/non couvert par les tokens
statiques est devenue CDP contre la vraie fenêtre `tauri dev` (Évaluation 11).
Construire une UI de preview dédiée dans la maquette pour ces 10 tokens irait
à contre-sens de cette évolution. Décision : ces tokens restent non mappés
dans `alias-map.json`, état permanent et voulu — se vérifient directement
dans l'app réelle, pas dans l'éditeur.

**Incident méthodologique à retenir** : pendant le debug en direct de
l'éditeur (test du rafraîchissement debounced), une série d'appels
`/validate` de test a laissé `frontend/styles.css` +
`design-tokens.{light,dark}.json` avec des couleurs de test polluantes
(`#ff0095`/`#fe3401` sur `text-success`) non révertées — découvert
seulement en vérifiant les VRAIES valeurs des tokens danger/warning pour la
conception du Groupe A. Corrigé (`git checkout`), confirmé propre par la
chaîne de sync complète. **Leçon retenue** : après toute session de test en
direct qui appelle `/validate` (même via des edits synthétiques automatisés),
vérifier `git status`/`git diff` sur les 3 fichiers cibles avant de
continuer — ne pas supposer qu'un revert manuel isolé suffit.

---

## Évaluation 11 — chantier triple exécuté : token-sync v3 + spike CDP + spike M8 n°2 (2026-07-05)

**Contexte** : exécution du design
`docs/superpowers/specs/2026-07-04-token-sync-v3-cdp-spike-m8-spike-design.md`
(3 volets parallèles par agents Sonnet, audit final Fable). Rien de committé
par les agents ; état vérifié indépendamment par la session principale.

**Volet A — token-sync v3, livré.** `frontend/styles.css` est désormais le
canonique unique. Nouveau cœur `styles-css.cjs` (parse/write des 3 blocs,
substitution in-place, round-trip no-op octet-identique, fail-fast partout :
blocs sombres divergents, token/préfixe inconnu, doublon) + `verify-v3.cjs`
(6 assertions, remplace les 2 anciens scripts de vérif). `editor-server.cjs`
parse styles.css à chaque `GET /tokens.json` et écrit via le writer sur
`/validate` ; `generate-theme-html`/`generate-design-md`/`apply-tokens`
lisent styles.css ; `editor.html` inchangé (shapes JSON conservées).
**Supprimés** : `design-tokens.{light,dark}.json`, `last-sync.json`,
`pull-styles-css.cjs`, `pull-theme-html.cjs`, `migrate-to-dtcg.cjs`,
`sync-core{,.verify}.cjs`, `verify-roundtrip.cjs`, `generate-styles-css.cjs`
(styles.css n'est plus une cible). L'heuristique `colorProdKey`
(startsWith) disparaît avec les pulls — classification par tables de
préfixes explicites, préfixe inconnu = throw. La baseline et la détection de
conflit n'ont plus d'objet (un seul fichier fait foi). Le DTCG v2 (Éval 9/10)
est abandonné : il ne servait que l'interop, qu'on n'a pas. Vérifié en
audit : `verify-v3.cjs` tout vert, `apply-tokens` no-op, `git diff` vide sur
`frontend/styles.css`.

**Volet B — spike CDP WebView2 : VALIDÉ.** Lancer `tauri dev` avec
`WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9222` expose
un endpoint CDP standard (`http://localhost:9222/json`, WebView2/Edg 149) sur
la vraie fenêtre Tauri. Preuves obtenues depuis la session Claude (Node 26,
WebSocket natif, zéro dépendance) : `Runtime.evaluate` →
`__TAURI_INTERNALS__: true`, titlebar custom `#sift-tb-title="Sift"`, écran
Revue actif, aucune trace du mock (`Mr. Fingers` absent) ;
`Page.captureScreenshot` → capture de la vraie app avec vraies données.
**C'est la première voie qui permet à Claude de voir/inspecter le code gated
`inTauri` sans mobiliser Antoine ni computer-use** — coût : 2-3 appels
ponctuels. Reste le défaut : Antoine regarde lui-même la fenêtre HMR ; le CDP
sert à la vérification ponctuelle par preuve (screenshot, style calculé,
DOM réel).

Complément vérifié dans les sources de notre version exacte (tauri 2.11.3 /
tauri-utils 2.9.3, `config.rs:2081-2083`) : Tauri expose aussi une option de
config par fenêtre `additionalBrowserArgs` (Windows-only) qui ferait la même
chose via `tauri.conf.json`. **Écartée volontairement, garder la variable
d'environnement** : (1) l'option écrase les arguments par défaut de wry
(`--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection` — à
re-fournir soi-même si on l'utilise, dixit le doc-comment de la struct) ;
(2) surtout, une valeur dans `tauri.conf.json` ouvrirait le port de debug
**aussi dans les builds de prod** — inacceptable. La variable
`WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS` est ad-hoc par session de dev,
n'altère pas les défauts wry, et ne peut pas fuiter dans un installeur.

**Volet C — spike M8 n°2 (masterPlaylists6.xml + colonnes USN) : exécuté,
verdict Rekordbox en attente.** Détail complet :
`~/Desktop/sift-masterdb-write-probe/FINDINGS-m8-spike-2.md` (run faisant
foi : `full-copy-main\`, artefacts `m8s2_*`). Résultats clés : (1) une modif
`FolderPath` via pyrekordbox change **exactement 3 colonnes** (`FolderPath`,
`rb_local_usn`, `updated_at` — posé par l'ORM, pas par un trigger SQLite) ;
(2) USN global `agentRegistry.localUpdateCount.int_1` +1 par changement, la
ligne reçoit la **nouvelle valeur globale** (vérifié code
`registry.py:311-347`) ; (3) `masterPlaylists6.xml` **est réécrit** par
`commit()` (tous les Timestamps resynchronisés depuis
`djmdPlaylist.updated_at`, seuil >1 s, `database.py:428-450`) — mais
probablement par décalage de fuseau dans le parsing pyrekordbox, pas par
nécessité sémantique pour une réparation de chemin pure ; (4) `commit()`
refuse si Rekordbox tourne (`get_rekordbox_pid()`, `database.py:418-422`) —
confirme l'invariant n°2 de la spec Rust. **Étape restante (Antoine,
manuelle, §6 du FINDINGS)** : swap backup→copie modifiée→ouvrir
Rekordbox→vérifier acceptation→restaurer. Le verdict conditionne le design
Rust (répliquer ou non la réécriture XML). La spec
`2026-07-04-m8-masterdb-write-path-rust-design.md` reste bloquante tant que
ce verdict n'est pas noté.

**Incident méthodologique à retenir (orchestration d'agents)** : les agents
Sonnet spawnés en arrière-plan ont **délégué récursivement** au lieu
d'exécuter (4 niveaux de cascade sur les volets A/C, chaque maillon "lançant"
le travail puis s'arrêtant), et deux exécuteurs concurrents du volet C se
sont écrasés mutuellement les artefacts (`full-copy\` pollué, run refait dans
`full-copy-main\`). Correctif efficace : reprendre chaque maillon via
SendMessage avec interdiction explicite de l'outil Agent + obligation
d'exécuter avec les outils directs. Pour un prochain fan-out : mettre cette
interdiction dans le prompt initial des agents.

---

## Évaluation 12 — pointeur visuel d'annotation, construit (2026-07-05)

**Contexte** : suite de l'inspecteur Alt+Clic dev-only existant
(`dev-inspector.ts` + `dev_locate.rs`, qui ne faisait que localiser un élément
cliqué vers son fichier:ligne). Besoin exprimé par Antoine : pouvoir **pointer
un problème visuel** dans la vraie app en marche et le décrire en langage
libre, pour que Claude le corrige — sans qu'Antoine touche au code, et sans
décrire l'emplacement avec des mots (frustration principale : Claude ne trouve
pas toujours le bon endroit). Écarté explicitement pendant le brainstorm :
éditeur visuel où Antoine édite lui-même, capture d'écran (mauvais canal, cf.
mémoire `screenshot-not-a-value-source`), serveur/daemon, migration de
framework (Electron/Neutralino/React — vérifié qu'aucun n'offre nativement
« clic → réécrit le fichier source », c'est toujours de l'outillage maison).

**Ce qui a été livré** (commits `5437a34`, `f370169`, `2ba1093`) :
- **`src-tauri/src/dev_annotate.rs`** : commande `save_annotation(annotation:
  serde_json::Value)`, gated `cfg!(debug_assertions)`, ajoute un champ `ts`
  epoch et **append** une ligne JSON dans `docs/annotations.jsonl`. Jamais
  d'écriture dans les sources. Seule cible d'écriture de tout l'outil.
- **`frontend/dev-annotate.ts`** : capture de contexte — identité de l'élément,
  **valeurs calculées réelles** (`getComputedStyle` filtré à ~25 propriétés
  visuelles, pas une image), ancêtres (≤8) et frères (≤6), écran actif
  (`#nav .nv.on` → `data-view`), et localisation code via `locate_source`.
- **`frontend/dev-inspector.ts`** (refondu) : Alt+Clic pose un **cadre de
  highlight** sur l'élément, bouton « ⬆ bloc parent » pour remonter au
  conteneur logique, `<textarea>` note libre + « Envoyer » (garde note-vide,
  bouton désactivé pendant l'envoi, fail-fast affiché sans retry). Les boutons
  de localisation restent à la demande dans le même panneau.

**Double gating dev-only** : `import.meta.env.DEV` (import dynamique dans
`main.ts:44`) côté front + `cfg!(debug_assertions)` côté Rust — ne peut pas
fuiter dans un build de prod.

**Workflow** : Antoine ouvre `tauri dev`, Alt+Clic sur ce qui le gêne, tape une
remarque (« trop tassé », « pas cohérent avec la Bibliothèque »), Envoyer. Puis
dans la session il dit « regarde » → Claude lit `docs/annotations.jsonl`,
traite chaque entrée (localise, compare contre l'autre écran ou `Sift.dc.html`
si la note évoque cohérence/maquette), applique le fix, et **retire l'entrée
traitée** du fichier. Aucune veille automatique (déclenchement conversationnel
seulement). Le fichier `docs/annotations.jsonl` est non gitignoré exprès : le
voir dans `git status` rappelle qu'il reste des notes à traiter.

**Hors scope v1** : problèmes de comportement animé (la note texte décrit la
séquence, le pointage donne l'élément de départ), capture d'image, édition dans
le panneau. **Construit via subagent-driven-development** (3 tâches Opus,
chacune revue PASS spec+qualité, revue finale de branche « Ready to merge »).
Vérifs : `cargo test dev_annotate` 2/2, clippy `-D warnings` clean, `tsc
--noEmit` clean. Spec/plan :
`docs/superpowers/specs/2026-07-05-visual-pointer-annotation-design.md`,
`docs/superpowers/plans/2026-07-05-visual-pointer-annotation.md`.

---

## Évaluation 13 — prompt externe « Figma local » (éditeur visuel DOM), audité et rejeté tel quel (2026-07-05)

**Contexte** : Antoine a soumis un prompt généré par un autre modèle
(« SIFT — STUDIO DESIGN-TO-CODE ») proposant de construire un éditeur visuel
DOM natif type Figma local (sélection hover/click, bounding box, panneau
`getComputedStyle`, drag/resize → écriture disque). Audit demandé avant tout
usage. **Verdict : rejeté tel quel** — bien structuré en surface, mais
factuellement faux sur le projet, redondant avec l'outillage déjà livré, et
muet sur le seul problème réellement difficile.

**Trois défauts rédhibitoires** :
1. **Faits faux sur le repo** : source of truth annoncée `theme.css`
   (n'existe pas ; le canonique est `frontend/styles.css` depuis token-sync
   v3, et créer un fichier de thème parallèle est interdit par CLAUDE.md) ;
   `app.js` présenté comme la logique DOM réelle (c'est la maquette figée —
   le piège n°1 documenté du repo) ; stack « Vanilla JS » alors que le front
   est en vanilla TypeScript modulaire.
2. **Sa « première tâche » est déjà livrée** : hover/click + bounding box +
   lock + styles calculés = `dev-inspector.ts`/`dev-annotate.ts`/
   `dev_locate.rs` (Évaluation 12), avec en plus la localisation
   fichier:ligne que le prompt ne prévoit pas ; l'écriture de variables CSS
   sur disque = `editor.html`/`editor-server.cjs` (token-sync v3).
3. **Le problème dur est esquivé** : le cœur d'un Figma local est le
   **mapping inverse mutation DOM → édition source**. Le DOM de Sift est
   généré par des fonctions de rendu TS (template strings, conditionnels,
   état) — pas de correspondance 1:1 DOM↔markup. Le prompt ne dit jamais ce
   qu'un drag/resize écrit ni où (style inline ? positionnement absolu ? les
   deux détruisent flex/grid + tokens), et sa boucle « mutation DOM d'abord,
   disque ensuite » est à l'envers : le premier re-render/HMR écrase la
   mutation. C'est exactement le mur qui a fait rejeter les outils de sync
   aux Évaluations 3/4/6 et pivoter vers le modèle annotation (Antoine
   pointe et décrit, Claude édite le code).

**Défauts secondaires** : aucune étape de vérification (pas de critère
d'acceptation, pas de mention du piège preview≠`inTauri` ni du CDP validé
en Évaluation 11) ; « PowerShell uniquement » arbitraire ; « code complet ou
patch » invite aux réécritures full-file ; sections finales = discours
commercial du modèle générateur, pas des instructions ; alternance
MODE CONCEPTION/IMPLEMENTATION sans gate, traversée en une réponse.

**Récupérable** : le format de sortie discipliné (fichier concerné → patch →
explication courte) et l'idée d'un contrat produit explicite en tête de
prompt. Rien d'autre.

**Si l'idée est poursuivie un jour** : le bon chantier n'est pas la
sélection DOM (faite) mais le **mapping inverse contraint** — un éditeur qui
n'autorise que des mutations exprimables dans le système existant (changer
un token, une classe, une valeur d'échelle `--space-*`/`--text-*`), jamais
de freeform drag écrivant des pixels ; ancré sur les vrais fichiers
(`styles.css`, `dev-inspector.ts`, `dev_locate.rs`), vérifié par CDP, avec
une réponse explicite à « que devient l'édit quand le render re-run ». Reste
la question de scope non tranchée (brainstorm du même jour, interrompu) :
job « éditer l'existant » (pile actuelle ≈ complète) vs job « explorer un
design qui n'existe pas encore » (rôle actuel de Claude Design, non couvert
localement).

---

## Évaluation 14 — spike M8 sur le fichier live Rekordbox : incident et repli (2026-07-05)

**Contexte** : suite des spikes M8 sur copie (Évaluations 5/7/11, `master.db`
jamais touché en live). Cette session a, pour la première fois, testé un
scénario "metadata reload" directement sur le **vrai** `master.db` de
l'utilisateur (backup → swap → vérification dans Rekordbox → restore), à sa
demande explicite. Ça a mal tourné, puis révélé un problème plus profond que
le test lui-même.

**Ce qui s'est passé** :
1. Un agent en arrière-plan issu d'une chaîne de délégation en cascade (le
   même phénomène que l'Évaluation 11) a rapporté le fichier de test "stable,
   `TrackInfoUpdated=7`, prêt pour le swap" — une relecture indépendante a
   montré qu'il avait en réalité dérivé à 9, avec des rapports contradictoires
   entre eux (7 puis 8). Un rerun propre et isolé (copié depuis
   `master.db.copy`/`masterPlaylists6.xml.pristine`, jamais depuis le dossier
   pollué) a confirmé l'hypothèse du spike : une édition metadata-only
   (`FolderPath`/tag) laisse bien `Analysed`/`AnalysisUpdated`/`CueUpdated`
   inchangés.
2. Malgré ce rerun propre, le swap réel dans le dossier Rekordbox live a fini
   par utiliser l'**ancien fichier pollué**, pas la version propre — écart
   d'exécution non totalement élucidé (probable désynchronisation de timing
   entre les instructions et le geste de copie). Le fichier pollué avait un
   `ArtistID` cassé (pointant vers une entrée `djmdArtist` inexistante),
   provoquant un champ Artist vide dans Rekordbox une fois ouvert — et
   Rekordbox a lui-même continué à écrire dans `master.db` à l'ouverture
   (`TrackInfoUpdated` 9→10).
3. **Le vrai problème** : le backup pris juste avant ce swap (censé être
   l'état "propre" à restaurer en cas de souci) s'est avéré **déjà
   contaminé** — le `FolderPath` de la piste canari (ID 165700329) pointait
   déjà vers un fichier de test probe, pas vers le vrai fichier `D:/MUSIQUE
   2025/MP3/Weekender - Route 1 (Version).mp3`. Preuve qu'une **session M8
   antérieure avait déjà écrit dans le `master.db` live** et n'avait jamais
   été correctement restaurée — la vraie bibliothèque de l'utilisateur avait
   une piste mal reliée, silencieusement, jusqu'à ce que cette session le
   détecte et corrige.

**Correctif appliqué** : réparation chirurgicale d'une seule piste
(`FolderPath`/`FileNameL`/`FileNameS` remis sur le vrai chemin D:), avec
double backup de sécurité conservé sur le Bureau
(`rb-backup-2026-07-06-metadata-retest/`,
`rb-backup-2026-07-06-before-path-repair/`). Vérifié : `ArtistID` de nouveau
lié à "Weekender", piste confirmée normale par Antoine dans Rekordbox.

**Décision** : la règle déjà actée aux Évaluations 5/7/11 ("jamais le fichier
live, toujours une copie") est **réaffirmée** — le test de ce soir était une
exception explicite demandée par Antoine, pas un nouveau mode opératoire.
Deux garde-fous à appliquer à toute future validation en direct sur le vrai
`master.db` : (1) ne jamais prendre pour argent comptant le rapport "stable/
prêt" d'un agent d'arrière-plan sur un état de fichier partagé — relire
indépendamment avant d'autoriser une écriture live ; (2) ne jamais supposer
qu'un `cp` de backup pris juste avant une action garantit un point de
restauration propre — le vérifier contre une référence connue avant de
compter dessus.

---

## Veille concurrente — MediaMonkey (2026-06-24)

Gestionnaire de biblio musicale ([mediamonkey.com](https://www.mediamonkey.com/)),
voisin de Sift. Ce qu'il fait et comment :

| Brique | MediaMonkey | Technique |
|---|---|---|
| Auto-tag | Metadata + artwork manquants | Empreinte acoustique → lookup **MusicBrainz** (fingerprint envoyé au serveur) |
| Doublons | Détecte/supprime | **MD5** → seulement les fichiers **octet-identiques** |
| Auto-organize | Déplace/renomme | **DSL de masks** déclaratif, déclenché à l'ajout/édition |
| Conversion | Compat appareils | À la volée |
| Stockage | Biblio 100k+ | **SQLite** (comme Sift) |

**3 enseignements pour Sift :**

1. **Dédoublonnage = différenciateur.** MediaMonkey ne fait que du **MD5**
   (octet-identique) → rate « même morceau, bitrate/encodage différent », le cas DJ.
   Le plan Sift (**Chromaprint**) détecte ces quasi-doublons → avantage produit à
   mettre en avant. Confirme le choix M5.
2. **Voler le DSL de masks** pour le renommage/rangement (Discogs). Éprouvé,
   transposable — en prendre une **version réduite** (pas le couteau suisse) :
   - Tokens : `<Artist>`, `<Album>`, `<Title>`, `<Track#:2>` (zero-pad), `<Year>`,
     `<BPM>`, regroupement alpha `<Artist@3>`.
   - Fonctions : `$If(crit,oui,non)`, `$Replace(s,a,b)`, `$RemovePrefix("The")`,
     `$Left/$Right/$Mid`, `$Upper/$Lower`.
   - Ex. : `C:\Music\<Artist>\<Album>\<Track#:2> - <Title>`.
3. **MusicBrainz vs Discogs pour M5.** MediaMonkey identifie par **fingerprint →
   MusicBrainz** (lié à AcoustID). **Discogs n'a pas de service d'empreinte**
   (matching texte) mais excelle sur pressages/électronique/vinyle. → Archi
   possible : **empreinte → MusicBrainz pour *identifier*, Discogs pour
   *enrichir/renommer***.

---

## Veille UX — design d'interface (2026-06-24)

- **[Designing user-friendly interfaces — a practical guide for putting people first](https://medium.com/design-bootcamp/designing-user-friendly-interfaces-a-practical-guide-for-putting-people-first-272a51cee37a)**
  (Medium / Design Bootcamp). _Statut : référence — checklist UX à appliquer au
  frontend Sift._ Principes retenus, mappés aux **gaps repérés sur Sift** :
  1. **Éviter les boutons icon-only** : la nav et les lignes (play / lien Discogs /
     identifier) n'ont que des icônes → ajouter **labels + `title` + `aria-label`**.
  2. **Microcopy** : nos erreurs IPC sont des chaînes techniques (`NO_TOKEN`,
     `NoLibraryRoot`…) → **humaniser** (messages, états vides, confirmations).
  3. **Accessibilité** : contraste, **navigation clavier** au-delà des raccourcis
     existants (Space/Enter/X/I en Revue).
  4. **Cohérence** (design system vivant via les `--color-*`), **simplicité**
     (1 action primaire par écran), **but clair par écran**, **états de chargement**
     (skeleton / waveform instantanée déjà en place).
  > À traiter dans la passe design du M6b Lot 2 (détail unifié) puis Lot 5 (audit
  > de conformité). Voir spec `docs/superpowers/specs/2026-06-24-m6b-library-design.md`.

---

## Titlebar custom (3/3 briques faites — macOS non vérifié visuellement)

> Statut mis à jour 2026-07-03 : les 3 briques sont livrées. La brique 1
> (détection OS + variante macOS), notée "pas commencée" plus tôt le même
> jour, a été codée dans la foulée : `tauri-plugin-os` 2.3.2 enregistré
> (`Cargo.toml`, `lib.rs`, `package.json`), `frontend/chrome.ts`
> (`injectTitlebar`) devient `async`, lit `platform()` une fois et ajoute la
> classe `sift-tb-mac` (feux tricolores à gauche, tokens `--color-text-danger/
> warning/success`) au lieu du layout Windows (droite) par défaut — le
> fallback en cas d'échec de `platform()` reste ce même layout Windows.
> **Non vérifiable en réel : pas de Mac disponible**, écrit à l'aveugle sur
> la doc officielle. Les 2 gaps connexes notés le même jour sont aussi
> traités : tooltip natif + ellipsis CSS sur `#sift-tb-title` (texte
> statique "Sift" aujourd'hui, mais protégé si un jour dynamique), et le
> bouton "Agrandir" dont le title/aria-label bascule en "Restaurer" une fois
> la fenêtre maximisée (via `getCurrentWindow().isMaximized()` +
> `onResized`) — **l'icône, elle, reste volontairement `ti-square` fixe**
> dans les deux états (retour arrière suite à un retour utilisateur : le
> glyphe `ti-restore` dynamique testé d'abord ne plaisait pas).
> Design + plan : `docs/superpowers/specs/2026-07-03-titlebar-os-detection-design.md`,
> `docs/superpowers/plans/2026-07-03-titlebar-os-detection.md`.

Trois briques, toutes faites :

1. **Détecter l'OS** → `tauri-plugin-os`. Place les contrôles au bon endroit :
   feux tricolores à gauche sur macOS, minimize/maximize/close à droite sur
   Windows. _Statut : **fait** (`chrome.ts` `injectTitlebar`), macOS non
   vérifié visuellement (pas de Mac)._
2. **Fenêtre sans décoration** → `decorations: false` dans `tauri.conf.json`
   + recréer la barre en HTML/CSS. Pas de plugin, config + DOM.
   _Statut : **fait** (`tauri.conf.json:21`, `chrome.ts`)._
3. **Actions fenêtre** → `@tauri-apps/api/window` (`getCurrentWindow().minimize()`
   / `.toggleMaximize()` / `.close()`) + attribut `data-tauri-drag-region`
   sur la zone de déplacement. Pas de plugin.
   _Statut : **fait** (`chrome.ts`)._

- **[agmmnn/tauri-controls](https://github.com/agmmnn/tauri-controls)** —
  contrôles de fenêtre d'apparence native pour Tauri 2 (boutons dessinés selon
  les prototypes de design officiels de chaque OS, PAS des contrôles natifs).
  ⚠️ Livré en React/Solid/Vue/Svelte+Tailwind — **pas de variante vanilla TS**.
  _Statut : **référence-only**._ Usage prévu : copier leur rendu CSS/SVG
  par-OS pour le pixel-perfect, réimplémenter en vanilla TS. Jamais en dépendance.
- **[agmmnn/tauri-ui](https://github.com/agmmnn/tauri-ui)** — **écarté**
  2026-06-30. Scaffolder shadcn/ui (React), sert à démarrer un projet de zéro,
  pas à enrichir l'existant ; hors scope vanilla TS ; maintenance douteuse
  (issue #21 upgrade Tauri 2 sans réponse). Rien à en tirer pour Sift.

---

## Design system Sift — audit tokens hauteur/radius/typo (2026-07-03)

**Contexte** : `/design-system audit` sur `frontend/styles.css` (603 lignes),
en continuation de l'audit d'états par composant du 2026-07-03
(`docs/design-system-states.md`).

**Findings** : (1) les 4 tokens `--h-32/36/40/44` avaient **zéro lecteur**
(`grep -rn "var(--h-"` sur tout `frontend/` : aucun match) — scale déclarée
jamais consommée. (2) `--border-radius-md`(6)/`-lg`(10) ne couvraient que 2
des 4 valeurs annoncées par `.interface-design/system.md` (sharp 4/pill 999
manquants), avec des littéraux 999px/4px répétés partout. (3) `--text-hero`
(26px) prétendait à un rôle de titre de morceau "30/600" qui n'existe plus
dans le layout actuel — son seul vrai usage était une icône de repli
(`library-detail.ts:57`), pas un titre.

**Décision** : correctif chirurgical plutôt que refonte de l'échelle —
(1) `--h-32`/`--h-44` supprimés (0 usage, pas de plan), `--h-36`/`--h-40`
gardés et câblés sur leurs 2 vrais consommateurs (`.sift-play-btn`,
`.jrnl-insp-revert`) ; (2) `--border-radius-sm`(4)/`-pill`(999) ajoutés et
câblés sur les 10 sites où le littéral correspondait exactement — le reste
des valeurs hors échelle (7px, 8px, 9px, 11px, 12px…) **laissé tel quel**,
étendre l'échelle pour les couvrir serait une décision de design, pas un
câblage de token existant ; (3) `--text-hero` renommé `--text-2xl` (valeur
inchangée), `.interface-design/system.md` confirmé stale sur la typo en plus
de la palette (déjà noté périmé côté couleurs le 2026-07-01).

**Détail complet** (par composant, historique des passes) :
`docs/design-system-states.md`. Note CLAUDE.md sur `system.md` étendue à la
typo en conséquence. Commits `b3569f8` (fixes) + `272fff0` (doc).

---

## Infra / Release — décisions en attente

- **tauri-plugin-updater** (2026-06-30) : pas encore intégré. Nécessite des décisions d'infra d'abord (clé de signature, hébergement du manifeste : GitHub Releases vs CrabNebula, config `tauri.conf.json`, signature au build). Reporté à la phase release — infra avant code.
- **tauri-specta** (IPC type-safe, 2026-06-30) : évalué et reporté post-RC. Mieux sur le papier, mais conversion invasive (~45 commandes), dépendance RC dans une couche critique, perte de la doc métier des wrappers manuels actuels. Le double-miroir `ipc.ts` + `shared/contracts.ts` reste la solution tant que le risque de migration dépasse le gain.

---

## Outillage Claude Code — purge plugins/skills cross-projet (2026-07-03)

Audit demandé par Antoine sur son setup global (pas spécifique à Sift, mais
loggé ici car Sift est le repo où la décision a été prise et où
`docs/skills-registre.md` sert de référence pour le routage skills). Portée :
22 plugins `claude plugin` + ~150 skills en session, 4 projets actifs (Sift,
Tuple, Tupline, tuple-controller).

**Constat clé** : les skills en session viennent de **deux sources
indépendantes** — les plugins `claude plugin`, et un installateur séparé par
lockfile (`~/.agents/.skill-lock.json`, outil `npx skills`) qui gère la
quasi-totalité des skills business/marketing/design-taste. `claude plugin
disable` n'a aucun effet sur cette deuxième source.

**Désinstallés** (`claude plugin uninstall`) : `appwrite`, `brightdata-plugin`,
`outputai`, `qdrant-skills` (déjà rejeté par écrit ci-dessus, voir section
Écarté), `coderabbit`, `frontend-design@claude-plugins-official` (collision de
nom avec le skill `frontend-design` réel utilisé par Sift/`design-flow` —
désinstaller l'officiel lève l'ambiguïté notée sans réponse dans
`docs/skills-registre.md:109`).

**Désactivé** : `code-modernization` (zéro usage sur les 4 projets, gardé au
cas où).

**Supprimés du lockfile skills** (`npx skills remove -g`, 28 skills) : tout le
pack business/stratégie/marketing/vente installé via `wondelai/skills`
(`blue-ocean-strategy`, `lean-startup`, `jobs-to-be-done`, `traction-eos`,
`storybrand-messaging`, etc.) — confirmé inutilisé sur les 4 projets, supprimé
malgré le risque signalé que Tuple en veuille pour sa page marketing
(`site/index.html`) ; réinstallable à la carte via `npx skills add
wondelai/skills -s <nom> -g -y` si le besoin se présente. Gardés : les
clusters code-craftsmanship, systems-architecture, ux-design du même bundle
(référencés par Sift/Tuple).

**Cassé, pas touché** : `maxmcp@signalcompose` (`Marketplace signalcompose not
found`) — pertinent pour Tuple/Max for Live selon son registre, mais aucune
URL de marketplace connue pour corriger sans deviner.

**Décision propagée** : `ecc` était off sur Sift depuis 2026-07-01 mais encore
listé comme actif dans les `CLAUDE.md` de Tuple et Tupline — corrigé le
2026-07-03 dans les deux fichiers.

Détail complet, table des doublons par job, et commandes `npx skills` :
`~/.claude/skills-registre-global.md` (hors repo, cross-projet).

**Répercuté dans `docs/skills-registre.md` le 2026-07-04** (commit `13ed62c`) :
le registre Sift, figé au 2026-06-30, a été mis en cohérence avec cette purge
et les décisions CLAUDE.md (ecc off, MCP stitch supprimé, system.md périmé
palette+typo, collision `frontend-design` résolue), et porte désormais une
règle d'entretien "dans le même geste" + une date de dernière mise à jour.

---

## Dette technique — audit Project Cleaner (2026-07-09)

Audit de nettoyage généraliste (fichiers morts, deps inutilisées, code mort,
config incohérente) mené par 3 agents lecture-seule en parallèle (frontend/CSS,
backend Rust/deps, config/docs), suivi de la correction des findings vérifiés
soi-même plutôt que renvoyés à Antoine — commits `26dd519`, `ff12268`, `aa939e1`.

**Findings corrigés :**
- **10 tokens CSS orphelins** (`--h-36`, `--overlay-bar`, `--color-cta-text`,
  6 variantes `--color-hue-*-border`/`-bg`) retirés de `styles.css` (3 blocs :
  clair + 2 sombres). `--h-36` en particulier était documenté comme câblé dans
  `docs/design-system-states.md` mais avait perdu son consommateur
  (`.sift-play-btn`) lors de la refonte Revue du 2026-07-06 sans que la doc
  soit mise à jour — corrigé dans le même geste.
- **Désynchronisation de version** : `tauri.conf.json` était resté à `0.1.0`
  (valeur par défaut du template Tauri) depuis le scaffolding M0, jamais aligné
  sur `package.json`/`Cargo.toml` (`0.0.1`, changés ensemble au même commit
  `60979fd`). Confirmé par `git log` avant correction — pas une régression
  récente, un oubli initial jamais rattrapé.
- **2 références mortes** à `.interface-design/penpot-detail-spec.md`
  (`report-view.ts`, `styles.css`) — fichier supprimé depuis le 2026-07-01
  (ère Penpot abandonnée), jamais retiré des commentaires qui le citaient.
- **`docs/INDEX.json`** en retard de 4 entrées (2 specs/plans du 2026-07-04
  token-sync-fixes, `2026-07-06-apple-system-colors-palette.md`,
  `2026-07-09-m8-tier3-metadata-sync-ipc-ui.md`) — catalogué.

**Findings vérifiés puis explicitement gardés (pas des faux positifs, mais
pas des vrais problèmes non plus)** :
- `frontend/ipc.ts::rescanSource` — signalé mort par l'agent (0 appelant
  frontend), mais la commande Rust `rescan_source` existe réellement
  (`ipc.rs:158`, enregistrée `lib.rs:114`) : fonctionnalité backend du design
  M1 pas encore câblée à un bouton UI, pas du code mort à supprimer.
- `components.json` (racine) — config légitime du MCP `shadcn` (pointe vers
  `frontend/styles.css`), pas un artefact de scaffolding oublié.
- `docs/annotations.jsonl` tracké en git — voulu : le fichier est conçu pour
  être vidé au fur et à mesure (entrées retirées après traitement par Claude),
  pas un log qui grossit indéfiniment.

**Reste hors scope (nécessite une décision produit, pas juste du nettoyage)** :
cluster de 16 `#[allow(dead_code)]` dans `rekordbox_masterdb.rs`
(sans justification inline par item, contrairement à `usb_format/macos.rs`) ;
candidats à visibilité réduite (`naming.rs::has_junk/is_clean`,
`analysis/verdict.rs::min_cutoff_hz_for_bitrate`,
`metadata/discogs.rs::parse_search` — appelés seulement depuis leur propre
fichier) ; échantillon de classes CSS potentiellement mortes (`.mf`,
`.mid-scroll`, `.jrnl-qrow-time`, `.jrnl-qempty`, `.jrnl-qcount`,
`.sift-src-dot-pink` — seulement 30/439 sélecteurs testés, taux de mortalité
extrapolé ~15-20%, une passe CSS-usage complète serait nécessaire avant
suppression).

**Note méthode** : `cargo test`/`clippy` volontairement pas relancés dans
cette passe — aucun fichier `.rs` n'a été touché (uniquement CSS/JSON), et
`tauri dev` tournait activement en session concurrente au moment du
nettoyage (voir [[concurrent-session-same-directory]]).

## Dette technique — double décryptage master.db par piste rangée, M8 Tier 1+3 (2026-07-09)

Repéré en revue (pas un audit dédié) : la boucle post-commit de `commit_file`
(`filing.rs`) appelait deux détecteurs indépendants quand un XML Rekordbox
est lié — `actions::maybe_detect_masterdb_repair` (Tier 1) et
`actions::detect_masterdb_metadata_sync_if_linked` (Tier 3) — chacun
rappelant lui-même `rekordbox_masterdb::read_rekordbox_masterdb`, qui
déchiffre l'intégralité de `master.db` (SQLCipher, ~20 Mo sur une vraie
bibliothèque). Sur le chemin verrouillé DB, ça faisait 2 déchiffrements
complets par piste rangée (donc jusqu'à 2N sur un batch de N pistes) —
exactement la classe "reparse per action" déjà signalée dans l'audit perf du
2026-07-05 (`docs/superpowers/reviews/2026-07-05-audit-performance.md`), ici
sur un chemin d'écriture ajouté après cet audit (Tier 3, 2026-07-09).

**Fix** : `actions::resolve_masterdb_index_if_linked` lit/déchiffre
`master.db` une seule fois ; `detect_masterdb_repair_if_linked` et
`detect_masterdb_metadata_sync_if_linked` restent des wrappers à lecture
unique (inchangés pour leurs autres appelants — `apply_tags`,
`update_metadata`, `record_with_meta` — qui n'appellent jamais les deux
détecteurs pour la même action), plus des variantes `_with_index` prenant
l'index déjà chargé. `filing.rs::commit_file` charge l'index une fois avant
sa boucle post-commit et le passe aux deux détecteurs au lieu de les laisser
relire le fichier chacun de leur côté.

Vérifié : `cargo test` 318/318 (0 régression), `cargo clippy --all-targets
-- -D warnings` clean — les deux lancés contre un `CARGO_TARGET_DIR` isolé
(scratch), une session concurrente ayant `tauri dev` actif au moment du fix
(voir [[avoid-concurrent-cargo-tauri-dev]]).

## Dette technique — nettoyage clippy `m7-rekordbox-xml` (2026-07-04)

`cargo clippy --all-targets -- -D warnings` échouait sur 2 erreurs pré-existantes,
sans rapport avec le travail M7 en cours sur cette branche. Corrigées en 2 commits
dédiés (`34cf912`, `83fad93`) :

- **`settings.rs` — `TRASH_PURGE_DAYS` mort** (`dead_code`) : constante jamais lue
  nulle part. Déjà signalée comme dette connue dans les reviews M4
  (`docs/superpowers/reviews/2026-06-12-m4-review.md`, `2026-06-13-full-audit.md`) —
  la purge de `.sift-trash` (M4b) s'est faite « sur demande » plutôt que sur une
  fenêtre de rétention configurable ; le champ n'a jamais eu de consommateur.
  Constante supprimée, pas de fallback ni de feature à câbler derrière (aucun plan
  de purge programmée à court terme).
- **`dedup.rs:134` — `needless_range_loop`** dans `scan_library_duplicates` : boucle
  interne `for j in (i+1)..n { ... fps[j] ... }` réécrite en
  `for (j, fj) in fps.iter().enumerate().skip(i+1)`, en gardant l'index `j` pour les
  appels `union`/`find_root` (pas juste un `#[allow]` — l'index restait nécessaire).

Vérifié : clippy clean, `cargo test` 193/196 verts (3 échecs pré-existants sans
rapport : 2 tests `analysis::decode` sur fixture manquante, 1 test `rekordbox_xml`
sur du travail M7 en cours non lié à ce nettoyage).

## Dette technique — chip verdict `.sift-vchips` jamais rendu (2026-07-10)

Repéré en revue de code post-audit (angle cross-file tracer) : `filing.ts`
insère les chips "DUPLICATE" et "LECTURE INCOMPLÈTE" dans
`mid.querySelector(".sift-vchips")`, mais cette classe n'existe nulle part
dans le markup réellement rendu — `verdictCardHtml()` (`report-view.ts:543`)
retourne `""` en l'état actuel du code (committé, pas un WIP de session
concurrente). Les deux chips étaient donc du code mort silencieux, pas
seulement le nouveau ajouté cette session.

**Fix appliqué** (commit `d8a056e`) : les deux `querySelector` retargetés
vers `.sift-fil-verdict`, le conteneur que `filing.ts` crée et contrôle
lui-même (`openFilingInto`), qui existe dans le DOM indépendamment de ce que
`verdictCardHtml()` produit — donc résilient à l'état actuel (stub vide) et
à un futur remplissage de cette fonction.

**Reste hors scope** : `verdictCardHtml()` elle-même est vide depuis un
moment (pas daté précisément, pas de commit isolé trouvé pour l'instant) —
si un futur chantier remplit cette fonction avec un vrai contenu de carte
verdict, vérifier qu'elle ne recrée pas SA PROPRE zone de chips concurrente
à `.sift-fil-verdict` (ce qui dupliquerait ou masquerait les chips déjà
insérés par `filing.ts`).

---

## Écarté

- **[vykee.co](https://vykee.co)** — écarté le 2026-06-24. SDK SaaS d'**onboarding
  produit cloud** (tours guidés, checklists, divulgation progressive servis depuis
  un service tiers). **Incompatible avec l'ADN de Sift** : gratuit, offline-first,
  100 % local, un seul binaire — pas de dépendance réseau ni de tiers pour l'UI.
  **Garder l'idée** de **divulgation progressive** (progressive disclosure : ne
  montrer que ce qui est pertinent au moment T, déplier le reste à la demande) mais
  l'**implémenter nativement** dans le frontend (comme le repli spectrogramme /
  « N autres résultats » déjà en place), jamais via un service externe.
- **SoundTouch.js (key-lock M3)** — écarté le 2026-06-24. Le key-lock actuel utilise
  le time-stretch natif du navigateur (`preservesPitch` via l'élément `<audio>` de
  WaveSurfer v7), suffisant pour le nudge DJ ±8 %. WaveSurfer v7 n'a plus de backend
  WebAudio lecture → SoundTouch imposerait de ré-architecturer toute la lecture
  (play/pause/seek/curseur) en Web Audio pour un gain marginal à ce ratio. À
  reconsidérer seulement si on veut du stretch « pro » à gros ratios.
- **Qdrant / vector DB** (et l'annuaire `qdrant.tech/documentation/frameworks/`) —
  écarté le 2026-06-24. Les tâches moteur de Sift (détection spectrale, empreinte
  Chromaprint comparée en Hamming, metadata Discogs) ne sont pas des problèmes de
  similarité vectorielle. ANN inutile à l'échelle d'une biblio perso ; serveur
  requis → casse l'esprit offline/léger. Si un jour « trouve-moi des morceaux qui
  *sonnent* pareil » (embeddings audio), préférer `sqlite-vec` (in-process) à
  Qdrant.
- **Graphify (graphe de connaissance du codebase)** — écarté le 2026-07-01,
  après éval réelle (extraction + clustering sur tout le repo,
  `graphify-out/GRAPH_REPORT.md` : 1174 nodes, 2695 edges, 50 communautés).
  Les communautés (noms générés) recoupaient correctement la structure connue
  (Filing Rail UI, Discogs Track Matching…) mais la section « Surprising
  Connections » — censée apporter la valeur *nouvelle* — contenait des faux
  liens cross-langage (ex. `add_source()` en Rust lié à tort à `state` en TS)
  par collision de nom d'identifiant lors du linking, pas par un artefact de
  construction du graphe (`graphify diagnose multigraph --directed` : 0 edge
  collapsé). Le mode gratuit (cluster-only) ne fait que du matching structurel ;
  le mode sémantique (`--backend claude-cli`, gratuit via la souscription Claude
  Code) n'a pas pu être testé jusqu'au bout — bloqué par le classifieur auto-mode
  (action jugée trop risquée : lecture + envoi de tout le corpus vers un
  sous-process externe sans autorisation explicite assez précise). Overhead
  d'entretien (`.graphifyignore`, hooks git, rebuild à chaque changement de
  structure) pas justifié pour un repo de la taille de Sift tant que
  l'attribution sémantique n'est pas fiable.

---

## Évaluation 15 — spectrogramme trop clair : deux bugs empilés, pas un réglage de couleurs (2026-07-06)

**Contexte** : suite d'une série d'annotations Alt+Clic sur l'écran Revue.
Retour répété d'Antoine sur « Preuve (spectre) » : « trop bright », « pas
assez contrasté », en insistant que ce n'était probablement pas les couleurs
choisies. Investigation poussée via CDP (mesure directe de pixels de canvas,
pas juste jugement visuel sur capture) plutôt que d'itérer à l'aveugle sur la
palette.

**Fausses pistes essayées et invalidées par la mesure** (pas par supposition) :
- Courbe gamma sur la valeur brute (`Math.pow(norm, 1.6)`) — aucun effet
  mesurable, la donnée d'entrée était déjà saturée.
- Étirement auto-contraste par percentile (5e/99,5e du fichier courant) —
  même résultat : le 99,5e percentile valait déjà 255.
- Recalage sur le modèle Gain/Range réel d'Audacity (20 dB / 80 dB, cf.
  manual.audacityteam.org/man/spectrogram_view.html) — a en fait *aggravé*
  le taux de blanc (65,6 % → 89,2 %), preuve que le mapping de couleur
  n'était pas le problème : un modèle plus permissif sur le seuil « fort »
  ne fait qu'empirer un problème qui vient d'ailleurs.

**Root cause réelle, confirmée par histogramme exact de pixels (pas un
échantillon)** : `spectrum.rs`'s `process_frame()` convertissait la magnitude
FFT brute (`Complex::norm_sqr()`, non normalisée, qui grandit avec `fft_size`)
directement en dB — un signal plein niveau donnait +50 à +100 dB avant le
`.clamp(-100.0, 0.0)`, donc quasi tout contenu réel se retrouvait collé au
plafond d'affichage (octet 255). Mesuré : 65,6 % des cellules temps-fréquence
du fichier étaient *identiques à l'octet près*. Aucune courbe de colormap ne
peut créer du contraste entre des valeurs déjà numériquement égales.

**Second bug qui a caché le premier** : le rapport d'analyse (spectrogramme
inclus) est caché en SQLite (`tracks.report_json`), gardé par
`analysis::REPORT_CACHE_VERSION`. Son propre commentaire dit exister
précisément pour les changements « contenu modifié, forme JSON identique » —
exactement le cas du fix ci-dessus. Non bumpée par le premier commit de fix
(`072b070`) → rebuild + restart du process n'ont rien changé, le cache
resservait les anciens octets. Root-causé en lisant `ipc.rs`'s logique de
cache (pas en devinant depuis les timestamps process/binaire, qui semblaient
tous les deux plausibles). Bump 2→3 (`a5f2e73`) → rendu correct immédiat
(0,4 % blanc au lieu de 89,2 %), texture/dynamique comparable à une vraie
capture iZotope RX de référence.

**Décision** : colormap Sift reconstruite sur le modèle Audacity (Gain 20 dB
/ Range 80 dB, stops noir→bleu→magenta→orange→blanc) — conservée telle
quelle, c'était déjà le bon calibrage, il manquait juste des données
correctes à afficher. Repère de coupure (ligne pointillée + étiquette sur
fond semi-opaque) recoloré selon le verdict (succès/danger/warning) au lieu
d'un rouge alarme fixe.

**Méthode retenue pour la suite** : avant de retoucher un colormap/réglage
visuel qui « ne marche pas », mesurer la distribution réelle des valeurs
brutes (histogramme exact, pas un échantillon strié — un stride naïf peut
aliaser) avant de changer la courbe une 3e fois. Voir
[[sift-cdp-webview2-verification]] et [[sift-spectrum-dbfs-normalization-fix]].

---

## Évaluation 16 — délégation Claude→Codex CLI, premier test réel + coût en tokens (2026-07-06)

**Contexte** : `mcp__codex__codex` (Codex MCP, `codex mcp-server`) est connecté
depuis la session précédente. Cette session en a fait le premier vrai test de
délégation (pas juste une vérification de connexion), sur une tâche réelle et
scopée : fragilité documentée dans
`design_handoff_sift_refonte/token-sync/generate-design-md.cjs` (Évaluation 8,
le "+2" de compensation pour la section `## Composants`).

**Panne d'infra trouvée et réparée (Windows)** : le paquet CLI standalone
(`~/.codex/packages/standalone/current/bin/`, symlinké depuis
`AppData\Local\Programs\OpenAI\Codex\bin\`) ne contient QUE `codex.exe` —
aucun binaire de sandboxing. Toute commande sous `-s workspace-write`
échouait immédiatement (`orchestrator_helper_launch_failed`). Le binaire
manquant (`codex-windows-sandbox-setup.exe`) existe dans l'appli desktop
séparée (`WindowsApps\OpenAI.Codex_*\app\resources\`) — copié dans le dossier
du CLI standalone. Un 2e échec est apparu ensuite
(`CreateProcessWithLogonW failed: 2`, un compte de logon restreint manquant) ;
réparé via deux shims (`sandboxcli.cmd`/`sandbox-cli.cmd` → `codex.exe
sandbox %*`) qui déclenchent le mécanisme de jeton restreint (`codex sandbox`,
"Windows restricted token sandbox") au lieu de `CreateProcessWithLogonW`.
Après ces deux fixes, `codex exec -s workspace-write` fonctionne normalement.

**Visibilité — `mcp__codex__codex` natif est inutilisable en confiance** :
l'appel bloque jusqu'à la fin sans aucun événement intermédiaire. Sur un
premier essai, Antoine a interrompu l'appel après un silence perçu comme
anormalement long, faute de moyen de distinguer "ça travaille" de "ça a
planté". **Pattern retenu** : `codex exec - -s workspace-write -C <repo>
--json < mission.txt > run.jsonl` lancé en arrière-plan (Bash
`run_in_background`), avec relecture périodique du JSONL (`thread.started` →
`turn.started` → `item.*` → `turn.completed`) pour donner un vrai statut
pendant l'exécution. Coût : setup plus lourd (fichier prompt temporaire,
process background, polling) — comparable à faire la tâche soi-même pour un
scope petit, rentable seulement si l'implémentation réelle (pas sa
description) est ce qui coûte cher.

**Test A/B sur la vraie mission (3 runs, même prompt de base, mesure du
`usage.input_tokens` de `turn.completed`)** :

| Run | Config | Input tokens |
|---|---|---|
| 2 | Aucun réglage | 530 854 |
| 3 | Profil `~/.codex/claude-delegation.config.toml` (8 plugins Codex désactivés : superpowers/ecc/impeccable/ui-ux-pro-max/feature-dev/code-review/claude-md-management/skill-creator) | 861 392 (pire) |
| 4 | Profil + note explicite dans le prompt de mission ("ne consulte pas docs/skills-registre.md ni aucun SKILL.md, le routage est déjà fait") | **432 719** (−18 % vs run 2) |

**Root cause du surcoût, confirmée par grep du log JSONL** : ce n'est **pas**
le système de plugins propre à Codex (qui existe, pointe vers les mêmes
marketplaces que Claude — `claude-plugins-official`/`ecc`/`impeccable`/
`ui-ux-pro-max-skill`, dans `~/.codex/config.toml`) — désactiver ces plugins
seul (run 3) n'a rien amélioré. La vraie cause : **`AGENTS.md`** (l'équivalent
Codex de `CLAUDE.md`, lu nativement par le CLI comme instructions de repo)
contient la même règle "RÈGLE IMPÉRATIVE — routage skills" que `CLAUDE.md` —
Codex l'a suivie fidèlement et est allé lire `docs/skills-registre.md` **et**
un `SKILL.md` externe (`~/.agents/skills/refactoring-patterns/SKILL.md`),
alors que la mission était déjà entièrement scopée par Claude en amont. Le
levier qui marche est donc une **ligne d'override explicite dans le prompt de
mission**, pas une config d'infra.

**Le patch livré était correct dans les 3 runs** (vérifié indépendamment via
`node generate-design-md.cjs` + `node verify-v3.cjs`, pas juste via le
rapport de Codex). Point notable : le run 2 a lui-même détecté un vrai bug
introduit par Claude dans cette même session (un token CSS
`--color-waveform-elapsed` ajouté sans variante sombre, cassant l'invariant
`styles-css.cjs`) et s'est arrêté sans le corriger, hors scope — bon signal
de discipline sur une mission bien cadrée.

**Décision** : profil `~/.codex/claude-delegation.config.toml` gardé (ne nuit
pas, désactive du bruit de plugins), mais le vrai geste à reproduire pour
toute future délégation Codex est la **ligne d'override anti-routage dans le
prompt de mission**, pas la config. Scope de délégation recommandé : tâches
où l'implémentation elle-même (pas sa description) coûterait cher en contexte
Claude si faite directement — refactor multi-fichiers, chasse de build error
itérative — pas les fix triviaux (1-2 lignes) ni tout ce qui a besoin de
vérification UI live (`inTauri`/CDP reste le terrain de Claude). Pas encore
testé dans un `Workflow` multi-agents : les scripts Workflow n'ont aucun accès
filesystem/process, donc le pattern CLI+log (seule source de vraie
visibilité) n'y est pas utilisable — seul `mcp__codex__codex` opaque le
serait, ce qui pèse moins dans ce contexte déjà async, mais le coût par
branche parallèle reste réel et non testé à l'échelle.

**Incident concurrent noté pendant ce test** : une session Claude Code
distincte, ouverte sur le **même dossier de travail** (pas un autre
worktree), a commité `77877ce` (spec "Agent Token Budget Operating Model",
`docs/superpowers/specs/2026-07-06-agent-token-budget-operating-model-design.md`)
pendant que cette session validait une approche plus minimale en parallèle,
sans coordination. Gardés séparés sur demande d'Antoine — pas de fusion. Voir
[[concurrent-session-same-directory]].

---

## Évaluation 17 — corruption d'encodage silencieuse (mojibake) sur 2 fichiers frontend (2026-07-07)

**Contexte** : pendant une session de polish UI Revue (nav, queue, boutons),
un audit de routine (`git diff` avant commit) a fait remonter des séquences
`â€"`/`â€™` dans les commentaires de `frontend/styles.css` — signature classique
d'un double encodage UTF-8→Windows-1252→UTF-8. Investigation menée avant de
continuer tout autre changement, pas après.

**Constat, confirmé par comparaison d'octets bruts** (`git show HEAD:fichier
| xxd` vs le fichier réel) : le tiret cadratin correctement encodé en UTF-8
(`E2 80 94`) avait été mal décodé en Windows-1252 (donnant les 3 caractères
`â`/`€`/`"`), puis ce résultat erroné réencodé en UTF-8 pour la sauvegarde —
une double-passe d'encodage classique. `frontend/styles.css` (140 occurrences,
corruption dès la ligne 1 du fichier) et `frontend/sift-live.ts` (149
occurrences) étaient touchés ; les deux avaient aussi gagné un BOM UTF-8
(`EF BB BF`) absent de la version `HEAD`. Aucun autre fichier modifié cette
session n'était affecté (vérifié par grep ciblé sur tous les fichiers
`git status` de la session). Origine exacte non identifiée avec certitude —
plausible qu'un outil/process ait rouvert et resauvé le fichier entier avec
une mauvaise détection d'encodage (Windows a plusieurs outils qui sauvent par
défaut dans l'encodage système au lieu d'UTF-8) — mais la cause n'était pas
le point important : le fichier est réparable indépendamment de la cause.

**Fix, mécanique et vérifié sans perte** : script Node jetable — décoder le
fichier actuel en UTF-8 (donne la chaîne mojibake), ré-encoder chaque
caractère en Windows-1252 (table manuelle pour le bloc `0x80`-`0x9F`, qui
diffère de Latin-1/ISO-8859-1 dans cette plage — c'est justement ce qui
distingue une vraie corruption Windows-1252 d'une simple Latin-1), puis
redécoder ces octets en UTF-8 pour récupérer le texte d'origine. Zéro
caractère non représentable sur les deux fichiers (confirmé par échantillon
avant/après : "Source de vérité" se relit correctement, accents et tirets
cadratins restaurés). `npx tsc --noEmit` reste clean après le fix — la
correction est purement au niveau des octets de commentaires/chaînes, aucune
logique touchée.

**Décision** : pas d'outil de détection automatique ajouté (pas de hook, pas
de CI check) — l'incident a été détecté par une relecture `git diff` de
routine avant commit, qui suffit à l'attraper tant que cette habitude est
maintenue. Si l'incident se reproduit sur d'autres fichiers, envisager un
grep `â€` ciblé dans la checklist de fin de session plutôt qu'un outillage
dédié — la réparation elle-même (script jetable, ~40 lignes) est assez simple
pour ne pas justifier d'investissement en amont.

---

## Évaluation 18 — test M8 Tier 1 contre une copie réelle : bug WAL trouvé et corrigé (2026-07-08)

**Contexte** : M8 Tier 1 (réparation de chemin `master.db`) était complet
côté code depuis le 2026-07-07 mais son seul test était le fixture
synthétique (`tests/fixtures/rekordbox_master.db`, généré par
`scripts/make-rekordbox-fixture.py`) — le statut notait explicitement "test
contre une copie d'un vrai `master.db` avant tout usage réel" comme
condition bloquante restante. Cette session a fait exactement ce test.

**Méthode** : copie de `~/Desktop/sift-masterdb-write-probe/master.db.copy`
(20 Mo, déjà validée lisible par les spikes Python précédents — Évaluations
5/7/11) vers un nouveau dossier jetable
`~/Desktop/sift-m8-tier1-rust-verify/pioneer/`, jamais le fichier live. Test
d'intégration Rust `#[ignore]`d ajouté dans `rekordbox_masterdb.rs`
(`repair_track_path_round_trips_on_real_masterdb_copy`, activé via
`SIFT_M8_REAL_COPY_DIR`) : lit la baseline réelle, répare une piste vers un
chemin de test, vérifie round-trip (compte de pistes inchangé, XML
inchangé), puis restaure le chemin d'origine et revérifie. Exécuté avec
`CARGO_TARGET_DIR` pointé vers un dossier scratch isolé — une autre session
avait `tauri dev` actif sur le même repo, et lancer `cargo test` sur le
`target/` partagé aurait risqué de corrompre son cache incrémental (règle
déjà connue, voir mémoire `avoid-concurrent-cargo-tauri-dev`).

**Premier run : échec** — `read_rekordbox_masterdb` sur la copie réelle
levait `Sqlite("unable to open database file")`, alors que les 19 tests sur
fixture passaient tous. Root-causé par un test diagnostic jetable (3 chemins
comparés : désérialisation seule, écriture sur fichier réel, désérialisation
avec en-tête patché) plutôt que deviner : `sqlite3_deserialize` (via
`rusqlite::deserialize_read_exact`) **rapportait un succès**, mais la
**première requête** sur la connexion échouait — écart clé pointant vers un
problème d'ouverture du schéma, pas de déchiffrement (le HMAC par page avait
déjà validé chaque page, donc les octets étaient corrects).

**Root cause confirmée** : les octets 18/19 de l'en-tête SQLite standard
("file format write/read version") valent **2/2 (WAL)** sur un vrai
`master.db` Rekordbox — et restent à 2/2 **même après fermeture propre de
Rekordbox** (vérifié : `master.db-wal`/`-shm` disparaissent complètement du
dossier `Pioneer/rekordbox/` une fois l'app fermée, mais l'en-tête du
fichier principal n'est jamais réécrit en mode rollback). La VFS mémoire
utilisée par `sqlite3_deserialize` ("memdb") n'a aucun fichier réel associé
où chercher un `-wal` correspondant — un en-tête qui annonce le mode WAL la
fait échouer avec `SQLITE_CANTOPEN` dès la première requête, même si le
buffer désérialisé est par ailleurs valide. Le fixture synthétique n'avait
jamais révélé ce bug car généré fraîchement en mode rollback (1/1) par
défaut — c'est précisément le genre d'écart entre "shape connu" et "vraie
géométrie" que ce test était censé attraper.

**Fix** : `decrypt_masterdb` force désormais les octets 18/19 à 1/1 lors de
la reconstruction de la page 1, avec le même schéma que le fix existant de
l'octet 20 (reserve) — commentaire inline explique pourquoi c'est sûr en
lecture (le WAL réel, s'il existait, a déjà été checkpointé dans les pages
avant que Sift ne voie le fichier, garanti par `is_rekordbox_running` avant
toute écriture) et en écriture (`encrypt_masterdb` réencode depuis une
connexion jamais mise en mode WAL, donc le fichier réécrit est légitimement
en mode rollback — Rekordbox est libre de repasser en WAL à sa prochaine
écriture).

**Vérification post-fix** : les 19 tests fixture repassent au vert (aucune
régression — le fixture était déjà en 1/1, donc no-op pour lui). Le test
real-copy passe en entier : backup → repair → vérif round-trip → XML
inchangé → restore → revérif, sur la vraie bibliothèque (2828 pistes,
`track_id=123420449` réparé puis restauré). Suite complète : 271 tests + `cargo
clippy --all-targets -- -D warnings` clean.

**Décision** : Tier 1 est maintenant **vérifié contre des données réelles**,
plus seulement contre un fixture — le statut M8 (`docs/plan-implementation.md`)
mis à jour en conséquence. Le test real-copy reste `#[ignore]`d (nécessite
`SIFT_M8_REAL_COPY_DIR` + Rekordbox fermé, non exécutable en CI) mais reste
dans le repo comme porte de non-régression manuelle pour ce type de bug —
une régression future sur ce chemin ne serait détectée qu'en le relançant
explicitement, pas automatiquement.

**Leçon méthode** : un fixture synthétique "shape correcte" (mêmes colonnes,
mêmes types) peut quand même diverger de la vraie donnée sur des détails
d'en-tête/format qui n'ont rien à voir avec le schéma applicatif (ici, le
mode journal SQLite) — le geste "tester contre une copie réelle avant tout
usage réel", déjà une règle établie sur ce projet (Évaluations 5/7/11/14),
vient encore une fois de payer immédiatement, cette fois côté moteur Rust
plutôt que côté Python.

**Suivi même jour — schéma réel de `djmdSongPlaylist` et doublon
pré-existant trouvé** : en cadrant le plan M8 Tier 2 (dédup playlist) plus
tard dans la même session, un test diagnostic Rust jetable (jamais commité,
depuis retiré) exécuté contre cette même copie réelle a extrait le vrai
schéma de `djmdSongPlaylist`/`djmdPlaylist` — plus riche que les colonnes
simplifiées du fixture, avec en plus `usn`, `rb_local_usn`,
`rb_data_status`, `rb_local_deleted`, `rb_local_synced`, `created_at`,
`updated_at`. Sa sortie incluait la ligne `duplicate (PlaylistID,ContentID)
groups in real data = 1` : la copie réelle contient un vrai doublon
pré-existant (même piste ajoutée deux fois à la même playlist), confirmant
que la fonctionnalité de dédup Tier 2 répond à un scénario réel et pas
seulement hypothétique. Ce doublon a depuis été dédupliqué par le test de
vérification Tier 2 sur copie réelle lui-même (voir le plan Tier 2, Task 4)
— il n'est donc plus présent dans cette copie précise.

---

## Évaluation 19 — spike stack UI (React/Vue/Avalonia/Flutter) écarté, racine du problème identifiée : pas de référence canonique (2026-07-08)

**Contexte** : après 2 semaines à construire l'outillage de design system
vanilla (token-sync v1→v3), Antoine a exprimé une frustration de fond en
regardant des catalogues de composants externes (uithing.com/goodies,
v-wave) : la boucle "je me souviens d'un effet vu ailleurs → je te le
décris en mots → tu l'implémentes avec des allers-retours → une incohérence
apparaît sur un autre écran, découverte par audit après coup" (exemple réel
cité : segmented control réimplémenté 4 fois avant unification, 2026-07-08)
lui a fait envisager sérieusement une migration de stack complète — jusqu'à
remettre en cause le choix Tauri lui-même (Wails, Avalonia, Flutter).

**Méthode** : deux spikes de recherche web (agents en arrière-plan,
sources citées, candidat 1 scindé en 1a/1b sur demande explicite) comparant
4 candidats de remplacement du frontend/stack : (1a) Tauri+React+shadcn/
Radix, (1b) Tauri+Vue+Nuxt UI/shadcn-vue, (2) Avalonia (.NET/XAML), (3)
Flutter+fluent_ui. Critère n°1 : préserver le moteur Rust existant (mois de
travail M0-M8, centaines de tests) — écrit comme contrainte dure, pas
négociable.

**Résultat du spike, verdict net** : **aucun des 4 candidats ne résout le
problème réellement posé**. Dans les 4 cas, Antoine ne touche jamais le code
lui-même — la boucle "décrire en mots → un agent traduit → incohérence
découverte après coup" persiste indépendamment du framework, elle change
juste de langue (JSX+hooks, SFC Vue, XAML/C#, ou Dart). Détail par
candidat :
- **Avalonia** : écarté — pont vers le moteur Rust existant seulement en FFI
  manuel (`csbindgen`/`uniffi-bindgen-cs` tiers non officiel), aucun outil
  ne transpose les `enum`/`struct` serde riches ou les `Result<T,E>`
  idiomatiques de Sift, aucune preuve d'un projet réel de cette taille
  (~45 commandes IPC) ayant fait ce pont. Risque jugé trop élevé sur un
  backend déjà validé.
- **Flutter+fluent_ui** : pont Rust le plus mûr des 4 (`flutter_rust_bridge`,
  "Flutter Favorite", 200+ PR) mais `fluent_ui` n'implémente que le design
  Windows/WinUI3 — macOS nécessiterait `macos_ui` séparé (mainteneur
  unique, risque de continuité), introduisant une nouvelle divergence
  cross-plateforme que Sift n'a pas aujourd'hui. Abandon du bundling Tauri
  déjà fonctionnel (installeur ~25 Mo vs ~8 Mo, RAM idle ~275 Mo vs ~90 Mo,
  chiffres marketing à prendre avec prudence).
- **1a (React+shadcn)** vs **1b (Vue+Nuxt UI)** : les deux gagnent sur le
  critère backend (IPC Tauri inchangé, zéro risque, zéro changement côté
  Rust). Entre les deux, 1b a une courbe d'apprentissage mesurée plus douce
  pour un non-dev (2-4 semaines vs 4-8, sources comparatives citées dans le
  rapport complet) et un paradigme (SFC Vue) plus proche du modèle mental
  déjà en place dans le vanilla TS actuel (template + manipulation DOM).
  Mais Nuxt UI n'a **pas** de composant segmented-control natif (issue
  GitHub `nuxt/ui#6281` toujours ouverte au moment du spike) — ironique vu
  que c'est exactement le composant que Sift vient d'unifier à la main.

**La vraie racine du problème, identifiée en creusant plus loin (pas dans
le spike stack)** : ce n'est pas un problème de framework, c'est l'absence
d'une **référence canonique** que je (Claude) suis censé consulter avant
d'inventer un style UI. Vérifié concrètement sur un cas réel : la scrollbar
de Sift (`frontend/styles.css:97-107`, introduite commit `a3d4ed9`,
13/06) n'a aucune source citée dans son commit ni ailleurs — elle a été
produite "de mémoire d'entraînement" (un pattern statistique issu du
pré-entraînement sur d'innombrables CSS/design systems vus, sans
traçabilité vers une source précise), jamais vérifiée contre une vraie
référence. C'est le même mécanisme qui a produit les divergences audit-
après-coup (segmented control, `.lk`/`.lk-icon`) : rien dans le processus
actuel ne force une recherche active avant de générer du CSS, sauf quand
Antoine fournit lui-même un lien/référence précise (comme pour v-wave,
2026-07-08, où la traduction a été fidèle car lue directement, pas
reconstruite de mémoire).

**Décision** : pas de migration de stack. Adoption d'un **pool de
références canoniques gratuites**, consultées via WebFetch avant tout
nouvel élément UI sans exemple donné par Antoine — jamais installées comme
dépendance (aucune n'entre dans `package.json`/`Cargo.toml`), juste lues et
portées à la main en vanilla TS/CSS avec les tokens déjà en place :

| Source | Cible native | Statut | Force pour Sift |
|---|---|---|---|
| [ui.shadcn.com](https://ui.shadcn.com/) | React | Gratuit, officiel, open source | Doc la plus complète et la mieux structurée par composant (props/variants/états), 64 composants + Blocks + Charts |
| [coss.com/ui](https://coss.com/ui) | React (Base UI) | Gratuit, open source | 492 composants ("particles") |
| [uithing.com/components](https://uithing.com/components) | Vue | Gratuit, open source | 96 composants, dont Scroll Area et Sidebar/Command absents ou faibles côté shadcn/Nuxt UI |
| [21st.dev](https://21st.dev/) | React | Gratuit à consulter | Catalogue communautaire très large, plusieurs variantes par composant |
| [Apple HIG](https://developer.apple.com/design/human-interface-guidelines) | — (guidelines desktop) | Gratuit, officiel | Macro-décisions déjà en place dans Sift (grammaire de carte Boxes, couleurs système, matériaux) — shadcn/COSS/uithing ne couvrent pas ce niveau, ce sont des kits de composants web génériques, pas une doc desktop-native |

**Sites explicitement écartés du pool** (tiers non officiels, payants, ou
hors sujet, vérifiés un par un) : `shadcn.io` (non affilié au vrai shadcn,
Pro à partir de 19$/mois — sert surtout à automatiser l'installation CLI
dans un projet React, inutile ici) ; `shadcndesign.com` (kit Figma non
officiel, payant, pour un pipeline Claude Design qu'on a déjà abandonné,
Évaluation 3/4) ; les libs `shadcn_ui`/`shadcn_flutter` (ports Flutter
tiers de shadcn, hors sujet puisque Flutter est écarté) ; la skill Claude
`shadcn-ui` de crossaitools.com (câblée pour CLI React/npm, contre-
productive sur du vanilla — pousserait vers des réflexes qu'on n'a pas).

**Répartition des deux registres** (pourquoi shadcn/COSS/uithing ET Apple
HIG, pas un choix unique) : shadcn et ses équivalents répondent aux
questions de **micro-composant** (comment un toggle/popover/scrollbar se
comporte exactement à l'état hover/focus/disabled) — un niveau que HIG ne
traite pas. HIG répond aux questions de **macro-décision desktop**
(vibrancy/matériaux, élévation, couleur système) — un niveau que les kits
web génériques ne traitent pas du tout. Les deux registres ne se
recouvrent pas, donc pas de contradiction possible entre eux.

**Sur l'usage des valeurs shadcn elles-mêmes (pas juste la structure)** :
décision de ne PAS copier la palette/l'échelle par défaut de shadcn en
bloc — shadcn le dit lui-même sur sa page d'accueil ("Start here then make
it your own"), ses valeurs par défaut sont un point de départ générique
que même les projets React recolorent presque toujours. Sift a déjà sa
propre palette validée (11 tâches, Apple system colors, 2026-07-06) qui
répond mieux à l'identité produit qu'un défaut générique — la remplacer en
bloc referait un travail déjà audité, pour un gain nul. Ce qui a de la
valeur chez shadcn, c'est la **structure** (quelles props/variants un
composant a, quels états il gère), pas les hex codes. Exception : pour un
composant que Sift n'a encore jamais construit (aucune valeur tranchée
existante), partir du chiffre shadcn comme brouillon plutôt que d'improviser
de mémoire reste raisonnable.

**Règle de travail actée** (voir aussi `CLAUDE.md`, section Front) : avant
tout nouvel élément UI sans référence donnée par Antoine, consulter le pool
ci-dessus (WebFetch la doc/le code source réel) plutôt que générer du
CSS/comportement "de mémoire d'entraînement" sans traçabilité. Si Antoine
fournit lui-même un lien précis (comme v-wave), le lire directement plutôt
que deviner depuis sa description verbale — c'est la même règle appliquée
au cas où la référence vient de lui plutôt que d'une recherche.

---

## Évaluation 20 — M8 Tier 3 Test 1 (flag `TrackInfoUpdated`) retesté et infirmé (2026-07-08)

**Contexte** : Tier 3 (synchro metadata via un flag de reload, sans écrire
les tables normalisées `djmdArtist`/`Album`/`Genre` soi-même) était le
dernier point M8 réellement ouvert, bloqué depuis le 2026-07-06 par deux
tentatives de test confondues par le même problème — un titre partagé entre
deux pistes distinctes de la bibliothèque d'Antoine, menant à consulter la
mauvaise piste dans Rekordbox à chaque fois (voir `FINDINGS-m8-spike-3.md`
section 5). Cette session a retesté proprement, avec une leçon méthode
appliquée dès le départ : sélectionner un canary à **titre unique** dans
toute la bibliothèque (requête, pas recherche manuelle) pour éliminer
structurellement cette classe d'erreur avant même de commencer.

**Méthode** : copie fraîche de `master.db`+`masterPlaylists6.xml` (jamais
les fichiers live) dans `~/Desktop/sift-m8-tier3-spike/`. Requête sur la
copie : 1434 pistes à titre unique trouvées sur 2828 ; canary retenu —
`ID=99795585`, `"Street Battle"`. Simulation du scénario réel Sift
(déplace + ré-encode + re-tag) : copie du fichier audio réel (original
jamais touché, hash SHA256 identique avant/après vérifié), tag `Artist`
modifié sur la copie uniquement (`M8 TIER3 SPIKE TEST`), puis dans la copie
de `master.db` : `FolderPath`/`FileNameL`/`FileNameS` repointés + incrément
`TrackInfoUpdated` `'6'`→`'7'` via `pyrekordbox`+`commit()`. Round-trip
SQLite vérifié avant tout swap réel : `Analysed`/`AnalysisUpdated`/
`CueUpdated` confirmés inchangés.

**Incident de permission pendant la préparation** : le classifieur de
permissions Claude Code a bloqué la première tentative d'exécution du
script de mutation (sur la COPIE, jamais le fichier live), lisant la
réponse d'Antoine à une question de clarification précédente ("Je prépare
le spike, tu fais la vérif quand prêt") comme "l'utilisateur va tout faire
lui-même" plutôt que "l'agent prépare, l'utilisateur vérifie" — une
ambiguïté de lecture du "Je" dans le libellé français de l'option.
Redemandé explicitement à Antoine avant de continuer, confirmé, poursuivi
sans contournement du blocage.

**Vérification manuelle dans le vrai Rekordbox (Antoine)** : fermeture
confirmée → backup horodaté (`~/Desktop/rb-backup-2026-07-08-tier3-test/`)
→ swap → ouverture → observation de "Street Battle" (sans ambiguïté,
titre unique) → fermeture → restauration. Deux tentatives de restauration
ont échoué à cause d'un problème de collage dans le terminal PowerShell
d'Antoine (deux commandes concaténées sans saut de ligne produisant des
noms de paramètre malformés type `-ForceCopy-Item`) — corrigé en fournissant
un script `.ps1` à exécuter en une seule ligne plutôt que des commandes à
coller. **Chaque affirmation de restauration a été vérifiée indépendamment
par comparaison SHA256 live-vs-backup avant d'être acceptée** (jamais pris
la parole d'Antoine ou une sortie de commande non vue pour argent
comptant) — cohérent avec la règle déjà actée sur ce projet
(`verify-before-live-production-write`, voir mémoire).

**Résultat, sans ambiguïté possible** :
- **Emplacement** : affiche le nouveau chemin correctement — confirme une
  nouvelle fois Tier 1 (déjà acquis, revalidé en passant).
- **Grille** : intacte.
- **Artiste** : reste l'ancienne valeur (`Hecher & Ward`), **pas**
  `M8 TIER3 SPIKE TEST` — malgré le flag incrémenté et Rekordbox ouvert.
  **La stratégie primaire du design v2 (flag seul → reload automatique) est
  infirmée.**
- **Action manuelle « Relire le tag »** : fonctionne correctement, le tag
  se met à jour. Le mécanisme de reload existe et marche — il n'est
  simplement jamais déclenché automatiquement par ce flag seul.

**Décision/implication** : Tier 3 reste ouvert au sens produit (pas de
route automatique viable identifiée), mais la question technique est
tranchée — plus un blocage en attente d'un retest, un verdict négatif
définitif sur cette route précise. Deux chemins restants, aucun tranché
dans cette session : (1) fallback à haut risque déjà anticipé par le
design v2 (écriture directe des tables normalisées, find-or-create FK) —
redevient la seule route automatique restante ; (2) renoncer à
l'automatique, documenter le geste manuel « Relire le tag » pour
l'utilisateur — le plus simple et sûr des deux, pas un fallback silencieux
puisque proposé explicitement plutôt que caché.

Détail complet : `~/Desktop/sift-masterdb-write-probe/FINDINGS-m8-spike-5-tier3-test1.md`.
Design mis à jour : `docs/superpowers/specs/2026-07-06-m8-masterdb-write-path-rust-design-v2.md`.

---

## Évaluation 21 — M8 Tier 3 : diff exact du write pattern « Relire le tag » (2026-07-09)

**Contexte** : suite directe de l'Évaluation 20. Le spike n°5 avait confirmé
que « Relire le tag » fonctionne, mais le restore avait été fait avant de
capturer précisément ce que Rekordbox écrit dans `master.db` lors de cette
action — un trou explicitement noté comme limite du spike précédent, pas un
oubli. Antoine a posé la question directement : "c'est toujours buildable,
tu peux pas simplement trigger l'update toi-même ?" — réponse : Sift ne
peut pas invoquer l'action UI de Rekordbox depuis l'extérieur (pas d'API),
mais peut en principe répliquer directement ce qu'elle écrit en base — à
condition de savoir précisément quoi. Ce spike comble ce trou.

**Méthode** : même canary que l'Évaluation 20 (`ID=99795585`, "Street
Battle", titre unique). Round-trip complet avec **snapshot avant ET après**
l'action « Relire le tag » (contrairement au spike n°5) : copie fraîche du
vrai `master.db` → modification combinée chemin+tag+flag (identique au
spike n°5) → snapshot complet des lignes `djmdContent`/`djmdArtist` →
swap réel → Antoine ouvre Rekordbox, clic droit « Relire le tag » (Artiste
confirmé mis à jour par capture d'écran) → ferme Rekordbox → **copie en
lecture seule du `master.db` live avant tout restore** → nouveau snapshot →
diff colonne par colonne → restore, vérifié indépendamment par hash SHA256.

**Incident méthodologique pendant la préparation** : deux erreurs
PowerShell distinctes ont retardé le swap/restore, toutes deux dues à des
problèmes de terminal côté Antoine, pas à des commandes fausses — (1) deux
lignes collées ensemble sans saut de ligne produisant des noms de paramètre
malformés (`-ForceCopy-Item`), corrigé en fournissant des scripts `.ps1` à
exécuter en une seule ligne plutôt que des commandes à coller ; (2) `cd`
et `Resolve-Path` échouaient avec des erreurs — diagnostiqué comme le
terminal d'Antoine étant en réalité **cmd.exe** (thémé Starship pour
ressembler à un prompt PowerShell moderne), pas PowerShell — corrigé en
lui faisant taper `powershell` explicitement avant les commandes.

**Diff exact capturé** (voir `~/Desktop/sift-masterdb-write-probe/FINDINGS-m8-spike-6-tier3-reload-diff.md`
pour le détail colonne par colonne complet) :

- **`djmdArtist` : nouvelle ligne créée, pas de mise à jour en place.**
  Nouvel `ID` numérique + nouvel `UUID` généré + `rb_local_usn` propre
  bumpé, `Name` = la nouvelle valeur du tag. L'ancienne ligne
  `djmdArtist` reste en base, non supprimée (orpheline potentielle, non
  vérifiée si Rekordbox la nettoie plus tard).
- **`djmdContent`** : `ArtistID` repointé vers la nouvelle ligne, `Commnt`
  aussi mis à jour (Reload Tag relit plus que le seul Artiste — tous les
  champs ID3 pertinents), `TrackInfoUpdated`/`rb_local_usn`/`updated_at`
  bumpés.
- **Règle non négociable M8 vérifiée tenue** : `Analysed`/
  `AnalysisUpdated`/`CueUpdated` tous confirmés inchangés.

**Implication** : le mécanisme "find-or-create" que le design v2 craignait
sans le connaître précisément est maintenant documenté empiriquement — un
futur moteur Rust voulant répliquer ce comportement (fallback écriture
directe) a un vrai patron de référence, pas une supposition. **Mais une
question critique reste non résolue** : ce test n'a exercé que le chemin
"création" (nom d'artiste test inédit, `M8 TIER3 SPIKE TEST 2`) — reste à
vérifier si Rekordbox réutilise une ligne `djmdArtist` existante quand le
nom correspond déjà à un artiste connu de la bibliothèque (cas réel très
fréquent pour l'usage de Sift : retaguer avec le bon nom d'un artiste déjà
présent chez l'utilisateur), ou s'il duplique systématiquement. Sans cette
réponse, le fallback reste risqué même avec ce patron en main — un doublon
d'artiste créé à chaque retag serait un vrai défaut UX pour l'utilisateur.

**Décision** : ni le fallback écriture-directe ni l'option "renoncer à
l'automatique" ne sont tranchés par ce spike — il documente seulement plus
précisément le coût/risque du premier chemin, sans le rendre "prêt à
implémenter" (la question réutilisation-vs-duplication reste bloquante
pour ça). Décision produit toujours en attente.

Détail complet : `~/Desktop/sift-masterdb-write-probe/FINDINGS-m8-spike-6-tier3-reload-diff.md`.
Design mis à jour : `docs/superpowers/specs/2026-07-06-m8-masterdb-write-path-rust-design-v2.md`.

---

## Évaluation 22 — M8 Tier 3 spike 7 : reuse vs duplicate d'un artiste déjà connu (2026-07-09)

**Contexte** : suite directe du spike 6 (Évaluation 21). Question laissée
ouverte : quand le nom d'artiste retagué correspond à un artiste **déjà
connu** de la bibliothèque, Rekordbox réutilise-t-il la ligne `djmdArtist`
existante ou en crée-t-il une nouvelle (doublon) ?

**Méthode** : même canary que les spikes 5/6 (`ID=99795585`, "Street
Battle"), artiste cible "Eat Static" (`ID=1521864440`, 31 pistes déjà liées
dans la vraie bibliothèque — clairement établi, pas un accident). Copie
fraîche, tag `Artist` modifié sur une copie du fichier réel, swap réel
(backup horodaté, restore vérifié par hash SHA256), Antoine fait « Relire le
tag » dans Rekordbox, diff avant/après capturé par script Python
(`pyrekordbox`) sur une copie en lecture seule du `master.db` live avant tout
restore.

**Résultat, sans ambiguïté** : **REUSE confirmé** — `ArtistID` repointé vers
la ligne existante (1521864440), **aucune nouvelle ligne** `djmdArtist`
créée (compte de lignes inchangé, 1108→1108), la ligne "Eat Static"
elle-même strictement intacte (`updated_at`/`UUID` inchangés). Combiné au
spike 6 (chemin création, nom inédit), le mécanisme find-or-create de
Rekordbox est documenté sur ses deux branches.

**Décision** : le risque de duplication d'artiste pour un futur fallback
d'écriture directe est maîtrisable — un moteur qui fait une recherche par
nom AVANT de créer reproduit fidèlement ce que Rekordbox fait lui-même.

Détail : `~/Desktop/sift-masterdb-write-probe/FINDINGS-m8-spike-7-reuse-vs-duplicate.md`.

---

## Évaluation 23 — M8 Tier 3 : moteur Rust livré (find-or-create Artist/Genre/Label) + spike 8 pochette (2026-07-09)

**Contexte** : suite du spike 7 ci-dessus. Décision produit prise par
Antoine : automatiser Tier 3 plutôt que documenter le geste manuel
« Relire le tag ». Moteur Rust implémenté et vérifié le même jour.

**Livré** : `sync_track_metadata` (`src-tauri/src/rekordbox_masterdb.rs`) —
find-or-create sur `djmdArtist`/`djmdGenre`/`djmdLabel` (schéma vérifié
identique entre les 3 tables via `pyrekordbox.db6.tables`), écriture directe
de `Title`/`ReleaseYear` (colonnes sans FK, aucun risque de doublon). Même
chaîne de sûreté que Tier 1/2 (garde process Rekordbox, backup horodaté,
transaction, ré-encrypt, écriture atomique, vérification round-trip,
rollback automatique). Périmètre dérivé exactement de ce que
`tagging::write_tags_full` écrit — cover et Album explicitement exclus au
départ (aucun mécanisme connu pour la pochette à ce moment-là).

**Fixture étendue** (`scripts/make-rekordbox-fixture.py`) avec les 3
tables FK + colonnes `ArtistID`/`GenreID`/`LabelID`/`ReleaseYear` sur
`djmdContent`. 8 tests unitaires + 1 test `#[ignore]`d contre une copie
fraîche de la vraie bibliothèque (REUSE + CREATE sur le canary "Street
Battle", restauration finale vérifiée indépendamment par lecture directe,
`ArtistID` revenu à sa valeur d'origine). 290+ tests + clippy clean, aucune
régression sur Tier 1/2.

**Matching normalisé, fermé le jour même** : `find_or_create_named_row`
comparait d'abord en égalité stricte (risque documenté : "Eat Static" vs
"eat static" créerait un doublon cosmétique). Corrigé en `TRIM(Name) =
TRIM(?1) COLLATE NOCASE` — testé (variante casse/espaces réutilise la ligne
existante). Limite assumée : `COLLATE NOCASE` de SQLite ne fait que du
pliage ASCII, un nom accentué ("Étienne"/"étienne") ne matche pas par ce
mécanisme.

**Spike 8 — pochette, question posée par Antoine après la livraison** :
« vu qu'on importe des pochettes, est-ce que ça pose un problème pour la
synchro ? ». Investigation : `ImagePath` (`djmdContent`) pointe vers 3
fichiers JPG séparés (`artwork.jpg`/`_m.jpg`/`_s.jpg`) sous
`%APPDATA%\Pioneer\rekordbox\share\PIONEER\Artwork\<uuid>\` — **un cache
local par machine**, pas sur le disque de la bibliothèque. Test : nouvelle
pochette embarquée (JPEG magenta 500×500 généré via Pillow) sur une copie
du fichier réel du canary, `ImagePath` volontairement laissé intact dans la
copie de `master.db`, swap réel, Antoine fait « Relire le tag », diff par
hash SHA256 des 3 fichiers live avant/après (backup+restore vérifiés
indépendamment).

**Résultat** : **`ImagePath` inchangé, les 3 fichiers réécrits EN PLACE**
avec le contenu de la nouvelle pochette (confirmé par dimensions 500×500 +
couleur moyenne (254,0,220) quasi identique à l'injectée, écart = perte
JPEG). **Plus simple** que Artist/Genre/Label : pas de FK, pas de ligne DB,
juste un fichier à écraser au même chemin. Reste non testé : dimensions
exactes des variantes `_m`/`_s`, cas `ImagePath` NULL (piste sans pochette
préexistante).

**Décision** : synchro pochette non implémentée dans ce chantier (scope
initial), mais le blocage « aucun patron de référence » est levé — c'est
désormais un ajout au moteur Tier 3 plus simple que ce qui existe déjà, pas
un nouveau risque à défricher.

Détail : `~/Desktop/sift-masterdb-write-probe/FINDINGS-m8-spike-8-artwork.md`.
Design mis à jour : `docs/superpowers/specs/2026-07-06-m8-masterdb-write-path-rust-design-v2.md`.
Plan : `docs/superpowers/plans/2026-07-09-m8-tier3-metadata-sync-rust.md`.
