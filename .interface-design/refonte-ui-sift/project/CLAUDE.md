# Sift — Refonte UI — état du projet

## Fichiers
- `Sift.dc.html` — maquette hi-fi interactive, à jour. Grammaire 3 colonnes fixes : Nav (152px) /
  Queue (272px) / Inspecteur (flexible). 6 écrans dans le nav, dans cet ordre : **Accueil, Revue,
  Écartés, Journal, Bibliothèque, Réglages** (labels français, jargon technique gardé en anglais :
  LOSSLESS, DUPLICATE, MATCH/CHECK MATCH, FAKE, kbps, kHz).
- `Sift — Wireframe.dc.html` — exploration basse-fidélité (mode canvas), 5 tours historiques
  (états Review, Batch, zone de progression, comparatif couleur, Accueil). Sert d'archive de
  raisonnement, plus la référence active — `Sift.dc.html` a dépassé ces explorations.
- Fichiers d'options ponctuels (mode canvas, jetables une fois la décision prise) :
  `Sift — Duplicate tag options`, `Sift — Time display options`, `Sift — Slider options`,
  `Sift — Empty state options`.

## Direction visuelle
Palette, typo, composants, tokens CSS (mode clair + sombre) → voir **`DESIGN.md`** (fichier
séparé, réutilisable tel quel pour un futur projet Sift). Ce fichier-ci (`CLAUDE.md`) ne garde que
le suivi de décisions et l'état des écrans.
- Mode sombre implémenté : toggle Auto/Clair/Sombre dans Réglages > Apparence, suit
  `prefers-color-scheme`, persiste en localStorage (`sift-theme`).

## Écrans (tous suivent la grammaire 3 colonnes)
- **Revue** : écran de référence. Fichier source fusionné sous le titre/artiste (plus de case à
  part). Barre de lecture : waveform pleine largeur avec le temps encadrant (début/fin), clic
  pour avancer, sliders custom en divs (pas de `<input type=range>` natif) — Volume (fill depuis
  la gauche, sans %) et Tempo (fill depuis le centre à 0%, avec repère visuel), Key-lock entre les
  deux. Preuves : chip DUPLICATE seulement s'il y a un vrai doublon (plus de chip "UNIQUE").
  Panneau Preuves déplié = grille de métriques réelles (Verdict, Durée, True-peak, Écrêtage,
  Silence début/fin, Tronqué, Fréquence d'échantillonnage, Coupure, Canaux, DC offset, Corrélation
  de phase, Conteneur OK, Pics) groundées dans les champs réels du moteur d'analyse (cutoff_hz,
  clip_pct, true_peak_dbtp, dc_offset, phase_correlation, silence_head/tail_ms, tags_cdj_ok,
  id3_version). Identification · Discogs : Label/Année/Genre (genre = tags/pills individuels, pas
  une string — prépare le tri auto par genre), Compatibilité CDJ, Version ID3 (Pochette retirée,
  redondante avec la pochette déjà visible). Bouton Modifier = petit bouton bordé (pas juste un
  lien) qui ouvre un popover de candidats Discogs alternatifs. Le badge MATCH/CHECK MATCH n'existe
  QUE quand il y a un doute réel (CHECK MATCH) — un MATCH propre n'affiche plus rien. Morceau non
  identifié (Discogs vide) : carte simplifiée avec bouton "Rechercher sur Discogs" → recherche
  (⟳) → liste de candidats cliquables qui appliquent label/année/genre.
- **Mode Lot (Batch)** : vraie vue dédiée (pas juste des checkboxes) — groupé par
  Prêts/À vérifier/En analyse, case à cocher par groupe et par piste. Cliquer "Déplacer la
  sélection" lance une VRAIE simulation de rangement par piste (wait→running→done/fail, ~15%
  d'échec simulé, ~450ms/piste) affichée en liste live dans le panneau, avec la tâche "Rangement"
  dans le rail (barre de progression + bouton Arrêter). Une fois terminé : les pistes réussies
  vont en Bibliothèque + une entrée Journal groupée (voir plus bas), les échouées reviennent en
  file.
- **Zone de progression** : cartes de tâches (Analyse/Identification/Rangement/Export) en bas de
  la nav (masquées si aucune tâche active), migrent dans le rail d'action pendant le Lot. Bouton
  Arrêter sur la tâche "file" en cours.
- **Destination** (popover ancré au bouton du rail bas) : arborescence réelle (carets, "+ nouveau"
  imbriqué, filtre → liste plate), légende du chemin disque réel au-dessus, checkbox "Sur place",
  et ligne "📁 Parcourir un autre dossier…" pour sortir de l'arborescence interne (`window.prompt`
  démo). Popover en `position:fixed` ancré aux coordonnées réelles du bouton (recalculées à
  l'ouverture + au resize via `getBoundingClientRect`) — PAS en `position:absolute`, qui se faisait
  clipper par l'`overflow:hidden` du conteneur racine au resize. Se ferme au clic extérieur
  (backdrop invisible) ou Échap.
- **Bibliothèque / Écartés** : "Mettre à jour les tags" (Bibliothèque) confirme par toast ; export
  Rekordbox/Clé USB lance une vraie tâche de progression simulée dans la zone (pas un placeholder).
- **Journal** : entrée groupée (filing par lot) affiche la liste des pistes filées en détail (pas
  juste "Filé N pistes"). Garde-fou : annuler (revert) une entrée de >10 pistes demande
  confirmation explicite avant d'exécuter.
- **Accueil** (5e item nav, en premier) : col2 = liste des sources surveillées avec statut
  (nouveau/à jour/inaccessible), col3 = détail de la source + toggle "Surveiller ce dossier" +
  bannière contextuelle (ex. viser `Completed` pas `Incomplete`). Gate "racine de bibliothèque non
  définie" si `rootPath` est vide (démontrable via Réglages > Bibliothèque > "Oublier le dossier
  racine").
- **Réglages** (6e item nav) : une seule page qui défile, séparée par carte (Discogs, Bibliothèque,
  Fichiers, Apparence) — PAS des onglets exclusifs. Le rail gauche (col2) liste catégories +
  sous-champs ; cliquer fait défiler jusqu'à la bonne carte (scroll-to, pas de switch de vue).
- **États vides** (file/bibliothèque/écartés à 0) : alignés en haut (pas centrés verticalement),
  titre + note explicative + lien "Aller à Revue →" pour Bibliothèque/Écartés (pas pour Revue, qui
  est déjà le point d'entrée). Le rail d'action se cache entièrement dans ce cas. Les compteurs nav
  + en-tête de colonne reflètent les VRAIES longueurs de listes (plus de faux gros nombres mock
  du genre "12480" déconnectés de l'état réel — ça contredisait l'état vide).

## Codebase réelle consultée (lecture seule, dossier local `dj-assistant-m6a`)
Vite vanilla TS + Tauri. Fichiers clés lus : PRODUCT.md, shared/contracts.ts, frontend/styles.css,
docs/brief-refonte-ui-2026-07-01.md, frontend/filing.ts, identify-shared.ts, chrome.ts,
report-view.ts, progress-zone.ts, home-sources.ts, batch-tracklist.ts, docs/superpowers/plans
(m2a-analysis-engine, m4b-ecartes), docs/plan-implementation.md.
Faits importants tirés du code (à ne pas réinventer) :
- Verdict réel : `ok|fake|grey` ; MATCH est qualitatif (`MATCH` vert / `CHECK MATCH` ambre selon
  confidence), PAS un pourcentage.
- KEY sur le transport = verrou de tonalité (key-lock), PAS un code Camelot. Tempo réel = slider
  entier -8%..+8%.
- Secondaire rail contextuel : fichier fake → "Ressourcer" (ambre) ; sinon "Écarter".
- Zone de progression : persistante en bas de la nav, migre dans le rail du Batch
  (`mountProgressZone`). Tâches : analyze/identify/file, états running/stopping/error/done, bouton
  Stop optionnel par tâche (implémenté pour "file" côté maquette).
- Rapport d'analyse réel (`AnalysisReport`) : cutoff_hz, clip_runs/clip_pct, true_peak_dbtp,
  dc_offset, phase_correlation, silence_head_ms/silence_tail_ms, truncated, container_ok,
  codec_error, id3_version, tags_cdj_ok, has_cover, spectrogram, peaks — tous repris dans le
  panneau Preuves de la maquette.
- Explorateur de destination = arborescence récursive réelle sous une racine bibliothèque choisie
  par l'utilisateur, avec filtre → liste plate, "+ nouveau" imbriqué, gate racine.

## Décisions verrouillées (ne pas revenir dessus sans redemander)
- Couleur : 2 couleurs sémantiques (vert/ambre) + gris neutre pour "en cours" — PAS de bleu/rouge,
  ça diluerait "couleur = sens uniquement".
- Accueil = 5e item nav, EN PREMIER (avant Revue). Réglages en dernier.
- Racine de bibliothèque : libellé neutre, pas d'adresse directe à l'utilisateur ("non définie" /
  "aucun dossier sélectionné"), pas "Collection"/jargon.

## Pont vers le code
Le transfert vers `dj-assistant-m6a` (styles.css + vues vanilla TS) reste un geste manuel/handoff
séparé une fois la direction validée ici — ce projet ne modifie jamais le repo réel (accès en
lecture seule via le dossier local monté).
