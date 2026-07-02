# Audit de fidélité — écran par écran, état par état

## Pourquoi ce document existe

`docs/refonte-ui-plan.md` (2026-07-02) déclare le chantier **"clos"**, avec
"Aucun écart trouvé" sur la quasi-totalité des écrans, verdicts appuyés par
`tsc --noEmit` / `npm run build` clean. Mais une capture réelle de Revue
(2026-07-02, session courante) a montré des écarts majeurs — bouton play,
temps, volume, pochette, section Preuves, nom final — sur l'écran *précisément
déclaré conforme* dans ce plan.

**Conclusion : "build clean" prouve que le code compile, pas qu'il ressemble à
la maquette.** Les affirmations "Aucun écart trouvé" de `refonte-ui-plan.md`
sont donc des THÉORIES NON VÉRIFIÉES tant qu'elles n'ont pas été confrontées à
une capture d'écran réelle de l'app relancée (`npx tauri dev`). Ce document
reprend chaque écran + chaque état listé au README du handoff, et distingue :

- ✅ **VÉRIFIÉ** — comparé à une vraie capture d'app dans cette session.
- ⚠️ **AFFIRMÉ SANS PREUVE** — `refonte-ui-plan.md` dit "conforme", jamais
  confronté à un screenshot réel.
- ❌ **ÉCART CONFIRMÉ** — comparé et divergent, à corriger.
- ❓ **NON COUVERT** — état existant dans le README mais absent des captures
  qu'on a comparées.

**Procédure pour chaque ligne ❓/⚠️** : relancer `npx tauri dev`, amener l'app
dans l'état exact décrit, capturer, comparer au screenshot du handoff
correspondant (`design_handoff_sift_refonte/screenshots/`), puis mettre à jour
ce document avant de corriger quoi que ce soit.

---

## 1. Accueil (`01-accueil.png`)

**Comparaison directe `Sift.dc.html` (lignes 596-629) vs `home-sources.ts` — écart
structurel majeur trouvé, pas un détail.**

La maquette décrit une vraie grammaire 2-colonnes pour cet écran : Queue = liste
des sources, **Inspecteur = détail de la source sélectionnée** avec fil d'Ariane
"Accueil › {source}", titre + chip statut, carte bordée "Dossier surveillé"
(`background:var(--card);border:1px solid var(--border)`), bannière ambre
contextuelle "Vise le sous-dossier Completed, pas Incomplete", toggle "Surveiller
ce dossier", barre du bas avec CTA "+ Ajouter un dossier".

`home-sources.ts` ne fait RIEN de tout ça : c'est une **liste plate à une seule
colonne** (`.home-left`), pas d'inspecteur, pas de fil d'Ariane, pas de carte
bordée "Dossier surveillé" par source sélectionnée, pas de bannière Completed/
Incomplete par source (il y a UNE bannière racine-manquante globale, différente
de la bannière "Completed vs Incomplete" par source de la maquette), pas de
toggle "Surveiller ce dossier" dédié (juste un point `.tog` inline par ligne).

**Bonus, trouvé en lisant le fichier en entier** : plusieurs chaînes sont encore
en **anglais** — `"Watched folders"`, `"add a folder"`, `"No watched folder."`,
`"new"` / `"up to date"`, `"Watching — click to pause"` / `"Paused — click to
watch"`. Le README exige tout en français (jargon technique excepté).

| État | Statut | Note |
|---|---|---|
| Structure 2 colonnes (liste + inspecteur détail) | ✅ **corrigé (2026-07-02)** | Refait pour matcher `Sift.dc.html:68-77` (liste, COL2 272px) + `:594-633` (inspecteur, COL3). `app.js:77` rend le shell `#homequeue`/`#homeinspector` (réutilise les classes `.queue`/`.sift-inspector` déjà utilisées par Revue) ; `home-sources.ts` réécrit en entier pour peupler les deux colonnes avec sélection persistante par id (`selectedSourceId`). |
| Fil d'Ariane "Accueil › {source}" | ✅ **corrigé** | `home-sources.ts:inspectorHtml` — reproduit `Sift.dc.html:597`. |
| Carte bordée "Dossier surveillé" par source | ✅ **corrigé** | `home-sources.ts:inspectorHtml` — reproduit `Sift.dc.html:612-615` (bordure `--color-border-tertiary`, radius lg, chemin en police mono). |
| Bannière ambre "Completed vs Incomplete" par source | ❓ **AMBIGUÏTÉ — voir section "Ambiguïtés en attente"** | Pas implémentée : `Source` (`shared/contracts.ts:26-32`) n'a pas de champ équivalent au `hint` de la maquette, et la donnée démo de la maquette (`hint:true` sur un chemin `...\Soulseek\complete`, ligne 826) est incohérente avec son propre texte — pas de heuristique fiable à en tirer sans deviner. |
| Toggle "Surveiller ce dossier" (dédié, dans l'inspecteur) | ✅ **corrigé** | `home-sources.ts:inspectorHtml`, `data-sift="togglewatch"` — reproduit `Sift.dc.html:621-625` (case + libellé), appelle `setSourceWatched`. |
| CTA "+ Ajouter un dossier" en barre du bas | ✅ **corrigé** | `home-sources.ts:listColumnHtml` — barre `border-top` en bas de la colonne liste (équivalent à `Sift.dc.html:628-632`, qui la place en bas de COL3 ; ici mise en bas de COL2 puisque c'est notre colonne liste — voir note ci-dessous), texte français. |
| Chaînes en français | ✅ **corrigé** | Toutes les chaînes anglaises (`Watched folders`, `add a folder`, `No watched folder.`, `new`/`up to date`, `Watching/Paused — click to…`) retirées avec la réécriture de `home-sources.ts`. |
| État racine bibliothèque non définie | ✅ | Bannière conservée, déplacée dans l'inspecteur (`home-sources.ts:inspectorHtml`) juste après le fil d'Ariane, comme `Sift.dc.html:599-605`. |

**Note (placement CTA)** : la maquette place la barre "+ Ajouter un dossier" en bas de COL3
(l'inspecteur, ligne 628-632 du fichier source), pas en bas de COL2 (la liste). Choix fait ici :
la garder en bas de COL2 (la liste des sources), par cohérence avec le geste "ajouter une
nouvelle source à la liste" plutôt qu'une action liée à la source sélectionnée dans
l'inspecteur — mais c'est une divergence de placement assumée, pas une lecture littérale de
la maquette. À signaler si Antoine préfère suivre la maquette à la lettre.

**Ancien plan (`refonte-ui-plan.md`) disait "Aucun changement nécessaire, divergence
de forme assumée" — c'était une conclusion prise sur un seul point (bannière racine)
sans avoir comparé la structure globale de l'écran à la source de la maquette. La
divergence réelle est bien plus large qu'assumé.**

## 2. Revue — Détail (`02-revue.png`)

**Capture fraîche obtenue (2026-07-02, après relance `npx tauri dev`) — dev-server
n'était plus en cause, les écarts ci-dessous sont confirmés sur du code à jour.**

| État | Statut | Note |
|---|---|---|
| Hero (pochette/titre/artiste/chemin) | ✅ **corrigé (2026-07-02)** | Titre/sous-titre/chemin déjà OK. Pochette : bug de branchement confirmé et corrigé — `.sift-report-cover` n'était rempli que par `onIdentityApplied` (identify FRAIS dans la session, `filing.ts:694-700`), jamais par `restoreIdentifiedLine` (réouverture d'une piste déjà identifiée). Ajouté le même bloc `convertFileSrc`+`hidden=false` dans `restoreIdentifiedLine`, `filing.ts:762-770`. Confirme décision #5. |
| Lecteur (play/temps/waveform) | ✅ | Bouton play, temps (0:00/2:45), waveform tous visibles — confirmé, c'était bien le dev-server périmé. |
| Slider Volume | ✅ | Visible et fonctionnel sur la capture fraîche. |
| Slider Tempo + Key-lock | ✅ | Toujours conformes. |
| Section Preuves (chip LOSSLESS pilule séparée) | ✅ **corrigé** | Chips extraits de `verdictCardHtml` vers un nouveau bloc `evidenceChipsHtml` (`report-view.ts`), positionné avant le panneau spectre repliable, donc avant Identification — reproduit `Sift.dc.html:221-232`. `.sift-vchips` reste le même sélecteur (les insertions MATCH/DUPLICATE de `filing.ts` continuent de le trouver sans changement). CSS `.sift-evidence`/`.sift-evidence-label` ajouté (`styles.css`). Confirme + corrige décision #1. |
| Carte Identification · Discogs (bordure, bouton Modifier) | ✅ **corrigé** | `.sift-fil-editor.sift-fil-editor-margin` — fond `--color-background-secondary`, bordure `--color-border-tertiary`, radius lg, padding 14px 16px (`styles.css`), reproduit `Sift.dc.html:309`. Bouton "Modifier" (`data-fil="ident-edit"`) existait déjà (`filing.ts:1018`), seule la carte manquait. Confirme + corrige décision #2. |
| Ordre Preuve → Identification → Verdict | ✅ | Ordre globalement correct sur la capture fraîche (verdict bien en dernier). Décision #3 de l'audit RÉSOLUE : ne pas toucher à l'ordre, il est bon. Reste vrai après le split Preuves : `.sift-fil-report` (player+Preuves+spectre) → `.sift-fil-editor` (Identification) → `.sift-fil-verdict` (conclusion), DOM inchangé. |
| Bandeau verdict "Prêt à ranger" + nom final | ✅ **corrigé** | `refreshPreview()` n'était jamais appelée à l'ouverture initiale (seulement après un edit/identify/changement de format) → `.sift-verdict-finalname` restait vide tant qu'on ne touchait à rien. Ajouté un appel à `refreshPreview()` en fin de `openFilingInto` (`filing.ts:1515-1518`). |
| Genres en tags séparés | ✅ | Conforme. |
| Popover Destination (arborescence, filtre) | ⚠️ | Pas dans cette capture, toujours à vérifier. |
| Doublon détecté (bannière) | ❓ | Pas testé. |
| Non identifié (bouton "Rechercher sur Discogs") | ❓ | Pas testé. |
| MATCH/CHECK MATCH ambre | ❓ | Pas testé. |

**Contradiction avec `refonte-ui-plan.md` maintenant précisément localisée** : sur
6 sous-éléments vérifiés, 3 sont conformes (lecteur, volume, ordre) et 3 sont de
vrais écarts (pochette, Preuves, Identification, nom final — techniquement 4).
Le plan précédent affirmait "aucun écart" sur la totalité de l'écran ; c'était
faux pour au moins 4 points sur ~10.

## 3. Revue — Mode Lot (`03-revue-lot.png`)

**Capture fraîche obtenue (2026-07-02, lot réel de 7793 pistes prêtes + fakes présents).**

| État | Statut | Note |
|---|---|---|
| 3 groupes (Prêts/À vérifier/En analyse) | ✅ | Code vérifié ligne par ligne (`renderBatch`, `sift-live.ts:387-524`) — les 3 groupes existent bien et se rendent correctement si non-vides. Le plan avait raison sur ce point, mais pour la mauvaise raison (voir écart réel ci-dessous). |
| Groupe "À vérifier · fake" atteignable en pratique | ❌ → ✅ **corrigé (2026-07-02)** | **Écart réel trouvé, pas un bug de rendu** : les 3 groupes sont dans le même conteneur scrollable, dans l'ordre Prêts→Fakes→En analyse. Avec 7793 pistes "Prêts", le groupe Fakes existe dans le DOM mais se trouve après ~7793 lignes — inatteignable en pratique par simple scroll. Confirmé par toi ("il y a bien des fakes") alors qu'aucun groupe fake n'était visible sur la capture. **Fix appliqué** : groupes repliables (chevron sur chaque en-tête, `data-sift="batchcollapse"`, état `batchCollapsed`), pour pouvoir replier "Prêts" et atteindre les deux autres groupes sans reordonner. Note : un chantier antérieur avait un mécanisme de collapse par groupe et l'avait retiré car absent de la maquette source — réintroduit ici sur demande explicite, override légitime documenté dans le code (`sift-live.ts:87-92`). |
| Cases à cocher de sélection (Prêts/Fakes) | ❌ → ✅ **corrigé (2026-07-02)** | Demande explicite : retirer la couleur (vert/rouge) des cases `readyRow`/`fakeRow`, les remplacer par le même style que la case "Sur place" (`filing.ts:362`, `<input type="checkbox">` natif sans accent de couleur). Fait pour les deux types de lignes (`sift-batch-ck`, `styles.css`). La case tri-state de l'en-tête de groupe (`sift-bgrp-box`, sélection groupée) n'a pas été touchée — pas demandée explicitement, comportement visuel différent (tri-state plein/partiel). À confirmer si elle doit aussi perdre sa couleur. |
| Sélecteur format global segmenté | ⚠️ | Non revérifié dans cette capture. |
| Simulation rangement piste par piste (wait→running→done/fail) | ❓ | Non couvert par une capture. |
| Tâche "Rangement" dans le rail de nav | ❓ | — |
| Garde-fou "fake jamais filable" | ⚠️ | Affirmé confirmé avec toi le 2026-07-01 — pas revu depuis, mais cohérent avec `fakeRow`/`batchFakeSel` lus dans le code (jamais mélangé à `batchSel`/File). |

**`tsc --noEmit` + `npm run build` clean après le fix collapse/checkboxes.**

## 4. Écartés (`04-ecartes-vide.png`)

| État | Statut | Note |
|---|---|---|
| État vide (titre+note+lien "Aller à Revue") | ✅ | Fait et vérifié dans CETTE session (`empty-state.ts`, `tsc`+`build` clean) — la seule ligne de tout ce document vérifiée par nous, pas juste affirmée. |
| Liste non-vide (À re-sourcer / Corbeille) | ❓ | Jamais comparé à une capture réelle. |
| Bouton "Purger la corbeille" | ❓ | — |
| Restaurer ("Remettre en revue") | ❓ | — |

## 5. Journal (`05-journal.png`)

| État | Statut | Note |
|---|---|---|
| Liste historique (Filé/Écarté + timestamp) | ❓ | — |
| Détail groupé (chaque piste listée individuellement) | ⚠️ | Plan dit "déjà livré", jamais vu en vrai. |
| Confirmation revert >10 pistes | ❓ | Cas limite jamais testé visuellement. |
| Layout colonnes Titre/Artiste/Destination | ⚠️ | Le plan lui-même note "pas encore comparé le layout visuel exact" — seule auto-critique honnête du document. |

## 6. Bibliothèque (`06-bibliotheque.png`)

| État | Statut | Note |
|---|---|---|
| Liste + facettes (dossier/genre) | ❓ | — |
| Export vers nav (Rekordbox/Clé USB, pastille ambre) | ⚠️ | Changement architectural important (nav vs boutons header) affirmé fait le 2026-07-02 — jamais revu en vrai depuis, et c'est un changement risqué (listener capture-phase pour contourner app.js). |
| Toast "Mettre à jour les tags" | ❓ | — |
| État vide (lien "Aller à Revue") | ✅ | Couvert par notre fix de cette session (branche `trulyEmpty` dans `sift-live.ts`), mais jamais vu rendu en vrai — code vérifié, pas l'écran. |
| Filtre actif → 0 résultat (pas de lien retour) | ❓ | Notre propre ajout de cette session, jamais vu rendu. |

## 7. Réglages (`07-reglages.png`)

| État | Statut | Note |
|---|---|---|
| Page scroll unique par carte | ⚠️ | — |
| Scroll-to depuis le rail Queue | ⚠️ | — |
| Toggle Apparence (Auto/Clair/Sombre) | ❓ | — |
| Carte Discogs (jeton + Modifier) | ❓ | — |

## Transverse

| Élément | Statut | Note |
|---|---|---|
| Popover Destination (fixed, backdrop, Échap) | ⚠️ | — |
| Zone de progression (cartes tâches, bouton Stop) | ⚠️ | — |
| Mode sombre (`prefers-color-scheme` + override) | ❓ | — |
| Hover states (rowActive, brightness) | ❓ | Jamais testé interactivement. |

---

## Décisions — tranchées par lecture directe du code source de la maquette

Source consultée : `.interface-design/refonte-ui-sift/project/Sift.dc.html`
(identique octet-pour-octet au handoff `design_handoff_sift_refonte/Sift.dc.html`).
Lire le code source du board règle l'ambiguïté mieux qu'un screenshot — ordre
DOM exact, classes, valeurs, commentaires du designer inclus.

1. **Section Preuves — TRANCHÉ, il faut séparer.** Ligne 221-232 :
   `<!-- EVIDENCE chips + inline proof -->`, rangée de chips "Preuves"
   autonome, positionnée juste après le lecteur, bien AVANT Identification.
   Le code actuel range ce chip DANS le bandeau vert "Prêt à ranger" — vrai
   écart de structure, pas une simplification voulue. À corriger : extraire
   un bloc Preuves séparé dans `report-view.ts`, cesser de le fusionner dans
   `verdictCardHtml`.
2. **Carte Identification — TRANCHÉ, bordure confirmée.** Ligne 309 :
   `background:var(--card);border:1px solid var(--border);border-radius:11px`
   — écrit noir sur blanc dans la source, pas une supposition de style. La
   capture réelle montre du plat (pas de carte, pas de bouton "Modifier"
   visible) → écart confirmé, à corriger dans `filing.ts`.
3. **Ordre final — confirmé par un commentaire du designer.** Ligne ~373 :
   `<!-- CONCLUSION : ready to file (dernière étape) -->`. Ordre voulu :
   Lecteur → Preuves (chips) → Identification (carte) → Verdict (bandeau, en
   dernier, avec Nom final aligné à droite). `report-view.ts` prétend déjà
   implémenter cet ordre ("verdict déplacé après Identification") — mais la
   capture réelle ne le montre pas correctement. Soit bug de branchement,
   soit même symptôme que le dev-server périmé constaté plus tôt dans la
   session. **À reconfirmer avec une capture fraîche avant de toucher au
   code** — ne pas corriger un ordre qui pourrait déjà être bon.
4. **État vide — écart mineur trouvé en comparant à la source.** Ligne ~394 :
   titre en `font-size:17px` (notre implémentation utilise `--text-xl`,
   16px — le token le plus proche mais pas une valeur en dur identique) et
   le lien "Aller à Revue →" est un texte souligné (`border-bottom:1px solid
   var(--borderStrong)`), pas un bouton comme dans notre `.sift-empty-link`
   actuel. Cosmétique, pas fonctionnel — à corriger si on vise le
   pixel-perfect strict, sinon acceptable tel quel.
5. **Pochette manquante — encore ouvert.** `heroHtml` a
   `<img class="sift-report-cover" hidden>` — reste à vérifier si l'attribut
   `hidden` est bien retiré quand une pochette est disponible, ou si le
   retrait ne se déclenche jamais (bug de branchement, pas un problème de
   CSS). Pas résolu par la lecture de la source maquette (elle utilise un
   placeholder rayé générique, cf. README § Assets) — nécessite de lire
   `identify-shared.ts`/`filing.ts` directement.

## 5bis. Journal — layout (complément)

Lu `journal.ts` en entier (structure). Architecture confirmée différente de la
maquette par choix assumé (décision "Option A" du 2026-07-01 déjà actée dans
`refonte-ui-plan.md` : 3 catégories Filé/Corbeille/Rejeté au lieu de 2 Filé/
Écarté, gardé tel quel). **Ne pas rouvrir ce point.** Seul le sous-point resté
non vérifié dans l'ancien plan (layout colonnes exact du détail groupé) reste
❓ — nécessite une capture réelle, pas de nouvelle preuve trouvée cette session.

## 6bis. Réglages — complément

Lu `renderReglagesLive` (`sift-live.ts:856+`). Structure une-page-scroll
confirmée par un commentaire du code lui-même citant la règle maquette ("PAS
des onglets exclusifs"). Cartes (Discogs/Bibliothèque/Fichiers/Apparence) non
vérifiées bordure-par-bordure faute de temps cette session — probable mais
pas confirmé à 100%. À vérifier par Claude Code contre `Sift.dc.html:642+`
(carte Discogs commence ligne 642, `background:var(--card);border:1px solid
var(--border)`) avant de considérer cet écran clos.

## 7bis. Ce qui n'a PAS pu être audité cette session (à faire par Claude Code)

Faute de temps, ces points du document restent ❓/⚠️ et n'ont pas été
recroisés avec `Sift.dc.html` :
- Bibliothèque : facettes dossier/genre, toast "Mettre à jour les tags".
- Popover Destination : "+ nouveau" imbriqué, fermeture Échap.
- Zone de progression : cartes de tâches, bouton Stop par tâche.
- Mode sombre (`prefers-color-scheme` + override persistant `sift-theme`).
- Hover states génériques (`rowActive`, `brightness(0.93–0.95)`).
- Revue-Détail : bannière doublon, flux "non identifié → Rechercher sur
  Discogs → candidats", badge MATCH/CHECK MATCH.
- Mode Lot : sélecteur format global segmenté (re-vérifier après le fix
  collapse de cette session), simulation rangement piste par piste, tâche
  "Rangement" dans le rail de nav.

**Pour chacun : ouvrir `.interface-design/refonte-ui-sift/project/Sift.dc.html`
en lecture (grep sur le libellé français exact du README), comparer au fichier
réel listé dans le README § Mapping vers le code réel, consigner le verdict
ici avec citation de ligne (comme fait pour Revue-Détail/Identification/
Preuves) AVANT de corriger quoi que ce soit.**

## 7ter. Verdicts 7bis traités cette nuit (2026-07-02, Claude Code)

Grep exhaustif sur `Sift.dc.html` pour chaque libellé français avant tout verdict,
comme demandé. Résultat inattendu : plusieurs points de la liste 7bis décrivent
des fonctionnalités qui **n'existent tout simplement pas dans `Sift.dc.html`** —
la maquette interactive est un prototype simplifié, moins riche que le vrai Sift
sur certains points (le vrai popover Destination a une vraie arborescence de
dossiers ; la maquette n'en simule qu'une version très plate). Dans ce cas il n'y
a rien à comparer, donc rien à corriger — pas la même chose qu'un écart confirmé.

| Point 7bis | Verdict | Preuve |
|---|---|---|
| Bibliothèque : facettes dossier/genre | ❓ **non modélisé dans la maquette** | `grep -n "facette\|filtre\|dropdown\|<select"` sur `Sift.dc.html` → **0 résultat**. La Bibliothèque de la maquette réutilise `isTrackScreen`/`showTrackRows` (ligne 1072-1075), la même liste plate que Revue/Écartés — pas de UI de facettes distincte à répliquer. Rien à corriger côté code ; si Antoine veut des facettes, c'est une feature nouvelle, pas un écart de fidélité. |
| Bibliothèque : toast "Mettre à jour les tags" | ✅ **couvert fonctionnellement** | La maquette (`secondary()`/`updateTags()`, lignes 968-969) réécrit les tags ID3 de la piste sélectionnée et montre un toast. Le vrai Sift a `updateMetadata` (`ipc.ts:239`, écrit fichier PUIS DB) câblé au bouton "Save" de `library-detail.ts:100` (`data-lib="save"` → `doSave`) — cliquer Save avec les champs inchangés fait exactement la même réécriture de tags. Différence : pas de bouton/raccourci dédié "Mettre à jour les tags" séparé de Save, juste le même geste sous un autre nom. Pas un écart fonctionnel, seulement un possible écart de libellé — laissé tel quel (Save est plus explicite que le "secondary action" implicite de la maquette). |
| Popover Destination : "+ nouveau" imbriqué, fermeture Échap | ❓ **non modélisé dans la maquette** | `grep -n "nouveau\|Échap\|Escape"` sur `Sift.dc.html` → **0 résultat**. Le vrai popover de destination de Sift (`filing.ts`, arborescence réelle + "Browse custom") est plus riche que ce que la maquette simule — rien à comparer ligne à ligne. Pas d'écart de fidélité identifiable depuis cette source ; à auditer plutôt via un test manuel (Échap ferme-t-il le popover ? à vérifier en `tauri dev`, hors scope de cette lecture de code). |
| Zone de progression : cartes de tâches, bouton Stop par tâche | ✅ **conforme** | Maquette : `showStop`/`onStop` par tâche (lignes 52-53, 466-467, `tk.showStop`). Réel : `progress-zone.ts` — `cancelHandlers` par `TaskKind`, bouton `.sift-pz-cancel` affiché seulement `state==='running' && !stopping && cancelHandlers.has(kind)` (lignes 100-119). Correspond. |
| Mode sombre (`prefers-color-scheme` + override persistant) | ✅ **conforme** | Maquette : 3 modes auto/light/dark, persistance `localStorage.getItem('sift-theme')` (lignes 732, 845). Réel : `theme.ts` — 3 mêmes modes, persistance via `getSetting`/`setSetting("ui_theme")` (backend Tauri au lieu de localStorage — différence attendue et correcte pour une app desktop, pas un écart). `apply()` bascule `[data-theme]` ou le laisse au CSS `prefers-color-scheme`, même logique que `isDark()`/`theme()` de la maquette (ligne 833-834). |
| Hover states génériques (`rowActive`, `brightness`) | ❓ **non vérifié** | Pas de citation trouvée en lecture statique seule (dépend du rendu réel au survol) — nécessite un test interactif en `tauri dev`, laissé pour Antoine. |
| Revue-Détail : bannière doublon, flux non-identifié, badge MATCH/CHECK MATCH | ❓ **non vérifié cette nuit** | Faute de temps — le code correspondant existe (`dupBanner`, `doIdentify`'s `dsIdle`/`onSearchDiscogs` équivalent, `vchipHtml("CHECK MATCH", ...)` déjà vus en lisant `filing.ts` pour la section Preuves ci-dessus) mais pas comparé ligne à ligne à `Sift.dc.html:273-378`. À faire dans une prochaine session. |
| Mode Lot : sélecteur format global segmenté, simulation piste par piste, tâche "Rangement" au rail | ❓ **non vérifié cette nuit** | Pas re-testé après le fix collapse de la session précédente — nécessite `tauri dev` + un vrai lot, hors scope de la lecture de code seule. |

---

## Instructions pour Claude Code — travail autonome (Antoine hors ligne)

1. **Ordre de traitement** : Accueil (écart majeur, section 1) d'abord — c'est
   la plus grosse divergence confirmée de tout l'audit. Puis Revue-Détail
   (section 2 : pochette, section Preuves séparée, carte Identification,
   nom final vide — 4 corrections indépendantes, peuvent être faites une par
   une). Puis la liste "7bis" ci-dessus, dans l'ordre où elle est écrite.
2. **Ne pas rouvrir** : l'ordre Preuve→Identification→Verdict (confirmé bon),
   Mode Lot 3-groupes + collapse + checkboxes décolorées (fait cette session),
   Journal (décision Option A actée), état vide Écartés/Bibliothèque (fait,
   vérifié). Une re-vérification rapide après un `npm run build` est OK ; une
   réimplémentation ne l'est pas.
3. **Méthode obligatoire pour chaque point ❓/⚠️** : lire la section
   correspondante de `Sift.dc.html` (citer les numéros de ligne trouvés, comme
   fait dans ce document pour Preuves/Identification), lire le fichier réel
   listé dans le README § Mapping, écrire le verdict dans ce document AVANT de
   modifier du code. Ne jamais écrire "Aucun écart trouvé" sans avoir cité une
   ligne précise de la maquette source à l'appui — c'est l'erreur qui a
   invalidé `refonte-ui-plan.md`.
4. **Après chaque correction** : `npx tsc --noEmit` puis `npm run build`,
   les deux doivent être clean avant de passer au point suivant. Mettre à
   jour la ligne correspondante de ce tableau (❌→✅) avec un commentaire
   citant le fichier/ligne modifié.
5. **En cas d'ambiguïté produit réelle** (pas juste "je ne sais pas où
   regarder", mais un vrai choix à trancher entre deux comportements
   plausibles) : écrire l'ambiguïté dans ce document sous une nouvelle
   sous-section "Ambiguïtés en attente", ne PAS deviner, continuer sur les
   autres points de la liste en attendant.
6. **Ne pas toucher** `src-tauri/` (aucun écart de ce document ne nécessite du
   Rust) ni introduire de nouvelle dépendance UI (contrainte README : vanilla
   TS + patterns existants uniquement).

## Ambiguïtés en attente

1. **Bannière ambre "Completed vs Incomplete" par source (Accueil, section 1).**
   Le vrai `Source` (`shared/contracts.ts:26-32`) n'a pas de champ équivalent au
   `hint` de la maquette, et la donnée démo de la maquette elle-même est
   incohérente sur ce point (`hint:true` sur un chemin qui contient déjà
   `complete`, ligne 826 de `Sift.dc.html`) — impossible d'en déduire une
   heuristique fiable sans backend. Deux options plausibles, à trancher par
   Antoine :
   - (a) heuristique conservative : n'afficher le hint que si le dernier
     segment du chemin est exactement `incomplete` (insensible à la casse) —
     zéro risque de faux positif, mais ne couvre que ce cas précis ;
     (b) ajouter un champ backend (`src-tauri/`, donc **hors scope** de cette
     session par contrainte explicite) pour une détection plus riche.
   Rien implémenté pour l'instant (banière absente), pas de guess fait.

## Prochaine étape

Relancer `npx tauri dev`, prendre une capture pour **chaque ligne ❓/⚠️** de
ce document (au moins Revue-Détail en priorité vu la contradiction trouvée),
comparer au screenshot handoff correspondant, mettre à jour ce tableau avec
✅/❌, puis seulement à ce moment-là grouper les corrections en un seul lot
cohérent plutôt que de les traiter une par une.
