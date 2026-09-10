# Sift Design System - Components

> Catalogue orienté usage. Pour les états précis, consulter
> `docs/design-system-states.md`.

## Shell

Sources principales :

- `index.html` (racine du dépôt) — le markup du rail y est **écrit en dur**, lignes 13+ :
  `<div class="nv" … data-view="…">`. Ce fichier manquait à cette liste jusqu'au
  2026-08-05 ;
- `frontend/chrome.ts` — title bar, routage, badges ;
- `frontend/styles.css`

Rôles :

- title bar native/custom ;
- rail de navigation ;
- espace central ;
- panneaux contextuels.

Règles :

- la title bar doit prolonger visuellement le rail gauche et le fond principal ;
- les séparations doivent être continues, sans décalage ni double bordure ;
- le rail est une structure permanente, pas une carte ;
- la navigation active reste neutre et lisible ;
- le survol d'une entrée du rail est `--overlay-hover`, jamais le plan de l'entrée active
  (`--color-nav-active`) : jusqu'au 2026-09-09, survolée et active se confondaient.

**Colonne au plan de la file + zone principale — la grammaire partagée de trois écrans
(Rekordbox 2026-09-08, Clé USB et Réglages 2026-09-09).** Une colonne B′ à `--pane-w`, au plan
de la file de Revue (`--color-background-queue`), bord à bord et pleine hauteur (marges
négatives sur l'inset de `#content`) ; une zone principale qui défile chez elle, bornée à
`--measure-data` (donnée : Rekordbox, Clé USB) ou `--measure-form` (formulaire : Réglages) ;
des entrées `.fld` avec compte ou capacité à droite. Une seule règle CSS (`.sift-rkb-layout`
et ses co-sélecteurs `.sift-usb-*`, `.sift-settings-*`) — le jour où un quatrième écran arrive,
c'est elle qui prend un nom générique. Références : Utilitaire de disque (sidebar des appareils,
infos à droite, action destructrice en sheet), Réglages Système (catégories, panneau étroit,
application immédiate).

## File De Morceaux

Sources principales :

- `frontend/queue-panel.ts` — le vrai propriétaire : virtualisation, navigation clavier,
  recherche, rendu des lignes, état Détail/Lot. Cette page citait `chrome.ts`, qui ne
  contient rien de la file (corrigé le 2026-08-05) ;
- `frontend/styles.css`

Rôles :

- montrer le contexte de traitement ;
- permettre la sélection rapide ;
- garder la progression mentale dans un lot.

Règles :

- le panneau peut être flottant/collapsible ;
- pas d'ombre portée si la position et le fond suffisent ;
- la ligne active doit être évidente sans écraser la liste ;
- les noms longs doivent rester scannables.

## Hero Lecteur

Sources principales :

- `frontend/report-view.ts`
- classes `.sift-player-row`, waveform, sliders.

Rôles :

- donner l'identité du morceau ;
- permettre l'écoute rapide ;
- montrer le signal audio au premier niveau.

Règles :

- le hero est la première ancre de l'écran Revue ;
- il peut être plus grand que les sections d'analyse ;
- waveform, lecture, volume, key-lock et tempo appartiennent au même groupe ;
- ne pas transformer le lecteur en carte décorative isolée.

## Sections Revue

Sources principales :

- `frontend/report-view.ts`
- `frontend/filing.ts`
- `frontend/styles.css`

Sections canoniques :

- Diagnostic audio — libellé réellement rendu (`report-view.ts:570`) ;
- Métadonnées — libellé réellement rendu (`filing-identify.ts:430`) ;
- ⚠️ **« À finaliser » n'est rendu nulle part.** Le libellé n'existe que dans des
  commentaires, et `report-view.ts:541-542` documente que cet état *a été essayé puis
  retiré* sur retour utilisateur (« on ne comprend pas ce qui reste à finaliser ?
  Redondant ? »). Cette page le listait comme troisième section canonique : c'était une
  intention abandonnée, décrite comme un fait. Constaté le 2026-08-05. Le nom reste
  utilisé dans le vocabulaire interne et dans `content.md` — à réconcilier.

Règles :

- Diagnostic audio au-dessus de Métadonnées en layout vertical ;
- les sections reposent sur le fond, pas dans des cartes imbriquées ;
- les titres de section peuvent être des bulles/pills ;
- pas de séparateurs internes gratuits ;
- espacement homogène par niveau : entre sections, entre groupes, entre lignes.

## Diagnostic Audio

Sources principales :

- `frontend/report-view.ts`
- spectrogramme et lignes de mesures.

Rôles :

- confirmer le verdict audio ;
- rendre visible le cutoff, la dynamique, le conteneur, les anomalies.

Règles :

- ⚠️ les catégories « Signal » et « Conteneur », décrites ici jusqu'au 2026-08-05,
  **n'existent pas dans l'UI**. Le seul rendu voisin est la ligne de mesure
  « Conteneur OK » (`report-view.ts:611`). La règle qui suivait — « ne pas les rendre
  ambre par défaut » — portait donc sur des éléments absents ; elle garde sa valeur
  d'intention pour toute catégorie future, mais ne décrit rien d'existant ;
- le spectrogramme est une preuve, pas une décoration ;
- les valeurs techniques doivent être compactes et comparables.

## Métadonnées

Sources principales :

- `frontend/filing-identify.ts` — propriétaire réel de la zone (Discogs, éditeur,
  apply-tags). Cette page citait `filing.ts`, qui n'est plus que l'orchestration
  résiduelle depuis le split (corrigé le 2026-08-05) ;
- `frontend/identify-shared.ts` — rendu des lignes de candidats, partagé avec
  `library-detail.ts`.

Rôles :

- confirmer ou corriger Discogs ;
- appliquer les tags utiles ;
- signaler clairement ce qui n'est pas encore gravé dans le fichier.

Règles :

- un warning ID3 dans Métadonnées suffit ; ne pas le dupliquer dans À finaliser ;
- les candidats Discogs doivent rester lisibles ;
- les genres sont des tags de sélection, pas des badges de statut ;
- "métadonnées fiables" doit rester un signal discret.

## À Finaliser

Sources principales :

- `frontend/filing.ts`
- `#filfoot`
- `#fldz`

Rôles :

- choisir la destination ;
- choisir le format ;
- voir le nom final après format ;
- convertir ou écarter — les verbes « ranger ou jeter » qui figuraient ici ont été
  remplacés dans l'UI le 2026-07-10 (`filing.ts:95` et `:175`), ce que cette page disait
  déjà correctement quinze lignes plus bas.

Ordre recommandé :

1. titre compact "Destination" ;
2. sélection de destination ;
3. choix de format ;
4. nom final ;
5. actions principales.

Règles :

- Destination est obligatoire, car elle dit où ranger ;
- le nom final vient après le format, car le format change son extension ;
- le nom final ne doit pas dominer visuellement la décision ;
- les actions Convertir/Écarter restent proches du résultat final ;
- `#filfoot` et `#fldz` restent des siblings de `.mid`. Précision apportée le
  2026-08-05 : la formulation d'origine ajoutait « pas des enfants de l'inspecteur », ce
  qui est faux et opposait deux choses compatibles. Markup réel, `app.js:122` :
  `<div class="sift-inspector" id="rvinspector"><div class="mid" id="mid"></div><div id="filfoot"></div><div id="fldz" hidden></div></div>` —
  les trois sont siblings **et** tous enfants de l'inspecteur.

## Feedback

Composants concernés :

- verdict ;
- warning ;
- toast ;
- progression ;
- états de sélection.

Règles :

- pas d'aplat vert permanent pour dire "c'est fait" si l'état est déjà compris ;
- utiliser un flash court lors d'une confirmation, puis revenir à un état neutre ;
- les erreurs bloquantes doivent être plus visibles que les recommandations ;
- aucune action destructive ne doit dépendre de `window.confirm()`.

Ajouts HIG Feedback :

- le canal doit correspondre à l'importance : un statut se consulte quand on veut, un
  risque de perte interrompt ;
- ne pas confirmer ce qui réussit toujours — l'utilisateur attend le succès, il n'a
  besoin d'un signal que pour l'échec ;
- un retour ne doit jamais reposer sur un seul canal : couleur **et** texte.

## Boutons

Tranché le 2026-09-09 (audit de fin de #24, décision d'Antoine ; valeurs dans `tokens.md`
§ Radius et hauteurs) :

- **deux familles** — Large 28 / 13 / r7, Small 22 / 11 / r7 — et aucun autre gabarit ;
- **un push button n'a pas de survol** : le kit Big Sur § 02 ne montre que Normal / Disabled ;
  `button:hover` générique et ses `filter: brightness()` ont été retirés. Un bouton plein répond
  à la pression, pas au passage ;
- un **bouton discret** (transparent : `.sift-settings-btn-quiet`, `.sift-tpl-chip`) répond en
  aplat `--overlay-hover`, comme une rangée de colonne ;
- exceptions documentées : `.sift-secondary-trash` peint le fond `danger` au survol
  (destructif), `.sift-lib-thead button` ne change que l'encre (padding 0, pas de place pour un
  aplat), `.sift-usage-lg` souligne ;
- un `<button>` sans classe de famille hérite du fond raised de `button{}` : c'est un push,
  et il n'a donc pas de survol (`.sift-empty-link`, rétabli après un aplat qui le faisait
  disparaître).

## Rail De Navigation

HIG Sidebars, transposé. Le rail de Sift tient le rôle de sidebar sans en avoir la forme.

**À viser — rien de ceci n'est acquis** (formulation corrigée le 2026-08-05 : c'était
écrit sous l'intitulé « Retenu », qui laissait croire à un état de fait) :

- laisser l'utilisateur masquer la navigation quand il veut de la place pour le contenu.
  Aucune affordance de repli n'existe : `styles.css:221` fixe `.sb` à `width:152px`, sans
  variante ni bascule ;
- si la liste s'allonge, grouper par disclosure plutôt qu'étirer ;
- l'ordre du rail reflète l'importance, pas l'ordre d'implémentation.

Non transposé : Liquid Glass, background extension effect, SF Symbols — organes ou API
natifs. Voir le test de transposition dans `foundations.md`.

## Listes Et Tables

HIG Lists and tables.

- une table qui sert à **naviguer** garde la ligne sélectionnée visible en permanence ;
  une table qui sert à **choisir** ne surligne que brièvement, puis marque le choix par
  un signe durable. Le retour de sélection doit dire lequel des deux rôles la table joue ;
- texte d'item court : troncature et retour à la ligne cassent le scan ;
- réordonnancement apprécié même quand l'ajout et la suppression sont impossibles.

Sur des listes virtualisées, une sélection persistante doit être portée par l'état, pas
par le nœud DOM — celui-ci est recyclé.

**La rangée de table `.lr`, lue contre le Finder — 2026-09-10 (Rangés, À re-sourcer,
Corbeille).** Les refs locales n'ont pas de table (Mail et Notes sont des listes multilignes,
le kit Big Sur n'a aucun composant table) : la référence est la présentation par liste du
Finder, et les HIG « Lists and tables » § macOS (« *Consider using alternating row colors in a
multicolumn table* »). Grammaire :

- **fonds alternés** `--overlay-alt`, par **parité d'index** posée par le rendu virtualisé
  (`rowHtml(item, index)` → classe `alt`) — jamais `:nth-child`, la fenêtre de rangées recyclées
  change de première rangée au défilement et la parité DOM s'inverserait ;
- **aucun filet** entre rangées, **aucun arrondi**, **aucun survol**, aucun enfoncement à la
  pression ; le seul trait de la table est le filet sous l'en-tête ;
- sélection `.sel` = `--color-background-secondary` pleine largeur ; piste ouverte `.cur` =
  `--color-background-info` — deux états qui se cumulent ;
- en-tête `--text-sm` en encre **secondaire**, un filet, indicateur de tri sur la colonne triée ;
- le format est une **colonne de texte** (« Format », `.sift-lib-col-fmt`, encre secondaire),
  jamais une pastille : quinze pastilles à fond secondaire faisaient quinze surfaces ;
- la pochette est le bouton de lecture (patron Musique) : le triangle se révèle au survol de la
  **pochette seule**, au focus clavier et sur la piste ouverte ;
- `--font-mono` sur Durée et Année seulement (chiffres alignés), jamais sur un chemin.

Ce qui était là avant et qui est parti : un filet sous chaque rangée ET un arrondi de 7 px, un
survol au plan de sélection (survolée = sélectionnée), `translateY(1px)` à la pression.

⚠️ **Écart relevé le 2026-08-05 : la file de Revue n'est pas focalisable.**
`queue-panel.ts:351` rend `<div class="qi" … title="…" style="cursor:pointer">` — ni
`tabindex`, ni `role`, ni `aria`. **Toujours vrai le 2026-08-19**, ligne re-vérifiée.
Le point de comparaison, lui, a changé : `home-sources.ts` rendait la même classe `.qi`
avec `tabindex="0" role="button" aria-pressed`, et ce fichier a été **supprimé** le
2026-08-19 avec la fusion d'Accueil dans le rail. La comparaison tient encore par
`library-views.ts`, qui donne `tabindex`/`role`/`aria-selected` à `.lr`, et par les
entrées de source du rail (`rail-sources.ts`), qui sont focalisables. Le même composant
visuel reste donc muet sur Revue seulement — l'écran principal.

Nuance : la navigation clavier existe (`queue-panel.ts:265-279`, flèches haut/bas via une
couche document). Ce qui manque n'est pas le déplacement, c'est l'**exposition** — focus
visible et rôle annoncé. C'est précisément ce qu'une capture d'écran ne montre jamais.

## Regle De Focus

Tout élément reconstruit à la main qui se comporte comme un contrôle doit porter
`tabindex` **et** `role`, comme le fait déjà la majorité du dépôt : arbre de destination
(`filing-bins.ts:241`), facettes (`bibliotheque-view.ts:312`), lignes et tuiles
(`library-views.ts:86,116`), rail de navigation (`index.html:13-21`).

Préférer l'élément natif quand il existe — les candidats Discogs
(`identify-shared.ts:23`) sont de vrais `<button>`, les divulgations de `report-view.ts:457`
et `:600` de vrais `<details>`, les en-têtes de tri de vrais `<th aria-sort>`. C'est la
bonne pente : un natif n'oublie jamais son focus.

Les tuiles de `library-views.ts` ont reçu leur `aria-label` composite le 2026-08-05.

⚠️ **La file de Revue reste l'exception, et pour une raison à connaître avant d'y
toucher.** Lui donner `role="button"` promettrait une activation par Entrée et Espace —
que l'app détourne déjà. `filing.ts:560` installe un `keydown` au niveau **document** dont
la seule garde est `if (!state.track) return` : sur Revue, Espace = écouter,
**Entrée = convertir**, Retour arrière = écarter. Et `installFilingKeys()` est enregistré
à `sift-live.ts:185`, *avant* `installQueueNavKeys()` à `:186`, donc un écouteur plus
tardif en phase bouillonnante ne peut pas le devancer.

Poser le rôle sans lever ce conflit remplacerait une lacune d'accessibilité par un piège :
la touche que le rôle annonce comme « activer » déclencherait l'action principale. Le
correctif demande un lot coordonné sur `queue-panel.ts` + `filing.ts`, pas une retouche de
markup. Vérifié le 2026-08-05, non corrigé.

## Indicateur De Progression

Source : `frontend/progress-zone.ts`.

- déterminé par défaut (`done/total`), jamais indéterminé sans raison ;
- `role="progressbar"` et `aria-valuenow` maintenus à chaque tick, pas seulement à la
  création ;
- animé par `transform`, jamais par un relayout ;
- transitoire : il disparaît à la fin et ne laisse pas d'état coloré permanent.

## Confirmation In-App

Source : `frontend/confirm-modal.ts`.

- remplace `window.confirm()`, qu'un clic synthétique a déjà traversée ;
- armée et horodatée contre le double clic ;
- HIG Alerts : un titre, un texte informatif optionnel, **trois boutons au maximum** ;
- ne jamais s'en servir pour informer sans action possible — un avertissement non
  actionnable vit dans le contexte concerné, pas dans une interruption.

## Contrôles De Disclosure

HIG Disclosure controls. Concerne les collapses Diagnostic audio et Métadonnées, et tout
groupe repliable du rail.

- ce qui est le plus utilisé reste en haut, toujours visible ; seul l'avancé se cache ;
- le libellé dit ce qui est masqué — "Diagnostic audio" nomme la zone, pas son contenu
  caché : à réévaluer si l'état fermé perd de l'information utile à la décision ;
- direction de l'indicateur : vers l'intérieur quand c'est fermé, vers le bas quand
  c'est ouvert.
