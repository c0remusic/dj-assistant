# Sift — Catalogue d'états des composants réels

> Source de vérité pour le portage design→code : liste, composant par composant,
> tous les états visuels **tels qu'ils existent déjà dans le vrai code**
> (`frontend/styles.css` + les fichiers `.ts` qui les rendent — jamais `app.js`,
> la maquette navigateur jetable, ni `Sift.dc.html`, qui a son propre vocabulaire
> de tokens et sa propre logique). Alimenté au fur et à mesure d'un audit
> composant-par-composant (méthode : cataloguer un à la fois, vérifier, avant de
> continuer — démarré 2026-07-03).
>
> Usage : avant de porter un nouveau design, vérifier ici si le composant existe
> déjà et quels états il a réellement, plutôt que de re-déduire toute la logique
> depuis `Sift.dc.html`. Avant de déclarer un portage "fini", cocher chaque état
> listé contre une preuve fraîche (voir `sift-audit-fidelite-methode` en mémoire).
>
> `.interface-design/system.md` est **périmé sur la palette/direction visuelle**
> (dark superseded, 2026-07-01) — ne pas s'y fier pour les couleurs, seulement
> pour espacement/radius/typo. Ce fichier-ci est la source à jour état-par-état,
> couleurs et comportements réels.
>
> **Audit référence canonique terminé (2026-07-08/09)** : les 8 écrans ont été
> comparés à shadcn/ui-thing/coss/Apple HIG (voir sections « Écran X — audit
> référence canonique » et `docs/superpowers/changes/2026-07-08-ui-reference-audit/`).
> Tout nouveau composant doit suivre la règle CLAUDE.md « Front — référence de
> design avant d'inventer » — consulter le pool avant d'improviser, pas après.
>
> Numéros de ligne vérifiés à jour le 2026-07-03 (après les fixes de cette
> session) — `styles.css` bouge vite, revérifier au grep si un doute.
> **Repasse ciblée du 2026-07-24** : `.qi`, `.nv`, `.sift-ranger-btn` (lignes)
> et la section « Carte verdict » (composant renommé/scindé) revérifiés par
> grep — le reste du fichier n'a pas été rebalayé, mêmes réserves qu'avant.
> **Session de fixes UI du 2026-07-24 (2ᵉ passe)** : nouveaux états ajoutés sur
> Revue/Écartés/Bibliothèque/Bibliothèque éditeur/Rekordbox/Accueil/Journal/
> Revue-bannières/Lot + 2 tokens couleur clair + 2 hover manquants — voir
> entrées dédiées ci-dessous et l'entrée d'Historique du 2026-07-24.
>
> **Repasse du 2026-08-19 — deux refontes portées ici** : le Journal est devenu
> une table (`.lr.jrnl-row`, groupes, inspecteur zone D — aucune classe de
> l'ancien écran n'a survécu) et la Bibliothèque a gagné une colonne Verdict
> (`verdictView()`, `.sift-lib-v-*`, la puce `verdictBadge` de fin de ligne est
> retirée). Les sections périmées ne sont pas réécrites en silence : elles
> gardent leur texte daté et portent un ⚠️ qui nomme ce qui les remplace. Les
> deux nouveaux états ont leur story — `frontend/journal-table.stories.ts` et
> `frontend/library-verdict.stories.ts` — Storybook étant le miroir vivant de ce
> fichier. Sommaire renuméroté dans le même geste, écarts sommaire → titre
> vérifiés inchangés entrée par entrée.
>
> **Repasse du 2026-08-27 — le lecteur simple et les surfaces de Revue portés ici** :
> la waveform a quitté Revue (lecteur = slider du kit + volume fin, `31c5d1a`), la
> pastille de verdict de file est passée aux teintes système pleines, et quatre
> surfaces ont bougé (cadre de lecture, pied en surface, rail en retrait, file bord
> à bord + séparateurs). Trois sections nouvelles en fin de fichier, chacune avec sa
> story (`player-audition.stories.ts`, `queue-verdict-dot.stories.ts` — rendus
> extraits en modules purs pour que les stories exécutent le vrai code) ; la section
> Sliders est marquée périmée (`.sift-slider-*` n'existe plus) ; « Autres couleurs
> non tokenisées » et « Grammaire de carte » portent un ⚠️ daté. Sommaire renuméroté,
> même vérification que le 2026-08-19.

## Sommaire

> Index de navigation (ligne + gist), pas une table des matières auto-générée.
> Charger ce fichier entier n'est plus automatique (retiré de `CLAUDE.md` le
> 2026-07-09) : ouvrir la section visée via son numéro de ligne plutôt que tout
> lire.

- L133 — Ligne de queue `.qi` — réécrit 08-27 : tokens (hex chauds morts), liseré de sélection retiré, curseur clavier `.qi-kbd`, interlignes #45 (46 px constante), séparateurs de rangées `::before` + pastille en fin de titre (08-27).
- L175 — Mot de verdict Détail `verdictWord()` — ⚠️ COMPOSANT RETIRÉ le 2026-08-26 (c4f65eb) ; pipeline repris par `verdictDot()` (L1980).
- L201 — Item de navigation `.nv` — glyphes en accent info uniforme, décision A du 09-03 (grilling #11).
- L212 — Bouton d'action principal `.sift-ranger-btn` — hover désormais déclaré explicitement (07-24), disabled/focus restent génériques.
- L231 — Chip/tag `.chip` — hover corrigé 07-03.
- L243 — Case à cocher `.cbx` — supprimée (code mort).
- L250 — Segmented control `.sift-seg-opt` (ancien, voir aussi pastille unifiée L930) — RAS.
- L260 — Ligne de journal `.lr.jrnl-row` — refonte 08-19 : table, colonnes, groupes, états, inspecteur zone D (`.jrnl-qrow` et toutes les classes de l'ancien Journal ont disparu).
- L328 — Toggle switch `.tog` — perf `transform` corrigée 07-03.
- L341 — Slots verdict `.sift-fil-verdict` / `.sift-verdict-stub` — renommés/scindés depuis `.sift-verdict-card` (le composant carte a été supprimé au redesign 07-06, `verdictCardHtml()` est un no-op), resynchronisé 07-24.
- L375 — Ligne candidat `.sift-cand` — hover discret volontaire (bordure seule).
- L389 — Bouton Destination `.sift-dest-btn` — hérite générique.
- L394 — Sliders volume/tempo `.sift-slider-*` — ⚠️ PÉRIMÉ : classes supprimées (08-21 puis 08-25) ; le volume vit dans « Lecteur simple » (L1906).
- L416 — Pochette/cover `.sift-cover-frame` — `alt` fixé 07-03, bug `[hidden]` réellement cassé fixé 07-05.
- L441 — Boutons icon-only — vérifiés, titlebar corrigée 07-03.
- L450 — Barre de progression `.pbar`/`.sift-pz-fill` — perf `transform` 07-03.
- L459 — Popover Destination `.sift-dest-popover` — CSS minimal, placement en JS : flip + recadrage viewport 08-13.
- L484 — Bouton Identifier `.sift-id-btn` — tokenisé+dark 07-03, exception 3ᵉ teinte levée 07-06.
- L501 — Bordure latérale `.sift-filed-banner` — anti-pattern side-stripe retiré 07-03.
- L510 — Ombres portées `.sift-toast`/`.sift-report-overlay-card` — tokenisées 07-03.
- L517 — Échelles hauteur/radius — audit 07-03, `--h-36` retiré 07-09 (0 lecteur).
- L565 — Token `disabled` de `Sift.dc.html` — vérifié non manquant.
- L584 — Autres couleurs non tokenisées — restant, pas classées bug ; ⚠️ 08-27 : deux lignes éteintes avec la waveform.
- L605 — `--text-hero` → `--text-2xl`.
- L617 — Cartes Réglages `.sift-settings-list` — refonte 4→1 carte 07-08.
- L661 — Zone de dépôt drag OS `.sift-dz-on` — token `--overlay-drop` 07-05.
- L685 — Lien rebuy Beatport `.sift-rebuy-btn` — créé 07-05.
- L701 — CTA « Revoir N morceaux → » Accueil — créé 07-05.
- L715 — Page Rekordbox `renderRekordboxLive()` — écran dédié + sections Tier 1/Tier 2 master.db.
- L798 — Écran Revue — zones repliables Diagnostic/Métadonnées, refonte 07-05.
- L857 — `.sift-applytags-btn` — déplacé header Genres 07-09.
- L878 — `.sift-zone-toggle` — accordéon exclusif + animation 07-09.
- L893 — Spectrogramme — légende incrustée + réticule interactif 07-09.
- L916 — `.lk` / `.lk-icon` — bug de réutilisation corrigé 07-07.
- L932 — Pastille segmentée `.sift-seg`/`.sift-seg-opt` unifiée — 6 sites, thumb glissant 07-08.
- L1039 — Grammaire de carte — 2 rôles (Groupée/Flottante), jamais 3 — 07-08 ; ⚠️ troisième décalage 08-27 (surfaces de Revue).
- L1102 — Tokens globaux — adaptation tweakcn "ZFlow" (ombres/tracking/radius/OKLCH) 07-08.
- L1130 — Écran Accueil — audit référence canonique 07-08.
- L1150 — Écran Revue — audit référence canonique 07-08/09.
- L1174 — Écran Écartés — audit référence canonique 07-09.
- L1192 — Écran Journal — audit référence canonique 07-09, conforme (rien corrigé).
- L1214 — Écran Bibliothèque — audit référence canonique 07-09.
- L1233 — Écrans Réglages+Rekordbox+Clé USB — audit référence canonique 07-09.
- L1253 — Pattern d'erreur/échec (`.sift-*-error`/`-fail`/`-warn`, 9 sites) — déjà cohérent, documenté ici (gap = défaut de doc, pas de code, audit 2026-07-19).
- L1312 — Écran Écartés — chargement + bouton "Réessayer" (07-24).
- L1323 — Écran Bibliothèque — chargement, tri en vue Grille, "Réinitialiser les filtres" corrigé (07-24).
- L1346 — Table Bibliothèque, colonne Verdict — RETIRÉE 09-08 (verdict dans l'inspecteur) ; 5 rendus de `verdictView()` 08-19.
- L1362 — Bibliothèque éditeur — suppression confirmée, borne Année, autocomplétion Genres (07-24).
- L1370 — Page Rekordbox — état d'erreur visible sur les 4 sections M8, boutons "en cours", CTA en `.sift-ranger-btn` (07-24).
- L1385 — Accueil — confirmation "Retirer", swatches `aria-pressed` (07-24).
- L1392 — Journal — titres de section datés lisibles (07-24).
- L1407 — Revue — bannières `role="status" aria-live="polite"`, légende "écarter" (07-24).
- L1415 — Lot — lignes de sélection accessibles au clavier, bouton "Annuler" sur confirmation armée (07-24).
- L1423 — `styles.css` — tokens `--color-text-warning`/`-success` clair recalibrés, hover réaffirmé (07-24).
- L1436 — Historique des corrections (chronologique, par date de session).
- L1618 — Conventions de cohérence (sémantique couleur, hiérarchie de poids, discipline classe partagée) — à consulter AVANT tout nouveau composant (07-24).
- L1688 — Ligne disque amovible (écran Clé USB) — trois états, rendu `usbRowHtml()` (07-31).
- L1723 — Teintes pleines `-solid` — neuf tokens pour les surfaces de donnée (08-01), dix depuis 08-27 (`red`, pastille de verdict).
- L1759 — Modale de formatage USB — états, trois corrections d'usage réel (08-02).
- L1793 — Menu contextuel `.sift-ctx-menu` — états catalogués + rangée de pastilles couleur de source (08-20).
- L1819 — Ligne de source du rail `.sift-rail-src` — teintes du cycle, `--error`, suspendue, « 0 audio » (#55), cadran de dépôt `.sift-rail-drop` (#56) ; story + module pur `rail-source-entry.ts` (08-20, 09-03).
- L1875 — Lecteur simple de Revue — rangée d'audition : slider kit, play 28, temps unique, volume fin ; module pur + story (08-27).
- L1926 — Pastille de verdict de file `verdictDot()` — teintes système pleines, 5 cas / 4 rendus ; module pur + story (08-27).
- L1951 — Surfaces de Revue — trois plans : rail en retrait, file bord à bord, cadre de lecture, pied en surface (08-27).
- L1994 — Carte de racine manquante du rail `.sift-railwarn` — remplace le bandeau `#sift-gate` supprimé ; états, survol en voile par-dessus l'ambre, rail replié ; module pur + story + vecteurs (09-02).
- L2024 — À re-sourcer · Corbeille — ex-Écartés, deux destinations du rail, table de Rangés, inspecteur Racheter (09-08).

## Ligne de queue — `.qi` (`styles.css:1127-1214`, revérifié au grep le 2026-08-27)

Rangée de la file de Revue, deux lignes de texte — titre, puis artiste en
`.qi-sub` — markup `queueRowHtml` concaténé dans la boucle virtualisée de
`renderQueueWindow` (`queue-panel.ts`).

| État | Sélecteur | Valeur (tokens — ils portent clair et sombre) |
|---|---|---|
| Normal | `.qi` | `color: var(--color-text-secondary)` |
| Hover | `.qi:hover` | `background: var(--color-row-active)` |
| Sélectionnée (piste ouverte en zone C) | `.qi.cur` | `background: var(--color-row-active)` + `color: var(--color-text-primary)` + `font-weight: 500` — aplat seul : le liseré gauche `box-shadow: inset 2px` a été retiré (commentaire « Left-edge inset shadow removed » vers `styles.css:1139`, annotation « la case est highlighted ça suffit » — même anti-pattern side-stripe que la bordure latérale L462) |
| Curseur clavier (08-26) | `#ql:focus-visible .qi.qi-kbd` | `outline: 2px solid var(--color-border-info)`, `outline-offset: -2px` — ANNEAU, distinct de l'aplat de `.cur` (sélection ≠ focus) ; le focus vit sur `#ql`, jamais sur la ligne (virtualisation) ; repli `#ql:focus-visible.ql-cursor-off` quand la ligne du curseur est hors fenêtre |
| Terminée | `.qi.done` | `color: var(--color-text-tertiary)` |
| Séparateur de rangées (08-27) | `.qi + .qi::before` | filet 1 px `--color-border-tertiary`, en retrait gauche (`--space-8`, aligné au contenu), peint en `::before` ABSOLU — hauteur 46 intacte (gelée par `test/queue-row-height.test.ts`), aucun nœud ajouté dans la boucle chaude de `renderQueueWindow`, et jamais de filet au-dessus de la première rangée (le filet borne ENTRE — le haut de zone est le rôle du filet de `.sift-qhead`) |
| Séparateur effacé (08-27) | `.qi:hover::before`, `.qi.cur::before`, `.qi:hover + .qi::before`, `.qi.cur + .qi::before` | `background:transparent` — le filet TOUCHANT la rangée survolée/ouverte s'efface (Mail récents : l'aplat arrondi remplace la borne) ; celui de la rangée elle-même ET celui de la suivante (son `::before` est le filet du bas de la rangée visée) |

Réécrite le 2026-08-27 : la table de 2026-07 citait des hex résolus de la
palette chaude (#5C554E, #F3EFE9, #C9C2B7…), absents de `styles.css` depuis
les gris froids système, et un liseré de sélection qui n'existe plus.
Séparateurs ajoutés le même jour (décision maquette du 26, portage #50). La
pastille de verdict est passée EN FIN DE TITRE (décision produit, assumée
contre le motif Mail des indicateurs au bord droit) : le sous-titre `.qi-sub`
s'aligne au titre, son indentation de 15px est partie.

**Interlignes explicites, hauteur constante (issue #45, 2026-08-26)** : titre
`12px/15px` (Callout du kit Big Sur), artiste `.qi-sub` en `--text-xs`/`13px`
(Caption 1, encre `--color-text-tertiary`). Hauteur dérivée : 15 + 2 + 13 +
2×`--space-8` = **46 px, constante** — condition de la virtualisation,
`measureQueueRowHeight` ne mesure qu'UNE hauteur et la met en cache pour toute
la file. Épinglé des deux côtés (règle CSS et sonde) par
`test/queue-row-height.test.ts`.

Sous-éléments : la pastille de verdict `verdictDot()` (voir sa section dédiée
en fin de fichier — teintes système pleines, story
`queue-verdict-dot.stories.ts`) · `.qi i` (`--text-base` — dimensionne les
icônes restantes de la rangée : alerte de conversion échouée, bouton
Réanalyser ; l'icône de verdict qu'il visait à l'origine n'existe plus) ·
`.qi .qi-dup` (pastille DUPLICATE neutre, `--overlay-selected` — un doublon
n'est pas un verdict, pas d'ambre) · `.qi-ck` (case du mode Lot). La rangée
entière n'a toujours pas de story (`queueRowHtml` vit dans `queue-panel.ts`,
qui importe `./ipc`) ; sa pastille en a une depuis le 2026-08-27.

## ⚠️ PÉRIMÉ — Mot de verdict Détail, `verdictWord()` retiré le 2026-08-26 (c4f65eb)

> Ses deux rendus de PIPELINE (« échec », « analyse… ») vivent depuis dans la
> pastille `verdictDot()` — section dédiée en fin de fichier.

| État | Condition | Rendu |
|---|---|---|
| Faux | `verdict === "fake"` | `"faux"`, `--color-text-warning` |
| À vérifier | `verdict === "grey"` | `"à vérifier"`, `--color-text-warning` |
| OK | `verdict === "ok"` | `""` (aucun mot), `--color-text-success` |
| **Échec** (nouveau) | pas encore de verdict **et** `analysis_attempts >= MAX_ANALYSIS_ATTEMPTS` | `"échec"`, `--color-text-warning` |
| En analyse | pas encore de verdict, tentatives restantes | `"analyse…"`, `--color-text-tertiary` |

✅ **Ajouté 2026-07-24** — avant ce fix, une piste ayant épuisé
`MAX_ANALYSIS_ATTEMPTS` (`shared/contracts.ts`) affichait encore "analyse…"
en mode Détail, indiscernable d'une piste réellement en cours d'analyse.
Distinction faite par comparaison à `analysis_attempts`, même logique que
`batch-panel.ts`'s `pendingRow()` utilisait déjà pour la même distinction en
mode Lot. Retry manuel par ligne conservé (`sift-live.ts:212-223`,
`reanalyzeTrack()`) — toast `"Réanalyse relancée"` sur succès
(`sift-live.ts:216`), `"Échec de la réanalyse : {détail}"` sur échec
(`sift-live.ts:219`). Retry de masse (`queue-panel.ts` `ensureQueueReanalyzeAllButton`)
gagne le même toast de confirmation sur succès :
`"{N} morceau(x) réanalysé(s)"` (`queue-panel.ts:523`), `"Échec de la
réanalyse — réessaie"` sur échec (`queue-panel.ts:528`).

## Item de navigation — `.nv` (`styles.css:183-192`)

| État | Sélecteur | Valeur |
|---|---|---|
| Normal | `.nv` | `color: var(--color-text-tertiary)` |
| Hover | `.nv:hover` | `background: var(--color-nav-active)` |
| Actif | `.nv.on` | `background: var(--color-nav-active)` + `color: var(--color-text-primary)` + `font-weight:500` |
| Export (variante) | `.nv-export` / `:hover` | `opacity:.55` → `.85` |

RAS.

## Bouton d'action principal — `.sift-ranger-btn` (`styles.css:388-389`, `filing.ts:192`)

| État | Source | Valeur |
|---|---|---|
| Normal | `.sift-ranger-btn` | `background: var(--color-background-info)`, `color: var(--color-text-info)` |
| Hover | `.sift-ranger-btn:hover` (déclaré explicitement, `styles.css:389`) | `background: var(--color-background-info)` + `filter:brightness(0.95)` |
| Disabled | **hérité de `button:disabled`** générique | `opacity:.4` |
| Focus | **hérité de `:focus-visible`** générique | outline 2px `var(--color-text-info)` |

✅ **Corrigé 2026-07-24** — jusqu'ici sans hover propre (bascule sur le gris
`button:hover` générique, cf. règle CLAUDE.md "un bouton qui redéfinit
`background` doit le réaffirmer dans son `:hover`"). `.sift-secondary-trash`
(`styles.css:391`) a reçu le même traitement (`background:
var(--color-background-danger)` + `filter:brightness(0.95)`), même famille de
boutons pleins. `.sift-confirm-btn` (`styles.css:1080`) a également gagné ce
hover dans la même passe. `.jrnl-revert:hover` (`styles.css:688`) recalé sur
`--color-border-secondary` (au lieu de `--color-border-primary`, token qui
n'existe pas — bordure invisible au hover) dans le même commit.

## Chip/tag — `.chip` (`styles.css:202`)

| État | Sélecteur | Valeur |
|---|---|---|
| Normal | `.chip` | `color: var(--color-text-secondary)`, bordure `var(--color-border-tertiary)` |
| Hover | `.chip:hover` | `background: var(--color-background-secondary)` |
| Sélectionné | `.chip.on` | `background: var(--color-background-info)`, `color: var(--color-text-info)` |
| Sélectionné + hover | `.chip.on:hover` | reste `var(--color-background-info)` (ne pas écraser l'état sélectionné) |
| Disabled | `.sift-chip-disabled` (classe séparée) | `opacity:.4;cursor:not-allowed` |

✅ Corrigé 2026-07-03 (était sans hover — cliquable sans retour visuel).

## Case à cocher — `.cbx`

✅ **Supprimée 2026-07-03** — c'était du code mort côté vraie app (aucun usage
dans `sift-live.ts`/`batch-tracklist.ts`, seulement dans `app.js`, la maquette
jetable). N'existe plus dans `styles.css`. Si un futur design montre une case à
cocher, elle est à construire, pas à réutiliser.

## Segmented control — `.sift-seg-opt` (`styles.css:542-544`)

| État | Sélecteur | Valeur |
|---|---|---|
| Normal | `.sift-seg-opt` | `color: var(--color-text-secondary)` |
| Hover | `.sift-seg-opt:hover` | `background: var(--color-row-active)` |
| Actif | `.sift-seg-opt.on` | `background: var(--color-surface-raised)` + `color: var(--color-text-primary)` |

RAS.

## Ligne de journal — `.lr.jrnl-row` (refonte 2026-08-19, `journal.ts` + `styles.css` § Journal d'actions)

⚠️ **`.jrnl-qrow` n'existe plus**, ni aucune autre classe de l'ancien Journal.
L'écran est passé à sa spec le 2026-08-19 (`docs/ui-specs/journal.md`) : une
table d'événements horodatés, groupée, avec un inspecteur en zone D. Le bloc
« REFONTE DU 2026-08-19 » en tête de la section Journal de `frontend/styles.css`
nomme chaque classe retirée et ce qui la remplace — résumé :

| Classe retirée | Ce qui la remplace |
|---|---|
| `.jrnl-qrow` / `-bar` / `-dot` | `.lr.jrnl-row` — la ligne de table de l'app, réutilisée telle quelle |
| `.jrnl-cat*` (catégories `<details>` FILÉS/JETÉS/REJETÉS) | `.jrnl-group*` — groupes de session, et de jour au-dessus en mode historique |
| `.jrnl-revert` (bouton par ligne) | l'inspecteur (zone D) et le menu contextuel — une colonne de boutons sur un historique long coûte de la largeur en permanence pour un usage rare |
| `.jrnl-mass` | le même bouton d'inspecteur, libellé « Annuler la sélection (N) » |
| `.jrnl-toast` / `.jrnl-banner` | le toast partagé `.sift-toast` |
| `.jrnl-voir-tout`, `.jrnl-mode`/`-btn`, `.jrnl-qmode` | le contrôle segmenté de la barre unifiée (`mountBarSegmented`, id `sift-jrnl-seg`) |
| `.jrnl-hd` | le titre de la barre unifiée (`router.ts::syncNav`) |
| `.jrnl-insp-card` | la zone D partagée `#sift-aside` (`toolbar.ts::openAside`) |
| `.jrnl-session-*` | `.jrnl-group-*` (deux niveaux au lieu d'un) |

**La ligne réutilise `.lr`** — hauteur (`--row-h`), survol, sélection neutre,
filet, focus : une seule grammaire de ligne pour Bibliothèque et Journal. Ne vit
dans la section Journal que ce qui lui est propre : colonnes, groupes, états.

| État | Sélecteur | Rendu |
|---|---|---|
| Normal | `.lr.jrnl-row` | heure en `--font-mono` tabulaire `--color-text-secondary`, piste en `--color-text-primary`, destination et état en `--color-text-tertiary` ; cellule État = « Appliqué » |
| Hover / focus | hérité de `.lr:hover` / `.lr:focus-within` | `background: var(--color-row-active)` |
| Sélectionnée | `.lr.jrnl-row.sel` | `background: var(--color-background-secondary)` — neutre, jamais un accent coloré ; `aria-selected` suit |
| Annulation en cours | `.jrnl-row--pending` | `.jrnl-c-state` en `--color-text-info` — **le seul état coloré, parce que le seul transitoire** ; la ligne et la table restent utilisables |
| Annulée (permanent) | `.jrnl-row--reverted` | piste et état en `--color-text-tertiary` — l'encre baisse d'un cran, elle ne s'éteint pas (aucune opacité) |
| Flash d'annulation | `.jrnl-row--flash` | `@keyframes jrnl-revert-flash` : `--color-background-success` → transparent en `--duration-base`, classe retirée à `animationend` (`journal.ts:659-660`) — **seule la transition se colore** |
| Échec | `.jrnl-row--failed` | piste, destination et état en `--color-text-danger` — un échec ne s'estompe jamais ; son motif se lit dans `.jrnl-insp-fail` |
| Chargement | `.jrnl-row--skel` + `.jrnl-skel` | squelette DANS la structure finale (mêmes colonnes, même hauteur de ligne), sans animation |

**Colonnes** — `.jrnl-c` + `.jrnl-c-time|-act|-track|-dest|-state`. Les trois
largeurs fixes sont déclarées une seule fois sur `.jrnl-wrap`
(`--jrnl-col-time:44px`, `--jrnl-col-act:68px`, `--jrnl-col-state:76px`, dérivées
du plus long contenu MESURÉ dans la vraie fenêtre le 2026-08-19) : hors de ce
wrapper, ces trois `width:var(…)` ne résolvent rien et la table se disloque.
`.jrnl-batch` porte la marque `×N` d'un lot. La destination tronque par la
GAUCHE (`direction:rtl` + `text-align:left`) et son chemin est enveloppé d'un
`<bdi>` **obligatoire** : sans isolation, l'algorithme bidi renvoie en fin de
ligne un segment initial neutre — « (2002) The Universal Sky » se peignait
« The Universal Sky … (2002) ». L'en-tête `.jrnl-thead` est collant, de même
matériau que `.sift-lib-thead` mais de classe distincte (le clic droit sur
`.sift-lib-thead` ouvre les réglages de colonnes, que le Journal n'a pas).

**Groupes** — `.jrnl-group--l1` (session) et `--l2` (session sous un jour, mode
historique, un seul cran d'indentation). L'en-tête `.jrnl-group-hd` est un vrai
`<button>` avec `aria-expanded`, portant `.jrnl-group-chev` (pivote en
`transform` seul), `.jrnl-group-label` et `.jrnl-group-count`. Replié, le corps
`.jrnl-group-body` est VIDÉ, pas caché.

**Zone D (inspecteur)** — `.jrnl-insp-title`, `.jrnl-insp-path`/`-pathval`,
`.jrnl-insp-fail`, `.jrnl-insp-note`, `.jrnl-insp-actions`, posés dans
`#sift-aside` et réutilisant `.col-h` / `.sift-sel-count` / `.sift-sel-rows` de
Bibliothèque. Trois contenus : résumé sans sélection, détail pour une entrée,
agrégat pour plusieurs. C'est lui qui porte « Annuler », plus la ligne.

**Autres états d'écran** — `.jrnl-noresult` (filtre sans résultat : reste SOUS
l'en-tête de colonnes, qui ne se retire jamais, sinon on emporte les commandes
qui permettent de défaire le filtre), `.jrnl-error` + `.jrnl-error-actions`
(lecture échouée : l'écran n'affirme RIEN du contenu, et propose « Réessayer »),
et l'état vide partagé `emptyStateHtml()` en deux textes selon le mode.

Stories : `frontend/journal-table.stories.ts` (« Journal — ligne de table »).

## Toggle switch — `.tog` (`styles.css:203`)

| État | Sélecteur | Valeur |
|---|---|---|
| On (défaut) | `.tog` | `background: var(--color-text-info)`, curseur `transform:translateX(13px)` |
| Off | `.tog.off` | `background: var(--color-border-secondary)`, curseur `transform:translateX(0)` |

✅ Retouché 2026-07-03 (audit Impeccable, perf) : le curseur bougeait via
`left`/`right` (propriété de layout, recalcul à chaque frame) — passé à
`transform:translateX()`. Comportement visuel identique, juste plus performant.
Toujours pas de hover/focus/disabled déclarés — composant discret, faible
priorité, pas classé bug.

## Slots verdict — `.sift-fil-verdict` / `.sift-verdict-stub` (renommé/scindé depuis `.sift-verdict-card`, catalogue resynchronisé 2026-07-24)

⚠️ **`.sift-verdict-card` n'existe plus** (0 occurrence dans `frontend/styles.css`,
vérifié par grep). Ce n'est pas juste un renommage de sélecteur : le composant
"carte verdict" tinté (fond coloré succès/danger/warning) a été **supprimé** au
redesign du 2026-07-06 — `verdictCardHtml()` (`report-view.ts:543-545`) est
aujourd'hui un no-op qui renvoie `""` (commentaire en place, `report-view.ts:528-542` :
la pastille de verdict pleine page faisait doublon avec le badge de qualité
tonalisé du panneau Diagnostic audio, et a été retirée intentionnellement — pas un
oubli). Ce qui reste sous ces deux sélecteurs, ce sont deux **slots** (conteneurs
vides remplis conditionnellement), pas une carte à tokens couleur :

- **`.sift-fil-verdict`** (`filing.ts:310`, requireEl `filing.ts:317`) — `<div>`
  créé par `openFilingInto()` dans le rail de classement (`.sift-fil-scroll`).
  Aucune règle CSS dédiée (mentionné seulement en commentaire, `styles.css:729`,
  pour son alignement de marge). Deux usages réels : (1) hôte des chips
  verdict-panel LOSSLESS/DUPLICATE/"LECTURE INCOMPLÈTE" injectées via
  `vchipHtml()` (`filing.ts:485-513`) ; (2) hôte du message d'échec d'analyse
  quand `verdictContainer` pointe dessus (`verdictHost()`, `report-view.ts:1141`,
  écriture à `report-view.ts:1204-1207`).
- **`.sift-verdict-stub`** (`styles.css:1055`) — vraie règle CSS (`display:flex;
  align-items:center;gap:6px;margin:2px 0 12px;font-size:var(--text-sm);
  color:var(--color-text-tertiary)`). Slot de repli créé par `report-view.ts`
  (`report-view.ts:1141`, `report-view.ts:1146`) uniquement quand aucun
  `verdictContainer` n'est fourni par l'appelant — cas `openReportModal`, qui n'a
  pas sa propre carte Identification/Verdict à côté.

Même rôle fonctionnel (slot loading/erreur pour l'analyse), deux sites d'usage
distincts avec un traitement CSS différent — d'où les 2 entrées plutôt qu'une
fusion. Pas de tokens couleur succès/danger/warning à documenter ici : la
tonalité (LOSSLESS/danger/warning) vit désormais dans le badge de qualité du
panneau Diagnostic (`qualityChipTone()`, `report-view.ts:549-553`, couleurs
`realQuality()` `report-view.ts:63-87`), pas dans un composant "carte verdict".

## Ligne candidat (identification) — `.sift-cand` (`styles.css:226-235`)

| État | Sélecteur | Valeur |
|---|---|---|
| Normal | `.sift-cand` | bordure `var(--color-border-tertiary)` |
| Hover | `.sift-cand:hover` | bordure `var(--color-border-secondary)` uniquement — **pas de `background`** |
| Erreur | `.sift-cands-error` (posée en JS) | `color: var(--color-text-warning)` |

Pas un bug — juste noté : le hover ici est délibérément discret (bordure seule,
pas de fond), cohérent avec une liste de résultats de recherche. Différent
pattern des lignes `.qi`/`.lr` (fond au survol ; `.jrnl-qrow`, cité ici jusqu'au
2026-08-19, a été remplacé par `.lr.jrnl-row`), à garder en tête pour ne
pas "corriger" par erreur vers l'uniformité lors d'un futur portage.

## Bouton Destination — `.sift-dest-btn` (`styles.css:564`, `filing.ts:185` et `:199`)

Même famille que `.sift-ranger-btn` — vrai `<button>`, hérite hover/disabled/focus
du sélecteur générique. RAS.

## ⚠️ PÉRIMÉ — Sliders (volume, tempo) `.sift-slider-*`, disparus en deux temps (08-21 puis 08-25)

Tempo & key-lock (l'« Écoute avancée ») ont quitté l'écran le 2026-08-21
(`f0ea751` — le pitch DJ n'est pas voulu sur un écran de décision) ; le volume
est passé par la capsule SVG du kit (2026-08-25, `e478623`, qui a supprimé les
règles `.sift-slider-*`) avant de devenir le slider fin `.sift-volume-*` du
2026-08-27 — voir la section « Lecteur simple » en fin de fichier. Seul
`dragSlider()` (report-view.ts) a survécu, réécrit sur la course du centre du
pouce ; sa classe `.dragging` est toujours posée mais n'a PLUS de règle CSS —
pas un oubli, le pouce du kit n'a ni scale de survol ni halo de drag.

| État (historique) | Sélecteur | Valeur |
|---|---|---|
| Normal | `.sift-slider-thumb` | `transform:translate(-50%,-50%) scale(1)` |
| Hover (survol de la track) | `.sift-slider-track:hover .sift-slider-thumb` | `scale(1.15)` |
| Drag actif | `.sift-slider-track.dragging .sift-slider-thumb` | `scale(1.3)` + halo `box-shadow:0 0 0 4px var(--overlay-selected)` |

✅ Corrigé 2026-07-03 — avant, aucun feedback visuel. La classe `.dragging` est
posée/retirée en JS (`mousedown`/`mouseup` dans `dragSlider()`, partagée par
volume et tempo) car le thumb a `pointer-events:none` (le drag est géré par la
track parente).

## Pochette / cover — `.sift-cover-frame` (`styles.css:356-357`, markup `report-view.ts` `playerHeaderHtml()`)

✅ Corrigé 2026-07-03 — `alt=""` (vide, sur une image de contenu réel, pas
décorative) remplacé par `alt="Pochette — {nom du morceau}"`, rempli
dynamiquement depuis le paramètre `name` déjà disponible dans la fonction.
`hidden` + pas de `src` au premier rendu reste volontaire (divulgation
progressive, pochette affichée une fois chargée) — pas une image cassée.

✅ **Corrigé 2026-07-05 — vraie image cassée trouvée et corrigée** (rapportée
via l'outil d'annotation Alt+Clic, voir Évaluation 12). Root cause : CSS, pas
réseau/backend. `.sift-player-cover{display:block}` (`styles.css:379`) n'avait
aucun garde `:not([hidden])` — une règle auteur bat toujours le user-agent
stylesheet (`[hidden]{display:none}`) quelle que soit la spécificité comparée,
donc poser `covEl.hidden = true` en JS n'avait **aucun effet visuel**. Résultat :
tout échec de chargement de la pochette (même transitoire) restait affiché comme
glyphe "image cassée" du navigateur **pour toujours**, par-dessus le vinyle,
puisque rien ne pouvait jamais le recacher. Vérifié en direct par CDP
(`getComputedStyle(img).display` restait `"block"` avec `hidden=true` avant
fix, bascule correctement `none`/`block` après). Fix : sélecteur changé en
`.sift-player-cover:not([hidden])`. Durci en même temps : `cover.rs` rejette les
téléchargements < 1 Ko (le "spacer" placeholder que Discogs sert parfois à la
place d'une vraie pochette) avant de les mettre en cache ; `filing.ts` pose un
`onerror` qui recache l'image en cas d'échec (inefficace avant ce fix CSS,
fonctionne maintenant).

## Boutons icon-only (lecture, lien Discogs, titlebar) — vérifiés

`sift-play-btn` (`report-view.ts:208`) a `title` + `aria-label`. Lien Discogs
icon-only dans la Bibliothèque (`sift-live.ts:1139`) a `aria-label="Page
Discogs"`. Le bouton "Voir la release" (`library-detail.ts:70`) a du texte
visible, pas besoin d'`aria-label`. ✅ Titlebar (`chrome.ts:122-124`, boutons
min/max/close) corrigée 2026-07-03 — n'avait que `title`, `aria-label` ajouté
sur les 3 boutons (seul manque trouvé dans cette catégorie).

## Barre de progression — `.pbar`/`.pfill` (`styles.css:149`) et `.sift-pz-fill` (`styles.css:135-136`, JS `progress-zone.ts`)

`.sift-pz-row.error .sift-pz-fill` bascule vers `var(--color-text-danger)` en
cas d'erreur — cohérent avec les tokens. ✅ Perf corrigée 2026-07-03 (audit
Impeccable) : `.sift-pz-fill` animait `width` (propriété de layout) — passé à
`transform:scaleX()` + `transform-origin:left`, `width:100%` fixe désormais.
`progress-zone.ts` mis à jour en conséquence (`style.transform` au lieu de
`style.width`, deux points d'écriture).

## Popover Destination — `.sift-dest-popover` (`styles.css:603`, `filing-bins.ts` + `popover-position.ts`)

Seul état géré **en CSS** : `[hidden]` (fermé). Pas de transition d'ouverture ni
d'état focus-trap dédié — minimal, et non classé bug.

Son **placement**, lui, est entièrement en JS : `position:fixed` sans repli CSS, donc
tout se joue dans `positionDestPopover`. Trois états de position, ajoutés le 2026-08-13
(issue #27) :

| état | déclencheur | résultat |
|---|---|---|
| **au-dessus** (défaut) | la hauteur tient au-dessus du bouton | `top = bouton.top − hauteur − 8` |
| **basculé en dessous** | ne tient pas au-dessus **et** il y a plus de place en dessous | `top = bouton.bottom + 8` |
| **recadré** | un bord sortirait de la fenêtre | ramené à 8 px du bord, sur les deux axes |

L'état basculé est **inatteignable dans la vraie fenêtre** : le bouton Destination vit
dans la barre d'action ancrée en bas, donc il y a toujours plus de place au-dessus. Il
est couvert par `test/popover-position.test.ts` précisément pour ça — sinon il partirait
en production sans avoir jamais été exercé. Le recadrage horizontal, lui, se déclenche
réellement : à la taille minimale déclarée (920×640), le bord droit tombait 73 px hors
fenêtre avant correction.

Deux points d'entrée, un seul chemin : mode Détail (`filing.ts:199`) et mode Lot
(`batch-panel.ts:663`) passent tous deux par `toggleDestPopover`.

## Bouton Identifier — `.sift-id-btn` (`styles.css:237-238`)

```css
/* Deliberate 3rd hue (gold), documented exception... */
.sift-id-btn{background:var(--color-accent-identify);border-color:var(--color-accent-identify-border);color:var(--color-accent-identify-text);...}
.sift-id-btn:hover{background:var(--color-accent-identify-hover)}
```

✅ **Corrigé 2026-07-03** — avant, doré (`#FFdc82`/`#f0cc6a`/`#1d1c1a`) en dur,
sans token, **sans variante sombre du tout** (même couleurs en mode sombre).
Contredisait la règle documentée en tête de fichier ("2 couleurs sémantiques
seulement, pas de 3e teinte"). Tokenisé (`--color-accent-identify`/`-hover`/
`-text`/`-border`, défini dans `:root` + les deux blocs sombres) et **gardé
comme exception documentée** — c'est le CTA de l'identification, pas un statut,
donc une 3e teinte assumée plutôt que supprimée. Commentaire inline explique le
choix pour la prochaine lecture.

## Bordure latérale colorée — `.sift-filed-banner` (`styles.css:478`)

✅ **Corrigé 2026-07-03** (audit Impeccable, anti-pattern) — avait un
`border-left:2px solid var(--color-text-success)`, ban explicite Impeccable
("side-stripe borders... never intentional", tell reconnaissable d'UI générée
par IA). Retiré, fond teinté (`background:var(--color-background-success)`)
conservé + `border-radius` ajouté pour ne pas perdre toute délimitation
visuelle.

## Ombres portées — `.sift-toast` (`styles.css:476`) / `.sift-report-overlay-card` (`styles.css:423`)

✅ **Tokenisées 2026-07-03** — `box-shadow` en `rgba(0,0,0,.4)`/`.5` en dur
remplacé par `var(--shadow-toast)`/`var(--shadow-overlay)`, définis dans
`:root` (mêmes valeurs, noir fixe volontaire, lisible dans les deux thèmes —
pas besoin de variante sombre pour une ombre).

## Échelles hauteur/radius (`/design-system audit`, 2026-07-03)

**Hauteur** (`--h-*`, `styles.css:47-48`) — les 4 tokens déclarés (32/36/40/44)
avaient **0 lecteur** (`grep -rn "var(--h-"` sur tout `frontend/` : aucun
match). `--h-32`/`--h-44` **supprimés** (aucun composant réel ne les utilise,
déclarés sans plan). `--h-36`/`--h-40` **gardés et câblés** sur leurs deux
vrais consommateurs : `.sift-play-btn` (`styles.css:348`, était `36px` en
dur) et `.jrnl-insp-revert` (`styles.css:591`, était `40px` en dur). Deux
autres 36px/40px existants (`.cov` avatar, `.sift-cand img/noart` vignette,
`styles.css:209`/`228`) **laissés en littéral, exprès** : ce sont des tailles
de cadre image/avatar, pas des hauteurs de contrôle — l'échelle ne couvre que
les boutons/contrôles interactifs.

**Mise à jour 2026-07-09 (audit Project Cleaner)** : `--h-36` a depuis perdu
son seul consommateur — la refonte Revue "surface continue" du 2026-07-06
(voir plus bas) a agrandi `.sift-play-btn` à une valeur littérale `46px`, sans
que ce paragraphe soit mis à jour en conséquence. Confirmé par grep (`var(--h-36)`
: zéro match) puis supprimé de `styles.css`. `--h-40` reste câblé sur
`.jrnl-insp-revert`, seul survivant de l'échelle hauteur.

⚠️ **Périmé depuis le 2026-08-19** : `.jrnl-insp-revert` est parti avec la refonte
du Journal (le bouton « Annuler » a quitté la ligne pour l'inspecteur). `--h-40`
est donc déclaré sans lecteur — vérifié, `var(--h-40)` a zéro occurrence dans
`frontend/` — et l'échelle hauteur n'a plus aucun consommateur. Le constat est
déjà porté par `styles.css` (commentaire du bloc des tokens de hauteur) et par
`docs/design-system/tokens.md` ; il n'est pas retouché ici, seulement rattaché à
sa cause. Même remarque pour les classes citées dans la partie **Radius**
ci-dessous (`.jrnl-cat-badge`, `.jrnl-qrow-dot`, `.jrnl-insp-dot`, `.jrnl-qmode`,
`.jrnl-qrow`, `.jrnl-cat`) : elles n'existent plus, seul le câblage
`var(--border-radius-pill)`/`-sm` des autres sites reste vrai.

**Radius** (`--border-radius-*`, `styles.css:29`) — `system.md` déclare 4
valeurs (sharp 4 / default 6 / soft 10 / pill 999) mais seuls `md`(6)/`lg`(10)
existaient. Ajouté `--border-radius-sm:4px` et `--border-radius-pill:999px`,
et câblés partout où la valeur littérale correspondait exactement : 3
occurrences de `4px` (`.sift-cand img/noart`, `.sift-bgrp-box`,
`.sift-time-elapsed`/`.sift-time-total`) → `var(--border-radius-sm)` ; 7
occurrences de `999px` (`.nv-export-dot`, `.nav-badge`, `.sift-genre-chip`,
`.jrnl-cat-badge`, `.sift-vchip`, `.jrnl-qrow-dot`, `.jrnl-insp-dot`) →
`var(--border-radius-pill)`.

**Reste hors scope, pas corrigé** : les valeurs qui ne correspondent à aucune
des 4 tailles déclarées (1px, 2px, 3px, 7px, 8px, 9px, 11px, 12px — ex.
`.jrnl-qmode`/`.sift-settings-seg`/`.jrnl-qrow` à `7px`, `.jrnl-cat` à `12px`)
n'ont pas été retouchées : les tokeniser demanderait d'étendre l'échelle
elle-même (décision de design, pas un simple câblage de token existant vers
un littéral identique).

## Token `disabled` de `Sift.dc.html` — vérifié non manquant (2026-07-04)

Investigation drift `Sift.dc.html`↔`styles.css` (docs/ressources-externes.md,
Évaluation 6) : sur les 17 clés de l'objet `theme()` (`Sift.dc.html:836-846`),
16 sont portées et câblées ; `disabled` (#CFC9BF clair / #57554D sombre)
semblait absente de `styles.css`. Vérification des 3 usages réels de
`T.disabled` dans la maquette (`Sift.dc.html:1135` dot "pending",
`1170`/`1210` barres de waveform inactives, `1339` fond de bouton primaire
désactivé) : **chacun a déjà un équivalent dans le vrai code**, avec un
mécanisme différent mais volontaire — dot "En analyse"
(`sift-live.ts:559`) réutilise `--color-text-tertiary` plutôt qu'une 5ᵉ
teinte neutre ; bouton désactivé hérite de `button:disabled{opacity:.4}`
(atténuation, pas couleur de substitution — voir `.sift-ranger-btn`
ci-dessus) ; waveform reste un canvas toujours sombre (voir note plus bas).
**Décision : ne pas ajouter de token `--color-disabled`** — aucun
consommateur réel n'en a besoin, cohérent avec le retrait de `--h-32`/`--h-44`
(audit du même jour, zéro lecteur). Cas classé non-bug, pas juste priorité
basse.

## Autres couleurs non tokenisées (audit complémentaire "tokens pour toutes les fonctions ?") — restant, pas classées bug

- `.sift-time-elapsed{color:#ff5500}` (`styles.css:371`) — orange en dur,
  horodatage sur la waveform, aucun token, aucune variante sombre. Pas
  corrigé — priorité basse, élément mineur sur canvas déjà fixe.
- Overlays waveform/spectrogramme (`rgba(255,255,255,.6)`, fond `#000`,
  badges temps `rgba(0,0,0,.55)`) — **volontaire** : ce canvas reste toujours
  sombre indépendamment du thème de l'app, comme un lecteur audio pro. Pas un
  manque de token, ne pas "corriger" à tort.
- `.tog::after{background:#fff}` — blanc en dur sur pastille colorée, mineur,
  pattern courant (curseur blanc sur fond coloré), pas de token nécessaire.

⚠️ **Deux des trois lignes sont éteintes depuis le 2026-08-27** (la waveform a
quitté Revue, `31c5d1a`) : `.sift-time-elapsed` n'existe plus (le temps unique
`.sift-time` est en tokens), et les overlays de WAVEFORM sont partis avec le
canvas — la bulle de survol survivante (`.sift-wave-hovertime`) est tokenisée
(`--overlay-scrim`/`--color-text-on-scrim`). Ce qui reste vrai : les overlays
du SPECTROGRAMME (canvas toujours sombre, `report-view.ts` peint en
`rgba(255,255,255,…)` — volontaire, inchangé). `.tog::after` est passé à
`--color-accent-ink` entre-temps : plus un blanc en dur.

## `--text-hero` → `--text-2xl` (échelle typo, 2026-07-03)

Token renommé, valeur inchangée (26px). Il s'appelait "hero" pour un rôle de
titre de morceau en 30px/600 (`system.md:103`) qui n'existe plus dans le
layout actuel — sa seule vraie utilisation (`library-detail.ts:57`) est la
taille de l'icône vinyle de repli dans le cadre pochette 72×72
(`library-detail.ts:59`), pas un titre. Renommé selon la convention de
l'échelle existante (`xs`→`2xl`) plutôt que de prétendre encore à un rôle
"hero" qu'il ne joue pas ; valeur gardée telle quelle (26px reste la bonne
taille visuelle pour cette icône, aucun changement rendu). Le vrai titre de
morceau (`.sift-player-name`) reste sur `--text-lg` (14px), non affecté.

## Cartes Réglages — `.sift-settings-list` (`styles.css`, structure `sift-live.ts` `renderReglagesLive()`)

**Refonte 2026-07-08** : avant, chaque section (Discogs/Bibliothèque/Apparence/
Clé USB) était sa propre `.sift-ui-card-soft` — 4 cartes empilées. Retour
utilisateur : "trop de boîtes" ; vérifié que chaque carte ne contenait
**qu'un seul réglage** — une boîte groupe des informations liées (règle HIG
"Boxes", voir spec `2026-07-07-hig-adaptation-design-spec.md`), en grouper
une seule n'ajoute que du chrome. Les 4 sections partagent maintenant **une
seule** `.sift-ui-card-soft` (id `sift-reglages-list`), séparées par un filet
horizontal (`.sift-settings-list-row`, bordure `--color-border-tertiary`
0.5px sur toutes les lignes sauf la première).

⚠️ **`.sift-settings-list-row` n'existe plus depuis le 2026-08-19/20** : le
filet a été retiré, et la classe avec lui — ses règles ont quitté
`frontend/styles.css` et l'attribut a quitté `reglages-view.ts`. Motif mesuré
dans la vraie fenêtre : depuis la colonne de catégories (étape 9),
`selectSettingsCategory` en cache trois sur quatre, mais `:not(:first-child)`
est structurel — un frère `hidden` compte encore. Bibliothèque, Nommage et
Apparence rendaient donc un `border-top` **au-dessus de leur titre**, Discogs
non : un séparateur qui ne séparait aucune paire visible et ouvrait le panneau.
Le rythme vertical vient maintenant de la carte seule
(`.sift-ui-card-soft-pad`), identique pour les quatre catégories. La ligne
« Ligne (section) » du tableau ci-dessous et la mention de la classe dans
« Structure DOM » sont périmées d'autant ; la règle qui tient, elle, est
inchangée : **toute nouvelle section s'ajoute à l'intérieur de `list`**.

| État | Sélecteur | Valeur |
|---|---|---|
| Boîte unique | `.sift-settings-list` sur `.sift-ui-card-soft` | fond `var(--color-background-secondary)`, bordure `var(--color-border-tertiary)` — une seule fois pour les 4 sections |
| Ligne (section) | `.sift-settings-list-row` | pas de fond/bordure propre — délimitée par un filet `border-top` sauf la première |
| Bouton (ex. "Changer…") | `.sift-settings-btn` | fond `var(--color-surface-raised)` |
| Bouton hover | `.sift-settings-btn:hover` | `filter:brightness(0.95)` |
| Lien "Oublier" | `.sift-settings-forget` | `color: var(--color-text-quaternary)` |
| Lien "Oublier" hover | `.sift-settings-forget:hover` | `color: var(--color-text-secondary)` |

**Structure DOM** : `renderReglagesLive()` construit un **wrapper unique**
`<div id="sift-reglages-live">` (règle 2026-07-04, inchangée) contenant
maintenant un second wrapper `<div id="sift-reglages-list" class="sift-settings-list sift-ui-card-soft sift-ui-card-soft-pad">`,
qui porte les 4 sections (`sift-reglages-discogs`/`-bibliotheque`/`-apparence`/
`-usb`, chacune `.sift-settings-list-row`). **Toute nouvelle section doit
s'ajouter à l'intérieur de `list`**, pas comme sibling direct de `wrap` ou de
`#content` — sinon elle redevient une carte isolée et le bug de duplication
2026-07-04 se reproduit sur un autre wrapper.

## Zone de dépôt drag OS — `.sift-dz-on` (`chrome.ts` `ensureDropStyle`/`setDropActive`)

Overlay de dépôt affiché **pendant un drag OS seulement** (jamais au repos —
`setDropActive` pose/retire `.sift-dz-on`). Style injecté une fois
(`ensureDropStyle`, un `<style>` partagé par les 3 zones). Chaque zone montre son
propre texte via `::after{content:attr(data-dz)}`.

| Élément | Valeur |
|---|---|
| Contour | `outline:1.5px dashed var(--color-text-info)` + `outline-offset:-4px` (affordance drop conventionnelle, gardée) |
| Voile `::after` | `background:var(--overlay-drop)` + `color:var(--color-text-info)`, centré |
| Zones cibles | `#filfoot` (→ nouvelle destination), `#ql` (→ file audio), `#sift-sources` (→ dossier surveillé) ; fallback `#content` |

✅ **Corrigé 2026-07-05** (chantier unification drop↔rail, grill-me) — le voile
était `rgba(20,20,24,.55)` **codé en dur, sombre, ignorant le thème clair chaud
actuel**. Remplacé par le token thème-aware `--overlay-drop`
(`styles.css`, `:root` + les 2 blocs sombres : clair `rgba(214,209,202,.93)`,
sombre `rgba(60,60,57,.93)` = base + ~10% tint info, à 93 % d'opacité). Le drop
adopte ainsi le langage info du rail (bordure+texte étaient **déjà** en
`--color-text-info`). Décisions grill-me : dashed gardé, même info pour les 3
zones, mouvement (pulse Destination au drop) **reporté**. Découvrabilité au repos
non ajoutée (drop reste drag-only, choix acté). Vérifié live par CDP :
`--overlay-drop` résout bien dans la vraie app Tauri.

## Lien rebuy Beatport — `.sift-rebuy-btn` (`styles.css:489`, `filing.ts` `refreshRebuyLink()`)

| État | Valeur |
|---|---|
| Absent | `.sift-rebuy` vide → `:empty{margin-bottom:0}`, aucun gap |
| Présent | `<button>` texte+icône, `background:var(--color-background-warning)` + `color:var(--color-text-warning)` + bordure `--color-border-tertiary`, `border-radius:var(--border-radius-md)`, pleine largeur |

Créé 2026-07-05. Gating strict : affiché **seulement** quand
`state.track.verdict === "fake"` **ET** `state.identified` (identité Discogs
appliquée) — chercher un nom de fichier brut est inutile. Container create-once
`.sift-rebuy` (après les genres dans `renderEditor`), rempli par
`refreshRebuyLink()` sur open / renderEditor / identify frais. Ouvre
`beatport.com/search?q=artiste+titre` via `openUrl` (commande Rust `open_url`,
http(s) uniquement, pas de whitelist domaine). Teinte ambre volontaire (cohérent
« le danger fusionne dans l'ambre »), pas de side-stripe.

## CTA « Revoir N morceaux → » — Accueil (`home-sources.ts` `listColumnHtml()`)

⚠️ **Composant disparu le 2026-08-19** : `home-sources.ts` est parti avec la fusion 1
(Accueil absorbé par le rail, commit `6d1cc85`), et les règles CSS `.sift-home-*` ont été
retirées le même jour (`813b83b`). L'entrée reste comme journal ; cliquer une source du rail
filtre Revue, ce qui remplace ce CTA.

Bouton pill dans l'en-tête de la colonne Sources, affiché **seulement quand**
`pending_count` cumulé sur toutes les sources > 0.
`background:var(--color-background-success)` + `color:var(--color-text-success)` +
`border-radius:var(--border-radius-pill)` — vert « prêt à revoir ». Clic →
dispatch un clic sur `[data-view="revue"]` (le pont Accueil→Revue). Créé
2026-07-05.

## Page Rekordbox — `renderRekordboxLive()` (`sift-live.ts`, `data-view="rkb"`)

Écran dédié (audit UI 2026-07-05, annotation « rekordbox = fonction d'export,
ce n'est pas le but ») remplaçant l'ancien comportement one-click nav → toast.
Déplacé depuis Bibliothèque (`rekordboxCardHtml()` n'y vit plus). Nav :
groupe renommé « Export » → « Intégrations », item Rekordbox passé de
`.nv-export` (puce ambrée, opacité .55) à `.nv` plein avec icône `ti-disc` —
même traitement que Bibliothèque/Journal. « Clé USB » reste `.nv-export`
inchangé (son propre brainstorm est à venir).

| État | Condition | Rendu |
|---|---|---|
| Non lié | `linked=false` | `empty-state.ts` (étendu ce chantier avec `actionHtml?`) : titre + note + bouton `data-bib="rkblink"` |
| Lié, sain | `linked=true, error=null` | Carte : chemin + compteurs, boutons « Réexporter maintenant » (`data-sift="rkbreexport"`) + « Changer de XML lié » |
| Lié, erreur | `linked=true, error≠null` | Carte : message illisible/corrompu, pas de bouton réexport (backend refuse déjà l'export) |
| Drift détecté | `drift_detected=true` | **Nouveau** — bannière `.sift-dup-banner` (fond `--color-background-warning`), **orthogonale** aux 3 états ci-dessus (peut s'afficher au-dessus de sain OU erreur, pas un `if/else if` à 4 branches). Signal backend existant depuis FIX-7, jusqu'ici invisible sauf en log serveur. |

`.sift-dup-banner-where` est conçu pour un chemin de fichier tronqué
(`nowrap`+`ellipsis`) — la bannière drift porte une phrase complète (tout le
message d'un warning auparavant invisible), donc override inline
`white-space:normal;overflow:visible;text-overflow:clip` sur cette instance
précise (trouvé en revue finale, corrigé avant merge).

Design/plan : `docs/superpowers/specs/2026-07-05-rekordbox-integration-page-design.md`,
`docs/superpowers/plans/2026-07-05-rekordbox-integration-page.md`. Construit
via subagent-driven-development (4 tâches, chacune approuvée + revue finale
de branche "Ready to merge"). `tsc --noEmit` clean après chaque tâche ;
vérification `tauri dev` par Antoine restante (code gated `inTauri`).

### Section réparations master.db (M8 Tier 1) — `masterdbRepairsSectionHtml()`

Ajoutée sous la carte de statut, **indépendante** de la bannière `drift_detected`
ci-dessus (signal XML existant — les deux ne sont jamais fusionnés en un seul
message, cf. spec). Visible seulement si `status.linked` et au moins une ligne
`pending`/`ambiguous` existe (sinon section absente, même règle que la bannière).

| Groupe | Condition | Rendu |
|---|---|---|
| Ambiguës (affichées en premier) | `status="ambiguous"` | Chemin avant→après + liste de boutons « Choisir cette piste — {chemin ou ID brut} » (`data-sift="mdbresolve"`) + « Ignorer » |
| Prêtes (pending) | `status="pending"` | Checkbox (`.sift-batch-ck`) + chemin avant→après + « Ignorer » ; barre « Appliquer la sélection (N) » sous la liste si ≥1 coché |
| Erreur d'application | ligne restée `pending` après un `apply_repairs` en échec | Message d'erreur humanisé en petit sous le chemin (état transitoire en mémoire, `mdbErrorById`, pas de colonne DB) |

Sélection (`mdbRepairSel`) **module-level, persistante entre rendus** (comme
`batchSel`, `sift-live.ts:271`) — refiltrée contre les lignes encore
présentes à chaque rendu, jamais réinitialisée en bloc. `confirmAction()`
obligatoire avant tout `apply_repairs` (jamais `window.confirm()`).

Design/plan : `docs/superpowers/specs/2026-07-06-m8-tier1-ui-screen-design.md`,
`docs/superpowers/plans/2026-07-06-m8-tier1-ui-screen.md`. Construit via
subagent-driven-development dans un worktree isolé (`dj-assistant-m8-tier1-ui`,
branche `m8-tier1-ui-screen`) — 4 tâches, revue finale (Opus) "Ready to merge",
mergé dans `m6a-discogs`. 276 tests Rust + `tsc --noEmit` clean ; vérification
`tauri dev` par Antoine restante.

### Section doublons de playlist (M8 Tier 2) — `playlistDuplicatesSectionHtml()`

Ajoutée sous la section réparations Tier 1 ci-dessus (même page Rekordbox),
**indépendante** — les deux sections coexistent, jamais fusionnées. Contrairement
à Tier 1, aucune persistance DB : scanné à la demande à chaque rendu
(`rekordboxMasterdbScanPlaylistDuplicates`, lecture seule), le résultat vit en
mémoire (`lastScannedDuplicateGroups`) pour que le clic référence le bon groupe.
Visible seulement si le scan retourne au moins un groupe (sinon section absente,
même règle show-nothing-when-empty que les autres sections Rekordbox).

| État | Condition | Rendu |
|---|---|---|
| Ligne de groupe | un groupe scanné | Nom de playlist (ou `Playlist {id}` si non résolu) + nom de fichier de piste (ou `Piste {id}`) + « N doublon(s) » + bouton « Dédupliquer » (`data-sift="mdbdedup"`), texte seul, pas de multi-sélection |
| Erreur de dédup | dédup échouée pour ce groupe | Message d'erreur humanisé en petit sous la ligne (état transitoire en mémoire, `mdbDedupErrorByKey`, clé `playlistId::contentId`, pas de colonne DB) |

Enrichissement backend display-only (`playlist_name`/`track_path`, ajoutés à
`PlaylistDuplicateGroupDto` après le câblage IPC de base) — jamais requis par
`dedup_playlist_group`, qui ignore ces champs (`From` inverse inchangé).
`confirmAction()` obligatoire avant tout `dedup_playlist_group` (jamais
`window.confirm()`), même pattern que Tier 1.

Design/plan : `docs/superpowers/plans/2026-07-08-m8-tier2-ui-screen.md`
(enrichissement backend), `docs/superpowers/plans/2026-07-08-m8-tier2-ipc-wiring.md`
(commandes IPC de base), `docs/superpowers/plans/2026-07-08-m8-tier2-playlist-dedup-rust.md`
(moteur). Construit via subagent-driven-development sur `m6a-discogs` directement
(pas de worktree isolé cette fois) — revues finales (Opus pour le moteur/IPC
d'écriture) "ready to merge" à chaque étape. `tsc --noEmit` clean ; vérification
`tauri dev` par Antoine restante (code gated `inTauri`).

## Écran Revue — zones repliables Diagnostic/Métadonnées (2026-07-05)

Refonte de `#mid` : la zone Diagnostic (`report-view.ts`, spectre + mesures)
et la zone Métadonnées (`filing.ts::renderEditor`, ex-« Identification ·
Discogs ») partagent maintenant un même mécanisme de disclosure — repliées
par défaut, badge de statut visible dans l'en-tête replié, caché déplié.

| État | Condition | Rendu |
|---|---|---|
| Badge qualité (Diagnostic) | zone repliée | `LOSSLESS` / `MP3 ≈ X kbps` dans l'en-tête, ton success/danger/warning selon `qualityChipTone(r)` |
| ~~Badge CDJ (Métadonnées)~~ | ⚠️ RETIRÉ 2026-08-25 | plus aucun badge sur ce header — voir le ⚠️ sous la table |
| Zone dépliée | clic sur l'en-tête | badge caché, corps affiché (`.sift-zone-toggle-body-open`) |
| Tag-warn (`.sift-tag-warn`) | ⚠️ PAS `!tags_cdj_ok` — écart aux tags saisis (`refreshDiscrepancy`) | bandeau nommant Artiste+Titre, `role="status"` ; le bouton Appliquer est parti le 2026-08-25 (gravure automatique au blur) |
| CTA Discogs | `c.artist && c.title` | `.sift-id-btn-neutral` (« Rechercher à nouveau ») au lieu du gold plein — le gold reste réservé à « rien identifié » |
| Sélection candidat | choix appliqué | flash bref sur `.sift-identified-line` (`.sift-identified-flash`), PAS un état permanent — la liste de candidats est remplacée par cette ligne, aucun `.sift-cand` ne survit à la sélection |

**Correction importante trouvée en cours de chantier** : `tags_cdj_ok`
(`tags.rs:73-76`) n'est PAS un signal de qualité audio — c'est littéralement
« Artiste+Titre déjà gravés dans les tags du fichier ». Le badge CDJ vit
donc désormais avec son critère et son fix (Métadonnées), pas avec le
diagnostic audio — retour en arrière volontaire par rapport à la décision
FIX-4 de `report-view.ts` (qui l'avait sorti vers les chips Preuves comme
différenciateur produit). En creusant `Sift.dc.html` pour la reconciliation
maquette, la maquette Claude Design portait déjà CDJ+ID3 dans Identification
depuis le début — FIX-4 avait donc dévié du design original, pas l'inverse.

> ⚠️ **Deux décisions datées ont dépassé ce texte — il garde sa forme d'origine,
> elles le corrigent :**
>
> **Badge CDJ retiré de l'UI le 2026-08-25.** Le passage de Métadonnées à un
> header STATIQUE (spec `docs/ui-specs/revue.md` § Zone C, direction B) a supprimé
> l'accordéon, et le badge qui n'existait que replié est parti avec. `tags_cdj_ok`
> n'a donc **aucun consommateur dans l'UI réelle** à ce jour — le seul rendu
> « CDJ compatible » qui subsiste est celui de `app.js`, la maquette, qui ne fait
> pas autorité (voir l'en-tête de ce fichier).
>
> **Critère backend recâblé le 2026-09-01** (issue #46). `tags_cdj_ok` ne teste
> plus la seule présence d'Artiste+Titre : il exige que ceux-ci vivent dans un
> tag d'un TYPE que la platine lit, jugé sur le couple conteneur × type contre
> `docs/cdj-metadata-formats.md` (`analysis/tags.rs::tag_type_readable_on_cdj`).
> Un WAV taggé RIFF INFO valait `true`, il vaut `false`. Le champ est donc juste
> aujourd'hui, et invisible — l'ordre inverse de ce qu'il était.
>
> **Le tag-warn n'a jamais été le badge.** La ligne « Tag-warn » de la table
> ci-dessus disait `!tags_cdj_ok` : c'est faux et ça l'était déjà. `.sift-tag-warn`
> est piloté par `refreshDiscrepancy` (`filing-identify.ts`), qui compare les
> champs AFFICHÉS au snapshot des tags du fichier — il ne lit pas le rapport
> d'analyse du tout.

Design/plan : `docs/superpowers/specs/2026-07-05-revue-screen-redesign-design.md`,
`docs/superpowers/plans/2026-07-05-revue-screen-redesign.md`. Prototype HTML
autonome itéré avant l'écriture du spec (jetable, jamais un livrable).
Construit via subagent-driven-development (4 tâches + revue finale « Ready to
merge with fixes », 3 minor fixés directement : docstring périmé, nom de
fonction obsolète en commentaire, CSS `.sift-highlight-flash` jamais câblée
— morte par décision, pas un oubli). `app.js` et `Sift.dc.html` réactualisés
dans la foulée (voir Historique ci-dessous) ; `tauri dev` restant à vérifier
par Antoine (code gated `inTauri`).

## `.sift-applytags-btn` — bouton Appliquer, header Genres (2026-07-09)

Déplacé du bas de carte (barre pleine largeur) vers le header "Genres"
(`.sift-genres-header`, `filing.ts::renderEditor`) — bouton compact,
`justify-content:space-between` avec le libellé "Genres". Reste dans le DOM
même quand rien à appliquer (`hidden`, pas omis du markup) : `onIdentityApplied`
doit pouvoir l'unhide + déclencher l'auto-apply sans un `renderEditor()` complet.

| État | Condition | Rendu |
|---|---|---|
| Caché | `!c.artist \|\| !c.title` (track pas identifiée) | `hidden` — `.sift-applytags-btn:not([hidden])` porte tout le style visuel, sinon un `display` d'auteur battrait silencieusement `[hidden]{display:none}` (même piège que `.sift-player-cover`, voir plus bas) |
| Idle, activable | `tagFieldDiffs().any === true` | fond/bordure pleins, texte `--color-text-secondary` (inline, `setApplyIdle`) |
| Idle, grisé | `tagFieldDiffs().any === false` | **PAS** `opacity` globale (le fond se noyait vers celui de la carte, illisible) — bg/border restent pleins, seul le texte passe `--color-text-tertiary` (inline, `refreshDiscrepancy`). `tagFieldDiffs` compare aussi `version` (fusionné dans le titre, cohérent avec l'écriture ID3 réelle) — éditer seulement la version dégrise correctement désormais |
| Appliqué | après succès `doApplyTags` | texte "Annuler", `btn.dataset.applied="1"` — `refreshDiscrepancy` ne touche jamais ce bouton dans cet état |

Auto-apply : identifier un titre Discogs déclenche `doApplyTags` automatiquement
(`onIdentityApplied`), plus besoin d'un second clic manuel. Badge CDJ
post-apply dérivé d'une vraie relecture du fichier (`trackFileTags`), pas
assumé depuis le succès de l'écriture — un échec d'écriture silencieux
n'affiche plus "CDJ compatible" à tort.

## `.sift-zone-toggle` (Diagnostic/Métadonnées) — accordéon exclusif + animation (2026-07-09)

Diagnostic audio et Métadonnées sont maintenant un accordéon exclusif (réf.
shadcn Accordion) : ouvrir l'un ferme l'autre. Coordination entre
`report-view.ts` et `filing.ts` (deux modules distincts, pas d'ancêtre commun
disponible) via un événement `document`-level `sift:accordion-open`, écouté
une seule fois au chargement du module (singleton ES) pour ne jamais
accumuler de listener sur les réouvertures de piste.

| État | Sélecteur | Rendu |
|---|---|---|
| Hover | `.sift-zone-toggle:hover .sift-zone-toggle-label` | texte-seul highlight (`--color-text-primary`), **pas** de case qui se superpose — `.sift-zone-toggle:hover{background:none}` réaffirmé explicitement (sinon le `button:hover` générique bat silencieusement la règle de base, même piège que `.sift-play-btn`, déjà documenté CLAUDE.md) |
| Bordure du cadre au survol | `.sift-spectro-box:has(.sift-zone-toggle:hover)`, idem `.sift-fil-editor.sift-fil-editor-margin` | `border-color` passe à `--color-border-secondary` — `:has()` cible l'ancêtre depuis l'état du bouton descendant |
| Ouverture/fermeture | grid `0fr`→`1fr` (même trick que Diagnostic avait déjà) | Métadonnées utilisait avant `display:none/block` sans transition — uniformisé. Padding sur un 3e wrapper imbriqué (`.sift-zone-toggle-body-pad`), jamais sur l'item que le track grid mesure directement : `overflow:hidden` ne zéro que la contribution de CONTENU au minimum automatique d'un track, jamais le padding porté par l'item lui-même — mis dessus deux fois par erreur (`#sift-meta-body` puis `.sift-zone-toggle-body-inner`), plancher resté coincé à 8px puis 16px au lieu de 0 |

## Spectrogramme — légende incrustée + réticule interactif (2026-07-09)

Remplace la ligne pointillée statique de cutoff (`report-view.ts::drawSpectrogram`)
par une légende permanente + un réticule au survol. Design :
`docs/superpowers/specs/2026-07-09-spectrogram-hover-crosshair-design.md`.
Plan : `docs/superpowers/plans/2026-07-09-spectrogram-hover-crosshair.md`.

| Élément | État | Rendu |
|---|---|---|
| Légende fréquence (haut-gauche) | permanent, dessinée une fois sur `.sift-spectro-canvas` | 3 paliers proportionnels à `nyquist` (jamais des kHz fixes), texte contour sombre+remplissage clair (`drawOutlinedText`) — lisible quelle que soit la couleur du spectrogramme sous le texte |
| Légende dB (haut-droit) | permanent | 6 paliers dérivés de `SPECTRO_GAIN_DB`/`SPECTRO_RANGE_DB` (0 à -100 dBFS), texte seul — pas de barre dégradée (testée en mockup, jugée peu claire) |
| Réticule | survol souris, `.sift-spectro-overlay` (2e canvas transparent superposé) | ligne horizontale+verticale pleine (pas pointillée), couleur claire fixe `rgba(255,255,255,0.85)` — **pas** un token thème-aware (`--color-text-secondary` s'assombrit en thème clair alors que le canvas reste toujours noir, trouvé en revue finale : réticule illisible en thème clair, le défaut de Sift) |
| Étiquette réticule | survol | pill sombre, `"{mm:ss} · {kHz} · {dB}"`, lus depuis `sg.mag_db` (même donnée que le pixel colorié) |
| Repos | `mouseleave` | overlay entièrement effacé, rien ne reste affiché — tout se découvre au survol |

Souris uniquement, pas d'équivalent clavier (canvas garde son `role="img"`/
`aria-label` statique). Construit via subagent-driven-development (3 tâches +
1 commit correctif post-revue-finale). Un finding hors-scope accepté sans
réécriture d'historique : le commit de la Tâche 3 (`ff1dc19`) embarque aussi
un mécanisme d'accordéon exclusif Diagnostic/Métadonnées développé plus tôt
dans la même session — voir `.superpowers/sdd/task-3-report.md` (gitignoré,
scratch de session) pour le détail.

## `.lk` / `.lk-icon` — bouton minimal, deux usages jamais mélanger (2026-07-07)

`.lk` (`styles.css`) est une boîte fixe 22×22px, centrée, sans padding —
conçue pour un **bouton icône seule** (lien Discogs, Identifier, Restaurer/
Corbeille dans Écartés). Elle était réutilisée par erreur pour des boutons à
**texte** ("Réexporter maintenant", "Changer de XML lié", "Lier un fichier
XML Rekordbox", "Résoudre") — le label se retrouvait compressé dans la boîte
22px et débordait en se chevauchant avec le contenu voisin (bug réel trouvé
via capture d'écran par Antoine, page Rekordbox non liée). Corrigé : `.lk`
renommée `.lk-icon` (mêmes propriétés), utilisée seulement sur les 4 vrais
boutons icône-seule ; tous les boutons texte retombent sur le reset `button{}`
de base (bordure, padding, hover — déjà correct partout ailleurs dans l'app).
**Règle à retenir** : `.lk-icon` = icône seule, taille fixe, jamais de texte
dedans ; un bouton avec label texte n'a besoin d'aucune classe particulière,
`button{}` suffit.

## Pastille segmentée — `.sift-seg`/`.sift-seg-opt`, composant unique (2026-07-08)

4 implémentations différentes coexistaient pour le même job ("choisir une
option parmi peu, exclusif") : Apparence (Réglages, déjà la bonne
référence) ; Format USB (FAT32/exFAT, réutilisait `.sift-seg-opt` pour la
pastille mais sans la piste encaissée — pastilles "à nu") ; Détail/Lot
(file d'attente Revue, **réimplémenté en styles inline** dans `sift-live.ts`,
dupliquant tout le composant en CSS-in-JS) ; Dossiers/Genres (Bibliothèque,
utilisait `.chip`/`.chip.on` — la grammaire filtre/tag, pas la grammaire de
choix exclusif). Unifiés en un seul composant :

| Élément | Classe | Rôle |
|---|---|---|
| Piste | `.sift-seg` | fond `--color-track`, radius 7px, padding 2px — encaisse les options |
| Option | `.sift-seg-opt` | `<span>` (texte seul) ou `<button>` (icône+label) ; `.on` = fond `--color-surface-raised` |

Sites : Apparence (Réglages), Format USB (`usb-format-modal.ts`), Détail/Lot
(`#sift-revseg`, `sift-live.ts` `ensureReviewSeg()`), Dossiers/Genres
(Bibliothèque, `sift-live.ts`), Session/Historique (Journal, `journal.ts`
`headerHtml()` — 6ᵉ site trouvé dans une 2ᵉ passe, `.jrnl-mode`/
`.jrnl-mode-btn`) — ⚠️ **périmé pour le Journal depuis le 2026-08-19** :
`headerHtml()`, `.jrnl-mode` et `.jrnl-mode-btn` n'existent plus, le contrôle
Session / Tout l'historique est monté dans la barre unifiée par
`mountBarSegmented()` (`toolbar.ts:139`, id `sift-jrnl-seg`). Même composant —
`.sift-seg.sift-seg-thumbed` > `.sift-seg-thumb` + `.sift-seg-opt` en vrais
`<button>` — seul le point de montage a changé. `#sift-revseg` garde une règle CSS propre
(`align-self:center;margin-bottom:10px`, positionnement dans `#qcol`) en plus
de `.sift-seg` — seul le positionnement reste spécifique au site, jamais le
style de piste/pastille. `white-space:nowrap` ajouté à `.sift-seg-opt` pour
les libellés longs du Journal ("Session courante"/"Tout l'historique").
`.jrnl-qmode`/`.jrnl-qmode-btn` (CSS mort, 0 consommateur trouvé, à ne pas
confondre avec `.jrnl-mode`/`.jrnl-mode-btn` — vraiment utilisées, elles)
supprimé dans le même geste.

**Thumb glissant — Détail/Lot seulement (2026-07-08)** : le crossfade
couleur/fond par bouton (`.sift-seg-opt.on`, transition 150ms) ne montrait
pas clairement un déplacement d'un état à l'autre. Ajouté `.sift-seg-thumb`,
un élément unique positionné en absolu (`transform:translateX()` +
`width`, transition 180ms) qui glisse physiquement vers le bouton
sélectionné — mesuré via `offsetLeft`/`offsetWidth` dans `ensureReviewSeg()`
à chaque toggle. Porte le fond/bordure/ombre (`--shadow-panel-subtle`,
seule pastille du composant à avoir une ombre — voir grammaire de carte
"Flottante" plus haut, exception assumée ici pour l'effet de profondeur du
thumb) ; `.sift-seg-thumbed .sift-seg-opt.on{background:none;border:none}`
annule le style "boîte" par-bouton pour ne pas dupliquer visuellement le
thumb. **Réservé à Détail/Lot** : c'est le seul des 6 sites où les boutons
persistent entre les changements d'état (mutation `classList` en place,
pas de reconstruction `innerHTML`) — un thumb sur les 5 autres sites
n'aurait rien à glisser, ils reconstruisent leur contenu (et donc perdent
toute mesure de position) à chaque changement.

**Format (MP3/AIFF/WAV) migré vers `.sift-seg`, thumb ajouté au rail Détail
uniquement (2026-07-08)** : 2 sites trouvés, tous deux en `.chip`/`.chip.on`
(pas `.sift-seg-opt`) — rail Détail (`filing.ts` `renderFoot()`, id
`#sift-fmt-seg`) et rail batch (`sift-live.ts`, `formatBlock`). Rail Détail :
même chirurgie que Détail/Lot — `renderFoot()` appelait un rebuild complet
sur chaque clic de format (`state.target` puis `renderFoot(...)` à nouveau),
détruisant les chips à chaque fois ; converti en mutation `classList` en
place (`positionFmtThumb()`, même mesure `offsetLeft`/`offsetWidth`).
`.sift-chip-disabled` (opacity/cursor) reste un modifier générique, réutilisé
tel quel sur `.sift-seg-opt` pour AIFF/WAV désactivés en mode lossy (aucun
`data-fil`, jamais `.on`, thumb ignore l'option — `onEl` reste `null`).
**Rail batch non thumbé, volontairement** : son clic (`renderBatchRail(...)`)
reconstruit tout le rail batch pour d'autres raisons (compteurs de
sélection), pas seulement le format — isoler ça en mutation en place
demanderait une restructuration plus large, hors scope de cette demande ;
converti en `.sift-seg`/`.sift-seg-opt` pour la cohérence visuelle
uniquement, garde son crossfade par bouton. `.sift-fmt-chips` (CSS,
0 consommateur après migration) supprimé.

**Bug de contraste sombre trouvé et corrigé** : `--color-track` et
`--color-surface-raised` valaient la **même couleur exacte** en sombre
(`#46453F`) — la pastille sélectionnée (fond `surface-raised`) était
invisible sur sa piste (fond `track`), seule la couleur du texte la
distinguait. En clair les deux tokens sont bien distincts. Vérifié que
`--color-track` n'a que 2 consommateurs (`.sift-slider-rail`, `.sift-seg`)
avant de changer sa valeur globale — nouvelle valeur sombre `#34332E`,
distincte de `surface-raised`. `.sift-seg-opt.on:hover` ajouté explicitement
(au lieu de compter sur l'ordre des règles, `:hover`/`.on` ayant la même
spécificité) pour que la pastille sélectionnée garde son fond au survol.

**Sweep élargi à tous les éléments sélectionnables/survolables (même
session)** : `.fld` (lignes dossier/genre Bibliothèque + lignes de bin
filing) n'avait **aucun hover** — même bug que `.chip` corrigé le
2026-07-03 ("cliquable sans retour visuel"), corrigé avec `.fld.on:hover`
explicite en prime. `.lr.cur:hover` ajouté explicitement (même fragilité
de spécificité que `.sift-seg-opt`). `.sift-bgrp-box`/`.sift-src-swatch`
vérifiés sans bug — logique d'anneau de sélection différente, pas de fond
à harmoniser.

**Hover texte-seul + transition animée (2026-07-08, retour utilisateur)** :
`.sift-seg-opt:hover` ne pose plus de fond (`--color-row-active`) — juste
un changement de couleur de texte (`--color-text-primary`), plus proche du
hover de Claude ("le texte qui highlight, pas une case qui se superpose").
`transition:background .15s ease,color .15s ease` ajoutée. Piège trouvé en
l'implémentant : une transition CSS n'anime que si l'élément **persiste**
entre les deux états — `ensureReviewSeg()` (Détail/Lot) reconstruisait les
2 boutons via `innerHTML` à chaque changement, donc rien à animer malgré la
transition posée. Corrigé : les boutons ne sont créés qu'une fois, les
appels suivants font juste `classList.toggle("on", ...)` dessus. Apparence
faisait déjà ça (mutation en place) donc anime aussi. Format USB/Dossiers-
Genres/Session-Historique **ne changent pas** — ils reconstruisent tout un
bloc de contenu dépendant (avertissement exFAT, liste de facette, lignes de
session), pas juste la pastille ; la reconstruction y est légitime, pas un
bug, et animer la pastille seule demanderait une restructuration plus
profonde pour un gain faible (contrôles peu re-cliqués en boucle).

## Grammaire de carte — 2 rôles, jamais 3 (2026-07-08)

Suite à la vérification de la règle Apple HIG "Boxes"
(developer.apple.com/design/human-interface-guidelines/boxes) : un groupe de
contenu utilise une **bordure ou un fond teinté**, jamais une ombre — l'ombre
signale une vraie élévation en z (quelque chose qui flotte AU-DESSUS d'autre
chose), pas du contenu inline. Avant cette passe, 3 traitements coexistaient
sans règle (ombre seule, bordure seule, les deux) ; ramené à 2 rôles :

| Rôle | Classes | Sites |
|---|---|---|
| **Groupée** (bordure ou fond teinté, jamais d'ombre) | `.sift-ui-card`, `.sift-ui-card-soft` | Spectrogramme (`.sift-spectro-box`), rail d'action (`.sift-action-rail`), carte lecteur (`.sift-player-row`), éditeur filing (`.sift-fil-editor-margin`), colonne queue (`#qcol`), colonne queue Accueil (`#homequeue`), Réglages, Bibliothèque, Journal (`.jrnl-insp-card`, bordure corrigée 1px→0.5px) |
| **Flottante** (bordure + ombre, réservée aux vraies superpositions) | `.sift-report-overlay-card` | Modales (confirmation, formatage USB) — seul site qui garde une ombre |

⚠️ **La colonne « Sites » de la ligne Groupée est périmée depuis le 2026-08-14** (issue #23).
Trois de ses sites — `.sift-spectro-box`, `.sift-player-row`,
`.sift-fil-editor.sift-fil-editor-margin` — n'ont **plus aucune surface** : ni fond, ni
bordure, ni rayon. Ce sont des surfaces de **contenu**, et la règle retenue est qu'une
surface est désormais la marque de la **charpente**. Restent Groupées : le rail d'action
(`.sift-action-rail`), la colonne queue (`#qcol`), la colonne queue d'Accueil (`#homequeue`),
Réglages, Bibliothèque, Journal.

⚠️ **Second décalage, du 2026-08-19** : le `.jrnl-insp-card` cité dans la colonne « Sites »
n'existe plus. L'inspecteur du Journal a rejoint la zone D partagée `#sift-aside`, qui n'est pas
une carte — ni fond, ni bordure complète, ni rayon, seulement un filet gauche
(`border-left:0.5px solid var(--color-border-tertiary)`). Le rôle **Groupée** du Journal se lit
donc désormais sur la charpente de la fenêtre, pas sur une carte à lui. La ligne du tableau reste
telle quelle, elle documente l'état d'avant, daté.

⚠️ **Troisième décalage, du 2026-08-27 (portage maquette, #50)** : quatre surfaces de Revue ont
bougé — le cadre de lecture `.sift-player-row` a RETROUVÉ une surface (exception consignée,
`patterns.md`), le pied de Détail est passé en surface sans carte (`.sift-action-rail--flat`),
la colonne file `#qcol` a quitté la famille carte (bord à bord), et le rail `.sb` est passé au
plan le plus en retrait. Détail : section « Surfaces de Revue — trois plans » en fin de fichier.
`#homequeue` (Accueil) garde sa carte, seul survivant bordé de la famille queue.

Les deux **rôles** ne changent pas — retirer une surface n'en crée pas un troisième, et la
grammaire à 2 rôles du 2026-07-08 tient. C'est la liste des sites qui a bougé, et cette ligne
est conservée telle quelle parce qu'elle documente l'état d'avant, daté. Détail et raison :
`docs/design-system/patterns.md` § Surface Continue.

**Harmonisation famille "queue" (2026-07-08)** : `#qcol` avait perdu sa
bordure lors d'une passe antérieure ("ajoute un cadre style pastille sans
bordure", fond teinté seul) tandis que `#homequeue` gardait la sienne, hors
scope de cette passe à l'époque — deux traitements divergents pour le même
rôle. Réunifiés : les 6 sites de la famille "queue"
(`.sift-spectro-box`/`.sift-action-rail`/`.sift-player-row`/
`.sift-fil-editor-margin`/`#qcol`/`#homequeue`) portent maintenant tous
`background:var(--color-background-queue)` + `border:0.5px solid
var(--color-border-tertiary)`, sans exception. `#qcol`/`#homequeue` partagent
désormais une seule règle CSS au lieu de deux déclarations séparées.

Une boîte n'entoure jamais un seul élément isolé (voir Cartes Réglages
ci-dessus) — un groupe implique plusieurs éléments liés, sinon c'est du
chrome sans fonction.

Contraste sombre atténué en même temps : `--color-border-tertiary` en sombre
`rgba(255,255,255,.12)` → `.09`, aligné sur la proportion du clair (`.09`
côté noir) — les bordures/filets fins étaient proportionnellement plus
marqués en sombre qu'en clair.

---

## Tokens globaux — adaptation tweakcn "ZFlow" (2026-07-08)

Suite au chantier audit-référence (Évaluation 19, `ressources-externes.md`) :
Antoine a demandé une adaptation intelligente du thème tweakcn "ZFlow" — technique
empruntée, valeurs propres à Sift, jamais un remplacement en bloc.

| Token | Avant | Après | Emprunt |
|---|---|---|---|
| `--shadow-panel-subtle`/`-toast`/`-overlay` | 1 couche `rgba(0,0,0,X)` | 2 couches (contact serré + diffusion large) | Technique ZFlow, couleurs/valeurs de base gardées |
| `--tracking-{normal,wide,wider,widest}` | littéraux `.01/.03/.05/.06em` dispersés sur 15 sites | tokens câblés sur les 14 occurrences qui recoupaient exactement (`.04/.08/.09/.1em`, un seul site chacun, laissés en littéral — même règle que l'audit radius 2026-07-03) | Concept d'échelle de tracking (ZFlow `tracking-*`), valeurs 100% Sift |
| `--border-radius-{sm,md,lg,pill}` | 4 valeurs indépendantes (4/6/10/999px) | dérivées par `calc()` d'une base unique `--border-radius-base` | Technique (base unique + `calc()`), deltas internes (base−6/base−4) gardés de Sift |
| `--border-radius-base` | — (n'existait pas) | `14px` (était `10px`/lg) | Valeur relevée sur demande explicite d'Antoine — plus arrondi |
| Couleurs `--color-*`/`--overlay-*`/`--color-hue-*` | hex/rgba | **mêmes couleurs**, reconverties en `oklch()` (script sRGB→OKLab→OKLCH, précis, pas à l'œil) | Notation seulement — zéro changement de teinte perçue |
| Police, palette de couleurs (teintes), échelle radius originale | — | **non touchés** | Contrediraient la palette Apple system colors (06/07), Outfit (marque) — décisions déjà validées, pas de gap trouvé |

Un bug de conversion trouvé et corrigé en cours de route : le script de
conversion OKLCH a d'abord altéré un commentaire de prose (`rgba(255,255,255,...)`
dans un texte explicatif, pas du CSS réel) — le regex alpha `[\d.]+` matchait
aussi une suite de points seuls. Repéré via `git diff` avant de continuer,
corrigé (`styles.css`, commentaire section "Neutral overlay tints").

Vérifié : `npx tsc --noEmit` clean, valeurs OKLCH parseées et résolues
correctement via un check navigateur (`getComputedStyle`, bascule clair/sombre,
`calc()` du radius résolu à 10px pour `md` comme attendu). Vérification
visuelle finale dans `tauri dev` par Antoine.

---

## Écran Accueil — audit référence canonique (2026-07-08)

Task 1 du chantier `docs/superpowers/changes/2026-07-08-ui-reference-audit/`.
Référence consultée : registre officiel shadcn (`new-york-v4`, MCP `shadcn`).

| Élément | Verdict | Détail |
|---|---|---|
| Scrollbar auto-hide | Conforme | Comparé à shadcn Scroll Area — écart (auto-hide vs toujours visible) volontaire, annoté par Antoine |
| Badge statut (`sm.color`→`sm.tone`) | **Corrigé** | Passé de fond neutre + texte teinté à fond réellement teinté par état (`sift-home-status-badge-{success,danger,info,neutral}`), réf. shadcn Badge "Custom Colors" |
| CTA "Revoir N →" / "+ Ajouter un dossier" | **Corrigé** | Fuite de hover : `button:hover{background:gris}` générique gagnait sur le fond inline teinté — classes dédiées (`sift-home-cta-revue`/`-add`) réaffirment le fond au survol |
| Checkbox "Surveiller ce dossier" | **Corrigé** | Aucune coche visible à l'état coché (ambigu), pas de clavier, `<div>` non focusable — coche `ti-check` ajoutée + `role="checkbox"`/`aria-checked` + Enter/Espace, réf. shadcn Checkbox |
| Nav rail (`.nv`) + lignes sources (`homerow`) | **Corrigé** | `<div>` sans `tabindex` ni `role` — accessibles au clavier maintenant (`installNavKeyboard()`, `chrome.ts`), réf. shadcn Sidebar. `app.js` (routage clic réel, non gated `inTauri` — voir `main.ts:6`) non modifié, le support clavier vient en supplément |
| Breadcrumb (`Accueil › {source}`) | **Corrigé** | `<div>` texte → `<nav aria-label="breadcrumb">` + `aria-current="page"`, réf. shadcn Breadcrumb — zéro changement visuel |
| Badge "With Spinner" | Différé | Pas de signal backend "scan en cours" sur `Source` (`shared/contracts.ts`) — primitive notée pour Task 2 (Revue, analyse/filing) et Task 4 (Journal, revert), où un vrai état async existe |

Vérifié : `npx tsc --noEmit` clean après chaque édit. Vérification visuelle
(clavier, hover, badge) dans `tauri dev` par Antoine.

---

## Écran Revue — audit référence canonique (2026-07-08/09)

Task 2 du chantier `docs/superpowers/changes/2026-07-08-ui-reference-audit/`.
Référence consultée : registre officiel shadcn (MCP `shadcn`).

| Élément | Verdict | Détail |
|---|---|---|
| `zoneToggleHtml` (Diagnostic/Métadonnées) | Conforme | `aria-expanded` déjà correct, réf. shadcn Collapsible |
| Badges verdict (`.sift-vchip`/`.sift-chip-badge`) | Conforme | Déjà le pattern Custom Colors (fond teinté), réf. shadcn Badge |
| Candidats Discogs (`sift-cand`) | Conforme | Vrais `<button>` + `<details>/<summary>` natif, accessible par défaut |
| Chips genre (`sift-genre-chip`) | Conforme | Display-only (pas cliquable), `<span>` correct |
| Sliders Volume/Tempo (`report-view.ts`) | **Corrigé** | Zéro `role="slider"`/`aria-valuenow` avant fix — drag-only, aucun clavier. Ajout `role="slider"` + `aria-valuemin/max/now` (tenus à jour en live) + flèches clavier (Volume ±5%, Tempo ±1%, Home/End), réf. shadcn Slider (Radix) |
| Toggle Key-lock | **Corrigé** | `<button>` déjà focusable mais sans `aria-pressed` — ajouté, synchronisé dans `refreshKey()` |
| Canvas spectrogramme | **Corrigé** | `role="img"` + `aria-label="Spectrogramme audio"` ajoutés (mineur) |
| Arbre de destination (`.fld`, `filing.ts`) | **Corrigé** | Même motif qu'Accueil : `<div data-fil="bin">` sans clavier — `tabindex`/`role="button"` ajoutés, `installNavKeyboard()` (chrome.ts) étendu pour couvrir `[data-fil="bin"]` |
| Overlay de confirmation (`confirm-modal.ts`) | **Corrigé** | Ni `role="alertdialog"`/`aria-modal`, ni focus déplacé à l'ouverture, ni Escape — seul le clic sur le fond annulait. Utilisée avant toute action destructive (règle CLAUDE.md anti-`window.confirm()`), donc corrigée en priorité : les 3 ajoutés, réf. shadcn Alert Dialog |
| Barre de progression (`.sift-pz-fill`, `progress-zone.ts`) | **Corrigé** | Aucun `role="progressbar"`/`aria-valuenow` — ajoutés, `aria-valuenow` mis à jour dans le fast-path de tick (pas seulement à la création de la ligne), réf. shadcn Progress |

Vérifié : `npx tsc --noEmit` clean après chaque édit. Vérification visuelle
(clavier sliders/arbre/modale, lecteur, spectrogramme) dans `tauri dev` par
Antoine restante.

---

## Écran Écartés — audit référence canonique (2026-07-09)

Task 3 du chantier `docs/superpowers/changes/2026-07-08-ui-reference-audit/`.
Écran compact, peu de composants — la plupart déjà conformes.

| Élément | Verdict | Détail |
|---|---|---|
| Boutons icône Restaurer/Corbeille (`.lk-icon`) | Conforme | Vrais `<button>`, `title`/`aria-label` déjà présents |
| Bouton Restaurer (corbeille) | Conforme | Texte visible + `title`, déjà focusable |
| Bouton Copier / Purger | Conforme | Vrais `<button>` |
| Chips raison (`sift-vchip`) / pills compteurs | Conforme | Display-only, `<span>` correct |
| Liens boutique (`ecStoreLinks`) | **Corrigé** | C'étaient des `<a data-ec="store">` **sans `href`** — non focusables, aucune activation clavier possible. Convertis en `<button>` (le handler délégué `sift-live.ts:2088` est agnostique du tag, `[data-ec]` générique) — cohérent avec le bouton "Copié" juste à côté |

Vérifié : `npx tsc --noEmit` clean. Vérification visuelle (clavier sur les
liens boutique) dans `tauri dev` par Antoine restante.

---

## Écran Journal — audit référence canonique (2026-07-09)

Task 4 du chantier `docs/superpowers/changes/2026-07-08-ui-reference-audit/`.
**Aucune divergence trouvée** — écran déjà conforme sur toute la ligne :
`<details>/<summary>` natif pour les catégories (FILÉS/JETÉS/REJETÉS,
accessible par défaut), vrais `<button>` partout (revert par ligne,
mass-revert, mode Session/Historique — déjà `.sift-seg` unifié le
2026-07-08), toasts déjà `aria-live="polite"`/`"assertive"`.

⚠️ **Cet audit décrit un écran qui n'existe plus** (refonte du 2026-08-19, voir
« Ligne de journal — `.lr.jrnl-row` » plus haut) : plus de `<details>/<summary>`,
plus de bouton d'annulation par ligne ni de mass-revert en pied de liste, plus de
toast propre au Journal. Les acquis d'accessibilité sont tenus autrement, et
c'est vérifiable dans `journal.ts` : en-tête de groupe = vrai `<button>` avec
`aria-expanded` (donc Entrée/Espace natifs), lignes en `role="option"` +
`aria-selected` + nom composite (`aria-label` : heure, action, piste,
destination, état), clavier ↑ ↓ ⇧ Début Fin ⌘A parcourant l'ordre VISIBLE (un
groupe replié ne fournit aucun voisin), menu contextuel dont les entrées
inapplicables sont DÉSACTIVÉES et non retirées.

---

## Écran Bibliothèque — audit référence canonique (2026-07-09)

Task 5 du chantier `docs/superpowers/changes/2026-07-08-ui-reference-audit/`.

| Élément | Verdict | Détail |
|---|---|---|
| Facettes Dossiers/Genres (`.fld`, `data-bib="pick"`) | **Corrigé** | Même motif que partout ailleurs — `tabindex`/`role="button"` ajoutés, clavier via `installNavKeyboard()` étendu |
| Segmented Dossiers/Genres (`data-bib="facet"`) | **Corrigé** | `<span>` → `<button>` — incohérent avec le reste de l'app où `.sift-seg-opt` est toujours un vrai bouton |
| Chips filtre qualité (`.chip`, `qual`/`dupscan`) | **Corrigé** | `<span>` → `<button>` — `.chip` avait déjà ses propres `border`/`padding`/hover réaffirmé, zéro changement visuel attendu |
| Ligne de piste (`.lr`, `data-bib="row"`) | **Corrigé** | Ligne cliquable (ouvre le détail) sans `tabindex`/`role` — ajoutés. **Piège trouvé et corrigé avant application** : la ligne contient un vrai `<button>` (lecture) imbriqué — sans garde, Entrée sur ce bouton aurait aussi déclenché le clic de la ligne parente (double action : lire + ouvrir/fermer le détail). `installNavKeyboard()` ignore désormais tout `keydown` dont la cible est déjà un élément natif interactif (`button`/`a`/`input`/`select`/`textarea`) — protège rétroactivement Accueil/Revue aussi |
| Recherche (`#bibq`) + 5 champs éditeur (`library-detail.ts`) | **Corrigé** | Placeholder seul (pas une vraie étiquette accessible) — `aria-label` ajouté sur les 6, réf. shadcn Field |
| Boutons icône lien/identifier (`.lk-icon`) | Conforme | Déjà de vrais `<button>` avec `aria-label` |

Vérifié : `npx tsc --noEmit` clean. Vérification visuelle (clavier facettes/
lignes/chips, double-déclenchement sur le bouton lecture) dans `tauri dev`
par Antoine restante.

---

## Écrans Réglages + Rekordbox + Clé USB — audit référence canonique (2026-07-09)

Task 6 du chantier `docs/superpowers/changes/2026-07-08-ui-reference-audit/`.

| Élément | Verdict | Détail |
|---|---|---|
| Segmented Apparence (`data-theme-choice`) | **Corrigé (G1)** | `<span>` → `<button>` — incohérence habituelle |
| Mojibake USB ("amovibles�", "d�tect�") | **Corrigé** | Trouvé en passant (pas un audit-ref à proprement parler) — encodage cassé sur 2 messages d'état de `renderUsbList()`, restaurés (« … » et « détecté ») |
| Jeton Discogs, boutons dossier racine/USB | Conforme | Vrais `<button>`, toggle œil avec `aria-label` synchronisé |
| Carte statut Rekordbox + bannière drift | Conforme | Boutons réels, structure `Card`/`Alert` déjà correcte |
| Ligne sélection réparations master.db (`.bx-row`, `data-sift="mdbpick"`) | **Corrigé (G3)** | Ligne-checkbox (case interne `tabindex="-1"` volontaire) sans clavier — `tabindex`/`role="checkbox"`/`aria-checked` ajoutés, clavier via `installNavKeyboard()` étendu. Bouton "Ignorer" imbriqué déjà protégé par la garde anti-double-déclenchement (B1) |
| Boutons candidats/dédup/apply (`mdbresolve`, `mdbdismiss`, `mdbdedup`, `mdbapply`) | Conforme | Vrais `<button>` |
| Modale formatage USB (`usb-format-modal.ts`) | **Corrigé (G2)** | La seule action vraiment irréversible de toute l'app — n'avait **aucune** sémantique modale (pire que R5/confirm-modal.ts). Ajout `role="alertdialog"`/`aria-modal` + Escape (désactivé pendant `busy`, pour ne pas interrompre un formatage en cours) + segmented FAT32/exFAT `<span>`→`<button>`. Fuite corrigée au passage : le listener `keydown` global n'était retiré que sur Escape — consolidé en une seule fonction `close()` appelée aussi par Annuler et le succès du formatage |

Vérifié : `npx tsc --noEmit` clean. Vérification visuelle (clavier ligne
master.db, Escape modale USB pendant/hors formatage, segmented) dans
`tauri dev` par Antoine restante.

---

## Pattern d'erreur/échec — `.sift-*-error`/`-fail`/`-warn` (audit 2026-07-19)

Investigation suite à un audit design qui signalait un gap : ni `empty-state.ts`
ni `progress-zone.ts` ne couvrent l'erreur, et aucun `error-state.ts` n'existe.
Vérifié sur pièce (`frontend/styles.css` + grep `frontend/*.ts`) : **le gap
n'est qu'un défaut de documentation, pas de code** — un pattern d'erreur
cohérent existe déjà, dispersé sur 9 sites mais réutilisant systématiquement
les 2 mêmes couples de tokens sémantiques (`--color-text-warning`+
`--color-background-warning` pour un échec récupérable/en attente,
`--color-text-danger`+`--color-background-danger` pour une action destructive/
irréversible) — la règle est déjà écrite en toutes lettres en tête de
`styles.css:6-9` : *"Color = meaning: green (ok/lossless), amber (doute/
pending/erreur), red (danger), blue (info)"*.

| Site | Classe | Sélecteur/valeur | Sévérité |
|---|---|---|---|
| Ligne batch (`batch-tracklist.ts:24`) | `.sift-bt-fail` | `color:var(--color-text-warning)` | warning |
| Candidats Discogs (`library-detail.ts:194/221`, `filing.ts:599/651`) | `.sift-cands-error` (+ `.sift-cand-error-icon`) | icône `ti-alert-triangle` + `color:var(--color-text-warning)` | warning |
| Bandeau tags non gravés (`filing.ts:885`) | `.sift-tag-warn` | fond+bordure+texte `--color-*-warning` | warning |
| Player (`report-view.ts:416/722`) | `.sift-player-error` | `color:var(--color-text-warning)` | warning |
| Codec (`report-view.ts:605`) | `.sift-codec-error` | `color:var(--color-text-warning)` | warning |
| Échec d'analyse (`report-view.ts:1199`) | `.sift-analysis-fail` | `color:var(--color-text-warning)` | warning |
| Formatage USB (`usb-format-modal.ts:76`) | `.sift-usbfmt-error` / `.sift-usbfmt-warning`/`-exfat-warning` | `error` = `color:var(--color-text-danger)` (irréversible) ; `warning` = ambre | danger (error) / warning |
| File d'attente (`.sift-pz-row.error`, `progress-zone.ts`) | `.sift-pz-fill` sur ligne `.error` | `background:var(--color-text-danger)` | danger |
| ~~Toasts/bannières Journal~~ → Journal (`journal.ts`, refonte 2026-08-19) | `.jrnl-error` (lecture échouée) · `.jrnl-insp-fail` (motif d'une annulation échouée) · `.jrnl-row--failed` (la ligne) — `.jrnl-toast--warn`/`.jrnl-banner--warn` n'existent plus, les confirmations passent par le toast partagé `.sift-toast` | `color:var(--color-text-danger)` | danger |
| Overlay modal (`styles.css:1052`) | `.sift-report-overlay-error` | `color:var(--color-text-danger)` | danger |
| Racine non définie, rail (`rail-root-warning.ts`, issue #54 du 2026-09-02) | `.sift-railwarn` | fond+texte `--color-*-warning`, mêmes tokens que `.sift-tag-warn` | warning |

⚠️ **Le Journal a changé de sévérité le 2026-08-19**, et c'est délibéré : ses
échecs sont peints en `danger` là où l'ancien écran les peignait en `warning`.
La raison est écrite dans `styles.css` (« un échec ne s'estompe jamais ») et
tranche pour la LISIBILITÉ de l'échec plutôt que pour la lecture stricte du
couple warning/danger de cette section, où `danger` désigne l'irréversible. Une
annulation qui échoue n'est pas destructive — elle est simplement l'information
que l'écran n'a pas le droit d'atténuer. À trancher pour de bon si un troisième
site adopte la même lecture ; noté ici pour que l'écart ne passe pas pour une
étourderie.

**Convention structurelle constante** (visible sur les 6 sites qui affichent
un message, pas juste une couleur de pastille) : `<div class="sift-{site}-{error|fail|warn}">`
contenant une icône `<i class="ti ti-alert-triangle">` + le message — pas de
composant JS partagé (`error-state.ts` n'existe pas), mais le HTML est
recopié à l'identique site par site, jamais réinventé visuellement.

**Décision (audit 2026-07-19) : ne pas créer `error-state.ts`.** Contrairement
à `empty-state.ts` (vrai doublon de logique JS avant extraction) ou
`progress-zone.ts` (état complexe avec transitions), chaque site d'erreur ici
est un `<div>` statique conditionnel dans un template déjà spécifique au
composant (candidat Discogs, ligne batch, modale USB...) — extraire un
`error-state.ts` commun n'apporterait aucune réduction de duplication réelle
(le HTML fait 1 ligne par site) et ajouterait un niveau d'indirection pour un
gain nul. Le vrai invariant à préserver n'est pas un composant partagé, c'est
la règle de token déjà en vigueur : **warning = récupérable/en attente,
danger = irréversible/destructif** — jamais l'inverse, jamais une 3ᵉ teinte.
Un futur site d'erreur doit suivre cette règle, pas inventer une nouvelle
couleur ni un nouveau composant.

---

## Écran Écartés — chargement + réessai (2026-07-24) — `ecartes-view.ts:91-118`

| État | Condition | Rendu |
|---|---|---|
| **Chargement** (nouveau) | `renderEcartes()` invoqué, avant résolution de `listEcartes()` | icône `ti-loader` (`sift-spin`) + "Chargement…", `--color-text-tertiary` (`ecartes-view.ts:105-108`) — même pattern que `bibliotheque-view.ts` (voir plus bas) |
| Échec | `listEcartes()` rejette | Message d'erreur + bouton **"Réessayer"** (nouveau, `data-ec="retry"`) qui relance `renderEcartes()` (`ecartes-view.ts:116-124`) |
| Chargé | résolution réussie | Liste re-sourcer/corbeille normale |

RAS sur le reste de l'écran (déjà catalogué dans la section audit référence
canonique Écartés plus haut).

## Écran Bibliothèque — chargement + tri Grille + réinitialisation (2026-07-24) — `bibliotheque-view.ts`

| État | Condition | Rendu |
|---|---|---|
| **Chargement** (nouveau) | `renderBiblioLive()` invoqué, avant résolution du `Promise.all` (liste + facettes + stats) | icône `ti-loader` + "Chargement…" (`bibliotheque-view.ts:156-158`) |
| Échec | une des 3 requêtes rejette | Message d'erreur, pas de retry dédié (`bibliotheque-view.ts:168-174`) |

✅ **Tri désormais appliqué en vue Grille aussi** — `sortedTracks =
sortTracks(bibState.tracks, bibState.sort)` (`bibliotheque-view.ts:231`) est
maintenant la source commune des deux branches de rendu : vue Tableau
(`bibliotheque-view.ts:309-316`, `items: sortedTracks`) **et** vue Grille
(`bibliotheque-view.ts:318-324`, `gridRows` construit en tranchant
`sortedTracks`). Avant ce fix, seule la vue Tableau consommait `sortedTracks` ;
la Grille lisait l'ordre brut de `bibState.tracks`.

✅ **"Réinitialiser les filtres" corrigé** — le bouton affiché sur "Aucun
résultat pour ce filtre" (`data-bib="stat" data-stat="all"`,
`bibliotheque-view.ts:280`) est câblé sur le handler délégué `sift-live.ts:299-307` :
`stat === "all"` remet désormais à `undefined` les 6 champs de
`bibState.filter` (`quality`, `verdict`, `q`, `folder`, `genre`, `artist`),
recherche et facette comprises — avant ce fix il ne couvrait que
`quality`/`verdict`.

## Table Bibliothèque — colonne Verdict (2026-08-19) — RETIRÉE le 2026-09-08 — `library-views.ts`

Colonne 1 de la table du 2026-08-19 au 2026-09-08 : pastille pleine + libellé (LOSSLESS /
AUTHENTIQUE / FAKE / À VÉRIFIER / —), tranchée contre les littéraux réels `ok` / `fake` /
`grey` / NULL de `worker.rs::verdict_str`, triable par rang catégoriel, entrée dans le
système de colonnes (`library-columns.ts`).

**Retirée le 2026-09-08** — audit de Rangés contre Revue (#24), décision d'Antoine : « pas
besoin de mettre le verdict ». Le verdict se lit dans l'inspecteur à l'ouverture
(`report-view.ts::verdictWordTone`, même paire de faits : verdict sain ET rail lossless) ; le
filtre Lossless / MP3 de la barre reste le seul geste de qualité sur la table. `verdictView()`,
`verdictRank()` et la story `library-verdict.stories.ts` sont supprimées avec elle ; un
stockage `sift-libcols-v1` qui la porte encore est filtré (`test/library-columns.test.ts`).
Spec : `docs/ui-specs/bibliotheque.md` § Décision 2026-09-08. Historique du rendu :
`git show 5a8a9ed:frontend/library-views.ts` (dernier commit qui la porte).

## Bibliothèque éditeur — confirmation suppression + bornes Année + autocomplétion Genres (2026-07-24) — `library-detail.ts`

| Élément | État | Détail |
|---|---|---|
| Suppression de piste | confirmation | `doTrash()` (`library-detail.ts:369-389`) passe désormais par `confirmAction("Envoyer ce morceau à la corbeille ? Annulable via Ctrl+Z.", "Envoyer à la corbeille")` avant tout appel `trashTrack` — bouton désactivé pendant l'appel (`btn.disabled = true` avant l'appel, `= false` dans le `finally`) |
| Champ Année | bornes | `<input type="number" min="1900" max="2100">` (`library-detail.ts:101`) |
| Champ Genres | autocomplétion | `<input list="sift-genre-list">` + `<datalist id="sift-genre-list">` (`library-detail.ts:98-99`), rempli depuis les genres déjà connus de la Bibliothèque (`library-detail.ts:168-186`, fetch dégradé silencieusement en liste vide sur échec — le champ texte libre reste utilisable) |

## Page Rekordbox — sections M8 : état d'erreur, boutons "en cours", CTA teintés (2026-07-24)

| Élément | État | Rendu |
|---|---|---|
| **Erreur de chargement** (nouveau, 4 sections) | `rekordbox_masterdb_pending_repairs`/`..._scan_playlist_duplicates`/`..._pending_metadata_syncs`/`..._pending_artwork_syncs` rejette | `sectionErrorHtml()` (`rekordbox-view.ts:148-154`) : "Impossible de charger — réessaie plus tard.", `--color-text-danger` — remplace la section, plutôt que de la faire disparaître silencieusement. Câblé aux 4 sections (`rekordbox-view.ts:584-622`) |
| Bouton "Appliquer la sélection" (Tier 1) | en cours | `disabled` + texte `"Application…"` (`rekordbox-view.ts:729-730`) |
| Bouton "Dédupliquer" (Tier 2) | en cours | `disabled` + texte `"Fusion…"` (`rekordbox-view.ts:768-769`) |
| Bouton "Appliquer" (Tier 3 metadata/pochettes) | en cours | `disabled` + texte `"Application…"` (`rekordbox-view.ts:854-855`, `953-954`) |

✅ **CTA teintés** — "Réexporter maintenant" (`rekordbox-view.ts:193`),
"Appliquer la sélection" (Tier 1/2/3, `rekordbox-view.ts:275/381/476`) et
"Dédupliquer" (`rekordbox-view.ts:524`) utilisent désormais `.sift-ranger-btn`
au lieu du `button{}` générique — se distinguent visuellement comme CTA
principaux plutôt que texte seul.

## Accueil — retrait de source + swatches couleur (2026-07-24) — `home-sources.ts`

| Élément | État | Détail |
|---|---|---|
| "Retirer" un dossier surveillé | confirmation | `confirmAction("Retirer ce dossier surveillé ?", "Retirer")` (`home-sources.ts:244`) avant `removeSource()` |
| Swatch de couleur de source | sélection | `aria-pressed="${on}"` + `title`/`aria-label="Couleur {label}"` en français (`home-sources.ts:157`) sur chaque `<button data-sift="setsrccolor">` |

## Journal — titres de section datés (2026-07-24) — `journal.ts`

| Élément | Avant | Après |
|---|---|---|
| Titre de section d'historique (vue étendue) | ID de session brut | `formatSessionLabel()` (`journal.ts:367-378`) dérive `"Session du {jj}/{mm} {hh}h{min}"` depuis le timestamp encodé dans l'ID (`{millis}-{pid}`) ; fallback sur l'ID brut si le format est inattendu ou la date invalide |
| Badge de catégorie (Session) | — | `"{N} actions"` (`journal.ts:115`), déjà présent au format compteur, RAS |

⚠️ **Renommé et dédoublé le 2026-08-19** : `formatSessionLabel()` s'appelle
`sessionLabel()` (`journal.ts:166`) et rend deux formes selon le niveau de
groupe — « Session du jj/mm/aaaa hhhmm » au niveau 1 (mode session, où la date
situe), « Session de hhhmm » au niveau 2 (sous un en-tête de jour, en mode
historique, où la répéter serait du bruit). Le repli sur l'ID brut est conservé,
et la dérivation depuis la partie `{millis}` de `{millis}-{pid}` aussi. Le
compteur « N actions » est passé sur `.jrnl-group-count`.

## Revue — bannières accessibles (2026-07-24)

| Élément | État | Détail |
|---|---|---|
| Bannière "Converti" (`showFiledConfirm`, `filing-actions.ts:105-124`) | affichée après un rangement | `role="status"` + `aria-live="polite"` (`filing-actions.ts:113-114`) |
| Bandeau avertissement tags non gravés (`.sift-tag-warn`, `filing-identify.ts`) | ⚠️ PAS `!tags_cdj_ok` : écart entre les champs affichés et le snapshot des tags du fichier, calculé par `refreshDiscrepancy` — il ne lit pas le rapport d'analyse (corrigé 2026-09-01) | `role="status"` + `aria-live="polite"` déjà sur l'élément ; visibilité par `style.display` seul |
| Légende clavier (`report-view.ts:355`) | permanent | dit désormais **"écarter"** (`BKSP`), plus "jeter" — cohérent avec le renommage de terminologie du 2026-07-10 (voir Historique) |

## Lot — lignes accessibles au clavier + Annuler (2026-07-24) — `batch-panel.ts`

| Élément | État | Détail |
|---|---|---|
| Ligne de sélection prête (`readyRow`, `batch-panel.ts:249-266`) | sélectionnable | `tabindex="0"` + `role="checkbox"` + `aria-checked="${on}"` sur `.bx-row` (checkbox interne `tabindex="-1"`, cohérent avec le pattern déjà utilisé pour `.bx-row[data-sift="mdbpick"]` sur la page Rekordbox) |
| Ligne de sélection "faux" (`fakeRow`, `batch-panel.ts:292-306`) | sélectionnable | même traitement clavier |
| Confirmation armée (Convertir → Confirmer) | armée | bouton **"Annuler"** ajouté (`data-sift="batchcancelconfirm"`, `batch-panel.ts:456`) à côté de "Confirmer — convertir N ?" — sortie explicite en plus du désarmement silencieux à 5s (`batchConfirmTimer`) |

## `styles.css` — tokens couleur clair + hover manquants (2026-07-24)

- **`--color-text-success`/`--color-text-warning` (bloc clair `:root`,
  `styles.css:25`)** — assombris (`L 49.19%→44%` / `L 53.34%→48%`, teinte/chroma
  inchangées) : plusieurs paires texte/fond réelles du thème clair tombaient
  sous le seuil AA. Le bloc sombre (`:root[data-theme="dark"]` et le média
  `prefers-color-scheme:dark`) n'a pas bougé dans ce commit.
- **`.sift-ranger-btn:hover`/`.sift-secondary-trash:hover`** réaffirmés — voir
  section "Bouton d'action principal" plus haut pour le détail (`.jrnl-revert:hover`
  et `.sift-confirm-btn:hover` corrigés dans le même commit).

---

## Historique des corrections

**2026-07-05 (pochette réellement cassée + sélection multi Alt+Clic)** :
signalée via l'outil d'annotation, corrigée après reproduction en direct par
CDP (voir section Pochette/cover ci-dessus pour le détail). En parallèle,
l'outil d'annotation lui-même a gagné la sélection multi-éléments (répéter
Alt+Clic accumule au lieu de remplacer) pour porter le contexte de plusieurs
zones dans une seule note.

**2026-07-05 (refonte écran Revue + réconciliation des 2 maquettes)** : voir
section dédiée ci-dessus. `app.js` réactualisé et vérifié en direct
(preview_*, partage `frontend/styles.css` — mêmes classes réelles
réutilisées). `Sift.dc.html` réactualisé (nouvel état `metaOpen`, pattern
`openChip` suivi) mais **non vérifié par rendu** — ce format propriétaire a
besoin de React+ReactDOM injectés (l'éditeur Claude Design le fait, pas un
navigateur nu).

**2026-07-05 (chantier 3 prompts : CTA Accueil, lien rebuy, unification
drop↔rail)** : ajout du CTA « Revoir N → » (Accueil), du lien rebuy Beatport
(Revue, gated fake+identifié), et tokenisation du voile de dépôt
(`--overlay-drop`, remplace un `rgba` sombre codé en dur). 3 nouvelles entrées
composant ci-dessus. `tsc --noEmit` clean, vérifié live par CDP (vraie app
Tauri, token `--overlay-drop` résout). Audit `/impeccable audit filing.ts` :
15/20, mes ajouts propres (0 finding détecteur), findings a11y chips/arbre
pré-existants non traités (chantier `harden` séparé).

**2026-07-03, passe 1 (audit design-system, 6 bugs)** : `.sift-id-btn` (3e
teinte + dark), carte verdict (tokens), `.chip` (hover), sliders (hover/drag),
pochette (`alt`), `.cbx` (suppression code mort).

**2026-07-03, passe 2 (`/impeccable audit`, 4 findings)** : bordure latérale
`.sift-filed-banner`, `aria-label` titlebar (`chrome.ts`), transitions
`width`/`left`/`right` → `transform` (`.sift-pz-fill`, `.tog::after`), ombres
portées tokenisées.

**2026-07-03, passe 3 (`/design-system audit`)** : correction de dérive de
ligne (`.sift-pz-fill` 133-134 → 135-136, décalé par un ajout antérieur) ;
échelle hauteur (`--h-32`/`--h-44` supprimés — 0 lecteur —, `--h-36`/`--h-40`
câblés sur `.sift-play-btn`/`.jrnl-insp-revert`) ; échelle radius (`sm`/`pill`
ajoutés, câblés sur 10 sites où le littéral correspondait exactement) ;
`--text-hero` renommé `--text-2xl` (rôle "hero"/track-title jamais tenu,
seul usage réel = icône de repli `library-detail.ts:57`).

Vérification : `npx tsc --noEmit` clean après chaque passe. Composants CSS
purs (non gated `inTauri`) vérifiés par inspection de style calculé via
`preview_eval`. `chrome.ts` (gated) reste à confirmer visuellement dans
`tauri dev` — changement d'attribut seul, régression improbable mais non vue.

**Reste ouvert, priorité basse** : `.sift-time-elapsed` non tokenisé (mineur,
voir section ci-dessus).

**2026-07-06 (rollout palette Apple system colors, 11 tâches)** : danger et
info séparés en vraies teintes rouge/bleu (au lieu de partager l'ambre) ; 5
teintes catégorielles ajoutées (chips de genre, points de source) ; nav
(items + badges) recolorée ; bouton Identifier replié dans info (la 3ᵉ teinte
dorée documentée ci-dessus, section "Bouton Identifier", n'est donc plus
d'actualité — l'exception est levée, plus de 3ᵉ teinte du tout) ; couleur
« elapsed » de la waveform déplacée vers info (`--color-waveform-elapsed`,
`var(--color-text-info)`) ; 2 popovers (Destination, candidats Discogs) ont
gagné un fond flouté. Rationale complet, palette et mapping teinte-par-teinte :
`docs/superpowers/specs/2026-07-06-apple-system-colors-palette-design.md` —
non répété ici.

Vérification de cette passe (tâche finale du plan, sweep seulement) :
`npx tsc --noEmit` clean ; `cargo clippy --all-targets -- -D warnings` clean ;
`cargo test` 261/269 verts, les 8 échecs sont tous
`RekordboxRunning`/message équivalent — Rekordbox tournait réellement sur la
machine au moment du run (garde-fou process de M8 qui fonctionne comme prévu),
sans rapport avec le rollout couleurs ; grep sweep confirme `color-accent-identify`
et les anciens hex danger/ambre partagés (`#8f6318`/`#f2c274`) absents de
`frontend/`/`src-tauri/`.

**Trouvailles mineures non corrigées dans cette tâche (sweep documentaire
seulement, triage laissé à la revue finale de branche)** : un commentaire près
de `.nv-export-dot` (`styles.css`, section "Export (Rekordbox/Clé USB)") décrit
encore Rekordbox comme "pas construit"/point seul, périmé depuis que Rekordbox
a son propre item de nav avec icône colorée (tâche 7) ; un commentaire près de
`--color-waveform-elapsed` prétend encore que "le canvas reste sombre quel que
soit le thème, donc pas de variante sombre", plus exact depuis que ce token
est thème-aware (`var(--color-text-info)`, tâche 9 — voir juste au-dessus).

**2026-07-07 (M8 Tier 1 écran UI + bug `.lk`)** : section réparations
`master.db` livrée sur la page Rekordbox (voir plus haut). Bug réel trouvé
par capture d'écran (Antoine) sur la page Rekordbox non liée : `.lk` (bouton
icône 22×22 fixe) réutilisée pour des boutons texte, labels compressés et
qui se chevauchent avec le texte voisin. Corrigé : `.lk` → `.lk-icon` (4
vrais boutons icône-seule), boutons texte retombent sur `button{}` (voir
entrée dédiée ci-dessus).

**2026-07-10 (audit cohérence palette + interactions, `styles.css` + 8
fichiers frontend)** : demande initiale de vérif deltas de contraste, élargie
en 3 passes successives (retour utilisateur à chaque étape) :

1. **Contraste badges/chips** — les 8 tokens `--color-background-{info,
   danger,success,warning}` + `--color-hue-{indigo,purple,pink,teal}-bg`
   étaient en alpha sur fond variable ; mesuré jusqu'à 3.37:1 (échoue AA
   4.5:1) sur les cartes les plus sombres. Passés en couleurs pleines, même
   teinte, résolus à ~5:1 partout (clair + sombre).
2. **Pastille segmentée `--color-track` en sombre** — plus claire que son
   environnement (32.04% entre bg-primary 27.57% et bg-tertiary 31.64%) au
   lieu du neutre le plus sombre comme en clair. 3 itérations avec retour
   utilisateur (32.04%→22.5%→25%, puis unifié sur `var(--color-background-
   primary)` pour ne plus avoir de valeur séparée à retuner).
3. **Cohérence teinte/chroma des gris sombres** — 3 clusters de hue
   distincts (92°/99°/107°) au lieu d'une seule direction comme en clair ;
   unifiés sur H=77.5° (la teinte dominante du clair), chroma resserrée.
4. **3 couleurs codées en dur retrouvées** (violaient la règle "tokens
   obligatoires") : `.sift-wave-hover` (blanc fixe), `.sift-kbd-hint-id`
   (noir fixe, illisible en sombre — 1.38:1 mesuré), `--color-text-on-accent`
   (jamais défini, fallback `#fff` caché).
5. **Champs éditables incohérents** — `.sift-editor-input` (Revue/
   Bibliothèque) utilisait un fond plus CLAIR que sa carte (flotte) au lieu
   du renfoncement du champ Jeton Discogs (fond plus sombre) ; unifiés sur
   `--color-background-primary` partout. Bug de spécificité trouvé au passage
   : le champ Jeton avait son style inline dupliqué, empêchant le focus de
   s'appliquer — migré vers la classe partagée.
6. **Focus des inputs texte** — l'outline générique offset (2px + 1px offset)
   jugée trop dure au clic ; remplacée par un simple changement de couleur de
   bordure (pas de box-shadow ajouté — un premier essai avec halo a été jugé
   "2 bordures").
7. **Audit suite ("check tout")** : `::selection` totalement absent (bleu
   navigateur non thémé) → ajouté. Checkboxes natives ne matchaient AUCUNE
   règle focus (ni générique ni celle des inputs) — au moins une vraie case
   (`filing.ts`, toggle "Sur place") tombait sur l'outline navigateur brut ;
   règle dédiée ajoutée (1.5px, plus fine que le générique). 10 widgets de
   type checkbox recensés au total (6 natifs + 4 `role="checkbox"` custom) ;
   les boutons désactivés (4 patterns différents dans le fichier) vérifiés un
   par un, tous lisibles.
8. **Renommage terminologie "Ranger"→"Convertir"** (retour utilisateur,
   "englobe tout ce que fait le bouton") — 20 chaînes utilisateur dans 8
   fichiers frontend ; identifiants internes (`doRanger`, `data-fil="ranger"`,
   `.sift-ranger-btn`) volontairement inchangés.

2 régressions introduites puis trouvées par le même audit de cohérence avant
tout commit : `--overlay-wave-hover` posé seulement dans les 2 blocs sombres
(oublié en clair au moment de sa création) ; diff systématique des noms de
tokens entre les 3 blocs de thème confirmant qu'aucune autre asymétrie ne
subsiste. Tout vérifié en direct sur le vrai process Tauri (CDP, port 9222,
`Emulation.setFocusEmulationEnabled` pour que `:focus-visible` matche sous
automatisation) dans les deux thèmes, pas seulement lu dans le code.

**2026-07-18 (contraste text-tertiary/quaternary WCAG AA)** : les deux tokens
échouaient AA (2.84/1.71 en clair contre bg-primary ; 2.40 en sombre contre
surface) alors qu'ils portent du texte signifiant (sous-labels, noms de
fichier mono, horodatages du Journal) — trouvé via l'audit CDP sur l'app
réelle (`docs/superpowers/changes/2026-07-18-ux-user-flow/audit-heuristique-visuel.md`,
finding F1). Assombris en clair / éclaircis en sombre (même teinte/chroma,
`L` seul) pour atteindre ≥4.5:1 partout où ils sont réellement peints, dans
les 3 blocs de thème.

**2026-07-18 (cartes de synchro Rekordbox : "indisponible" vs "à jour")** :
les 4 cartes ("Fichiers"/"Métadonnées"/"Pochettes"/"Playlists") affichaient
"à jour" dès qu'il n'y avait rien en attente, même quand le XML lié était
illisible — contredisant le bandeau rouge "XML Rekordbox illisible"
au-dessus. Trouvé via l'audit CDP sur l'app réelle (finding F3,
audit-heuristique-visuel.md). Elles lisent désormais "indisponible" dans ce
cas, via un cache `lastLinkStatus` partagé par les 4 sections.

**2026-07-24 (session de fixes UI — 9 écrans/composants + 2 tokens couleur clair)** :
nouvel état "échec" sur le mot de verdict Détail (`verdictWord()`) distinct de
"analyse…", + toast de confirmation retry ligne/masse (Revue/Queue) ; état de
chargement + bouton "Réessayer" sur Écartés ; état de chargement Bibliothèque +
tri désormais appliqué en vue Grille (pas seulement Tableau) + "Réinitialiser
les filtres" corrigé pour couvrir recherche+facette ; suppression de piste par
`confirmAction` + bouton désactivé pendant l'appel, borne Année 1900-2100,
autocomplétion Genres (datalist) sur l'éditeur Bibliothèque ; état d'erreur
visible sur les 4 sections M8 Rekordbox (au lieu de disparaître), boutons
Appliquer/Dédupliquer/Fusion avec état "en cours", CTA principaux passés en
`.sift-ranger-btn` ; confirmation sur "Retirer" un dossier surveillé + swatches
de couleur avec `aria-pressed`/libellés FR (Accueil) ; titres de section
d'historique lisibles ("Session du…") sur le Journal ; bannières "Converti" et
avertissement tags avec `role="status" aria-live="polite"`, légende clavier
"écarter" (Revue) ; lignes de sélection Lot accessibles au clavier
(`tabindex`/`role="checkbox"`/`aria-checked`) + bouton "Annuler" sur la
confirmation armée ; `--color-text-warning`/`--color-text-success` recalibrés
en thème clair (`styles.css:25`, sous le seuil AA sur plusieurs fonds réels) ;
hover réaffirmé sur `.sift-ranger-btn`/`.sift-secondary-trash`/`.jrnl-revert`/
`.sift-confirm-btn`. Détail par composant dans les sections dédiées ci-dessus
(voir Sommaire). Vérification : `tsc --noEmit` non rejoué dans cette passe de
documentation (pas de code touché) — chaque affirmation vérifiée par grep sur
le code réel au moment de l'écriture de ce catalogue.

## Conventions de cohérence — à consulter avant tout nouveau composant (07-24)

Issu d'un audit de cohérence visuelle cross-écran (7 écrans, 2026-07-24) qui a
trouvé plusieurs divergences que ni `tsc` ni le lint de tokens ne peuvent
attraper mécaniquement — le token existant était juste mal choisi pour le
rôle, pas absent. `scripts/lint-tokens.mjs` (ratchet CI, `--write-baseline`)
attrape les valeurs littérales hors-token ; **rien n'attrape automatiquement
un mauvais choix sémantique parmi des tokens valides** — ce qui suit comble
ce trou par une convention documentée, à vérifier au moment de la revue.

**Sémantique des couleurs de texte/icône interactives** (`--color-text-*`) :
- `danger` — action qui retire quelque chose de la liste/vue courante
  (soft-delete vers une corbeille, suppression définitive, retrait d'une
  source). PAS réservé aux actions strictement irréversibles : `envoyer à la
  corbeille` (récupérable) est danger au même titre que `purger` (définitif) —
  le signal est "ça disparaît d'ici", pas "c'est permanent".
- `warning` — état qui appelle une vigilance sans action de l'utilisateur
  (échec d'analyse, tag manquant, coupure détectée).
- `info` — élément interactif de navigation/correction (lien, bouton
  "Restaurer"/"Annuler"/"Remettre en file") OU badge de notification passif
  ("N nouveaux" à traiter) — les deux usages coexistent, distingués par le
  contexte (bouton cliquable vs badge de statut), pas par une règle unique.
- `success` — confirmation qu'un état est correct/à jour, jamais un badge
  interactif.
- Un même **rôle fonctionnel répété sur plusieurs catégories** (ex. "annuler
  un lot" sur 3 catégories différentes) n'implique PAS forcément la même
  couleur si le *contenu réel de l'action* diffère derrière une interface
  commune — vérifier ce que fait vraiment le handler/la commande backend
  avant d'harmoniser une couleur qui semble incohérente en surface (piège
  vécu 2026-07-24 : `journal.ts` `massColor`, voir historique ci-dessous).

**Hiérarchie de poids** : le libellé principal d'une ligne de liste dense
(nom de piste, chemin de fichier réparé) porte `font-weight:500` — jamais
seulement une différence de taille ou de couleur pour le distinguer de sa
ligne méta (chemin, timestamp). Convention déjà posée par `.qi`,
`.sift-lib-tile-title`, les lignes Écartés — à répliquer sur tout nouveau
composant de liste plutôt que de laisser le texte par défaut (400) porter
la hiérarchie seul.

**Avant de dupliquer un style inline répété ≥3 fois** : chercher une classe
existante avec le même rendu (`grep` sur la valeur exacte dans `styles.css`)
avant d'en créer une nouvelle ou de continuer à dupliquer inline — un style
inline répété sur plusieurs sites du même fichier est le signal qu'une classe
partagée manque, pas que chaque site est un cas particulier. Ne pas réutiliser
une classe existante dont une seule propriété diffère (fond, radius) sans
vérifier qu'elle rend identique à l'usage visé — `.sift-ui-card-soft` avait un
radius et un fond différents de ce qu'il aurait fallu à `rekordbox-view.ts`,
d'où l'extraction d'une classe dédiée (`.sift-ui-card-outline`) plutôt qu'une
réutilisation forcée.

**Prévention mécanique disponible** : `npm run lint:tokens` tourne dans un job
CI dédié (`lint-tokens`, `build.yml`), bloquant en mode ratchet — échoue
seulement si le nombre de valeurs hors-token AUGMENTE par rapport à
`scripts/lint-tokens-baseline.json` (pas sur la dette existante : 122
couleurs + 3 z-index + 120 px-spacing au 2026-07-24, baseline régénérée après
correction d'un double comptage — voir historique ci-dessous ; l'espacement
`styles.css` seul recoupe partiellement le chantier documenté séparément dans
`docs/superpowers/changes/2026-07-19-spacing-scale-sweep/design.md`, ≈262
sites, jamais exécuté). **Limite connue** : `build.yml` ne se déclenche que
sur `push` de `main` (+ `workflow_dispatch`), pas sur `pull_request` — ce
n'est PAS un gate avant merge, seulement un filet qui détecte la dérive une
fois atterrie sur main. Après un nettoyage volontaire de dette, rejouer
`node scripts/lint-tokens.mjs --write-baseline` **depuis un arbre propre
(`git status` vide, aucun dossier sous `.claude/worktrees/`)** et committer le
fichier baseline mis à jour pour verrouiller le gain — un worktree agent
présent au moment du calcul double silencieusement chaque compte (piège vécu
2026-07-24, voir historique).

---

## Ligne disque amovible (écran Clé USB) — trois états (2026-07-31)

Story : `frontend/usb-drive-row.stories.ts`. Rendu par `usbRowHtml()`
(`frontend/usb-row.ts`), la fonction que `renderUsbList()` appelle elle-même —
pas une copie du markup.

Deux de ces trois états **ne pouvaient pas exister avant cette date** :
l'énumération partait du volume logique, donc un disque sans volume monté ne
sortait jamais de `list_removable_drives`. Elle part maintenant du disque
physique (`usb_format::windows`).

| État | Ce qu'affiche la ligne | Bouton |
|---|---|---|
| Formatée et montée | Lettre (`E:`) · modèle · taille · système de fichiers | `Formater…` |
| Non formatée / RAW | `Disque N` (aucune lettre à afficher) · modèle · taille · « non formaté » | `Formater…` |
| Sans média | `Disque N` · modèle · « aucun média inséré » | **aucun** |

L'identifiant affiché vient de `driveDisplayName()` : `RemovableDrive.id` est
devenu un chemin de disque physique (`\.\PHYSICALDRIVE2`), illisible sur une
ligne.

**« Sans média » est listé exprès, pas masqué.** Un lecteur de cartes vide garde
sa lettre dans l'explorateur Windows indéfiniment : afficher « aucun disque
amovible détecté » pendant que l'explorateur montre un lecteur USB est une
contradiction que l'utilisateur ne peut pas résoudre. Le vide de la liste porte
la même explication.

Modale de formatage, état ajouté le même jour : FAT32 refusé au-delà de 32 Gio
(`.sift-usbfmt-error`, sévérité `danger` déjà couverte par
`error-pattern.stories.ts`). Windows ne sait pas créer de volume FAT32 plus
grand — `diskpart` subit la limite comme l'explorateur, contrairement à ce que
le module et le texte de l'écran affirmaient tous les deux.

---

## Teintes pleines `-solid` — neuf tokens (2026-08-01), dix depuis le 2026-08-27

Story : `frontend/hue-solid.stories.ts`. Ajoutées pour les **surfaces de donnée** : les segments
du graphique d'occupation disque de l'écran Clé USB, où `-bg` (fond de puce, chroma 0,035–0,05)
rend délavé et `-text` est une couleur d'encre. Le dixième, `red` (systemRed du kit), est arrivé
le 2026-08-27 pour un second rôle : l'**indicateur d'état plein** — la pastille de verdict Faux
de la file (voir § « Pastille de verdict de file »), où un indicateur est une teinte système
vive, jamais une encre de texte.

| Variante | Rôle | Teintes disponibles |
|---|---|---|
| `-bg` | fond de puce, texte par-dessus | indigo, teal, purple, pink |
| `-text` | encre sur ce fond | indigo, teal, purple, pink, yellow |
| `-solid` | aplat de donnée, indicateur d'état plein | **les dix** : blue, indigo, teal, green, orange, yellow, purple, pink, red, gray |

`blue`, `green`, `orange`, `red` et `gray` n'existent **qu'en `-solid`** — `-bg`/`-text`
n'auraient de sens que pour une puce, et rien n'en demande.

**Ne jamais poser de texte sur un `-solid`** : ces valeurs sont calibrées comme aplats, pas comme
fonds lisibles. Pour une puce, `-bg` + `-text` restent le couple.

**Origine des valeurs.** Ce sont les couleurs système Apple officielles converties en oklch, jeu
clair et jeu sombre — Apple en publie deux, et réutiliser les valeurs claires sur fond sombre les
ferait plonger. Ce n'est pas une palette étrangère plaquée : les cinq teintes déjà présentes
étaient **déjà** dérivées d'Apple, en gardant l'angle et en abaissant la chroma (`indigo` et
`purple` portent exactement les angles d'Apple, 278,34 et 312,41 ; `yellow` à 90,23 contre 90,38).
`-solid` conserve l'angle ET la chroma d'origine.

Les trois blocs de thème (`:root`, `@media (prefers-color-scheme:dark)`,
`:root[data-theme="dark"]`) portent les neuf tokens. Vérifié sur les valeurs **résolues** dans
l'app réelle : aucune manquante, et aucune identique entre les deux thèmes — le piège du bloc
sombre resté avec les valeurs claires. `red` (08-27) est présent dans les trois blocs avec des
valeurs clair/sombre distinctes — vérifié au grep dans `styles.css`, pas re-mesuré en fenêtre.

---

## Modale de formatage USB — états (2026-08-02)

Trois changements issus d'un usage réel, chacun corrigeant un défaut constaté en
se servant de l'écran et non en relisant le code.

**La dictée de confirmation est supprimée.** Il fallait retaper le nom du disque
au caractère près (`SSK SSD Portable SSD (I:)`) ; le bouton restait grisé sans
que rien n'explique pourquoi, et on se croyait bloqué par l'application. Il reste
le cycle armé/confirmé horodaté : premier clic arme, second exécute, et un
doublon d'événement arrivant dans la foulée est rejeté. `CLAUDE.md` exige une
confirmation in-app armée et horodatée pour une action destructive — la dictée
était une couche par-dessus la règle, pas la règle.

**Champ « Nom du volume »**, prérempli avec le nom actuel de la clé
(`RemovableDrive.volume_name`). Reformater en gardant son nom est le cas courant.
Le champ est libre ; le backend l'assainit deux fois avec la même règle — 11
octets, majuscules ASCII, `_` et `-`, le reste devient `_`.

**Le bouton porte l'étape réelle pendant l'opération**, pas une animation :
autorisation Windows demandée, partitionnement, attente du montage, verrouillage,
écriture. Le travail se déroule dans un processus élevé séparé dont la sortie ne
peut pas être redirigée ; il dépose donc son étape dans un fichier que l'écran
interroge toutes les 400 ms.

**FAT32 au-delà de 32 Go n'est plus refusé.** C'était juste tant que Sift
subissait le plafond de Windows ; il écrit maintenant les structures lui-même. Le
bloc n'est plus une erreur mais une explication — une autorisation administrateur
va être demandée, et l'utilisateur doit savoir pourquoi une invite surgit.

⚠️ Le chemin de formatage FAT32 sur matériel réel **n'a pas encore abouti une
seule fois**. Premier essai : partition créée, écriture de la FAT échouée sur un
défaut d'alignement secteur, disque laissé RAW. Corrigé (`sector_io`) mais non
rejoué. Ne pas documenter cet état comme acquis avant un succès mesuré.

## Menu contextuel `.sift-ctx-menu` / `.sift-ctx-item` — 2026-08-20

Composant de l'étape 5 (`frontend/context-menu.ts`, 2026-08-19), catalogué à
l'ajout de la rangée de pastilles. États réels :

- **Entrée** `.sift-ctx-item` : repos transparent · survol `--overlay-hover` (aplat) ·
  désactivée `.sift-ctx-item--disabled` (encre `--color-text-tertiary`, jamais
  d'opacité, `aria-disabled`, survol neutralisé) · danger `.sift-ctx-item--danger`
  (encre `--color-text-danger`) · séparée `.sift-ctx-item--sep` (filet
  `--color-border-tertiary` au-dessus).
- **Rangée de pastilles** `.sift-ctx-swatchrow` (2026-08-20, couleur de source —
  patron Finder Tags, forme actée par wireframe, variante A) : étiquette
  `.sift-ctx-swatchlabel` en encre tertiaire + 5 boutons `.sift-ctx-swatch` 18 px à
  fond transparent **réaffirmé au survol** (sans quoi `button:hover` générique, plus
  spécifique, peint son gris et son `filter`) ; la teinte vit sur le span
  `.sift-ctx-swatch-fill` via `.sift-rail-src-dot-<teinte>`. Survol : bordure
  `--color-border-secondary`. Actif `.on` : anneau `--color-text-primary` sur la
  teinte **résolue** (override sinon cycle), déclaré après `:hover` pour lui survivre.
  « Couleur automatique » suit la doctrine du menu stable : désactivée sans override,
  jamais retirée.
- Fermeture : clic extérieur (capture), scroll, resize, Échap — le menu est ancré à un
  point, pas à un élément.

Stories : `frontend/context-menu.stories.ts` (SourceCouleurAuto ·
SourceCouleurOverride). Spec : `docs/ui-specs/rail.md` § Interactions.

## Ligne de source du rail `.sift-rail-src` / pastille `.sift-rail-src-dot` — 2026-08-20

Section Sources du rail (fusion 1). Markup : `frontend/rail-source-entry.ts` — module
pur extrait de `rail-sources.ts` pour que story et Vitest exécutent le VRAI rendu
(même séparation que `source-color.ts`, même motif que les stories du Journal :
une copie ne peut que diverger). États réels :

- **Repos** : entrée `.nv` ordinaire — encre `--color-text-tertiary` (c'est la règle
  `.nv` elle-même, pas une règle dédiée), pastille 8 px PLEINE sur la teinte
  d'identité (`--src-hue`, posée par `.sift-rail-src-dot-<teinte>` : override manuel
  sinon cycle par ordre d'ajout, `source-color.ts`). La pastille est un accent
  catégoriel : elle identifie la source, elle ne porte aucun état (DESIGN.md § 4).
- **Survol / sélection** : génériques du rail — `.nv:hover` et `.nv.on` fond
  `--color-nav-active`, `.on` ajoute encre primaire + graisse 500.
- **Échec de scan / dossier inaccessible** `.sift-rail-src--error` : encre de ligne
  `--color-text-danger`, motif dans le `title`. Jamais atténué, et il PRIME sur la
  suspension — `rail-source-entry.ts` ne pose `--suspended` que sans `--error`,
  précédence gelée par `test/rail-source-entry.test.ts`.
- **Surveillance suspendue** `.sift-rail-src--suspended` (2026-08-20) : pastille
  VIDÉE — fond transparent, contour `box-shadow:inset` sur la TEINTE CONSERVÉE
  (1 px, ratio 1/9 dérivé de l'anneau du picker 2 px/18 px ; 1.5 px en rail replié
  où la pastille fait 14 px). AUCUNE règle d'encre, et c'est mesuré, pas oublié : le
  repos de `.nv` est déjà `--color-text-tertiary`, la valeur que la spec prescrit —
  état permanent donc neutre. L'état est porté par la forme pleine/creuse, jamais
  par la couleur seule ; le `title` porte le motif (« surveillance suspendue »).
- **Aucun fichier audio reconnu** `.sift-rail-src--empty` (2026-09-02, issue #55,
  décision B2) : `track_count === 0` — un dossier surveillé où le scan n'a reconnu
  AUCUN fichier portait un badge vide, indiscernable d'un dossier entièrement
  traité. Badge textuel « 0 audio » en `--color-text-warning`, pastille ÉTEINTE :
  teinte conservée, `opacity:.45` (wireframe B2) — ni pleine ni vidée, le creux
  disant déjà « suspendue ». Précédence : échec > vide > suspension — le vide est
  une information corrective (mauvais dossier probable), la suspension un choix ;
  gelée par `test/rail-source-entry.test.ts`. Le `title` porte le motif.
- **Replié** (`body.sift-rail-collapsed`) : pastille 14 px — seule identité visible
  d'une source —, badge de compte clippé en point de 6 px (en `--color-text-warning`
  au lieu d'info quand la source est `--empty` : la présence signalée est un
  problème, pas du travail en attente).

- **Cadran de dépôt** `.sift-rail-drop` (2026-09-03, issue #56, décision A « puits
  calme », amendée le même jour : le cadran suit le FLUX des sections, sous
  Exporter — le vide flexible du rail vit entre lui et Réglages) : cadre hairline
  permanent — `--color-border-secondary` 1 px, rayon `--border-radius-md`, padding
  `--space-4`, fond transparent, `margin-top:var(--space-8)` ; `.nav-foot` garde
  son `margin-top:auto`. Patron HIG § Image wells : seul
  puits de dépôt permanent qu'Apple dessine, cadre calme jamais pointillé — le
  pointillé info (`.sift-dz-on`) reste le langage du drag, posé par-dessus. Pas de
  hint texte (retiré sur demande, même décision). Replié : cadre et padding à 0,
  même règle de survie que `.sift-railwarn`. La marque (`.nav-brand`) a quitté le
  rail dans le même geste — logo + repli dans le segment gauche de la barre
  (`#sift-tb-brand`, `chrome.ts`).

Stories : `frontend/rail-sources.stories.ts` (TeintesDuCycle · ScanEchoue ·
SurveillanceSuspendue · AucunFichierReconnu, toutes rendues dans le cadran — le rail
replié n'y est pas représentable, la classe vit sur `<body>`). Spec :
`docs/ui-specs/rail.md` § États.

## Lecteur simple de Revue — rangée d'audition (2026-08-27)

La waveform a quitté Revue (`31c5d1a`, décision Antoine sur comparatif maquette) :
le lecteur est le slider fin du kit (Pickers/Linear/Small 53:118, copie SVG).
WaveSurfer RESTE le moteur audio — son conteneur est réduit à zéro
(`.sift-progress-engine`, jamais `display:none` : son ResizeObserver doit
survivre). Markup : `player-audition.ts` (module pur, extrait de `report-view.ts`
pour que la story exécute le vrai rendu) ; wiring : `report-view.ts::mountPlayer`.
Story : `frontend/player-audition.stories.ts`.

**Progression** (`.sift-progress`, `role="slider"`) :

| État | Sélecteur / condition | Rendu |
|---|---|---|
| Piste | `.sift-progress-track` | 4 px rayon 2, `--overlay-bar` — PAS `--color-track`, qui vaut le fond de fenêtre en sombre (piste invisible, « on ne voit pas la longueur de la barre ») ; inner-shadow littérale (copie du kit) |
| Remplissage | `.sift-progress-fill` | `--color-accent-fill`, `width` en % posé par `updateTime` — mutation seule, jamais de rebuild |
| Pouce | `.sift-progress-knob` | 20 px, `--color-accent-ink` (blanc theme-invariant), drop-shadow littérale kit ; `hidden` tant que la durée est inconnue ; `pointer-events:none`, AUCUN scale au survol (le pouce du kit n'en a pas — voir § Sliders périmé : `.dragging` n'a plus de règle) |
| Survol | `.sift-wave-hovertime` | bulle mm:ss (`--overlay-scrim`/`--color-text-on-scrim`), patron QuickTime — seule survivante du survol d'onde, le ghost et la ligne sont partis avec les barres |
| Focus | `.sift-progress:focus-visible` | anneau 2 px `--color-border-info`, offset 2 |
| Clavier | flèches ±5 s, Home/End | APG `role="slider"` ; `aria-valuenow` tenu par `updateTime` |
| Seek | pointerdown/drag sur toute la surface | le canvas moteur a `interact:false` — le slider custom est le SEUL chemin de seek |
| Fin de piste | `finish` | stop + pouce ramené à 0, pas d'auto-avance (patron Musique, piste isolée) ; l'icône play/pause porte seule l'état — le dim de pause est parti avec la waveform |

**Play** `.sift-play-btn` : 28×28 (était 46), glyphe Tabler 22 — du kit on copie la
géométrie, jamais ses glyphes (SF Symbols non licenciable, patterns § 5). Hover :
glyphe `scale(1.08)` ; enfoncé : `scale(.92)` ; `background:none` réaffirmé aux
deux. Aligné au bord de conduite (marge gauche retirée).

**Temps** `.sift-time` : un seul, mono `--text-xs` 500, cliquable (et
Entrée/Espace) — bascule écoulé ↔ restant (préfixe `-`), patron Musique/Podcasts.
Son `:hover` est injecté au runtime (`ensureStyles`, report-view.ts), pas dans
`styles.css`.

**Volume fin** (`.sift-volume`, 90 px — remplace la capsule SVG du 25, « couleur,
taille et style vraiment goofy » dans la rangée fine) :

| État | Sélecteur / condition | Rendu |
|---|---|---|
| Piste | `.sift-volume-track` | 4 px, `--overlay-bar` — garde 3:1 dans les deux thèmes là où le blanc@10 % de la capsule le perdait en clair |
| Remplissage + pouce | `.sift-volume-fill` / `.sift-volume-knob` | `--color-accent-ink` BLANC theme-invariant (un volume n'est pas une progression : pas d'accent) ; pouce 14 ; `width`/`left` = course du CENTRE du pouce (`volumeCentreCss`, partagée markup ↔ story) |
| Muet | icône de `.sift-volume-mute` | bascule `ti-volume` ↔ `ti-volume-off` (`volumeIconClass`) — l'état se dit par l'ICÔNE, plus par un slash permanent ; clic = mute/démute, dernier volume non nul mémorisé |
| Hover haut-parleur | `.sift-volume-mute:hover` | encre secondary → primary, fond none |
| Focus | `.sift-volume:focus-visible` | anneau info ; `:focus` seul : `outline:none` |
| Clavier | flèches ±5 %, Home/End | audit-ref R1 (réf. shadcn Slider), `aria-valuenow` tenu |

Retirés de l'écran : tempo & key-lock (« Écoute avancée », 2026-08-21 — le pitch
DJ n'est pas voulu sur un écran de décision). Leurs règles CSS
(`.sift-listen-advanced*`, `.sift-key-*`), `.sift-player-controls` et le token
`--color-waveform-elapsed` sont retirés de `styles.css` le 2026-08-27 — zéro
consommateur au grep, déclinaisons `#sift-aside` comprises.

## Pastille de verdict de file — `verdictDot()` (2026-08-27)

Module pur `queue-verdict-dot.ts` (extrait de `queue-panel.ts` le 2026-08-27,
même motif que `rail-source-entry.ts`) ; rendue par `queueRowHtml` en FIN de
titre (décision produit, assumée CONTRE le motif Mail des indicateurs au bord
droit). Styles INLINE (la ligne est concaténée dans la boucle virtualisée), 9 px.
Story : `frontend/queue-verdict-dot.stories.ts`.

Teintes SYSTÈME pleines depuis le 2026-08-27 (avant : encres
`--color-text-success/danger/warning`) : un indicateur d'état est une teinte
système vive, jamais une encre — le point non-lu de Mail est systemBlue plein.
Les encres `text-*` restent aux MOTS (badge LOSSLESS de zone C, libellés).

| Cas | Condition | Rendu |
|---|---|---|
| Authentique | `verdict === "ok"` | pastille pleine `--color-hue-green-solid`, `title="authentique"` |
| Faux / sur-encodé | `verdict === "fake"` | pleine `--color-hue-red-solid` (token AJOUTÉ ce jour — systemRed du kit) |
| Zone grise | `verdict === "grey"` | pleine `--color-hue-yellow-solid` |
| Échec terminal | verdict nul **et** `analysis_attempts >= MAX_ANALYSIS_ATTEMPTS` | pleine `--color-hue-red-solid` — PARTAGE la teinte de Faux ; la distinction est le bouton Réanalyser, propre à la ligne non analysée |
| En attente | verdict nul, tentatives restantes | ANNEAU `1.5px solid var(--color-text-tertiary)` — l'attente reste neutre |

Cinq cas pour quatre rendus. Généalogie : `verdictWord()` (retiré 2026-08-26,
section ⚠️ plus haut) rendait les états de pipeline que la pastille porte seule
désormais.

## Surfaces de Revue — trois plans (2026-08-27, #50)

Le portage maquette a redistribué les surfaces du shell de Revue en TROIS plans
(motif sidebar Mail) — c'est le troisième décalage de la « Grammaire de carte »
(voir son ⚠️ daté) :

- **Rail `.sb`** — le plan le plus EN RETRAIT : `--color-background-primary`
  (avant : `--color-background-tertiary`), `border-right` 1 px
  (`styles.css`, règle `.sb`). `#sift-tb-left` ET `#sift-titlebar` suivent
  (`chrome.ts::injectLeanStyle`) ; filet 1 px bord à bord sous la barre (motif
  Mail, en-tête de colonne).
- **Milieu (zone C)** — le sol : fond du `body` (`--color-background-tertiary`),
  aucune surface propre. Deux exceptions, la charpente de la zone : le cadre de
  lecture et le pied (ci-dessous).
- **File `#qcol`/`.queue`** — `--color-background-queue`, BORD À BORD : collée au
  rail, à la barre et au bas de fenêtre (`.sift-revue-row`, marges négatives qui
  annulent le padding de `#content`), `border:0` sauf le filet de flanc droit ;
  la carte flottante (bordure + rayon 14) est partie. Séparateurs de rangées :
  § `.qi`. La tranchée de `.sift-qresize` est refermée (emprise nulle, zone de
  saisie 16 conservée). `#homequeue` (Accueil) garde sa carte — il vit au milieu
  d'une page, pas en colonne de shell.

**Cadre de lecture `.sift-player-row`** : fond queue + rayon `--border-radius-md`,
inseté de 16 par `.mid` (padding 16/16/12), `max-width` retiré — le « cadre Y »
validé sur comparatif. EXCEPTION CONSIGNÉE à « une surface de contenu ne peint
rien » (2026-08-14) : le bandeau de lecture est LA seule surface du milieu,
consignée dans `patterns.md`. Filet interne en retrait entre en-tête et lecteur
(`border-top` de `.sift-player-audition`) ; le filet de section avant la fiche a
été posé puis RETIRÉ le soir même (la boîte borne déjà — une frontière ne se dit
qu'une fois). ⚠️ Une variante « bande full-bleed sans rayon » a été livrée puis
RETIRÉE le même soir (« ce n'est pas un cadre », `861911f`) — fausse route
consignée ici et dans `revue.md` § Après-midi.

**Pied `.sift-action-rail--flat`** (Détail) : surface pleine largeur, fond queue,
`border:0`, `border-radius:0` — une zone de boutons se distingue par surface ou
par l'espace, jamais par un filet (Big Sur). Emplacement re-questionné puis
CONFIRMÉ (comparatif P1/P2/P3 — verdict : P1, bas de panneau, motif bas
d'inspecteur des pro apps). La carte (bordure + rayon lg) avait sauté de la règle
de BASE `.sift-action-rail` (`31c5d1a`, qui ne visait que le pied), règle que le
rail de Lot partage — divergence spec ↔ code constatée le 2026-08-27, SOLDÉE le
2026-08-29 : carte restaurée sur la base (rail de Lot conforme à « garde sa
carte », `revue.md` § Après-midi), `--flat` continue de la retirer pour Détail.

## Carte de racine manquante du rail — `.sift-railwarn` (`rail-warn-card.ts`, issue #54, 2026-09-02)

Remplace le bandeau pleine largeur `#sift-gate`, **supprimé** le même jour (code,
plomberie et feuille). Motif du remplacement : depuis #54 la racine de bibliothèque
n'est plus un prérequis de la conversion — on convertit EN PLACE ou vers un dossier
externe sans elle (`filing::needs_library_root`) — donc un bandeau permanent au-dessus
de la charpente entière criait un prérequis qui ne mord qu'à une destination de l'arbre.

| État | Rendu |
|---|---|
| **Racine réglée** | Rien. Le nœud est **retiré du DOM**, pas masqué — il n'y a donc pas de second rendu à cataloguer |
| **Racine absente** | Carte ambre sous la section Sources : `<strong>Racine non définie</strong>` puis « Choisir dans Réglages › ». Fond `--color-background-warning`, encre `--color-text-warning` (mêmes tokens que `.sift-tag-warn`), rayon `--border-radius-md` |
| **Survol** | `--overlay-hover` posé en `background-image` PAR-DESSUS l'ambre, jamais en remplacement : un `background` écraserait le fond warning, c'est-à-dire l'information |
| **Lecture du réglage échouée** | Rien ne change — l'état précédent tient. Peindre dirait « aucune racine », un fait non mesuré ; retirer dirait l'inverse. La chaîne brute part en console (impasse A8, issue #15) |
| **Rail replié** | Masquée entière (`body.sift-rail-collapsed`). Ses deux lignes sont du texte de bout en bout : rien à garder en icône, contrairement aux entrées de source qui gardent leur pastille |

**Ce n'est pas une entrée de navigation.** Pas de classe `.nv` : la carte dit un fait et
propose UNE suite, elle ne mène pas à un huitième écran. Le clic navigue par
`data-view="reglages"`, capté par le délégué unique de `router.ts` sur `#pa`.
`aria-label` en une phrase, parce que le nom accessible concaténé des deux nœuds se
terminerait par un chevron typographique.

**Montage idempotent** (`rail-root-warning.ts`) : `renderRailSources` reconstruit sa
section par `innerHTML` et emporte la carte, donc elle est reposée après chaque
reconstruction — et ne l'est PAS sur le chemin rapide, où elle survit. C'est ce qui
justifie l'id stable `sift-railwarn` : sans lui, deux cartes s'empileraient.

Story : `rail-root-warning.stories.ts` (exécute `rootWarningHtml`, jamais une copie).
Gelé par `test/rail-warn-card.test.ts` — attribut de navigation, absence de `.nv`, id.

## À re-sourcer · Corbeille — ex-Écartés, deux destinations du rail (2026-09-08) — `ecartes-view.ts`

Spec : `docs/ui-specs/ecartes.md`. L'écran unique « Écartés » (deux cartes, pastilles de
compte et « Purger » dans la zone C, lignes à styles inline avec deux boutons icône, « Copier »
et six liens boutique au survol — audit du 2026-07-09 ci-dessus, périmé) devient deux entrées
de rail dans la grammaire de Rangés.

| Élément | État | Détail |
|---|---|---|
| Rail | deux entrées | `data-view="resourcing"` « À re-sourcer », `data-view="trash"` « Corbeille » (index.html) ; `ViewId` suit (router.ts) |
| Barre | titre + compte, action | compte dans `#sift-tb-count` ; « Vider la corbeille » (`.sift-secondary-trash.sift-ec-purge`) sur la Corbeille non vide, confirmation in-app |
| Table | au sol | `.sift-library-main` sans carte, `.sift-lib-thead` figé, `.lr` 32 px virtualisées ; colonnes Raison (pastille + libellé : FAKE / TRONQUÉ / À VÉRIFIER / —) · Artiste · Titre · Fichier (`.sift-ec-file`, mono) · pastille de format |
| Tri | Artiste, Titre, Fichier | `data-ecsort`, flèche ▴▾, `aria-sort` ; Raison non triable (`.sift-lib-colhead-static`) |
| Inspecteur au repos | résumé | destination, compte, répartition par raison, règle en une phrase (`.sift-ec-rule`) |
| Inspecteur, piste ouverte | composants de Revue | en-tête `.sift-player-header` (sans pochette ni lecteur), fiches Raison (`.sift-ec-sentence`) · Racheter (une `.sift-row` par boutique, bouton « Ouvrir la recherche » `data-ec="store"`, puis « Copier le nom ») · Actions (`.sift-ec-actions` : Remettre en file / Envoyer à la corbeille, ou Restaurer) |
| Clic droit | menu de Rangés adapté | Ouvrir l'emplacement · Ouvrir / Masquer le détail · Copier le nom · Racheter… · Remettre en file · Envoyer à la corbeille / Restaurer — sans sous-menu |
| Chargement / erreur / vide | squelette statique · phrase danger + Réessayer · `emptyStateHtml` par destination, inspecteur fermé |

Les liens boutique sont des recherches, pas une disponibilité : API boutiques essayées et
écartées (Antoine, 2026-09-08). Vérifié dans la vraie fenêtre le jour même (spec § Vérifié).
