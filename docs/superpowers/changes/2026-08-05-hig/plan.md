# Plan — repliage HIG

Inventaire et preuves : `design.md` (même dossier). Ce plan ne réénumère pas les écarts,
il ordonne le travail.

## Étape 1 — Replier ce qui est déjà lu (documentation seule)

Aucune ligne de code touchée. Les six documents de `docs/design-system/` reçoivent les
règles confrontées, avec leur statut et leur preuve.

- `foundations.md` — Layout, Typography, Color, Motion, Accessibility, Dark Mode,
  Materials : ce que Sift respecte, et le test organe-du-système vs fait-humain.
- `tokens.md` — E1 (contraste augmenté absent) et E2 (plancher 10 pt), avec les usages
  recensés. Ne recopier aucune valeur : `frontend/styles.css` reste canonique.
- `patterns.md` — Undo/redo, Feedback, Loading, Modality ; la tension Alerts vs
  `BATCH_CONFIRM_THRESHOLD` ; la tension Liquid Glass vs « Surface Continue ».
- `components.md` — Sidebars, Lists and tables, Progress indicators, Alerts.
- `content.md` — Writing ; D2 (jargon conservé) posé comme divergence assumée.
- `governance.md` — la source HIG, sa méthode d'accès (Browser pane, pas `WebFetch`),
  et la règle : une règle HIG citée sans preuve dans l'app ne se replie pas.

## Étape 2 — Corriger E3 (Cmd+Z sur macOS) — FAIT le 2026-08-05

Le seul défaut **fonctionnel** de l'inventaire. La garde de `installUndoShortcut()`
(`frontend/filing.ts`) ne testait que `e.ctrlKey` ; la cible macOS n'avait donc aucune
voie d'annulation au clavier, et pas de menu Édition non plus. ⚠️ Ne pas re-citer de
numéro de ligne pour l'état d'avant : le correctif a décalé le fichier de cinq lignes.
La garde actuelle est à `filing.ts:605`.

Corrigé : la garde teste `(e.ctrlKey || e.metaKey)`. `tsc --noEmit` passe. **Non vérifié
en exécution** — le code vit dans `installLiveWiring()`, hors de portée du navigateur et
du Browser pane.

Reste ouvert, volontairement séparé : le raccourci n'est documenté nulle part dans l'UI
(`frontend/report-view.ts:364` liste SPACE, ENTER, BKSP, HAUT/BAS). Décision de design,
pas correctif.

## Étape 3 — Trancher E2 — **TRANCHÉ ET FAIT le 2026-08-05**

Décision d'Antoine : remonter toutes les polices au plancher, et faire l'hygiène qui en
découle. Les 14 sites ont migré vers `--text-xs`, `--text-2xs` et `--text-3xs` sont
retirés, `.nv-grp` (9 px en dur) et `.sift-tags-title` (morte) traités dans le même geste.
Le découpage en familles ci-dessous n'a donc plus valeur de décision — il reste comme
trace des mesures, et parce que ses coûts sont ce qu'il faut regarder à la vérification.

Résidus hors `styles.css`, documentés dans `review.md` § 1 : les glyphes de boutons de
fenêtre macOS (exemption légitime) et quatre sites de `app.js` dont le statut réel se
tranche à la mesure, pas à la lecture.

### Découpage d'origine, conservé pour ses mesures

Comptage revu le 2026-08-05 (passe adverse) : **22 franchissements** du plancher, pas 18.
Cinq décisions, pas trois :

1. **libellés de verdict** (4 usages : `.sift-chip-badge`, et `DUPLICATE`/`DUP`/`FAKE` de
   `batch-panel.ts`) — famille prioritaire : le label texte est la compensation
   d'accessibilité du code couleur, l'affaiblir la vide de sa fonction ;
2. **en-têtes uppercase** (6) — densité assumée pour un outil à fort volume, candidat à
   une divergence documentée plutôt qu'à un correctif ;
3. **métadonnées mono** (4) et `.sift-slider-label` (1) — cas par cas ;
4. **glyphes d'icône des Écartés** (3, `ecartes-view.ts:22,24,25`) — **rien à décider** :
   `--text-2xs` n'y porte que l'icône, le libellé est à 10px via `.sift-vchip`. Ils
   étaient comptés à tort dans la famille verdict ;
5. **hors famille** (4 : `library-detail.ts:50`, `batch-panel.ts:363`, `styles.css:784`,
   `styles.css:1242`) — ne relevaient d'aucune décision planifiée jusqu'ici.

Rappel de méthode : concept avant chiffres. Nommer la surface et la décision utilisateur
avant de descendre au px.

## Étape 4 — Mesurer E4 — **axe corrigé le 2026-08-05**

L'hypothèse « le zoom natif compense peut-être » est **réfutée** : le zoom existe et
scale bien les px, mais Sift le rend inatteignable (`zoomHotkeysEnabled` absent donc
`false`, permission de zoom absente). Détail et sources dans `design.md` § E4. Aucune
refonte de l'échelle typo n'est justifiée — `rem` ne changerait rien.

Ce qui reste à mesurer demande la vraie fenêtre : ni le navigateur ni le Browser pane
n'exécutent `installLiveWiring()`. Par CDP (`.claude/scripts/cdp.cjs`, port en variable
d'environnement au lancement, jamais dans `tauri.conf.json`) :

1. **Windows, Taille du texte système à 225 %, Sift relancé** — lire
   `window.devicePixelRatio` et
   `getComputedStyle(document.querySelector('.sift-chip-badge')).fontSize`. DPR ≈ 2,25
   attendu si le scaling s'applique. C'est la mesure qui tranche Windows ;
2. **dans le même état, parcourir Revue / Bibliothèque / Clé USB** et chercher le
   clipping : comparer `scrollWidth` et `clientWidth` sur `.pa`, `.mid`, `.home-body`.
   C'est le point qui peut faire échouer Windows malgré le premier ;
3. **confirmer sur le binaire** que Ctrl+`+` et Ctrl+molette ne font rien ;
4. **macOS** : rien à mesurer, seulement à constater — aucun mécanisme n'existe.

Le correctif de configuration (deux lignes) ne règle que l'atteignabilité. Le clipping à
fort zoom est un problème de layout : zéro media query de largeur et `overflow:hidden` sur
`html,body`. Ne pas confondre les deux.

## Étape 5 — Lire les Patterns restants — LARGEMENT FAIT le 2026-08-05

Repliés depuis : Searching, Settings, Entering data, File management, Charting data,
Drag and drop, Playing audio (dans `patterns.md`), Disclosure controls (dans
`components.md` et `patterns.md`).

**Terminée** : Launching, Onboarding et Offering help ont été lus et repliés dans
`patterns.md` § Lancement Et Aide, sur base mesurée — géométrie de fenêtre restaurée
(`lib.rs:176`) mais ni vue ni défilement, zéro onboarding dans le dépôt, aide entièrement
portée par 46 `title=` et 37 `aria-label`, six modules à état vide. C'est cette passe qui
a fait apparaître E7.

Deux observations sorties de cette passe, à traiter comme des questions ouvertes et non
comme des défauts :

- **glisser-déposer** : la convention système veut que déposer entre conteneurs copie.
  Déposer un dossier sur Sift déclare une source. L'écart est légitime mais silencieux ;
- **lecture audio** : les HIG demandent une pause immédiate au débranchement du casque.
  Comportement **non mesuré** dans Sift.

Même exigence qu'à l'étape 1 : une règle ne se replie qu'accompagnée de sa preuve dans
l'app.

## Étape 6 — Traiter E5 et E6, nés de la passe adverse

Ces deux écarts n'existaient pas au moment d'écrire ce plan : ils sont apparus en
vérifiant les affirmations de `design.md`, qui les présentait comme des **conformités**.

- **E5 — `prefers-reduced-motion` ne couvre presque rien.** 35 `transition:` et 9
  `animation:` dans `styles.css`, 2 blocs de garde contenant 3 règles, 1 seule règle
  neutralisée sous `reduce`. Décision à prendre : étendre la garde à tout ce qui bouge,
  ou l'assumer. Ce n'est pas une retouche — c'est une revue de chaque animation ;
- **E6 — aucune commande IPC asynchrone** (0 `pub async fn` sur 71 commandes). Cohérent
  avec l'architecture, mais l'absence d'async ne dit rien de la réactivité. **À mesurer
  d'abord** : quelles commandes font un travail long en ligne. La piste `ipc.rs:259`
  `analyze_path` sur cache miss est signalée mais non revérifiée.

- **E7 — Accueil n'affiche rien pendant son chargement**, alors que c'est le premier écran
  peint et que les trois autres listes ont un « Chargement… ». Correctif le plus court de
  tout le chantier, sur l'écran le plus exposé.

Priorité relative : E7 d'abord — un placeholder, sur le premier écran. Puis E5, visible
par l'utilisateur et sans risque. E6 en dernier : il demande une mesure avant toute
conclusion et touche l'architecture — ne rien y changer sur la foi d'un rapport.

## Étape 7 — Exposition clavier — **à moitié faite, et le reste est bloqué**

Fait : les tuiles de `library-views.ts` ont leur `aria-label` composite.

**Non fait, volontairement.** La file de Revue reste sans `tabindex` ni `role`, parce que
les poser créerait un défaut pire que celui qu'ils corrigent : `filing.ts:560` capte
`keydown` au niveau document avec pour seule garde `if (!state.track) return`, donc sur
Revue Espace = écouter, **Entrée = convertir**, Retour arrière = écarter. Un
`role="button"` annonce une activation par Entrée que cette couche détourne vers l'action
principale.

`installFilingKeys()` étant enregistré avant `installQueueNavKeys()`
(`sift-live.ts:185-186`), aucun écouteur bouillonnant plus tardif ne peut le devancer. Le
correctif demande un lot coordonné `queue-panel.ts` + `filing.ts` — hors du périmètre
d'une retouche de markup, et à faire quand personne d'autre ne tient `filing.ts`.

## Étape 7 bis — trace de l'analyse d'origine

Sorti de la cartographie des composants, hors HIG Components proprement dits.

La file de Revue (`queue-panel.ts:332`) rend `div.qi` sans `tabindex` ni `role`, quand la
même classe `.qi` sur Accueil (`home-sources.ts:80`) porte `tabindex="0" role="button"
aria-pressed`. Les flèches fonctionnent (`queue-panel.ts:265-279`), donc ce n'est pas la
navigation qui manque : c'est l'exposition. Second cas, plus léger : les tuiles de
`library-views.ts:116` ont `tabindex`/`role` mais pas d'`aria-label`, contrairement aux
lignes (`:86`).

Règle correspondante écrite dans `components.md` § Règle De Focus. Le reste du dépôt est
déjà conforme — ce sont deux exceptions, pas un chantier de fond.

## Non planifié — E1, et pourquoi il le reste

E1 (variantes de contraste augmenté) n'a toujours pas d'étape, et les mesures du
2026-08-05 expliquent pourquoi mieux que la première rédaction.

La palette actuelle vit entre **5:1 et 8,5:1** — calibrée à ~5:1 dès l'origine pour les
paires ton-sur-ton. Un registre `prefers-contrast: more` doit viser sensiblement plus haut
que l'existant pour avoir un sens : les HIG parlent de 7:1 sur du petit texte, et d'une
différenciation « significantly higher » que la variante par défaut. Concrètement cela
veut dire **redériver un jeu complet** — surfaces, textes, bordures, overlays — dans les
deux thèmes, soit un troisième registre à tenir en cohérence pour toujours.

Ce n'est pas une extension de l'existant : c'est une décision de palette, du même ordre
que le rollout Apple system colors du 2026-07-06. Elle ouvre son propre dossier le jour
où elle est prise.

Ce qui **a** été fait entre-temps, et qui n'était pas E1 : les quatre échecs AA réels que
la campagne de mesure a exhumés — opacité du mode Lot, scrim de pochette, bannière de
rangement, ton neutre en sombre. Ceux-là ne relevaient d'aucun arbitrage : un seuil
mesurable était franchi.
