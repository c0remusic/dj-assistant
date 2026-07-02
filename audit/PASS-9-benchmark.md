# PASS 9 — Benchmark concurrentiel & opportunités

> Audit seul, aucune modification de code. Méthode : chaque feature attribuée à
> un concurrent cite une source (URL) trouvée via recherche web récente
> (juillet 2026). Sans source fiable = "non trouvé / à vérifier manuellement",
> jamais une affirmation de mémoire d'entraînement. Contexte Sift documenté par
> renvoi aux rapports sources (`PASS-0`, `PASS-4`, `PASS-6`,
> `docs/ressources-externes.md`), pas réinventé.

---

## Contexte Sift (rappel, sourcé sur les rapports d'audit déjà produits)

- **Architecture** : Tauri v2, Symphonia (décodage analyse) + FFmpeg sidecar
  (encodage CDJ), SQLite, `rustfft`, `lofty`, `rusty-chromaprint`, Discogs via
  `ureq`. 8639 lignes Rust / 6154 lignes TS. Source : `audit/PASS-0-carte.md`.
- **Détection de faux lossless** : pipeline `analysis/spectrum.rs` (FFT 4096,
  LTAS pleine résolution) + verdict pédagogique (débit MP3 estimé, légende,
  spectrogramme annoté cutoff) — point fort UX confirmé. Source :
  `audit/PASS-6-produit.md` (Constat 2).
- **Dédoublonnage** : `rusty-chromaprint` + comparaison Hamming locale
  (`dedup.rs`), pas de MD5 seul. Source : `docs/ressources-externes.md`
  (Évaluation 2 + veille MediaMonkey).
- **Performance conversion/analyse** : FFmpeg sidecar pour l'encodage
  (streaming, pas de fichier entier en mémoire), Symphonia pour le décodage
  d'analyse (choix hybride motivé par un bench réel Symphonia vs FFmpeg,
  ~200-700ms pour un fichier de 5-8 min). Batch de filing actuellement
  **100% série** (un seul thread FFmpeg à la fois, gain de parallélisation non
  quantifié). Spectrogramme à la demande **ignore le cache** et redécode tout
  à chaque clic (b2, priorité High). Source : `audit/PASS-4-perfs.md`.
- **Export Rekordbox/USB** : confirmé être une **simulation frontend pure**
  (`sift-live.ts:300-327`, `startExportSim`), aucune commande Tauri
  correspondante, aucun fichier XML écrit, aucune clé USB touchée. Priorité
  Critical dans PASS-6 (Constat 3) — ne JAMAIS présenter cette feature comme
  existante dans les comparatifs ci-dessous.
- **Différenciateur CDJ** (32-bit float, header EXTENSIBLE, erreur Pioneer
  E-8305) : réel dans le moteur (`tags_cdj_ok`, `encode.rs`), mais noyé dans
  l'UI (badge secondaire dans la carte Identification, jamais nommé "CDJ" dans
  le vocabulaire de l'écran Revue). Source : `audit/PASS-6-produit.md`
  (Constat 1).
- **Renommage/rangement** : DSL de masks (`naming.rs`, 379 lignes),
  placeholders `{artist} {title} {version}`, défaut simple. Ambition affichée
  en doc de viser un DSL plus riche façon MediaMonkey (`<Track#:2>`, `$If`,
  `$Replace`) mais contenu réel non vérifié ligne à ligne. Source :
  `audit/PASS-6-produit.md` (Constat 4), `docs/ressources-externes.md`
  ("Veille concurrente — MediaMonkey").
- **Prix/modèle** : Sift = gratuit, offline-first, mono-binaire, pas de
  service tiers pour l'UI (position actée dans `docs/ressources-externes.md`,
  section "Écarté — vykee.co").

---

## Catégorie 1 — Préparation de bibliothèque DJ (Rekordbox, Engine DJ, Serato, Lexicon, VirtualDJ)

| Capacité | Sift (preuve, réf. PASS-N) | Concurrents (source) | Verdict |
|---|---|---|---|
| Détection de faux lossless intégrée | Oui, moteur natif FFT + verdict pédagogique (`analysis/spectrum.rs`, PASS-6 Constat 2) | Rekordbox : **non intégré nativement** — "does not have built-in fake lossless detection capabilities" ; nécessite un outil tiers ([Spectro](https://www.getspectro.app/), source citée dans [résultats WebSearch juillet 2026]). Serato/VirtualDJ/Engine DJ : rien trouvé de natif — tous dépendent d'outils tiers type [Music Library Doctor](https://musiclibrarydoctor.com/how-to-detect-fake-320-mp3.html). | **Sift unique** parmi les DJ software (les autres délèguent à un outil externe payant) |
| Dédoublonnage par empreinte acoustique (pas juste MD5/nom) | Oui, `rusty-chromaprint` + Hamming (PASS-0, `dedup.rs`) | Rekordbox : outil natif "Duplicate Search" **limité au matching metadata**, pas d'empreinte — [Pioneer DJ Community](https://community.pioneerdj.com/hc/en-us/community/posts/22978035992729-Help-Duplicate-Entries-in-Collection) confirme l'absence d'auto-eraser fiable ; empreinte disponible seulement via add-ons tiers (Music Library Doctor, Rekordbox Collection Tool, Lexicon) — [source](https://musiclibrarydoctor.com/). Serato : "Show Duplicate Items" natif basé sur bitrate/taille de fichier, pas d'empreinte — [source](https://wearecrossfader.co.uk/blog/serato-dj-delete-duplicate-missing-files/). Lexicon : dédoublonnage par empreinte confirmé natif — [lexicondj.com/manual/find-duplicates](https://www.lexicondj.com/manual/find-duplicates). Engine DJ : non trouvé / à vérifier manuellement (résultats ne mentionnent qu'un bug de sync avec doublons, pas de fonction dédiée). | **Parité avec Lexicon** (natif) ; **Sift fait mieux** que Rekordbox/Serato natifs (qui n'ont pas d'empreinte native) |
| Intégration Discogs pour enrichissement metadata | Oui, natif (`metadata/discogs.rs`, 525 lignes, search/master/release) | Lexicon DJ : confirmé — "searches through Beatport, Spotify **and Discogs** for track information and artwork" — [lexicondj.com/manual/find-tags-and-album-art](https://www.lexicondj.com/manual/find-tags-and-album-art). Rekordbox/Serato/Engine DJ/VirtualDJ : aucune mention Discogs trouvée dans les résultats — probablement absent nativement. | **Parité avec Lexicon** ; **Sift en avance** sur Rekordbox/Serato/Engine/VirtualDJ (si confirmé absent chez eux) |
| DSL de renommage/organisation de fichiers | Oui, `naming.rs` (masks `{artist} {title} {version}`), ambition MediaMonkey-like non vérifiée en détail | Lexicon : renommage/déplacement basé sur les tags via "Send to → Move files", pas de syntaxe DSL détaillée trouvée — [lexicondj.com/manual/moving-and-renaming-f-iles](https://www.lexicondj.com/manual/moving-and-renaming-f-iles). Rekordbox/Serato/Engine/VirtualDJ : rien trouvé de comparable en DSL de renommage — organisation via playlists/crates plutôt que masks de nommage fichier. | **Sift potentiellement en avance** si le DSL prévu est implémenté (à vérifier ligne à ligne dans `naming.rs`, cf. hypothèse non vérifiée PASS-6) |
| Export natif vers Rekordbox/USB CDJ | **NON — simulation frontend pure, confirmé PASS-6 Constat 3, aucun backend** | Rekordbox : natif par définition (USB Export intégré, plan gratuit "Core" le couvre — [rekordbox.com/en/plan](https://rekordbox.com/en/plan/)). Engine DJ, Serato, VirtualDJ : export USB natif à leur propre format. Lexicon : transfert natif vers Rekordbox **sans XML**, écriture directe dans `master.db` — [source Music Library Doctor comparatif](https://musiclibrarydoctor.com/folder-library-to-rekordbox.html) (Lexicon cité comme faisant ce transfert nativement). | **Sift en retard, gravement** — les 5 concurrents ont un export USB/Rekordbox réel, Sift n'en a aucun (juste une barre de progression trompeuse) |
| Prix / modèle | Gratuit, offline, mono-binaire, pas d'abonnement | Rekordbox : plan gratuit "Core" (bibliothèque + USB export) mais fonctions avancées à $9-30/mois — [rekordbox.com/en/plan](https://rekordbox.com/en/plan/). Serato : DJ Lite gratuit, Pro par abonnement ou licence perpétuelle $250-450 — [serato.com/dj/pro/pricing](https://serato.com/dj/pro/pricing). Lexicon : $9.99-19.99/mois ou $199-399 à vie — [lexicondj.com/pricing](https://www.lexicondj.com/pricing). | **Sift unique** — gratuit sans palier payant, alors que tous les concurrents ont un modèle payant pour les fonctions avancées |

---

## Catégorie 2 — Analyse qualité audio / détection de faux (Mixed In Key, Platinum Notes, fakin' the funk, Spek)

| Capacité | Sift (preuve, réf. PASS-N) | Concurrents (source) | Verdict |
|---|---|---|---|
| Détection de faux lossless par analyse spectrale | Oui, natif, intégré au flux Revue (pas un outil séparé), verdict + spectrogramme annoté (PASS-6 Constat 2) | "Mixed In Key Integrity" : **non trouvé / à vérifier manuellement** — les recherches ne confirment aucun produit "Integrity" chez Mixed In Key ; leur offre trouvée porte sur la détection de tonalité (key detection), pas la qualité audio. "Fakin' the Funk" : confirmé — "scans audio files for discrepancies between the encoded bitrate and the actual bitrate" — [DJ TechTools review](https://djtechtools.com/2018/01/30/review-fakin-funk-low-bitrate-detection-utility/), outil séparé et payant historiquement. Audio Fake Detector PRO (gratuit, alternative citée dans `docs/ressources-externes.md`) : v7.7 confirmée active en 2026 — [alessandrocomito.github.io](https://alessandrocomito.github.io/audiofakedetectorpro/). | **Sift unique** sur l'intégration native dans un flux de préparation DJ (les autres sont des outils autonomes séparés, pas intégrés à un gestionnaire de bibliothèque) |
| Intégré au flux de travail (pas un outil à part) | Oui — analyse automatique au scan, verdict visible directement en Revue | Tous les outils de cette catégorie (Fakin' the Funk, Audio Fake Detector PRO, Spek, Spectro, Fabl) sont des **utilitaires autonomes** : on exporte/pointe une bibliothèque vers l'outil, on obtient un rapport, puis on revient dans son DJ software pour agir. Aucun n'est un gestionnaire de bibliothèque complet avec rangement intégré. | **Sift unique** — pas de round-trip entre deux applications |
| Score de qualité chiffré compréhensible DJ | Oui — débit MP3 estimé en kbps depuis `cutoff_hz` (PASS-6 Constat 2) | Music Library Doctor : score 1-10 par FFT — "files that score low (typically 3–5 despite a 320 tag) are re-encodes" — [musiclibrarydoctor.com/how-to-detect-fake-320-mp3.html](https://musiclibrarydoctor.com/how-to-detect-fake-320-mp3.html). Audio Fake Detector PRO : vote majoritaire par segment agrégé en verdict fichier — [filecr.com](https://filecr.com/windows/audio-fake-detector-pro/). | **Parité** — les deux approches (kbps estimé vs score 1-10) sont des formes différentes de la même pédagogie |

---

## Catégorie 3 — Spectrogramme / inspection (Spek, Sonic Visualiser, iZotope RX)

| Capacité | Sift (preuve, réf. PASS-N) | Concurrents (source) | Verdict |
|---|---|---|---|
| Spectrogramme annoté avec ligne de cutoff | Oui, `drawSpectrogram` + légende explicite (PASS-6 Constat 2) | Spek : "reveals frequency cutoffs characteristic of MP3, AAC, OGG, and lossy-transcoded files" — [Spek GitHub / Spectro blog comparatif](https://www.getspectro.app/blog/spectro-vs-spek). Sonic Visualiser : outil d'inspection fichier par fichier, pas de verdict automatique — [source résultats WebSearch]. iZotope RX : capacités spécifiques non confirmées par la recherche — **non trouvé / à vérifier manuellement**. | **Sift fait mieux** sur l'intégration (verdict auto + annotation), **parité** sur la lisibilité brute du spectrogramme lui-même |
| Vitesse d'affichage / cache | Spectrogramme **jamais mis en cache**, redécode le fichier entier à chaque clic (PASS-4 b2, priorité High) | Spek/Sonic Visualiser : outils mono-fichier, chaque ouverture recharge le fichier — comportement comparable en pratique côté redécodage, mais leur usage est du "un fichier à la fois", pas un flux Revue répété sur des centaines de pistes. | **Sift en retard sur son propre usage prévu** (flux de masse) même si comparable aux outils mono-fichier pris isolément — le problème est le contexte d'usage (Revue, redite fréquente), pas l'algorithme |
| Analyse par lot sur toute une bibliothèque | Oui — pipeline de scan + worker sur toute source ajoutée (PASS-0) | Spek : fichier par fichier, pas de mode batch bibliothèque trouvé. Sonic Visualiser : idem, outil d'inspection ponctuelle. | **Sift fait mieux** — ces outils sont conçus pour l'inspection ponctuelle, pas le traitement de masse |

---

## Catégorie 4 — Conversion / batch (dBpoweramp, XLD, fre:ac)

| Capacité | Sift (preuve, réf. PASS-N) | Concurrents (source) | Verdict |
|---|---|---|---|
| Conversion batch multi-format | Oui, FFmpeg sidecar streaming, skip si déjà conforme (PASS-4 a1) | dBpoweramp : "Batch Converter facilitates converting large numbers of files... with 1 click" — [dbpoweramp.com/Help/dMC/FileSelector](https://www.dbpoweramp.com/Help/dMC/FileSelector). XLD : glisser un dossier entier sur l'icône pour batch-convert récursif — [zexwoo.blog XLD guide](https://zexwoo.blog/en/posts/tutorials/xld-ripping/). fre:ac : conversion batch confirmée, multi-format (MP3/FLAC/Vorbis/Opus/AAC/WAV/WMA) — [freac.org/manual/en/howto.html](https://www.freac.org/manual/en/howto.html). | **Parité** sur la capacité brute de conversion batch |
| Parallélisme du batch | **Non — 100% série, un seul thread FFmpeg actif** (PASS-4 a2, priorité Medium, gain non quantifié) | dBpoweramp : support 32-bit float confirmé dans le pipeline DSP — [forum.dbpoweramp.com bit-depth-dsp](https://forum.dbpoweramp.com/forum/dbpoweramp/music-converter/41474-bit-depth-dsp-32-bit-float) — mais **le parallélisme du batch n'est pas confirmé par la recherche** (non trouvé / à vérifier manuellement). fre:ac / XLD : non trouvé / à vérifier manuellement sur le parallélisme interne. | Indéterminé pour les 3 concurrents (pas de source fiable) — **ne pas conclure "Sift en retard" sans preuve**, juste noter le gain interne non exploité (PASS-4) |
| Rename pattern / DSL au moment de la conversion | Oui, `naming.rs` (masks), déclenché au rangement, pas seulement à l'export | fre:ac : pattern simple confirmé — `<artist> - <title>`, `<artist> - <album> - <track> - <title>` — [sourceforge.net fre:ac howto](https://sourceforge.net/p/bonkenc/discussion/85470/thread/f31c83ebb6/), pas de fonctions conditionnelles (`$If`, etc.) trouvées. dBpoweramp/XLD : capacités de rename à la conversion non détaillées dans les résultats — non trouvé / à vérifier manuellement pour la richesse du DSL. | **Sift potentiellement en avance** sur fre:ac (DSL prévu plus riche selon `docs/ressources-externes.md`, mais contenu réel non vérifié) ; indéterminé vs dBpoweramp/XLD |
| Compatibilité hardware CDJ ciblée (32-bit float refusé, header EXTENSIBLE, erreur Pioneer E-8305) | Oui, natif et spécifique DJ (`encode.rs`, `is_conformant`) | Aucun des 3 outils (dBpoweramp, XLD, fre:ac) n'est orienté DJ/CDJ — ce sont des convertisseurs audio généralistes (ripping CD, formats lossless). Aucune mention Pioneer/CDJ/E-8305 trouvée dans les résultats. | **Sift unique** — le ciblage hardware CDJ n'existe dans aucun convertisseur généraliste audité |

---

## a) Ce que les concurrents font mieux

1. **Export USB/Rekordbox natif et fonctionnel.** Rekordbox (plan gratuit
   "Core" inclus), Serato, Engine DJ, VirtualDJ ont tous un export natif vers
   leur médium de lecture. Lexicon DJ va plus loin : transfert natif vers
   Rekordbox **sans passer par XML**, écriture directe dans `master.db`
   ([musiclibrarydoctor.com](https://musiclibrarydoctor.com/folder-library-to-rekordbox.html),
   citant Lexicon comme référence de ce pattern). Pour un DJ, c'est l'étape
   finale indispensable — sans elle, toute la préparation en amont
   (dédoublonnage, tags, rangement) doit être ré-exportée manuellement ou via
   un outil tiers. Confirmé absent côté Sift (simulation, PASS-6 Constat 3).

2. **Dédoublonnage par empreinte acoustique déjà "grand public" chez un
   concurrent direct positionné pareil (Lexicon DJ).** Lexicon vise
   explicitement le même segment que Sift ("library management for
   professional DJs") et a l'empreinte acoustique en natif depuis longtemps —
   [lexicondj.com/manual/find-duplicates](https://www.lexicondj.com/manual/find-duplicates).
   Ce n'est donc pas un avantage exclusif à revendiquer sans nuance ; Sift est
   à parité, pas en avance, sur ce point précis face à Lexicon (bien qu'en
   avance sur Rekordbox/Serato natifs).

3. **Écosystème d'outils tiers spécialisés autour de Rekordbox déjà mature.**
   Music Library Doctor combine à la fois FFT quality scoring, dédoublonnage
   par empreinte ET écriture native dans la base Rekordbox
   ([musiclibrarydoctor.com](https://musiclibrarydoctor.com/)) — c'est
   quasiment le même triptyque de valeur que Sift revendique (faux lossless +
   dédoublonnage + rangement), mais scoping "add-on Rekordbox" plutôt
   qu'application autonome. Pour un DJ déjà investi dans Rekordbox, ce
   positionnement d'add-on est moins de friction qu'une appli séparée comme
   Sift qui demande de gérer sa bibliothèque en dehors du logiciel DJ
   principal.

## b) Ce que les concurrents ont en plus (absent de Sift, standard ailleurs)

1. **Export USB/CDJ natif** — absent de Sift (simulé), présent chez tous les
   DJ software audités et chez Lexicon en tant qu'outil de préparation.
   Source : PASS-6 Constat 3 (côté Sift) + sources ci-dessus (côté
   concurrents).

2. **Multi-source d'enrichissement metadata (Beatport, Spotify, en plus de
   Discogs)** — Lexicon combine plusieurs sources
   ([lexicondj.com/manual/find-tags-and-album-art](https://www.lexicondj.com/manual/find-tags-and-album-art)),
   Sift n'a que Discogs (`docs/ressources-externes.md` confirme le choix
   assumé Discogs pour l'électronique/vinyle, décision déjà motivée — pas un
   oubli).

3. **Import natif depuis d'autres logiciels DJ (Rekordbox, Serato, Traktor,
   Engine)** — Lexicon et Engine DJ importent les bases de leurs concurrents
   directement ([enginedj.com](https://enginedj.com/software/enginedj-desktop) —
   "supports third-party library imports from rekordbox, Apple Music/iTunes,
   Serato DJ, and TRAKTOR databases"). Sift n'a pas cette capacité de
   migration — un DJ qui a déjà une bibliothèque Rekordbox/Serato taguée doit
   repartir de ses fichiers bruts.

## c) Opportunités

### Opportunité 1 — Export Rekordbox XML fonctionnel (remplacer la simulation)

- **Description** : implémenter un export réel au format XML Rekordbox (via
  la crate `rbox`, déjà identifiée et évaluée comme "candidat n°1" dans
  `docs/ressources-externes.md`), a minima en lecture/écriture playlist +
  métadonnées de base, sans viser l'écriture directe `master.db` binaire
  (approche Lexicon/MLD, plus complexe et non documentée publiquement).
- **Problème utilisateur résolu** : élimine le Constat 3 critique de PASS-6 —
  un DJ qui clique "Rekordbox" aujourd'hui croit avoir exporté sa bibliothèque
  alors que rien ne s'est produit. C'est un risque de confiance cassée sur le
  terrain (club, sans bibliothèque exportée).
- **Alignée avec le différenciateur CDJ ?** Oui — c'est l'aboutissement direct
  du pipeline "analyse → range → convertit au format CDJ" : sans un export
  réel vers le device qui LIT le format CDJ, la conversion 32-bit float /
  EXTENSIBLE reste une préparation sans destination.
- **Effort** : Élevé. `rbox` n'est pas encore intégré (dépendance à ajouter,
  audit de compatibilité MSRV/Tauri à faire, cf. protocole d'audit dépendances
  du CLAUDE.md). Le README (`M7`) place déjà ce jalon "à venir, gelé" —
  cohérent avec un effort non trivial, pas un simple bugfix.
- **Bénéfice** : Différenciation forte + rétablit la confiance produit
  (retire un point Critical identifié). C'est la seule opportunité de cette
  liste qui ferme un vrai trou de fonctionnalité de base plutôt que d'ajouter
  un nice-to-have.
- **Risque de bloat** : Faible — ce n'est pas une feature "parce que les
  autres l'ont", c'est l'aboutissement logique du pipeline déjà construit
  (Sift promet déjà "convertit au format CDJ" en README) et un point déjà
  marqué Critical en interne (PASS-6), indépendamment de ce benchmark.
  Alternative moins coûteuse en attendant : désactiver/masquer l'entrée nav
  simulée (déjà proposé en PASS-6, effort Petit) — **à faire en premier, sans
  attendre l'implémentation réelle**, pure question d'honnêteté produit.

### Opportunité 2 — Remonter le différenciateur CDJ dans le vocabulaire de l'écran Revue

- **Description** : reprendre le Constat 1 de PASS-6 (déjà spécifié :
  remonter "Compatibilité CDJ" dans le verdict principal, nommer
  explicitement le CDJ dans le langage de l'écran le plus visité).
- **Problème utilisateur résolu** : aligne la perception utilisateur sur le
  vrai différenciateur produit — actuellement un DJ perçoit "détecteur de
  faux lossless générique", alors que la vraie proposition de valeur unique
  face à TOUS les concurrents audités dans ce benchmark (aucun n'a de
  ciblage CDJ spécifique, catégorie 4) est la compatibilité hardware.
- **Alignée avec le différenciateur CDJ ?** Oui, directement — c'est le sujet.
- **Effort** : Faible (confirmé PASS-6 : déplacement UI + reformulation d'un
  label, pas de nouveau code métier).
- **Bénéfice** : Fort rapport effort/bénéfice — ce benchmark confirme que
  cette différenciation CDJ n'existe nulle part ailleurs (catégorie 4,
  "Sift unique"), donc la rendre visible a un ROI élevé sans risque technique.
- **Risque de bloat** : Aucun — c'est une clarification de message produit
  existant, pas un ajout de fonction.

### Opportunité 3 — Import/migration depuis une bibliothèque Rekordbox/Serato existante

- **Description** : lire un export XML Rekordbox ou une base Serato existante
  pour importer les tags/metadata déjà en place (pas nécessairement le moteur
  d'analyse CDJ, juste les métadonnées) au moment d'ajouter une source.
- **Problème utilisateur résolu** : un DJ qui a déjà des années de tags
  Rekordbox/Serato n'a aucune incitation à essayer Sift s'il doit tout
  retagger depuis zéro — friction d'adoption identifiée chez Lexicon/Engine
  DJ comme un argument de vente explicite (import multi-source confirmé,
  section b).
- **Alignée avec le différenciateur CDJ ?** Non directement — c'est une
  feature de confort d'adoption, pas un renforcement du moteur CDJ/faux
  lossless.
- **Effort** : Élevé (parsing XML Rekordbox en lecture est plus simple que
  l'export en écriture, mais reste un nouveau module + mapping de champs vers
  le schéma SQLite `PASS-0`, avec des cas limites de bibliothèques
  volumineuses).
- **Bénéfice** : Réduit la friction d'adoption pour l'utilisateur cible le
  plus probable (DJ déjà équipé Rekordbox/Serato qui découvre Sift).
- **Risque de bloat** : Modéré — utile mais hors du "no list" du différenciateur
  CDJ ; à ne prioriser qu'après l'Opportunité 1 (export), qui est plus urgente
  et plus alignée. Un import sans export reste une demi-mesure (le DJ importe
  dans Sift mais ne peut toujours rien réexporter).

### Opportunité 4 — Second champ de résolution de la source Discogs (Beatport en complément)

- **Description** : ajouter Beatport comme deuxième source de lookup metadata
  quand Discogs ne trouve rien (fallback), à l'image de Lexicon qui combine
  Beatport/Spotify/Discogs.
- **Problème utilisateur résolu** : Discogs est fort sur vinyle/pressages mais
  plus faible sur les sorties électroniques très récentes ou digital-only —
  un DJ électronique moderne aurait un meilleur taux de match avec Beatport en
  complément.
- **Alignée avec le différenciateur CDJ ?** Non — enrichissement metadata
  généraliste, pas lié au moteur d'analyse hardware.
- **Effort** : Moyen (nouveau client API `ureq` sur le modèle de
  `metadata/discogs.rs`, mais logique de fallback/priorité entre 2 sources à
  concevoir, plus la gestion d'un 2e système de rate-limit/token).
- **Bénéfice** : Incrémental — améliore le taux de match mais Discogs a déjà
  été choisi consciemment pour sa force sur l'électronique/vinyle
  (`docs/ressources-externes.md`), donc le gain net est incertain sans données
  réelles de taux d'échec Discogs en usage.
- **Risque de bloat** : Élevé relativement au bénéfice incertain — ajoute une
  deuxième dépendance externe, un deuxième point de friction d'onboarding
  (deuxième token/clé API potentiel), pour un gain non mesuré. **Ne pas
  prioriser** avant d'avoir des données réelles sur le taux d'échec Discogs
  actuel (télémétrie locale, pas de service tiers).

### Opportunité 5 — Corriger le cache spectrogramme (PASS-4 b2) avant tout ajout de feature concurrentielle

- **Description** : ce n'est pas une feature concurrentielle nouvelle, mais
  ce benchmark renforce l'urgence de PASS-4 b2 (spectrogramme jamais caché,
  re-décodage complet à chaque clic) : la catégorie 3 (Spek/Sonic Visualiser)
  montre que Sift est déjà "Sift en retard sur son propre usage prévu" à
  cause de ce défaut, alors que c'est précisément l'usage répété en flux de
  masse (Revue) qui devrait être le point fort de Sift face à des outils
  mono-fichier comme Spek.
- **Problème utilisateur résolu** : latence perçue à l'ouverture du
  spectrogramme sur des pistes déjà analysées.
- **Alignée avec le différenciateur CDJ ?** Oui indirectement — le
  spectrogramme EST la preuve visuelle du différenciateur (détection de faux
  lossless), une latence dessus dilue la crédibilité de ce même
  différenciateur à l'usage.
- **Effort** : Faible pour l'option (a) de PASS-4 (étendre `report_json` pour
  inclure le spectrogramme, déjà borné à 204 800 octets max).
- **Bénéfice** : Élimine 100% des re-décodages pour toute piste déjà
  analysée — gain déjà prouvé par lecture de code (PASS-4), pas une
  hypothèse de ce benchmark.
- **Risque de bloat** : Aucun — c'est un fix de dette technique déjà identifié,
  ce benchmark ne fait que confirmer sa priorité relative face à la
  concurrence (Sift doit être meilleur qu'un outil mono-fichier sur l'usage
  répété, condition nécessaire pour justifier l'intégration dans un flux
  complet plutôt qu'un outil séparé comme Spek).

---

## Synthèse des verdicts par catégorie

| Catégorie | Verdicts dominants |
|---|---|
| 1. Préparation bibliothèque DJ | Sift unique (gratuit + faux lossless natif + CDJ), mais en retard grave sur l'export USB/Rekordbox (simulé, pas réel) |
| 2. Analyse qualité / détection de faux | Sift unique sur l'intégration native au flux (les concurrents sont des outils séparés) |
| 3. Spectrogramme / inspection | Parité sur l'algorithme, Sift en retard sur son propre cas d'usage de masse (cache manquant, déjà identifié PASS-4) |
| 4. Conversion / batch | Parité sur la capacité brute, Sift unique sur le ciblage hardware CDJ spécifique |

**Note méthodologique** : plusieurs points n'ont pas pu être confirmés par
recherche web fiable et sont marqués "non trouvé / à vérifier manuellement"
plutôt qu'affirmés : existence d'un produit "Mixed In Key Integrity" distinct
(semble ne pas exister sous ce nom — leur offre trouvée porte sur la
détection de tonalité), parallélisme interne des batchs dBpoweramp/XLD/fre:ac,
capacités précises d'iZotope RX pour la détection de cutoff, richesse exacte
du DSL de renommage dBpoweramp/XLD à la conversion, et fonctionnalités de
dédoublonnage natif d'Engine DJ.
