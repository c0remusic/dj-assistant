# Sift Design System - Tokens

> Cette page cartographie les familles de tokens. Les valeurs exactes vivent dans
> `frontend/styles.css`; ne pas recopier un thème parallèle ici.

## Canon

Les tokens sont déclarés dans `frontend/styles.css`, principalement dans
`:root`, `@media (prefers-color-scheme: dark)` et `:root[data-theme="dark"]`.

Règle : tout nouveau style durable doit utiliser un token existant ou ajouter un
token dans `styles.css` avec un rôle clair. Les valeurs hardcodées sont
acceptables seulement pour une mesure locale non thémable, jamais pour une
couleur d'état ou une surface.

## Couleurs

### Surfaces

Tokens principaux :

- `--color-background-primary`
- `--color-background-secondary`
- `--color-background-tertiary`
- `--color-background-queue`
- `--color-surface-raised`
- `--color-track`
- `--color-row-active`
- `--color-nav-active`

Usage :

- `primary` : fond principal de l'espace de travail, **et les champs**, creusés
  (convention du 2026-07-10, `styles.css:1372-1375`) ;
- `tertiary` / `queue` : rail, chrome latéral, file de morceaux **et cartes groupées** —
  même valeur depuis la fusion du 2026-08-05 ;
- `secondary` : contrôle monté, c'est-à-dire ancré dans la charpente (issue #8) ;
- `surface-raised` : cran saillant d'un contrôle monté (état actif, déclencheur) et
  surface flottante. ⚠️ **Pas** réservé au flottant : il peint aussi `.sift-play-btn`,
  `.sift-seg-thumb`, `.sift-settings-btn`, `.sift-ident-search-btn` ;
- `track` : rail creusé d'un segmented control ;
- `row-active` / `nav-active` : sélection, hover structurel, état courant.

### États Sémantiques

Tokens principaux :

- `--color-background-info`
- `--color-background-danger`
- `--color-background-success`
- `--color-background-warning`
- `--color-text-info`
- `--color-text-danger`
- `--color-text-success`
- `--color-text-warning`

Règle UX : un état permanent reste sobre. La couleur sémantique doit signaler
un risque, un blocage ou une confirmation utile, pas décorer une zone déjà
comprise.

### Bordures Et Overlays

Tokens principaux :

- `--color-border-tertiary`
- `--color-border-secondary`
- `--color-border-info`
- `--color-border-danger`
- `--overlay-hover`
- `--overlay-selected`
- `--overlay-alt` — fonds alternés d'une table (2026-09-10 : un cran au-dessus de
  `--overlay-hover`, clair .045 / sombre .05), posé par parité d'index, jamais `:nth-child`
- `--overlay-drop`
- `--overlay-wave-hover`

Les overlays sont préférables aux aplats colorés pour les états subtils :
sélection, survol, hover de waveform.

**Survol — tranché le 2026-09-09 (audit de fin de #24, décision d'Antoine).** Un seul
traitement : `--overlay-hover` en aplat, sur ce qui agit et n'est pas plein — rangées de
colonne (`.fld`), entrées du rail (`.nv`), boutons discrets (transparents), candidats. **Un push
button n'a aucun survol** (kit Big Sur § 02 : Normal / Disabled seulement ; le survol n'existe
que sur les scope buttons et les icon buttons). **Une rangée de table n'a aucun survol** non plus
(2026-09-10, Finder). `filter: brightness()` n'existe plus sur un `:hover` — il assombrissait
l'encre en sombre et n'était ni un aplat ni un token. Le survol ne peint jamais le plan de
l'actif : `.nv:hover` peignait `--color-nav-active`, survolée et active se confondaient.

**Segment libre d'une barre de donnée** : `--color-border-secondary` (alpha), pas
`--color-track` — ce dernier vaut le fond de page en sombre et disparaît au sol (Clé USB,
2026-09-09).

### Contraste

Les HIG demandent, pour toute couleur personnalisée, une variante de **contraste
augmenté** en plus des variantes claire et sombre, et chiffrent la cible : 4,5:1 au
minimum, 7:1 visé sur du petit texte.

`styles.css` n'a aucune règle `prefers-contrast` ni `forced-colors` — ce troisième
registre n'existe pas. Écart ouvert (E1), sans étape planifiée : le définir est un
chantier à part entière, pas une retouche.

## Typographie

Police UI canonique : `--font-ui`.

Police mono canonique : `--font-mono`, réservée aux **chiffres qui s'alignent en colonne**
(durées, BPM, mesures, `tabular-nums`). ⚠️ Cette page disait « chemins, formats, noms de
fichier » : faux depuis `DESIGN.md` § 2 (2026-08-21) — un chemin, un format ou un nom de fichier
se rend en `--font-ui`. Dernier site corrigé : la colonne Fichier d'À re-sourcer (2026-09-10).

**Encres neutres — écart rouvert le 2026-09-09.** Mesuré dans la vraie fenêtre : secondary et
tertiary étaient à 1,6 point de clarté en sombre (83,5 / 81,92) et 0,75 en clair (45,4 / 46,15),
la même encre à l'écran, alors que toute la hiérarchie des rangées (libellé ↔ valeur, `.col-h`,
comptes) repose sur ces deux crans. Valeurs depuis : sombre 95,9 / 83,5 / **79** / 76,72 ; clair
31,16 / **41** / 46,15 / 50,30. En sombre c'est tertiary qui descend (76 tombait à 4,23:1 sur le
plan de la file) ; en clair le sol à 86,22 % interdit de descendre tertiary, donc c'est secondary
qui monte en contraste. AA (4,5:1) tenu sur les cinq écrans peuplés, sauf le bouton primaire
(blanc sur `--color-accent-fill`, 3,65 sombre / 4,02 clair — antérieur, non tranché).

Règles :

- pas de typo hero dans les panneaux compacts ;
- les titres de section doivent être courts et scannables ;
- les valeurs techniques doivent privilégier la lisibilité tabulaire ;
- ne pas utiliser le poids fort comme substitut à la hiérarchie spatiale ;
- pas de graisse légère : les HIG l'écartent explicitement, et `styles.css` n'en
  contient aucune (100/200/300 absents) — le rester.

### Plancher De Lisibilité

Les HIG donnent pour macOS une taille par défaut de 13 pt et un **minimum de 10 pt**.
`--text-base` vaut exactement 13px ; `--text-2xs` (9px) et `--text-3xs` (8px) passent
sous ce plancher.

Les usages sont recensés et classés par famille dans
`docs/superpowers/changes/2026-08-05-hig/design.md` § E2 : en-têtes uppercase,
métadonnées mono, badges de verdict. Les trois n'ont pas la même gravité. La densité
d'un en-tête de colonne est défendable pour un outil qui affiche 15k à 100k lignes ; un
**badge de verdict** ne l'est pas, et pour une raison qui n'est pas typographique : le
libellé texte (`FAKE`, `DUPLICATE`) est ce qui rattrape le code couleur pour un
daltonien. Le rendre illisible garde la forme de la compensation et en perd la fonction.

**Tranché le 2026-08-05 : les deux tokens n'existent plus.** Antoine a décidé de remonter
toutes les polices au plancher. Le geste n'a pas été d'éditer leur valeur — les porter à
10px aurait donné trois barreaux identiques dans l'échelle — mais de **migrer les 14 sites
vers `--text-xs`** puis de retirer les déclarations. `.nv-grp`, qui codait `9px` en dur et
qu'une édition de token aurait manqué, a migré aussi.

L'échelle commence donc à `--text-xs:10px`, et le plancher HIG est tenu dans
`styles.css`.

**Mesures du 2026-08-05, à lire avant de trancher.** Elles retirent deux arguments que
tout le monde aurait employés de bonne foi.

- **Le contraste n'est pas le problème.** Les six en-têtes uppercase passent AA dans les
  deux thèmes, au repos comme au survol, avec 22 % à 88 % de marge (pire cas 5,50:1 en
  clair, 8,45:1 en sombre). Les badges de verdict aussi (FAKE 5,00:1 dans les deux
  thèmes, DUPLICATE 6,24 / 5,00). Le 9 px de Sift n'est pas un 9 px pâle : les paires
  ton-sur-ton ont été calibrées à ~5:1 dès l'origine. **Remonter à 10 px ne change aucun
  ratio** — la taille n'entre pas dans le calcul WCAG en dessous du seuil « large text »
  (18 pt, ou 14 pt gras), dont on est très loin.
- **L'argument densité est faux sur les en-têtes.** Aucun des six n'est un élément
  par-ligne : un par tableau, un par run, un par session, un par catégorie. Le coût est en
  O(nombre de groupes), pas O(nombre de lignes) — à 15k ou 100k pistes, passer à 10 px ne
  coûte **pas une seule ligne** de liste. Le Journal n'est même pas virtualisé. Si l'on
  garde 9 px ici, la divergence documentée doit dire la vraie raison — *hiérarchie
  typographique : un en-tête de groupe reste subordonné à ses lignes de données* — et
  surtout pas « densité ». Une raison fausse gravée dans le design system se retourne
  contre la revue suivante.

Deux contraintes mécaniques à ne pas rater :

- **le token ne doit pas bouger.** `--text-2xs` a 13 consommateurs dans `styles.css` seul ;
  le porter à 10 px le rendrait identique à `--text-xs` et écraserait un cran de l'échelle.
  Le geste correct est par sélecteur. À noter : `.nv-grp` (`styles.css:224`) code
  `font-size:9px` **en dur** et contourne le token — une édition de token le raterait ;
- `.sift-tags-title` (`styles.css:1122`) n'a **aucun consommateur** dans le dépôt. Lui
  appliquer une décision de taille serait décider du sort d'un élément qui ne s'affiche
  nulle part. La supprimer est une décision, pas un correctif.

## Espacement

Six paliers déclarés (`frontend/styles.css:97`) :

- `--space-4`
- `--space-8`
- `--space-12`
- `--space-16`
- `--space-24`
- `--space-32`

Rôles :

- 4 : micro-gap entre icône, label, métadonnées ;
- 8 : groupe compact, ligne, chip ;
- 12 : respiration interne d'un module ;
- 16 : séparation entre sections ;
- 24 et 32 : respirations de haut niveau, entre blocs d'écran. 13 usages réels — cette
  page n'en listait que quatre paliers jusqu'au 2026-08-05 et attribuait à 16 seul un
  rôle que 24 et 32 assument aussi.

Si une zone paraît dense, augmenter d'abord l'espacement entre groupes, pas la
taille des cartes.

## Radius Et Hauteurs

Le radius sert à distinguer les éléments interactifs, les badges et les panneaux,
pas à rendre toute la page "douce".

⚠️ **`--h-40` est un token mort.** Il est déclaré (`frontend/styles.css:115`) et n'a
**aucun** consommateur : `var(--h-40)` a zéro occurrence dans tout `frontend/`, et aucune
règle `height:40px` ne le remplace. Cette page le présentait comme « hauteur canonique du
contrôle principal » — c'était faux, corrigé le 2026-08-05. Le commentaire
`styles.css:111` qui l'accompagne est lui aussi périmé. Ne pas s'en servir comme
référence ; ne pas le supprimer sans décision (une valeur morte ne coûte rien, une
suppression peut casser un usage futur déjà prévu).

Règle : les dimensions de contrôles répétés doivent être stables. Un hover,
un état actif ou un libellé long ne doit jamais faire bouger le layout.

**Deux familles de boutons, et deux seulement — tranché le 2026-09-09** (audit de fin de #24 :
cinq gabarits mesurés, 22 / 23 / 28 / 30 / 31 px, pour ce que le kit Big Sur § 02 rend en Large
et Small) :

| Famille | Hauteur | Police | Rayon | Porteurs |
|---|---|---|---|---|
| Large | `--h-control` 28 | `--text-base` 13 | `--border-radius-md` 7 | `.sift-bar-btn`, `.sift-ranger-btn`, rail d'action de Revue, `.sift-usage-btn`, `.sift-usage-disclose` |
| Small | `--h-control-sm` 22 | `--text-sm` 11 | `--border-radius-md` 7 | `.sift-settings-btn`, `.sift-meta-ident-btn`, `.sift-qfoot-go`, `.sift-empty-link`, `.sift-id-btn` |

Le rayon 5 (`--border-radius-sm`) reste aux champs et aux segmentés, jamais à un push.

### Géométrie Concentrique

Tranché le 2026-08-14 (issue #26), portée bornée le 2026-08-19 (commentaire de correction
sur #26). Deux règles, qui ne s'appliquent pas au même endroit :

> Un rayon **de surface** se choisit dans l'échelle. Un rayon **imbriqué serré** — un élément
> dans une barre, une pastille dans une carte — se calcule à partir de celui de son conteneur
> moins l'inset. **La règle ne remonte pas jusqu'à la coque** : au-delà de quelques pixels
> d'inset, la concentricité ne décrit plus rien, et Apple ne la réclame pas.

`calc(<rayon du conteneur> - <inset>)`. C'est ce qui fait « parenter » visuellement deux
écrans sans rapport : les courbes intérieure et extérieure restent concentriques au lieu de
diverger.

Trois choses à ne pas confondre :

- **Apple énonce une propriété, pas une arithmétique.** `/toolbars` § Best practices dit que
  les boutons, champs, en-têtes et pieds standard ont des rayons *concentriques* avec ceux de
  la barre, et qu'un composant custom doit l'être aussi. Aucune formule n'est publiée, et
  `/layout` n'en parle pas du tout (vérifié : une seule occurrence de « corner radius », sur le
  châssis des iPhone). La soustraction est la **définition géométrique** de concentrique, pas
  une valeur Apple recopiée — ce que `governance.md` interdirait de toute façon.
- **« By default » n'existe pas ici.** Apple l'obtient gratuitement de ses composants système ;
  sa consigne ne s'active que pour du custom. L'issue #25 a établi que Sift n'emprunte aucun
  kit : **tout** y est custom, donc la règle n'est jamais gratuite et se tient à la main.
- **L'échelle ci-dessus n'est pas remplacée.** `sm`/`md`/`lg` dérivent d'une base unique par
  deltas fixes (`styles.css:112-113`) : c'est un jeu fermé de 4, pour les surfaces. La règle
  concentrique, elle, calcule, et peut tomber n'importe où. Les deux cohabitent parce
  qu'elles ne s'adressent pas au même objet.
- **Ce que la généralisation produisait chez Sift, mesuré le 2026-08-19** : `#content` pose un
  inset de 24 px (`styles.css:402`) et la première carte porte 14 px ; « 14 = R − 24 »
  exigerait une fenêtre à 38 px — elle est à 0, aucun rayon de coque n'étant posé nulle part,
  ni en CSS ni par `DWMWA_WINDOW_CORNER_PREFERENCE` (question distincte, ouverte en #41).
  C'est cet écart, maximal et à l'endroit le plus visible de l'app, qui a fait borner la
  règle à sa source : une barre et ses éléments, pas la chaîne fenêtre → surface.

**Nettoyé le 2026-08-20.** Le compte de 12 était faux : `styles.css` en portait **34**. Les 22
qui doublonnaient un token sont passées dessus, toutes à géométrie **identique au pixel** —
11 pilules (un rayon déjà ≥ la moitié du petit côté : pistes de 3 à 6 px de haut, pouce
d'ascenseur, piste du switch, poignée `.qdrag`), 10 cercles sur des boîtes carrées à taille
fixe (`50%` et `999px` rendent le même disque dès que la boîte est carrée) et un `8px` qui
était `--border-radius-sm` au chiffre près.

Les **12 qui restent** ne doublonnent rien, et c'est pour ça qu'elles restent :

| Littéral | Sites | Pourquoi il reste |
|---|---|---|
| `0` | `.sift-qfoot-btn`, `.jrnl-group-hd`, `.sift-usage-seg` | remise à zéro explicite ; `0` n'est pas un cran de l'échelle. Le troisième est une correction datée (2026-08-19) avec sa justification en place |
| `3px` | `.sift-pz-cancel` (15×15), `.kbd`, `.sift-identified-cover`/`-noart` (28×28), `.sift-usage-swatch` (12×12), `.sift-usage-lg:focus-visible` | boîtes trop petites pour `sm` : à 8 px, un carré de 12 à 15 px devient une pastille. L'anneau de focus, lui, n'a pas de boîte à lui — son rayon est propre au site, comme son offset |
| `1px` / `1px 1px 0 0` | `.bars span`, `.spec span` | barres d'égaliseur : adoucissement d'un pixel, aucun cran à moins de 7 px — et la seconde a **quatre** valeurs, qu'aucun token n'exprime |
| `6px` | `.cov` (40×40) | à 2 px de `sm`. Écart réel, pas nul : l'aligner unifierait la vignette avec `.sift-cover-frame` (68×68, `sm`), mais c'est une décision de surface, pas une exécution |

Aucun de ces 12 n'est un rayon **imbriqué serré** : les deux seules paires « barre et ses
éléments » du fichier — `.sift-seg`/`.sift-seg-opt`+`.sift-seg-thumb` et
`.sift-usage-bar`/`.sift-usage-seg` — sont déjà tokenisées et satisfont déjà la règle
concentrique ci-dessus.

## Mesures

Tranché le 2026-08-13 (issue #9). Un **jeu fermé de trois mesures**, adressées par rôle :

| Rôle | Mesure |
|---|---|
| donnée | 1200 |
| carte et formulaire | 560 |
| dialogue | 760 |

Aucune n'a été inventée : les trois existaient déjà dans le dépôt sans être écrites nulle
part. **La mesure borne la surface, pas le contenu.**

⚠️ **Seule la mesure « donnée » est déclarée**, comme `--measure-data` (`styles.css`), et son
nombre est **dupliqué côté Rust** dans `analysis::spectrum::MAX_COLS` — c'est le plafond de
colonnes du spectrogramme, donc la largeur au-delà de laquelle la donnée serait étirée et se
présenterait comme mesurée. Le désaccord serait silencieux, d'où un test Rust qui **lit
`styles.css`** et compare : `analysis::spectrum::tests::css_data_measure_matches_max_cols`
(issue #30, 2026-08-14). Éditer l'un sans l'autre fait tomber `cargo test`.

**`--measure-form` (560) est déclarée depuis le 2026-09-09** : son premier consommateur est le
panneau de Réglages (`.sift-settings-panel`, déclinaison #24, septième écran), où elle a
remplacé le `max-width:560px` en dur de `.sift-settings-stack`. La mesure « dialogue » (760)
n'est toujours **pas** déclarée : un token sans consommateur est un token mort (voir `--h-40`
plus haut). Elle se déclare au moment où on l'applique.

**Réserve héritée de #9, non tranchée** : 560 px porte 99 à 107 caractères par ligne
(mesuré en **Outfit** 400, 12–13 px — ⚠️ chiffre à refaire, `--font-ui` est passé à **Inter** le
2026-08-26 et sa chasse diffère par casse : plus large en capitales, plus étroite en minuscules),
bien au-dessus de la fourchette confortable. La mesure bornant la
surface et non le contenu, une carte à 560 qui porterait de la prose n'est pas protégée.

## Ombres

Les ombres sont rares. Sur Sift, elles servent surtout à détacher une surface
flottante utile. Si le panneau est déjà séparé par position, couleur ou bordure,
ne pas ajouter d'ombre.

Cas récent : le panneau File flottant ne doit pas porter d'ombre s'il est déjà
compris comme une surface latéralement séparée.

## Mouvement

Racine tranchée par l'issue #10 (2026-08-19), appliquée le 2026-08-20 : échelle à
trois crans, une seule courbe.

| Token | Valeur | Rôle |
|---|---|---|
| `--duration-fast` | 75 ms | micro — survol, focus, pression, bascule |
| `--duration-base` | 150 ms | **référence** — tout retour d'action |
| `--duration-slow` | 300 ms | ample — déplacement de matière : panneau, pouce, progression |

La référence est **mesurée, pas choisie** : 150 ms était la médiane du cluster portant
30 des 44 déclarations de `transition` d'alors. Courbe unique `--ease-out`.

Deux règles au-dessus de l'échelle :

- **zéro mouvement sur le geste fréquent** (HIG `/motion`) — le pas de la file de
  Revue, répété des centaines de fois par session, n'a droit à aucun cran ;
- seuls `transform` et `opacity` s'animent (CLAUDE.md § Front) — les exceptions sont
  des mécanismes nommés dans `styles.css`, pas de la décoration.

## Mise À Jour

Quand un token change :

1. modifier `frontend/styles.css` ;
2. vérifier les composants touchés dans `docs/design-system-states.md` ;
3. mettre à jour cette page seulement si le rôle du token change ;
4. vérifier l'app réelle, pas seulement une maquette HTML.
