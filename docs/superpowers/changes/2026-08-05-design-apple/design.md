# Chantier design Apple — design

Date d'ouverture : 2026-08-05. **État : ébauche, avant accord L2.** Aucune valeur de token
n'est proposée dans ce document, et c'est délibéré (voir § Ce qui n'est pas tranché).

## Pourquoi ce chantier existe

Le chantier `2026-08-05-hig/` du même jour était de la **mise en conformité** : 7 écarts
mesurés contre les Apple HIG, 5 corrigés. Antoine a constaté qu'il ne voyait **aucun
changement à l'écran**, et il avait raison — tous les correctifs étaient sous-pixel (+1 px
sur du texte secondaire), conditionnels (un réglage système, un raccourci) ou transitoires
(un placeholder de chargement).

La demande n'était pas « audite Sift contre les HIG ». Elle était : **que ça ressemble à
une app Apple**. Ce chantier-ci répond à celle-là.

## Ce qui a été mesuré, et à quel titre

Trois couches de preuve, de la plus faible à la plus forte. Elles sont données dans cet
ordre exprès : la première a failli servir de conclusion, et elle ne le pouvait pas.

### Couche 1 — le centre de gravité du design system (compte de source)

`grep` sur `frontend/**` (`.css` + `.ts`), occurrences de `var(--token)` :

| Token | Valeur | Occurrences |
|---|---|---|
| `--text-xs` / `--text-sm` / `--text-md` | 10 / 11 / 12 px | 88 + 86 + 76 = **250** |
| `--text-base` | 13 px (= défaut macOS des HIG) | 19 |
| `--text-lg` / `--text-xl` | 14 / 16 px | 12 + 5 |
| `--text-2xl` | 26 px | 3 |

Espacement : `--space-4` 107 · `--space-8` 160 · `--space-12` 74 · `--space-16` 45 ·
`--space-24` 12 · `--space-32` 4.

⚠️ **Ce compte ne dit pas ce qu'on voit.** Une règle `--text-xs` peut peindre 200 lignes
d'une liste virtualisée là où une règle `--text-2xl` peint un titre par piste. Il établit
une propriété réelle — où le système a son centre de gravité — et rien de plus.

### Couche 2 — ce qui est réellement peint (mesure CDP, vraie fenêtre)

Fenêtre `tauri dev`, port CDP 9333, identité vérifiée (`document.title` = « Sift — prépa
sons DJ »). Les 8 vues du rail parcourues, 3 000 ms de stabilisation par vue, comptage des
éléments porteurs d'un nœud texte direct, visibles et de surface non nulle, **dans
`#content` seul** (le rail est exclu, il est persistant).

| px | 10 | 11 | 12 | 13 | 14 | 16 | 26 |
|---|---|---|---|---|---|---|---|
| nœuds peints | 39 | 52 | 137 | 33 | 5 | 19 | **1** |

**228 / 286 = 80 % des nœuds peints sont sous 13 px.** Le compte de source annonçait 86 % ;
l'écran dit 80 %. La couche 1 était donc directionnellement juste, et c'est la couche 2 qui
l'autorise à être citée.

Trois faits que seule cette couche donne :

- `--text-2xl` (26 px) est peint **exactement une fois dans toute l'application** : le titre
  de piste sur Revue. Le seul endroit où l'app parle fort est le bon.
- `--text-lg` (14 px) est peint **5 fois** au total.
- Le mot-symbole « Sift » du rail est à **18 px — une taille absente de l'échelle**
  (`styles.css` déclare 10/11/12/13/14/16/26). Les 8 libellés du rail sont à **12,5 px**,
  également hors échelle (`styles.css:296`, littéral en dur).

Conséquence directe : sur **7 des 8 écrans**, le plus gros texte visible est le nom de
l'application. Seule Revue a plus gros, avec son unique 26 px. L'app se nomme elle-même
plus fort qu'elle ne nomme ce que l'utilisateur est en train de faire.

⚠️ Piège de mesure rencontré, à ne pas rejouer : à 1 100 ms de stabilisation, Réglages
rendait **1** nœud ; à 3 000 ms, **29**. Les quatre écrans « pauvres » du premier passage
étaient un artefact de rendu, sauf Écartés (4), Journal (6) et Rekordbox (5) qui sont de
vrais états vides — vérifié en refaisant la mesure.

### Couche 3 — ce que la capture montre, et que rien de ci-dessus n'annonçait

Captures de référence en fenêtre réelle, **3440 × 1400, dpr 1** (`scratchpad/avant-*.png`).

Sur **Accueil** : le contenu occupe la bande supérieure et laisse le reste de la fenêtre
vide. La carte « DOSSIER SURVEILLÉ » fait ~325 px de large ; la ligne « Surveiller ce
dossier » juste en dessous en fait ~1 880, cases à cocher à gauche et boutons
« Resconner » / « Retirer » à l'extrémité droite — **~1 900 px entre une commande et son
libellé**.

Sur **Revue** : les deux sections repliées (« Diagnostic audio », « Métadonnées ») sont des
barres pleine largeur dont le badge (`LOSSLESS`, `CDJ COMPATIBLE`) est à ~1 500 px du
libellé qu'il qualifie.

Mesuré pour confirmer l'impression : **51 éléments dépassent 1 200 px de large** dans le
`#content` de Revue. Et `frontend/styles.css` ne contient que **10 `max-width` sur
1 751 lignes**, dont **3 seulement** bornent une mesure de contenu (`.sift-home-source-path`
560 px, `.sift-settings-stack` 560 px, `.sift-report-overlay-modal` 760 px) — aucune sur la
colonne principale.

## Le constat dominant

**Sift est composé pour une fenêtre étroite et s'exécute dans une fenêtre large.** Rien ne
borne la mesure : les surfaces s'étirent jusqu'au bord, donc un libellé et sa commande
finissent à 1 500–1 900 px l'un de l'autre, et l'œil ne peut plus les associer. Verticalement
le contenu se tasse en haut.

Cela **réordonne** les quatre directions candidates notées le 2026-08-05. La typographie
n'est pas la cause : du texte à 12 px étiré sur 1 900 px n'est pas illisible parce qu'il fait
12 px, il l'est parce que la longueur de ligne et la distance libellé → commande ne sont
bornées par rien. La densité et le rythme vertical sont dans le même cas — ils se règlent
dans une colonne de mesure décidée, pas dans le vide.

Autrement dit : **hiérarchie, densité, typographie de titre et rythme vertical sont quatre
symptômes ; la mesure et la composition sont en amont des quatre.**

## Reformulation L2 — à valider avant tout chiffre

Conformément à `docs/skills/sift-ui-design-governance.md` § Lexical Granularity : nommer la
surface, la décision utilisateur qu'elle sert, deux directions candidates.

**Surface** : la colonne de contenu (`#content`), sur les 8 vues — c'est-à-dire la zone qui
change quand on clique dans le rail.

**Décision utilisateur qu'elle sert** : sur Revue, « ce fichier, je le convertis, je l'écarte
ou je le range où ? » — la décision centrale de l'app. Sur les 7 autres, « où en est ma
bibliothèque, et qu'est-ce qui demande mon attention ? ».

**Direction A — la colonne bornée.** Le contenu reçoit une mesure maximale et se centre ;
la fenêtre large devient de la marge, pas de l'étirement. Libellé et commande se retrouvent
à portée d'œil. C'est la lecture la plus proche d'une app Apple de bureau, et la moins
coûteuse : elle ne demande aucun changement de token, seulement une contrainte de layout et
la réparation des quelques surfaces qui supposaient la pleine largeur.

**Direction B — la largeur habitée.** La fenêtre large est assumée et remplie : les écrans
passent en plusieurs colonnes, ce qui est aujourd'hui empilé se juxtapose, et le vide
vertical se comble. Plus ambitieux, plus proche d'un outil pro dense, et plus risqué — ça
touche la structure de chaque vue, pas seulement une contrainte transverse.

Les deux sont compatibles à terme (A d'abord, B écran par écran ensuite), mais **la première
décision est de savoir laquelle porte l'identité voulue**, parce qu'elles ne donnent pas la
même app.

## ACCORD L2 — issu du grill du 2026-08-05

Les deux directions A et B ci-dessus ont été soumises à Antoine. **Aucune des deux n'a été
retenue** : ses réponses en ont fait émerger une troisième, et ont tué la A par un fait
d'usage.

### Ce que le grill a établi

**1. La fenêtre réelle est maximisée sur ultrawide — ce n'est pas un cas de bord.**
`AppData/Roaming/com.sift.app/.window-state.json` porte `"maximized": true` et une taille de
repli de **1200 × 820**. L'app est donc utilisée à 3440 px et composée pour 1200 px, en
permanence. Aucune supposition là-dedans : c'est l'état persisté par le plugin window-state.

**2. La waveform sert à lire la STRUCTURE MUSICALE, pas la qualité.** Réponse d'Antoine :
« ça m'aide à voir les ponts et les moments faibles/forts dans les sons ». C'est un usage de
DJ. Vérifié au passage, et **réfuté** : `analysis/structure.rs` ne calcule pas la structure
musicale malgré son nom — seulement le silence en tête/queue et la troncature. L'app ne sait
rien des ponts ; ils se lisent bien à l'œil.

**3. La largeur aide RÉELLEMENT, et son plafond est chiffré.** `MAX_PEAKS = 4_000`
(`analysis/mod.rs:126`). À 3000 px la waveform affiche ~1,3 pic par pixel, proche de la
résolution de la donnée ; bornée à 1200 px elle en afficherait 3,3 par pixel, soit **les deux
tiers de l'information jetés** — précisément sur la surface qu'Antoine dit lire. Au-delà de
4000 px, on étire du vide.

⇒ **La direction A est écartée pour Revue**, par un fait d'usage confirmé par une mesure.

**4. L'impression d'ensemble est L1 et se mesure.** « Tous [les écrans me gênent], on n'a
aucune homogénéité, c'est moche, on n'a pas une impression de fluidité continue ». Traduit
par ce qui a été mesuré :

| ce que le système offre | ce que l'app en fait |
|---|---|
| 7 tailles de texte | 3 portent 80 % du texte peint ; 2 tailles hors échelle (18 px du mot-symbole, 12,5 px du rail) contournent le token |
| 6 paliers d'espacement | 2 portent 66 % de l'espacement ; les deux larges sont quasi morts |
| une mesure de contenu | 3 `max-width` dans 1751 lignes, **aucune** sur la colonne principale |

Le design system existe et l'app ne s'en sert pas. « Pas de fluidité continue » n'est pas un
jugement vague : c'est le symptôme de l'absence de règle.

### La direction retenue : borner par NATURE DE SURFACE

Ni « tout borné » (A) ni « tout rempli » (B), mais une distinction que ni l'une ni l'autre ne
faisait :

- **Surfaces de donnée** — waveform, spectrogramme, listes, graphiques d'occupation. Elles
  prennent toute la largeur utile, **jusqu'à leur propre plafond d'information** (4000 px pour
  la waveform ; à établir pour les autres). La largeur y est de l'information, pas du vide.
- **Surfaces de lecture et de commande** — libellés, formulaires, réglages, bannières,
  en-têtes. Elles se bornent à une mesure lisible. C'est là que vit l'absurdité actuelle :
  une case « Surveiller ce dossier » à ~1900 px de ses propres boutons.

Ce qui rend cette direction défendable plutôt qu'un compromis mou : elle donne une **règle
décidable** là où il n'y en avait aucune. Pour chaque surface on demande « est-ce que la
largeur y porte de l'information ? » ; la réponse détermine le traitement. C'est aussi ce qui
répond à « aucune homogénéité » — l'homogénéité viendra de la règle, pas d'une valeur.

### Le grill a séparé DEUX chantiers, pas un

Antoine a répondu « 2 et 4 » : oui à la règle de mesure **et** « c'est surtout que c'est
moche ». Ce sont deux sujets, et les confondre referait l'erreur du chantier HIG — livrer du
correct qui ne change rien à ce qu'il ressent.

- **Ce chantier-ci** traite la MESURE et la COMPOSITION : quelle surface prend la largeur,
  laquelle se borne, quel rythme vertical. Il rend l'app plus utile et plus cohérente.
- **Un chantier d'IDENTITÉ VISUELLE reste à ouvrir** : couleur, matière, caractère
  typographique, densité perçue. C'est lui qui répond à « c'est moche ». La règle de mesure
  n'y répond pas, et une waveform plus haute non plus.

### Démo de la waveform haute — PROPOSÉE puis REJETÉE le 2026-08-05

`report-view.ts:778` porté de 58 à 210 px, collapse Diagnostic ouvert, dans la vraie fenêtre
sur la piste d'Antoine. Ce que ça donnait objectivement : la structure du morceau devenait
lisible — intro, montée, creux, reprises — et les ~1100 px de vide se remplissaient
d'information au lieu de rien.

**Antoine a répondu « révoque, c'est pire ».** Révoqué intégralement, `git diff` vide, aucune
trace. Captures conservées dans le scratchpad de session (`avant-revue.png`,
`demo-revue-v2.png`).

Ce que ce rejet apprend, et c'est plus utile que si ça avait marché : **une surface plus
utile n'est pas une surface plus belle**, et c'est la seconde qui manque. La démo servait
l'usage déclaré (lire la structure) et a quand même été refusée. Donc le vide vertical de
Revue n'est pas un problème d'occupation — le remplir d'information ne le règle pas.
Deux propositions de mise en page rejetées d'affilée (direction A par l'usage, waveform haute
à l'œil) : c'est le signal que `sift-ui-design-governance.md` décrit — remonter d'un cran au
lieu de préciser. Le cran au-dessus est l'identité visuelle, pas la mesure.

### Reste ouvert après le grill

- **Le vide vertical de Revue** (~1100 px). Antoine : « je ne sais pas, il faut voir comment
  tu le remplirais » — c'est une demande de proposition, pas une question. Piste directe
  issue du point 2 : si la waveform sert à lire la structure, elle mérite de la **hauteur**
  autant que de la largeur, et le spectrogramme mérite d'être visible sans clic. À proposer
  et à montrer, pas à décider ici.
- **La tension matériaux** (`patterns.md:40`) reste non posée : la direction retenue ne la
  force pas.

## Ce qui n'est pas tranché — et ne doit pas l'être par défaut

- **A ou B**, ci-dessus. C'est la question du grill.
- **La tension matériaux.** `docs/design-system/patterns.md:40` l'écrit depuis longtemps :
  HIG Materials veut une couche de contrôles visuellement distincte du contenu, Sift a
  choisi la surface continue. Le document dit lui-même de trancher « le jour où elle se pose
  sur un écran réel ». La direction B la pose ; la direction A ne la pose pas.
- **Les titres de page.** Il n'existe **aucun** `<h1>`–`<h6>` dans l'app — ni dans
  `index.html`, ni dans `frontend/app.js`, ni dans aucun `.ts` — et **aucun** repli
  `role="heading"` / `aria-level` (vérifié). C'est un défaut d'accessibilité indépendant de
  A/B : un lecteur d'écran n'a aucun plan de document. À corriger dans les deux cas, mais la
  **taille** de ces titres dépend de la direction retenue.
- **Les deux tailles hors échelle** (18 px du mot-symbole, 12,5 px des libellés du rail).
  Elles sont dans le rail, pas dans `#content` : hors surface de ce chantier tant que A/B
  n'est pas tranché, mais l'inversion de hiérarchie qu'elles produisent est un argument pour
  y revenir tout de suite après.

## Contraintes du projet qui s'appliquent à ce chantier

- **Étudier, pas importer.** L'UI Kit macOS 27 fourni par Antoine
  (`https://www.sketch.com/s/57153a31-3379-4737-8ac6-dbfd6525f052`) sert pour les
  **structures, états et cotes**. Son bouton « Export Design Tokens » fait exactement ce que
  `CLAUDE.md` proscrit — la palette de Sift dérive déjà des couleurs système Apple.
- **`frontend/styles.css` reste le canonique unique** des tokens. Pas de fichier de thème
  parallèle, pas de valeur extraite d'une capture.
- **Les pages HIG se lisent par le Browser pane** (`get_page_text`), jamais par `WebFetch` —
  ce sont des SPA et l'échec ressemble à un refus du modèle.
- **Licence non vérifiée** sur les Apple Design Resources, et Sift cible aussi Windows. À
  trancher avant qu'un élément dérivé parte en release.
- Tout état nouveau demande **sa story Storybook**, pas seulement une ligne dans
  `docs/design-system-states.md`.

## Trouvailles hors sujet — traitées le jour même

- **Rejet non géré au démarrage, CORRIGÉ.** `window.set_shadow not allowed. Permissions
  associated with this command: core:window:allow-set-shadow`, à chaque lancement. Ce
  n'était pas une nuisance de log : `chrome.ts:222` appelle `setShadow(false)` pour
  supprimer la bordure claire d'1 px que Windows 11 dessine autour d'une fenêtre sans
  décoration — précisément le défaut signalé par l'annotation « il semble y avoir un cadre
  ou une bordure le long de la fenêtre ». La permission manquant de
  `capabilities/default.json`, **le correctif n'avait jamais pris effet** et le `void` devant
  la promesse enterrait le rejet. Permission ajoutée, appel doté d'un `.catch` bruyant.
  Vérifié dans la vraie fenêtre : la commande est acceptée, et le log de boot ne contient
  plus aucune occurrence de `set_shadow` ni aucun rejet non géré.
  ⚠️ Ce que ça ne prouve pas : que la bordure a visuellement disparu. La commande passe, son
  effet à l'œil reste à confirmer sur ta machine.
- **`cdp.cjs` coupe sa socket à 15 s** (`.claude/scripts/cdp.cjs:39`) — une IIFE async qui
  parcourt 8 vues à 3 000 ms dépasse le budget et échoue en `timeout` sans rien rendre.
  Consigné dans `.claude/skills/run-sift/SKILL.md`, avec le mode de panne du log vide.
