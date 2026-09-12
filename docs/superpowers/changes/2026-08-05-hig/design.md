# Repliage des Apple HIG dans le design system de Sift

Date d'ouverture : 2026-08-05.

## Pourquoi

`docs/design-system/` porte déjà la taxonomie d'Apple — `foundations.md`,
`patterns.md`, `components.md` — plus `content.md`, `tokens.md`, `governance.md`.
745 lignes au total. Un `grep -n "Apple\|HIG"` sur ce dossier ne renvoyait **rien**
avant ce chantier : le parallèle de structure existait, la source n'était jamais citée.

`CLAUDE.md` désignait « Apple HIG » comme référence des décisions desktop sans dire
quelle section, ni quelles règles avaient été effectivement confrontées à l'app. Ce
chantier ferme cet écart : chaque règle retenue est classée et rattachée à une preuve
citable.

## Méthode d'accès

**Les pages HIG ne se lisent pas avec `WebFetch`.** Ce sont des SPA : le contenu arrive
vide et le modèle de résumé répond « I don't have access to browse web pages or retrieve
real-time content from the internet », ce qui ressemble à un refus et n'en est pas un.
Le Browser pane (`preview_start {url}` puis `get_page_text`) rend le JS correctement.

`developer.apple.com/design/` n'est qu'un hall d'entrée — il ne contient **aucune** page
« design principles ». Les sections réelles sont HIG, Apple Design Resources, Icon
Composer, SF Symbols, Pass Designer, Reality Composer Pro, vidéos, Apple Design Awards.

## Pages lues

Foundations : Layout, Typography, Color, Motion, Accessibility, Materials, Dark Mode,
Writing.
Patterns : Undo and redo, Feedback, Loading, Modality, Searching, Settings, Entering
data, File management, Charting data, Drag and drop, Playing audio.
Components : Sidebars, Lists and tables, Progress indicators, Alerts, Disclosure
controls.

**Sautées, et pourquoi.** Foundations : App icons, Branding, Icons, Images, Immersive
experiences, Inclusion, Privacy, Right to left, SF Symbols, Spatial layout — soit hors
sujet (spatial, immersif), soit à traiter séparément (icônes, marque). Patterns encore
non lus mais pertinents : Launching, Onboarding, Offering help. Patterns hors sujet :
Collaboration, Managing accounts, Notifications, Printing, Ratings, Workouts,
Live-viewing, Haptics, Video, Multitasking. Components : les 7 catégories n'ont été
parcourues que sur les 5 organes desktop ci-dessus — les contrôles de sélection et
d'entrée n'ont pas été ouverts un par un.

Ce document ne prétend donc pas couvrir les HIG. Il couvre ce qui a été lu.

## Conformités mesurées

| Règle HIG | Preuve dans Sift |
|---|---|
| Motion — « Make motion optional » | ~~Conforme~~ — **affirmation retirée le 2026-08-05, elle était fausse.** Voir E5. |
| Typography — « avoid light font weights » | Aucun `font-weight` 100/200/300 dans les 1587 lignes de `styles.css`. |
| Progress indicators — « When possible, use a determinate progress indicator » | `frontend/progress-zone.ts:114,124` : `done/total` et pourcentage calculés, `role="progressbar"` + `aria-valuenow` (`:129`). |
| Progress indicators — « Keep progress indicators moving » | `:129` anime `transform:scaleX()`, conforme aussi à la règle projet « animer transform/opacity ». |
| Undo — « Let people undo multiple times » | Revert par ligne (`frontend/journal.ts:107`) et revert de masse par catégorie (`:123`), les trois catégories passant par `revert_batch` côté Rust (`:67`). |
| Undo — « Consider giving people the option to revert multiple changes at once » | Action de masse en en-tête (`journal.ts:123`) ; le pied « annuler le dernier batch » a été retiré comme redondant (audit 2026-07-05, noté `:111`). |
| Undo — « Show the results of an undo or redo » | `journal.ts:232` bascule la ligne en `jrnl-row--reverted` : le résultat est marqué là où l'action a eu lieu. |
| Undo — prévisibilité du résultat | Le revert hors ordre est refusé — **côté Rust**. `journal.ts:176-177` n'est qu'un bras de `humanError()` qui traduit l'erreur levée (`raw.includes("newer action")`) en « Action plus récente à annuler d'abord. ». Le libellé est là, l'invariant vit ailleurs. |
| Loading — « Let people do other things while they wait » | ~~Conforme~~ — **affirmation retirée le 2026-08-05, jamais mesurée.** Voir E6. |
| Writing — « Determine your app's voice » / « Create a list of common terms » | `docs/design-system/content.md` § Voix et § Vocabulaire Canonique existaient déjà. |

Convergence notable : `--text-base:13px` (`styles.css:94`) est exactement la taille par
défaut macOS des HIG (13 pt). Le calage avait été fait sans citer la source.

## Écarts

### E1 — Aucune variante de contraste augmenté

HIG Color : « If you define a custom color, make sure to supply light and dark variants,
**and an increased contrast option for each variant** ».
HIG Dark Mode chiffre la cible : contraste minimum 4,5:1, **7:1 visé pour les couleurs
personnalisées, en particulier sur du petit texte**.

Preuve : `prefers-contrast` — zéro occurrence dans `styles.css`. `forced-colors` non plus.
Sift a clair + sombre, pas de troisième registre.

### E2 — Deux tokens sous le plancher de lisibilité macOS — partiellement traité

`--text-3xs` est passé de 8px à 10px le 2026-08-05. C'était le seul cas sans arbitrage :
un unique consommateur (`.sift-slider-label`), deux libellés courts et fixes, ~64 px de
marge mesurés dans un bloc de 130, derrière un `<details>` replié. Et le pire écart du
dépôt — 20 % sous le plancher.

**Puis tranché entièrement le même jour** : Antoine a décidé de remonter toutes les
polices. Les 14 sites sous le plancher ont migré vers `--text-xs`, et `--text-2xs` comme
`--text-3xs` ont été retirés. `.nv-grp`, qui codait `9px` en dur, a migré aussi — c'est le
seul site qu'une édition de valeur de token aurait manqué. `.sift-tags-title`, sans aucun
consommateur, a été traitée dans le même geste.

⚠️ **Correction d'une erreur de ce document.** Il affirmait que `--h-32`/`--h-44` avaient
été « commentés plutôt que supprimés ». Faux : `styles.css:161` dit
« had zero consumers, **dropped** rather than declared speculatively » — ils ont été
supprimés, et documentés en prose. L'erreur a été propagée dans la consigne du chantier
avant qu'un agent ne la relève.

**Quatre échecs WCAG AA réels**, exhumés par la campagne de mesure E2 et corrigés le
2026-08-05. Aucun ne relevait d'un arbitrage : un seuil chiffré était franchi, et aucun
n'aurait été réparé en changeant la taille.

| Échec | Avant | Après |
|---|---|---|
| Mode Lot, `opacity:.6` sur la ligne « En analyse » (`batch-panel.ts`) | badge DUP 2,22:1 ; libellé d'échec 1,96:1 (clair) | 6,21:1 et 5,49:1 |
| Overlay « changer » de pochette (`library-detail.ts:50`) | 1,42:1 sur pochette sombre | 7,26:1 au pire cas |
| `.sift-filed-banner-path` sur fond sémantique | 4,37:1 (sombre) | passe AA via `--color-text-secondary` |
| Ton neutre (badge qualité, chip Écartés) | 4,39:1 (sombre) | `--color-text-secondary` remonté à 83,5 % de L, dans les **deux** chemins sombres |

Deux enseignements gravés dans le code au passage :

- **l'atténuation d'un état ne se fait pas à l'opacité.** Aucune valeur d'opacité ne
  franchit 4,5:1 sur ce texte avant ~0,92, où l'atténuation ne se voit plus. Le levier
  sûr est le token (`--color-text-secondary` au lieu de `primary`), qui garde 5,69:1 ;
- **« échec » est l'information qu'on n'a pas le droit d'estomper.** Un fichier dont
  l'analyse a échoué doit se voir mieux que les autres, pas moins bien.

### E2 (suite) — état d'origine de la mesure

HIG Typography et HIG Accessibility donnent la même table : macOS, taille par défaut
13 pt, **minimum 10 pt**.

`styles.css:94` déclare `--text-3xs:8px` et `--text-2xs:9px`. Usages recensés :

Recompte du 2026-08-05 (passe adverse — le premier comptage annonçait 20 usages et n'en
classait que 18) : `var(--text-2xs)` a **21** usages réels — 13 dans `styles.css`, 4 dans
`batch-panel.ts`, 3 dans `ecartes-view.ts`, 1 dans `library-detail.ts` — et
`var(--text-3xs)` en a **1**. Soit 22 franchissements du plancher.

- `--text-3xs` — **1 usage** : `.sift-slider-label` (`styles.css:929`).
- **Libellés de verdict (4)** — `.sift-chip-badge:1068`, et en style inline
  `DUPLICATE`/`DUP`/`FAKE` (`frontend/batch-panel.ts:340,361,380`).
- **En-têtes uppercase (6)** — `.sift-lib-thead:488`, `.sift-bt-head:638`,
  `.jrnl-session-hd:774`, `.jrnl-cat-label:746`, `.sift-tags-title:1122`,
  `.rb-session-toggle:782`.
- **Métadonnées en mono (4)** — `.sift-pz-count:255`, `.sift-bt-src:642`,
  `.jrnl-dest:755`, `.sift-filed-banner-path:1289`.
- **Glyphes d'icône (3)** — `frontend/ecartes-view.ts:22,24,25`. Ces trois-là portent
  `--text-2xs` sur le `<i class="ti …">`, pas sur le texte : le libellé du verdict
  (« tronqué », « faux », « à re-sourcer ») est rendu par `.sift-vchip`, qui déclare
  `font-size:var(--text-xs)` = 10px (`styles.css:971`). **Ils sont au plancher, pas
  dessous.**
- **Hors famille (4)** — `frontend/library-detail.ts:50` (overlay « changer » du bouton de
  pochette : aucun verdict, aucune information portée par la couleur),
  `batch-panel.ts:363` (libellé d'état de ligne en attente), `styles.css:784`
  (`.rb-session-selectall`), `styles.css:1242` (`.sift-kbd-hint-id`).

Correction de l'argument, qui était surétendu. Un en-tête de colonne à 9 px est un choix
de densité défendable pour un outil qui affiche 15k à 100k lignes. Un **libellé de
verdict** à 9 px ne l'est pas, et pour une raison qui n'est pas typographique : HIG Color
interdit de s'appuyer sur la seule couleur pour distinguer un état, et le label texte
`FAKE` est précisément ce qui rattrape le badge rouge pour un daltonien. Le rendre
illisible garde la forme de la compensation et en perd la fonction.

Mais cet argument ne porte que sur les **4** libellés ci-dessus. La première rédaction en
comptait 7 en y agrégeant les chips d'Écartés (qui sont à 10 px) et un overlay de
pochette (qui n'est pas un verdict) — gonflement d'une famille par sa conclusion.

### E3 — Cmd+Z inopérant sur la cible macOS — **CORRIGÉ le 2026-08-05**

HIG Undo : « people generally expect to initiate undo and redo in system-supported ways,
such as choosing the items in a macOS app's Edit menu, **using keyboard shortcuts on a
Mac** ».

État **avant** correctif, dans `installUndoShortcut()` (`frontend/filing.ts`) :

```ts
if (!(e.ctrlKey && (e.key === "z" || e.key === "Z"))) return;
```

`e.metaKey` n'était jamais testé, et le commentaire de la fonction disait « Ctrl+Z » sans
mentionner macOS. Sift publie des installeurs macOS. Sur cette cible, l'annulation au
clavier ne répondait pas. **Défaut fonctionnel, pas écart de documentation.**

⚠️ Ne pas re-citer de numéro de ligne pour cet état : le correctif a ajouté cinq lignes de
commentaire, donc les anciens `:597` et `:600` désignent aujourd'hui autre chose. La garde
actuelle est à `frontend/filing.ts:605`.

Aggravant : Sift n'a pas de barre de menus (`decorations: false`,
`src-tauri/tauri.conf.json:22`), donc le clavier est la seule voie système restante pour
l'annulation — l'autre voie HIG, le menu Édition, n'existe pas non plus.

**Correctif appliqué.** La garde teste désormais `(e.ctrlKey || e.metaKey)`. Les deux
modificateurs sont acceptés plutôt que de brancher sur `platform()` : ce lookup a un
chemin d'échec (`frontend/chrome.ts:194` retombe sur la disposition Windows quand il
lève), et un raccourci clavier est le mauvais endroit pour en hériter. `npx tsc --noEmit`
passe (exit 0).

Non vérifié dans l'app réelle : `filing.ts` vit dans `installLiveWiring()`, donc ni
`npm run dev` ni le Browser pane ne l'exécutent. La vérification demande la fenêtre
`tauri dev` — ou un Mac.

Découverte annexe, **non corrigée**, et rectifiée le 2026-08-05 : le bandeau d'indices
clavier de la Revue (`frontend/report-view.ts:364`) liste SPACE, ENTER, BKSP, HAUT/BAS et
ignore l'annulation. Mais affirmer « aucun indice » était faux :
`frontend/library-detail.ts:352` annonce « Annulable via Ctrl+Z. » dans la modale de mise
à la corbeille. La chaîne reste exacte après le correctif — la garde accepte `ctrlKey`
*ou* `metaKey`, donc Ctrl+Z fonctionne aussi sur Mac — mais elle n'est pas idiomatique
sur cette plateforme, où l'on attend ⌘Z. Deux questions de design distinctes, aucune
n'étant un défaut : compléter le bandeau, et adapter le libellé à la plateforme.

### E4 — Le zoom n'est pas atteignable — axe réécrit puis **CORRIGÉ le 2026-08-05**

Appliqué : `"zoomHotkeysEnabled": true` sur la fenêtre `main`
(`src-tauri/tauri.conf.json`) et `"core:webview:allow-set-webview-zoom"` dans
`src-tauri/capabilities/default.json`. `npm run check:security` passe — scope asset et CSP
inchangés.

⚠️ **Ne règle que l'atteignabilité.** Le clipping à fort zoom décrit plus bas reste
entier : c'est un problème de layout, pas de configuration.

HIG Accessibility : « give people the option to enlarge text by at least 200 percent ».

**Le raisonnement d'origine était faux.** Il disait : toute l'échelle typo est en px
(`styles.css:94`), donc le texte ne suit aucun réglage système. Le px **suit** le page
zoom — MDN, `devicePixelRatio` : « When a page is zoomed in, the size of a CSS pixel
increases ». WKWebView documente `pageZoom` comme « equivalent to web content setting the
CSS "zoom" property on all page content ». À 200 %, un `font-size:9px` occupe 18 px CSS.
Passer l'échelle en `rem` n'apporterait **rien** ici.

Le vrai défaut est **une ligne de configuration absente**, pas 94 lignes de tokens.

| | Windows | macOS |
|---|---|---|
| le px suit le page zoom | oui | oui |
| zoom atteignable par l'utilisateur | **non** | **non** |
| réglage système de taille de texte | **oui**, jusqu'à 225 % | **non** |
| bilan | écart faible à nul | **écart plein** |

Vérifié dans le dépôt :

- `zoomHotkeysEnabled` n'est déclaré nulle part dans `src-tauri/tauri.conf.json`, donc il
  vaut `false` — le défaut de Tauri. Or le défaut natif de WebView2 est l'inverse
  (`IsZoomControlEnabled` = `true`). **Sift retire donc un zoom que WebView2 offrait** :
  Ctrl+`+`, Ctrl+`-`, Ctrl+molette et le pinch ;
- `core:webview:allow-set-webview-zoom` est absent de `src-tauri/capabilities/default.json`.
  Même en activant le flag, le polyfill que Tauri injecte sur macOS échouerait faute de
  permission ;
- macOS n'a aucune voie de repli : WKWebView n'a pas de raccourci de zoom intégré, et
  Dynamic Type est une API d'adoption qu'un WKWebView embarqué ne suit pas ;
- Windows en a une, indépendante du point précédent : le `TextScaleFactor` du système
  (Accessibilité > Taille du texte, jusqu'à 225 %) est appliqué par le runtime WebView2 à
  tout le contenu, pas seulement au texte, et n'est pas gouverné par
  `IsZoomControlEnabled`.

**Second écart, que le chantier n'avait pas vu, et qui peut annuler le bénéfice Windows.**
À 225 %, la fenêtre 1200×820 (`tauri.conf.json:17-18`) devient un viewport d'environ
533×364 px CSS — très en dessous du `minWidth: 920` que Sift déclare lui-même (`:19`). Or
`frontend/styles.css` n'a **aucune** media query de largeur : ses quatre `@media` réels
sont un `prefers-color-scheme` (`:136`) et trois `prefers-reduced-motion`
(`:695`, `:1264`, `:1575`). Et `html,body` porte `overflow:hidden` (`:189`), donc rien ne
défile au niveau page. WCAG 2.1 SC 1.4.4 exige 200 % « without loss of content or
functionality ». Le risque n'est pas que le texte reste petit : c'est qu'il grossisse et
sorte du cadre sans barre de défilement.

**Correctif** : `zoomHotkeysEnabled: true` plus la permission de zoom. Deux lignes, qui
rendent le levier atteignable sur les deux cibles — et qui ne règlent **pas** le clipping,
lequel est un problème de layout.

Nom de la permission — **tranché le 2026-08-05, la réserve est levée**. C'est
`core:webview:allow-set-webview-zoom`, pas la forme courte `webview:…`. Trois preuves
concordantes :

1. `src-tauri/gen/schemas/desktop-schema.json` — le schéma que le validateur utilise —
   contient cette chaîne, et **uniquement** celle-ci ;
2. les neuf permissions déjà déclarées dans `src-tauri/capabilities/default.json` suivent
   la même règle : préfixe `core:` pour les modules cœur (`core:default`,
   `core:window:allow-minimize`…), nom nu pour les plugins (`dialog:default`,
   `window-state:default`) — et `webview` est un module cœur, comme `window` ;
3. Tauri est épinglé en 2.11.5 (`Cargo.lock`), bien après l'introduction du préfixe
   `core:` — la forme courte appartient à la période bêta de la v2, ce qui explique les
   pages de documentation contradictoires.

### E5 — La garde `prefers-reduced-motion` ne couvre presque rien — **CORRIGÉ le 2026-08-05**

Correctif **additif** : un unique bloc `@media (prefers-reduced-motion:reduce)` en fin de
`frontend/styles.css`, plutôt que d'extraire 38 déclarations vivantes dans des blocs
`no-preference` — aucune règle existante ne bouge, et le pire cas d'une erreur est une
durée trop courte, pas un état de fin perdu.

Deux précautions qui valent d'être retenues :

- on neutralise la **durée**, pas la propriété. `animation:none` supprimerait l'événement
  `animationend` dont `filing-identify.ts` dépend pour retirer `.sift-identified-flash` et
  `.sift-applytags-flash` — les classes resteraient collées à vie ;
- les spinners sont **exemptés dans le sélecteur**
  (`*:not(.sift-spin):not(.sift-bt-spin):not(.jrnl-toast)`), pas rejoués en dessous avec
  leurs valeurs. Un indicateur figé se lit comme une application plantée, ce que HIG
  Progress indicators proscrit explicitement — la règle « rendre le mouvement optionnel »
  ne s'applique pas à un mouvement qui *porte de l'information*.

Le commentaire trompeur de `styles.css:125` a été corrigé dans le même geste.

**Écart trouvé en corrigeant une fausse conformité de ce document même** (passe adverse du
2026-08-05).

HIG Motion : « Make motion optional. Not everyone can or wants to experience the motion in
your app. »

Ce document affirmait que « tout est sous `@media (prefers-reduced-motion:no-preference)` ».
Recompté sur `frontend/styles.css` :

- 35 déclarations `transition:` et 9 `animation:` ;
- **2** blocs `prefers-reduced-motion:no-preference` (`:695`, `:1264`), qui contiennent
  3 règles en tout ;
- **1** seule règle neutralisée sous `reduce` (`:1575`, `.sift-usage-prow.flash`) ;
- les tokens `--duration-*` / `--ease-out` ont **2** consommateurs dans tout le dépôt.

Animent donc sans garde, entre autres : `.sift-pz-fill` (la barre de progression — citée
comme conforme deux lignes plus haut dans ce même tableau), `.sift-seg-thumb`,
`.sift-spectro-body`, `.tog`, `.lr`, et six `animation:` dont `.sift-spin`, `.jrnl-toast`,
`.sift-identified-flash`.

**Origine de l'erreur, à retenir.** Le commentaire `styles.css:125` dit « EVERY consumer of
these tokens sits inside `@media (prefers-reduced-motion: no-preference)` ». C'est vrai —
et sans portée, puisque ces tokens ont deux consommateurs. J'ai lu une déclaration
d'intention et l'ai enregistrée comme une mesure, dans un tableau intitulé « Conformités
mesurées ».

### E6 — Aucune commande IPC asynchrone

Même origine : conformité affirmée, jamais mesurée.

HIG Loading : « Let people do other things in your app while they wait for content to
load. »

Mesuré : `pub async fn` dans `src-tauri/src/ipc.rs`, `ipc_filing.rs`, `ipc_identify.rs`,
`ipc_library.rs`, `ipc_usage.rs`, `ipc_usb.rs` — **zéro**, sur 71 `pub fn` derrière
`#[tauri::command]`. C'est cohérent avec l'architecture du projet (aucun runtime async,
`CLAUDE.md` § Backend), mais l'absence d'async ne dit rien sur la réactivité : ce qui
compte est de savoir quelles commandes font un travail long **en ligne**.

**Mesuré le 2026-08-05, la piste est confirmée.** `ipc.rs` déclare
`#[tauri::command] pub fn analyze_path(...)` — synchrone — et sur cache miss il appelle
`analyse()` en ligne, puis écrit le rapport en base pour que l'ouverture suivante soit
instantanée. Une analyse froide s'exécute donc entièrement dans une commande synchrone.

**Non corrigé, et ce n'est pas un oubli.** Les deux issues sortent du périmètre d'un
correctif : passer la commande en `async` contredit frontalement `CLAUDE.md` § Backend
(« Aucun runtime async. Ne pas proposer de patterns async »), et la déporter sur un thread
avec un événement de retour change le contrat IPC de l'écran Revue. C'est une décision
d'architecture, pas une retouche.

Ce qui manque encore, et qui décide de l'urgence : **combien de temps** dure une analyse
froide sur un fichier réel. Le dépôt a déjà l'outil pour le dire —
`bench_sqlite.rs` mesure le coût d'une analyse sur de vrais fichiers via
`SIFT_BENCH_TRACKS_DIR` (`--ignored`). Tant que ce chiffre n'existe pas, « ça gèle l'IPC »
reste vrai en droit et inconnu en amplitude.

### E7 — Accueil est le seul écran sans rien à montrer pendant qu'il charge — **CORRIGÉ le 2026-08-05**

Placeholder ajouté dans `renderHomeSources()`, calqué sur le motif existant
(`queue-panel.ts`, `bibliotheque-view.ts`, `ecartes-view.ts`) avec le même
`ti-loader sift-spin`. Deux effets de bord traités dans la foulée :

- un garde contre le re-flash à chaque re-render — `renderHomeSources()` est rappelée par
  `refresh()` sur événement backend, sans quoi le rail blanchirait à chaque fois ;
- le chemin d'erreur, qui faisait `console.error` puis `return` sec : le spinner aurait
  tourné indéfiniment. Un spinner permanent est un échec silencieux, ce que le projet
  interdit. Remplacé par une carte d'erreur avec « Réessayer ».

HIG Loading : « Show something as soon as possible. If you make people wait for loading to
complete before displaying anything, **they can interpret the lack of content as a problem
with your app**. »

C'est le premier écran que voit un utilisateur — `app.js:22` fixe `view="home"`, il n'y a
ni mémoire du dernier écran ni choix. Et `renderHomeSources()` (`frontend/home-sources.ts`)
ne peint **aucun** placeholder : les deux colonnes restent littéralement vides jusqu'à
résolution de `listSources()`. `grep -c "Chargement" home-sources.ts` renvoie **0**, alors
que `bibliotheque-view.ts:254`, `ecartes-view.ts:108` et `queue-panel.ts:425` affichent
tous un « Chargement… » avec spinner.

L'écran qui a le plus besoin de rassurer est donc le seul à ne rien montrer.

Observations connexes du même relevé, moins graves, non traitées :

- Accueil est aussi le seul écran live **sans titre de page** : la branche maquette pose
  un `.h1` (`app.js:91`), la branche Tauri écrit une coquille à deux colonnes sans lui
  (`app.js:106`) ;
- **aucun texte de l'interface ne dit ce que fait Sift ni par quoi commencer.** Ce n'est
  pas un défaut au sens des HIG — « Ideally, people can understand your app simply by
  experiencing it » — mais c'est un pari, et il n'est adossé à aucune astuce contextuelle ;
- l'avertissement « Racine de bibliothèque non définie » (`home-sources.ts:116-122`)
  s'affiche dès le premier lancement, au sujet d'une notion que rien n'a présentée. Sa
  **forme** est en revanche exactement celle que les HIG préconisent : bloc en ligne,
  masquable, avec un lien vers Réglages — et non une alerte modale au démarrage, que HIG
  Alerts proscrit.

## Divergences assumées

Elles contredisent une règle HIG et **ne doivent pas être « corrigées »**. Elles sont
listées ici pour cesser d'être des oublis apparents.

### D1 — Sift expose un réglage de thème applicatif

HIG Dark Mode : « **Avoid offering an app-specific appearance setting.** »

Sift en a un : `ui_theme` (`frontend/reglages-view.ts:46-51`, bloc rendu `:280`),
`ThemeChoice` dans `frontend/theme.ts`.

Atténuation réelle : le défaut est `auto` (`reglages-view.ts:46`), donc le système est
respecté tant que l'utilisateur ne demande rien. Justification : Sift cible aussi Windows,
où la bascule de thème applicative est une convention courante, et la règle d'Apple
suppose un utilisateur qui n'a qu'un seul OS à régler.

### D2 — Jargon anglais conservé

HIG Writing : « Choose simple, plain language [...] **avoiding jargon** ».

Sift garde délibérément LOSSLESS, DUPLICATE, MATCH, CHECK MATCH, FAKE, kbps, kHz, MP3,
AIFF, WAV (`CLAUDE.md`). Ce n'est pas du jargon d'implémentation : c'est le vocabulaire
professionnel des DJ, sa cible. Traduire dégraderait la reconnaissance.

### D3 — Liquid Glass hors de portée

HIG Materials décrit Liquid Glass comme la couche fonctionnelle qui flotte au-dessus de
la couche contenu, et prescrit de « différencier les contrôles du contenu ».

C'est un matériau système natif, non reproductible dans une WebView sans imitation
coûteuse. Le **principe** — séparer visuellement la couche des contrôles de celle du
contenu — est en revanche transférable, et il est en **tension** avec le pattern
« Surface Continue » de `docs/design-system/patterns.md`, qui pose l'inverse : les
contenus reposent sur le fond de l'application, les groupes se forment par l'espacement.
Tension à trancher, pas à résoudre par défaut.

## Tension à documenter, pas à corriger

HIG Alerts : « **Avoid displaying alerts for common, undoable actions, even when they're
destructive.** » et HIG Feedback : « don't warn people when data loss is the expected
result of their action ».

Sift confirme les lots au-delà de `BATCH_CONFIRM_THRESHOLD = 10`
(`frontend/batch-panel.ts:42,905`), avec armement et horodatage anti-double-clic. Or le
rangement Sift **est** annulable (journal + `revert_batch`). Lu littéralement, HIG
demanderait de retirer cette confirmation.

Elle existe pour une raison qui prime : un clic synthétique a déjà traversé un
`window.confirm()` et rangé 265 pistes. Le garde-fou de `CLAUDE.md` (« jamais
`window.confirm()` ; confirmation in-app armée et horodatée ») est né de cet incident.

Arbitrage, après correction d'E3. L'hypothèse des HIG — une annulation atteignable — est
maintenant vérifiée sur les deux cibles. **La confirmation reste malgré tout**, parce
qu'elle n'a jamais visé la réversibilité : elle vise un clic qui n'est pas humain. Les
HIG raisonnent sur un utilisateur qui décide ; le garde-fou existe pour le cas où
personne ne décide. Les deux règles ne parlent pas de la même chose.

Ce qui peut bouger : le seuil, et le fait de traiter au même niveau une action annulable
et une action irréversible. Deux réglages, pas une suppression.

## Règles retenues comme non transposables

À ne pas replier dans le design system, sous peine de produire une app qui n'est native
nulle part (Sift cible Windows **et** macOS) :

- barre de menus globale (organe absent de Windows ; `decorations: false` ici) ;
- plein écran comme Space dédié ;
- position des boutons de fenêtre ;
- Dynamic Type (API système) ;
- SF Symbols (Sift utilise Tabler ; licence à vérifier avant tout usage) ;
- `backgroundExtensionEffect()` / scroll edge effect (API SwiftUI).

Test à appliquer phrase par phrase : la règle nomme-t-elle un **organe du système** ou un
**fait humain ou matériel** ? Le premier est lié à la plateforme, le second est universel.
