# Plan — Refonte UI Sift (`design_handoff_sift_refonte/`)

> Lecture seule effectuée : README.md, DESIGN.md, Sift.dc.html (structure), les 6
> screenshots + mode Lot, et le code réel (`styles.css`, `chrome.ts`,
> `report-view.ts`, `progress-zone.ts`, `batch-tracklist.ts`, `filing.ts`,
> `home-sources.ts`, `sift-live.ts`, `shared/contracts.ts`).

## Constat préalable — important

Ce handoff décrit **la même direction visuelle** qu'un chantier de refonte déjà
mené sur ce repo (21 tâches complétées, voir liste de tâches active). Vérifié
concrètement, pas supposé :

- Palette : `frontend/styles.css:13-22` a **déjà** exactement les valeurs de
  `DESIGN.md` (`#E7E2DB` canvas, `#EDE9E2` nav, `#EAE5DE` queue, `#F1EDE7`
  cartes, textes `#34302B/#5C554E/#8A857D/#B3AEA5`, bordures
  `rgba(40,34,28,…)`).
- Nav 152px fixe : `styles.css:90` (`.sb{width:152px}`).
- Mode sombre `prefers-color-scheme` + override `[data-theme]` + persistance
  `sift-theme` : `theme.ts`, `styles.css:43-59`.
- Sliders custom (pas de `<input type=range>`) : `report-view.ts:406-491`
  (`tempoFill`, `dragSlider`, fill centre pour Tempo).
- Popover Destination `position:fixed` + `getBoundingClientRect` : `filing.ts:797-823`.
- Garde-fou revert Journal >10 pistes : `journal.ts:258`.
- Champs `AnalysisReport` (Preuves) : tous présents dans `shared/contracts.ts:74-91`,
  rien d'inventé côté maquette à ajouter.

**Conclusion : ce n'est pas un chantier greenfield.** Le travail réel est un
**audit de fidélité gap-by-gap** entre ce nouveau handoff et l'état actuel du
code, pas une réimplémentation des 7 écrans. Le plan ci-dessous liste, écran
par écran, les écarts **réels** trouvés (pas de suppositions) et les points où
je n'ai pas assez d'éléments pour trancher seul.

---

## 1. Accueil

**Fichiers à modifier** : `frontend/home-sources.ts` (déjà la source du
layout col2/col3 décrit), `frontend/sift-live.ts` (gate racine, déjà présente
d'après tâche #14/#17).

**Tokens DESIGN.md concernés** : carte "Dossier surveillé" (`--card`,
`--border`), bannière contextuelle ambre (`--color-background-warning`/
`--color-text-warning`), état vide aligné en haut.

**Vérifié (2026-07-02)** : `home-sources.ts` n'utilise PAS `empty-state.ts`
pour la racine non définie — c'est un choix délibéré, pas un oubli. La racine
manquante n'implique pas une liste de sources vide (les sources surveillées
existent indépendamment du dossier racine bibliothèque), donc le pattern
"État vide" (0 item, écran de remplacement) ne s'applique pas structurellement
ici. Le code affiche à la place une **bannière d'avertissement persistante**
("Racine de bibliothèque non définie — ... le rangement sera bloqué",
`home-sources.ts:47`) au-dessus de la liste de sources normale — cohérent
avec le ton "avertissement contextuel", pas un cul-de-sac. Aucun changement
nécessaire ; divergence de forme avec le README assumée et justifiée par la
structure réelle des données, pas un écart à corriger.

**Ambiguïté** : aucune détectée sur cet écran.

---

## 2. Revue (écran de référence)

**Fichiers à modifier** : `frontend/report-view.ts`, `frontend/filing.ts`,
`frontend/identify-shared.ts`.

**Tokens DESIGN.md** : slider custom (piste 3px, poignée 13px), chip de statut
pilule (opacité fond 30-45%), tag genre discret (`--track`/`--text2`), bouton
bordé "Modifier" (jamais un lien pour ouvrir un popover).

**État constaté** : sliders, chips DUPLICATE conditionnelle, MATCH/CHECK MATCH
conditionnel, genre en tags séparés — tous déjà livrés (tâches #8/#9/#10).
Aucun écart trouvé à ce stade sur la Détail elle-même.

**Décision (2026-07-02, "comme sur la maquette")** : implémenté. La comparaison
pixel-à-pixel a trouvé un vrai écart structurel — le verdict (bandeau
"Prêt à ranger"/conclusion) passait AVANT la carte Identification, alors que
la maquette le place en DERNIER, juste au-dessus du rail d'action. Corrigé :
`verdictCardHtml` exporté depuis `report-view.ts`, retiré de l'assemblage
`reportHtml`/`openReportInto` (qui ne rend plus que Hero+Audition+Preuves), et
`openReportInto`/`renderReportInto` acceptent maintenant un `verdictContainer`
optionnel. `filing.ts` (Revue) et `library-detail.ts` (Bibliothèque) ajoutent
chacun un slot `.sift-fil-verdict`/`.lib-verdict` APRÈS leur carte Identification
et le passent en 3e argument — le verdict s'affiche donc en dernier sur les
deux écrans. `openReportModal` (popup debug, pas de carte Identification)
rajoute `verdictCardHtml(r)` explicitement à la fin, comportement inchangé
pour ce cas. Vérifié : `tsc --noEmit` + `npm run build` clean.

---

## 3. Revue — Mode Lot (batch)

**Fichiers à modifier** : `frontend/sift-live.ts` (fonction `renderBatch`,
ligne 335 — c'est elle qui implémente le Mode Lot réel, PAS
`batch-tracklist.ts` qui n'est que la petite liste de progression pendant
l'encodage).

**Tokens DESIGN.md** : groupes avec case à cocher tri-state, chips de format
par groupe, palette dark déjà branchée (tâche #16).

**Écart réel trouvé** :
1. **Groupement différent du README.** La maquette décrit 3 groupes par statut
   (*Prêts / À vérifier / En analyse*). Le code réel (`renderBatch`,
   `sift-live.ts:337-476`) groupe autrement : les prêts sont sous-groupés par
   **rail d'encodage** (lossless/lossy/unknown) avec un sélecteur de format par
   groupe (`groupChipsHtml`), et la review est scindée en Fakes (sélectionnables
   pour écarter) + reste (lecture seule). C'est un enrichissement fonctionnel
   réel (le format de sortie devient visible/choisissable par groupe), pas un
   oubli — mais ça diverge de ce que montre `Sift.dc.html`/`03-revue-lot.png`.
2. **Français incomplet** : labels encore en anglais dans `renderBatch` —
   `"READY TO FILE"` (sift-live.ts:453/467), `"NEEDS REVIEW"` (:470),
   `"Fakes"` (:472), `"Unknown rail"` (:484), `"Nothing clean to file yet."`
   (:467), `"Library root"` (:556/588), `"open in Detail"` (:405/426). Le
   README exige "tout le reste en français" (jargon technique excepté — ces
   chaînes ne sont pas du jargon).

**Décision (2026-07-01, "la maquette prime")** : implémenté. `renderBatch`
(sift-live.ts) suit maintenant les 3 groupes de la maquette (Prêts · lossless
/ À vérifier · fake / En analyse), un seul sélecteur de format global
(segmented MP3/AIFF/WAV dans `renderBatchRail`) au lieu des chips par rail
d'encodage, et tous les libellés sont en français (toggle Détail/Lot compris,
"Ouvrir en Détail", "Écarter (N)", "Rangement en arrière-plan…", etc.). Un
seul garde-fou métier a été conservé malgré "la maquette prime" — confirmé
explicitement avec Antoine (`AskUserQuestion` du 2026-07-01) : **un FAKE n'est
jamais filable**, seulement écartable (deux sets de sélection distincts,
`batchSel`/`batchFakeSel`), car c'est une règle d'intégrité des données (Sift
ne range jamais un fake lossless), pas un choix esthétique. Le blocage "pas
d'upscale depuis une source lossy" a lui été supprimé comme demandé : le
sélecteur de format global s'applique à toute la sélection à filer, y compris
les sources lossy. `railTarget`/`groupTarget`/`groupChipsHtml`/`railLabel`/
`batchCollapsed` (collapse par groupe, absent de la maquette) ont été retirés.
Vérifié : `tsc --noEmit` + `npm run build` clean.

---

## 4. Écartés

**Fichiers à modifier** : `frontend/ecartes-view.ts`.

**État constaté** : état vide (titre "Rien dans Écartés" + note + lien "Aller
à Revue →") déjà livré via `empty-state.ts` (chantier précédent, vérifié dans
cette session). Aucun écart trouvé.

---

## 5. Journal

**Fichiers à modifier** : `frontend/journal.ts`.

**État constaté** : entrée groupée détaillée + garde-fou revert >10 pistes
déjà livrés (tâche #13/#19, `journal.ts:258`). Aucun écart trouvé à ce stade.

**À vérifier** : pas encore comparé le layout visuel exact (colonnes Titre/
Artiste/Destination dans le détail groupé, `05-journal.png`) au code — à faire
avant de clore cet écran.

---

## 6. Bibliothèque

**Fichiers à modifier** : `frontend/sift-live.ts` (`renderBiblioLive`).

**État constaté** : toast "Mettre à jour les tags", export Rekordbox/Clé USB
avec vraie tâche de progression simulée (`startExportSim`), "Remettre en
revue" — tous déjà livrés (tâche #20 + le chantier export du background task
précédent).

**Ambiguïté produit — architecture Export** : le README/`Sift.dc.html`
placent Rekordbox/Clé USB comme **items de nav** persistants en bas de la
sidebar (section "Export", pastille ambre, `index.html:20-22` a exactement
cette structure — `data-grp="export"`, `.nv-export-dot`). Mais
`injectLeanStyle()` (`chrome.ts:96`) **masque actuellement ces items de nav**
("pas encore réel"), et le vrai export vit comme **boutons dans le header de
Bibliothèque** à la place. Résultat : deux implémentations d'un même concept
coexistent dans le code (nav stub caché + boutons Bibliothèque actifs). À
trancher : démasquer les items de nav et y router l'export (fidèle à la
maquette, mais l'export nécessite un contexte — quelle piste ? toute la
bibliothèque ?), ou documenter que l'implémentation Bibliothèque est le choix
retenu et retirer/laisser masqués les stubs de nav pour éviter la confusion.
**Décision (2026-07-01, "la maquette prime") — implémentée le 2026-07-02** :
export routé vers les items de nav, boutons du header Bibliothèque retirés.
- `chrome.ts` : `injectLeanStyle` ne masque plus `[data-view="rkb"]`/`"cle"`
  ni `.nv-grp[data-grp="export"]`.
- `sift-live.ts` : `exportGroup` (boutons header) et le handler `data-bib="export"`
  supprimés. Nouveau `runNavExport(target)` — fetch `listLibrary()` pour le
  total réel, toast "Bibliothèque vide — rien à exporter" si 0 piste, sinon
  `startExportSim` (déjà existant, inchangé). Réutilise la contrainte
  découverte dans le fichier maquette lui-même (`Sift.dc.html:970`,
  `exportTo(kind)`) : l'action nav n'ouvre PAS d'écran, elle agit
  immédiatement sur toute la bibliothèque filée (`s.library.length`) — pas de
  sélecteur, conforme à la maquette source, pas une interprétation.
- Un piège d'ordonnancement DOM a été identifié et contourné : `app.js`
  (chargé avant `sift-live.ts`) a son propre listener `click` bubble-phase sur
  `#pa` qui route TOUT `[data-view]` cliqué vers un changement d'écran mock
  (`renderRkb`/`renderCle`, jamais réels). Un listener **capture-phase** posé
  sur `#pa` dans `installLiveWiring()` intercepte les clics
  `[data-view="rkb"|"cle"]` et appelle `stopPropagation()` avant que le
  listener bubble-phase d'`app.js` ne s'exécute — la capture précède
  toujours la bubble quel que soit l'ordre d'enregistrement des listeners.
- Vérifié : `tsc --noEmit` + `npm run build` clean.

---

## 7. Réglages

**Fichiers à modifier** : `frontend/sift-live.ts` (`renderReglagesLive`).

**État constaté** : page unique scroll par carte (Discogs/Bibliothèque/
Fichiers/Apparence), pas d'onglets exclusifs, rail gauche scroll-to — déjà
livré (tâche #15/#21). Aucun écart trouvé à ce stade.

---

## Popover Destination (transverse)

Déjà conforme : `position:fixed` recalculé à l'ouverture + resize
(`filing.ts:797-823`), fermeture backdrop/Échap. Aucun écart trouvé.

## Zone de progression (transverse)

`progress-zone.ts` : tâches analyze/identify/file/**export** toutes gérées,
bouton Stop par tâche. Conforme au README. Aucun écart trouvé.

---

## Synthèse — chantier clos (2026-07-02)

1. **Mode Lot** — ✅ fait (2026-07-01) : regroupement 3 groupes maquette,
   sélecteur de format global, traduction complète, garde-fou "fake jamais
   filé" préservé sur confirmation explicite.
2. **Revue-Détail — ordre Verdict/Identification** — ✅ fait (2026-07-02) :
   verdict déplacé après Identification (`verdictContainer` optionnel sur
   `openReportInto`/`renderReportInto`), appliqué à Revue (`filing.ts`) ET
   Bibliothèque (`library-detail.ts`, même pattern réutilisé).
3. **Architecture Export (nav vs Bibliothèque)** — ✅ fait (2026-07-02) :
   export routé vers les items de nav (`runNavExport`), boutons du header
   Bibliothèque retirés. Piège d'ordonnancement DOM app.js/sift-live.ts
   identifié et contourné (listener capture-phase).
4. **Journal** — décision prise ("Option A") : architecture actuelle
   (catégories groupées + revert de masse + historique complet) gardée
   telle quelle, aucun changement.
5. **Accueil** — vérifié : bannière d'avertissement au lieu du pattern
   "État vide", divergence de forme assumée (structure de données réelle),
   pas un écart.
6. Tous les autres écrans (Écartés, Bibliothèque hors Export, Réglages,
   popover Destination, zone de progression) : aucun écart trouvé.

**tsc --noEmit + npm run build clean après chaque changement.** Rappel
"REBUILD BACKEND" non applicable — aucun fichier `src-tauri/` touché de tout
le chantier (frontend uniquement). Chantier refonte UI (handoff
`design_handoff_sift_refonte/`) considéré clos ; reprendre ce document si un
nouvel écart est découvert en usage réel (`npm run tauri dev`).
