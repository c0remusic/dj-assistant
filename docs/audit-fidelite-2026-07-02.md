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
| Structure 2 colonnes (liste + inspecteur détail) | ❌ | Absente — liste plate à la place. Écart majeur, pas cosmétique. |
| Fil d'Ariane "Accueil › {source}" | ❌ | Absent. |
| Carte bordée "Dossier surveillé" par source | ❌ | Absente. |
| Bannière ambre "Completed vs Incomplete" par source | ❌ | Absente (existe seulement pour "aucune racine définie", concept différent). |
| Toggle "Surveiller ce dossier" (dédié, dans l'inspecteur) | ❌ | Remplacé par un point cliquable inline dans la liste — fonctionnellement proche mais visuellement très différent. |
| CTA "+ Ajouter un dossier" en barre du bas | ⚠️ | Un bouton "add a folder" existe mais en bas de LISTE, pas en barre fixe, et en anglais. |
| Chaînes en français | ❌ | Plusieurs chaînes en anglais listées ci-dessus. |
| État racine bibliothèque non définie | ✅ | Bannière présente et fonctionnelle (`rootGateHtml`), juste pas positionnée/stylée comme la maquette. |

**Ancien plan (`refonte-ui-plan.md`) disait "Aucun changement nécessaire, divergence
de forme assumée" — c'était une conclusion prise sur un seul point (bannière racine)
sans avoir comparé la structure globale de l'écran à la source de la maquette. La
divergence réelle est bien plus large qu'assumé.**

## 2. Revue — Détail (`02-revue.png`)

**Capture fraîche obtenue (2026-07-02, après relance `npx tauri dev`) — dev-server
n'était plus en cause, les écarts ci-dessous sont confirmés sur du code à jour.**

| État | Statut | Note |
|---|---|---|
| Hero (pochette/titre/artiste/chemin) | ❌ | Titre/sous-titre/chemin OK et bien ordonnés (le "build périmé" expliquait bien ce point). Pochette toujours absente — bug confirmé, pas un souci de build. |
| Lecteur (play/temps/waveform) | ✅ | Bouton play, temps (0:00/2:45), waveform tous visibles — confirmé, c'était bien le dev-server périmé. |
| Slider Volume | ✅ | Visible et fonctionnel sur la capture fraîche. |
| Slider Tempo + Key-lock | ✅ | Toujours conformes. |
| Section Preuves (chip LOSSLESS pilule séparée) | ❌ | Toujours fondu — ce qu'on voit ("Preuve (spectre) ▸ afficher") est l'ancien panneau technique repliable (`spectroAndTagsHtml`), pas le nouveau composant Preuves séparé attendu par la maquette (ligne 221-232 de `Sift.dc.html`). Confirme décision #1. |
| Carte Identification · Discogs (bordure, bouton Modifier) | ❌ | Toujours plate, pas de carte visible. Confirme décision #2. |
| Ordre Preuve → Identification → Verdict | ✅ | Ordre globalement correct sur la capture fraîche (verdict bien en dernier). Décision #3 de l'audit RÉSOLUE : ne pas toucher à l'ordre, il est bon. |
| Bandeau verdict "Prêt à ranger" + nom final | ❌ | "NOM FINAL" toujours vide sur capture fraîche → **vrai bug de branchement confirmé**, plus explicable par un build périmé. `verdictCardHtml`/`filing.ts` à corriger. |
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

## Prochaine étape

Relancer `npx tauri dev`, prendre une capture pour **chaque ligne ❓/⚠️** de
ce document (au moins Revue-Détail en priorité vu la contradiction trouvée),
comparer au screenshot handoff correspondant, mettre à jour ce tableau avec
✅/❌, puis seulement à ce moment-là grouper les corrections en un seul lot
cohérent plutôt que de les traiter une par une.
