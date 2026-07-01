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

## Titlebar custom (chantier UI prévu, pas démarré)

> Décision 2026-06-30 (option « noter sans coder ») : remplacer la titlebar
> native par une barre custom est **prévu** mais **pas en chantier**. Aucune
> dépendance ajoutée pour l'instant (éviter le bloat d'une dep qui dort).
> À attaquer comme un vrai chantier UI — routage CLAUDE.md : `design-flow`
> (nouveau screen) ou `impeccable`.

Trois briques distinctes le jour où on s'y met (et `tauri-plugin-os` n'en
couvre qu'UNE) :

1. **Détecter l'OS** → `tauri-plugin-os` (officiel, suit la version majeure
   Tauri). Sert à placer les contrôles au bon endroit : feux tricolores à
   gauche sur macOS, minimize/maximize/close à droite sur Windows. C'est le
   SEUL rôle du plugin ici. _Statut : à ajouter au moment du chantier, pas avant._
2. **Fenêtre sans décoration** → `decorations: false` dans `tauri.conf.json`
   + recréer la barre en HTML/CSS. Pas de plugin, config + DOM.
3. **Actions fenêtre** → `@tauri-apps/api/window` (`getCurrentWindow().minimize()`
   / `.toggleMaximize()` / `.close()`) + attribut `data-tauri-drag-region`
   sur la zone de déplacement. Pas de plugin.

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
