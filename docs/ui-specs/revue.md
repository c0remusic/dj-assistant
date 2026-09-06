# Spec — Revue

> **Réconciliée le 2026-08-21**, **complétée le 2026-08-24** avec les six décisions A–F du
> wireframe (§ 08, tranchées sur visuel) : ~~Batch armé par une **icône de sélection dans la
> barre** (§ 11 option 4)~~ → **bouton texte en tête de file** depuis le 2026-08-26, **filtre** en
> pop-up cochable, **popover Destination**, **états**,
> ~~**volume replié**~~, **recherche en tête** de la file, **genres** en texte + tag et
> ~~badge **« Prêt CDJ »**~~ (les deux barrées ont été retranchées le 2026-08-25, ci-dessous).
> Wireframe aux tokens réels
> (https://claude.ai/code/artifact/cec49229-4c84-4e6b-bfac-0843488ecb35), kit Big Sur lu sur
> les **PDF vectoriels** (`docs/design-refs/*.pdf`). Implémentation à faire (issue
> [#47](https://github.com/c0remusic/sift/issues/47)), en **série stricte** par lane de
> fichiers.

> **Révisée le 2026-08-25.** Quatre points du wireframe ont été retranchés depuis — bec du
> popover, « + Nouveau dossier », badge « Prêt CDJ », taille de la pochette : voir la section
> suivante. S'y ajoutent quatre décisions qui vivent dans leur section respective : largeur de
> lecture **dynamique** (Zone C), bouton de lecture en **triangle nu** (Zone C, 3), **volume**
> (Composants) et **rayons re-racinés sur le kit** (Composants). Sur tout le reste, le
> wireframe fait foi.

> L'écran de décision. Le seul en profil **Poste de décision** (`DESIGN.md` § 14).

## Décisions postérieures au wireframe — tranchées le 2026-08-25

Quatre points où une décision prise **après** le wireframe diverge du dessin. Les quatre
gardent la décision postérieure, **pas** le wireframe — tranchés par Antoine.

| Point | Wireframe | Retenu le 2026-08-25 |
|---|---|---|
| **Bec du popover Destination** | carte + **bec** vers l'ancre (§ 12) | **Sans bec** : un bouton « ▾ » ouvre un *pulldown* macOS ancré à un point, structuré en sections « Bibliothèque » / « Autres ». Le bec appartient au `NSPopover` détaché |
| **« + Nouveau dossier »** | crée un sous-dossier in-library, **inline** (§ 12) | **Sélecteur natif** : les deux boutons du pied versent dans « Autres » ; la commande IPC `create_bin` est retirée |
| **Badge « Prêt CDJ »** | **retenu**, dans l'en-tête Métadonnées (§ 16) | **Retiré** : le helper `paintCdjBadge` est supprimé du code |
| **Taille de la pochette** | **taille fixe**, jamais étirée (*fix 6*) | **Mesure JS conservée** : la pochette carrée prend la hauteur du bloc texte de l'en-tête (`sizeCoverToBody`) |

## Décisions postérieures — tranchées le 2026-08-26

Session de design sur maquette Figma (page « Maquette — Revue », fichier
`ujwh18pjb9ZKSY8XqlvDh0`). Sept points, tous tranchés par Antoine sur visuel. Les décisions
renversées ne disparaissent pas : elles sont nommées ici avec ce qui les remplace.

| Point | Avant | Retenu le 2026-08-26 |
|---|---|---|
| **Place de la recherche** | en **tête** de colonne (décision E, 2026-08-24, livrée `3d73aab`) | **sous la rangée de filtre**, collée à la liste — retour au wireframe § 15. « Trop haut, détaché de la liste » |
| **Mot de verdict dans la ligne** | `faux` / `à vérifier` rendus à droite de la ligne | **retiré** — « la pastille est là pour ça ». Rend la file à § Zone B′, qui ne l'a jamais listé |
| **Échec d'analyse** | cercle vide, identique à « en attente » | **pastille pleine danger + bouton Réanalyser**. Sans ça, couper le mot rendait les deux états indistinguables |
| **Déclencheur du mode Lot** | icône au bord droit de la **barre** (§ 11 option 4, livré `2d2c6d4`) | **bouton texte en tête de file** — « Sélectionner », qui devient « Terminé » une fois armé. Règle `CLAUDE.md` § Front : CTA à label descriptif = texte seul |
| **Compte de la file** | compte des pistes **visibles** en tête de colonne | **retiré de la colonne** ; le compte de file reste dans la **barre**, à côté du titre, comme § Zone A le prévoyait déjà |
| **« Non analysés uniquement »** | bascule en pied de colonne | **retirée** — doublon du pulldown de filtre |
| **Badges du rail** | pilule grise (`--overlay-badge`, rayon pill) | **chiffre nu**, aligné à droite, sans fond — motif Notes (`docs/design-refs/08-notes.png`) |

**Séparateurs — la règle, mesurée sur `03-mail.png`.** Mail en porte deux, traités différemment :
sous l'en-tête de **colonne**, **bord à bord** (0 → 1189 sur 1190 px de crop) ; sous l'en-tête de
**message**, **en retrait** aligné sur le contenu (volet 2253→3549, filet 2340→3491). Le premier
borne une zone, le second sépare à l'intérieur d'une zone. Conséquence pour Revue : le filet de
`.sift-qhead` passe **bord à bord de la colonne**. Voie CSS retenue — `margin-inline: calc(-1 *
var(--space-8))` + `padding-inline: var(--space-8)` sur `.sift-qhead`, une seule règle, plutôt que
de démonter le padding de `.queue`. Le corps de la zone C reste **sans aucun filet** (conforme à
l'éditeur de Notes et au corps de Mail, et à `patterns.md:54`).

**État d'implémentation — les sept décisions sont LIVRÉES et vérifiées dans la vraie fenêtre**
(2026-08-26, CDP) : `c4f65eb` place de la recherche, retrait du mot de verdict, pastille à quatre
états · `dd4ed1c` badges en chiffre nu, filet d'en-tête bord à bord · `3c1e05e` déclencheur Lot en
tête de file, compte de colonne retiré et remonté dans la barre, « Non analysés uniquement »
retirée · `114325e` police Inter.

✅ **Les deux pertes de fonction sont RENDUES le 2026-08-26** (issues #48 et #49), chacune par le
mécanisme que la spec désignait déjà plutôt que par le retour du contrôle retiré :

1. Le retrait de « Non analysés uniquement » avait supprimé le seul moyen d'**isoler les pistes
   bloquées en analyse** — mesuré le 2026-08-26 : le popover n'offrait que Lossless · MP3 · Faux ·
   Doublons, et les 3 pistes `needs_analysis` d'une file de 3 124 n'étaient atteignables que par le
   hasard du défilement. Le critère **rejoint le filtre cochable** en facette `Non analysés` (voir
   § Zone B′), et non en bascule séparée : le popover est déjà le lieu où l'on compose des critères.
   Le critère est `needs_analysis`, **jamais** `verdict === null` — c'est cette confusion qui avait
   causé le bug du 2026-07-20 (file entière cachée), aggravée par un défaut ON que le popover n'a
   pas : rien n'y est coché au montage.
2. Le retrait du compte de colonne avait supprimé le seul **retour chiffré du filtre**. Le compte
   de la barre **devient contextuel** (« N pistes filtrées », § Zone A) au lieu de faire revenir un
   second compte dans la colonne — la direction 1 du ticket rouvrait ce que le doublon
   compte/badge venait de fermer trois lignes plus bas. Ce qui tranche, encore une fois par les
   refs : sur `03-mail.png`, l'en-tête de colonne de Mail porte le compte (« 34 messages ») **et**
   le bouton de filtre sur la même rangée, et ce compte décrit ce que la colonne contient. Un
   compte de file qui ignore le filtre posé était donc l'écart au motif ; le rendre contextuel
   ramène Sift dessus.

✅ **Doublon compte-de-barre / badge-de-rail — TRANCHÉ le 2026-08-26** (`a927377`), par les refs et
non par un choix. Trois apps relevées : **Mail** = compte en tête de colonne, zéro compteur de
sidebar · **Notes** = compteurs de sidebar (chiffre nu), zéro en-tête de colonne · **Photos** = ni
l'un ni l'autre. Aucune ne montre les deux, et **Mail est le cas identique à Sift** : « Inbox » est
écrit AUX DEUX endroits, sidebar et tête de colonne, mais son compte n'est qu'à UN seul, celui de la
colonne. Notes fait l'inverse pour la raison inverse — sa colonne n'a aucun en-tête, donc le compte
remonte sur l'entrée qui nomme la liste.

Règle commune, et c'est elle qui tranche : **le compte va contre le nom de la liste, à un seul
endroit**. Sift écrit « Revue » dans le rail ET dans la barre ⇒ cas Mail ⇒ le **badge de rail de
Revue est retiré**, le compte reste dans la barre. Les badges de **sources** restent : leur compte
n'est écrit nulle part ailleurs, c'est le cas de Notes. `--overlay-badge` est supprimé avec le
dernier porteur de la pilule.

## Décisions — tranchées le 2026-08-27, portées le même jour (#50)

Tranchées sur la maquette Figma (comparatifs A/B construits côte à côte, verdicts d'Antoine),
puis portées dans le code dans la même session. La maquette reste la référence visuelle.

| Point | Avant | Retenu le 2026-08-27 |
|---|---|---|
| **Teintes de pastille** | encres de texte (`--color-text-success/danger/warning`) | **teintes système vives** : Authentique `hue-green-solid`, Faux `hue-red-solid` (token AJOUTÉ — systemRed du kit, 255,59,48 clair · 255,69,58 sombre), Zone grise `hue-yellow-solid`. Motif : un indicateur d'état est une teinte système pleine, jamais une encre (point non-lu de Mail = systemBlue). L'échec terminal suit Faux ; l'attente reste un anneau neutre ; les MOTS (badge LOSSLESS) restent en encre |
| **Place de la pastille dans la ligne** | ouvre la ligne, avant le titre | **collée à la fin du titre** — décision produit, assumée CONTRE le motif Mail (indicateurs au bord droit). Le sous-titre s'aligne au titre (indent de 15px retirée) |
| **Respiration de la zone C** | pas uniforme 16 entre les 4 sections | **groupes** (« serré dedans, aéré entre », motif Réglages Système / en-tête Mail) : bloc écoute (en-tête + lecteur, gap 12) · écart 32 · bloc fiche (métadonnées gap 8 — pitch ≈ 30, rangées de Réglages —, puis Diagnostic à 16). Les filets par rangée de `.sift-attr` tombent : l'espace sépare |
| **Plan du rail** | `.sb` peint `background-tertiary`, même plan que la file | **fond de fenêtre** — trois plans : rail (le plus en retrait), milieu (le sol), file (seule zone élevée). Motif sidebar Mail ; `#sift-tb-left` suit. `patterns.md` § plans amendé |
| **File** | carte flottante (bordure + rayon 14, insetée par le padding de `#content`) | **bord à bord** — collée au rail, à la barre, au bas de fenêtre ; filets sur ses seuls flancs. Séparateurs **entre rangées**, 1 px, en retrait (`--space-8`), effacés autour de la rangée survolée/ouverte (comportement des Mail récents) ; hauteur de rangée 46 inchangée (`::before` absolu) |
| **Filet sous la barre** | aucun | **1 px bord à bord** sous `#sift-titlebar` — le séparateur qui borne la zone (motif Mail, en-tête de colonne) |
| **Barre** | `--toolbar-h: 44` (dérivation locale) | **48** — la toolbar du kit App Window (72:165). L'ancienne dérivation reste un plancher |
| **Entrée de rail** | `.nv` 34, padding 8/8 | **28**, padding 7/8 — l'item sidebar du kit (61:1606) |
| **Titre de piste** | 32/600 (`--text-2xl`) | **26/32** — Large Title du styleguide kit. Littéral sourcé, à rationaliser avec #31 |
| **Épaisseur des filets** | 0,5 px hairline partout | **1 px partout** (le kit dessine à 1 ; 0,5 rend inégal selon DPI). Sweep complet styles.css + vues TS, `app.js` (maquette héritée) exclu |

⚠️ Deux amendements au § du 2026-08-26 : « le corps de la zone C reste sans aucun filet »
s'étend aux rangées de la fiche (leurs `border-bottom` tombent) ; et la file, elle, GAGNE ses
séparateurs de rangées — ils étaient déjà la décision maquette du 26, portés ce jour.

### Après-midi du 2026-08-27 — lecteur simple, volume fin, bandeau en boîte

Cinq décisions de plus, chacune tranchée sur comparatif maquette (ou correction directe
d'Antoine), portées le jour même :

| Point | Avant | Retenu |
|---|---|---|
| **Lecteur** | waveform wavesurfer (40 px, barres) | **slider fin du kit** (Pickers/Linear/Small 53:118, COPIE SVG) : piste 4 px, remplissage accent, pouce blanc 20. WaveSurfer reste le MOTEUR, rendu dans un conteneur réduit à zéro (`.sift-progress-engine`). Seek au drag/clic sur la piste, flèches ±5 s, bulle mm:ss au survol (ghost et ligne partis avec les barres) |
| **Play** | 46 px, glyphe 32 | **28 px, glyphe 22**, aligné au bord de conduite (marge gauche retirée). Le glyphe reste Tabler — SF Symbols n'est pas licenciable (patterns § 5) : ce qui se copie du kit, c'est la géométrie, jamais ses glyphes |
| **Volume** | capsule SVG kit 112×24 (inlinée le 25) | **slider fin 90 px assorti au lecteur** (patron Music, maquette « Volume (lecteur) ») : haut-parleur cliquable (mute, bascule ti-volume/ti-volume-off), remplissage et pouce BLANCS (`--color-accent-ink` — un volume ne porte pas l'accent), pouce 14. « Couleur, taille et style vraiment goofy » dans la rangée fine — renverse la copie capsule du 25 |
| **Piste des sliders** | `--color-track` | **`--overlay-bar`** — color-track vaut le fond de fenêtre en sombre (token du segmenté) : piste invisible, « on ne voit pas la longueur de la barre ». overlay-bar est l'overlay themed des barres de lecture, qui venait de perdre la waveform |
| **Bandeau de lecture** | sections nues | **BOÎTE pleine largeur** (fond queue, rayon md, padding 16) — verdict X filets / Y cadre : Y, sourcé HIG § Boxes. Filet interne en-tête\|lecteur (12/12), filet de section avant la fiche (16\|filet\|16, border-top de `.sift-meta-header`). Exception à « une surface de contenu ne peint rien », consignée dans patterns.md. Pleine largeur : le `max-width:--measure-data` datait du bloc sans boîte |
| **Pied de Détail** | à plat + filet (wireframe v2, 2026-08-21) | **surface pleine largeur** (fond queue, sans filet ni marges latérales) — une zone de boutons se distingue par surface ou espace, jamais par un trait (Big Sur). Le rail de Lot garde sa carte |

L'état « analyse en cours » (squelette) ne prend PAS la boîte : sans lecteur, le bandeau
n'existe pas encore — le squelette couvre l'emplacement, gap 32 simple.

Deux corrections du soir, mêmes principes : le **filet de section** avant la fiche (posé le
matin, motif Mail) est **retiré** — la boîte borne déjà, une frontière ne se dit qu'une fois
(HIG § Boxes, Réglages : boîte puis espace seul) ; l'écart inter-sections de 32 reste. Et
l'**emplacement du pied re-questionné puis CONFIRMÉ** (comparatif P1 bas-de-panneau actuel /
P2 réglages en fiche + actions en barre / P3 réglages en fiche + pied mince — verdict Antoine :
P1). L'adresse de #26 tient : réglages et engagement au bas du panneau, motif bas d'inspecteur
des pro apps (Compressor). P2 aurait rompu l'adresse et rendu la portée du bouton ambiguë en
mode Lot ; P3 reste la variante de repli si le bandeau du pied redevient trop lourd.

Retouches de fin de soirée, sur retours dans la vraie fenêtre :

- **La tranchée entre file et zone C est refermée** : `.sift-qresize` occupait une colonne
  flex de 16 px transparents (fond de fenêtre au travers, flagrant entre les deux surfaces
  queue du bas). La poignée passe en emprise nulle — marges négatives symétriques, à cheval
  sur le filet de la file, zone de saisie 16 conservée. Le piège historique des marges
  compensées (2026-07-24) est éteint : les voisins sont bord à bord, plus rien à compenser.
  C'était l'« espace » des retours « collé au panneau ».
- **Le CADRE de lecture reste le cadre Y validé** : boîte arrondie (rayon md), insetée de 16
  par `.mid`, pleine largeur du panneau MOINS les insets (le `max-width:--measure-data` reste
  retiré — il faisait flotter le cadre à gauche sur écran large). ⚠️ Une variante « bande
  full-bleed sans rayon » a été livrée par erreur d'interprétation puis RETIRÉE le soir même
  (« ce n'est pas un cadre ») ; en chemin, la marge négative multi-conteneurs a produit 16 px
  d'overflow-x (temps et volume clippés) — deux fausses routes consignées pour ne pas y
  retourner. L'inset HAUT (16 sous la barre) tient.

## Décision — 2026-08-30 : la boîte de lecture absorbe la conversion (V2b)

Tranchée sur wireframes comparatifs (artefact « Fusion lecture × conversion », V0–V4 puis
raffinement V2a/V2b/V3a/V3b) — verdict Antoine : **V2b, « pied de boîte »**. Elle **renverse,
pour le mode Détail seulement, le verdict P1 du 2026-08-27** (réglages + engagement au bas du
panneau) — direction jamais testée par P1/P2/P3, qui mettaient les réglages dans la fiche,
pas dans la boîte.

La boîte de lecture (`.sift-player-row`) devient le poste de décision complet, quatre étages :

1. **En-tête piste** (inchangé) ;
2. filet en retrait, **lecteur** (inchangé) ;
3. filet en retrait, **rangée réglages** : Destination (pulldown) · Format (segmenté) ·
   Nom final — le contenu de l'ex-rangée du haut du pied, tel quel ;
4. **pied de boîte** : bande distincte par surface (`--color-background-secondary`,
   filet haut `--color-border-tertiary`), **bord à bord de la boîte** (marges négatives),
   coins bas au rayon de la boîte — **légende clavier à gauche**, **Écarter** puis
   **Convertir** au trailing. Motif : l'alerte du kit (§ 06-02), carte + rangée de boutons en
   bas. La légende a été retirée le 2026-09-03 (audit œil-Apple : Apple n'écrit jamais les
   raccourcis en dur dans une fenêtre) puis **restaurée le 2026-09-05 sur retour d'Antoine**
   (« je préférais avec la légende ») — préférence produit assumée contre le patron Apple, à
   ne pas re-retirer sur le seul argument HIG. Les tooltips posés au retrait restent
   (Convertir « Entrée », Écarter « ⌫ », lecture « espace ») : deux canaux.

Le **pied de panneau disparaît en Détail** : `#filfoot` n'y reçoit plus de contenu (le
bandeau « Rangé » suit les contrôles dans la boîte). Bas de panneau = espace (volet de
lecture Mail). **Le mode Lot ne bouge pas** : son rail garde carte et emplacement bas ;
l'adoption éventuelle de la carte-console par le Lot est notée comme suite possible, pas
décidée. L'état « analyse en cours » (squelette) continue de ne pas prendre la boîte.

⚠️ Le § « Zone C, pied — rail d'action » plus bas décrit l'état d'AVANT cette décision pour
le mode Détail — il reste exact pour le contenu des contrôles (pulldown Destination, formats,
nom final, boutons), seul leur EMPLACEMENT change.

## Contexte dans le shell

Patron macOS : **Finder** pour la file et la sélection · **Utilitaire de disque** pour
le mode Lot (cible → action → progression → rapport).

Trois zones : rail (`--rail-w`, fixe) · **file** (`--pane-w`, fixe, redimensionnable
220–480 px, valeur persistée) · **surface de travail** (flexe). Pas d'inspecteur droit :
la surface de travail *est* l'inspecteur, agrandi.

**Pourquoi cette zone flexe et pas la file.** La surface porte le spectrogramme, borné
par `--measure-data` (1200 px) et **dupliqué côté Rust** dans
`analysis::spectrum::MAX_COLS`, épinglé par
`analysis::spectrum::tests::css_data_measure_matches_max_cols`. Une largeur fixe y
présenterait de la donnée étirée ou tronquée — donc fausse, dans l'app qui détecte du
faux. La contrainte est technique, pas confortable.

L'utilisateur répond ici à quatre questions, dans cet ordre :
**le fichier est-il sain ? l'identification est-elle fiable ? où va-t-il ? sous quel
format et quel nom ?**

## Layout

### Zone A — barre unifiée

Titre « Revue » + **compte de la file**, à gauche, contre le titre. Le compte est le SEUL de
l'écran depuis le 2026-08-26 : le badge de rail de Revue est retiré, et le compte de pistes
visibles a quitté la tête de colonne (voir § Décisions postérieures). En mode Lot il devient
« N sélectionnées » à la teinte d'accent — il REMPLACE le total plutôt que de s'y ajouter, deux
nombres côte à côte se liraient comme une fraction. Contrôles de fenêtre à droite (convention hôte
Windows).

**Trois états, pas deux** (issue #49, 2026-08-26) : au repos « N pistes » = la file entière ; dès
qu'une facette est cochée ou qu'une recherche est saisie, « **N pistes filtrées** » = ce que la
colonne montre ; en mode Lot, « N sélectionnées ». C'est le retour chiffré du filtre — le pulldown
dit la combinaison cochée, ce compte dit ce qu'elle laisse voir. Le qualificatif « filtrées » n'est
pas un ornement : chez Mail le compte touche le bouton de filtre, ici il en est séparé par toute la
largeur de la barre, et un nombre nu s'y lirait comme la taille de la file. Un seul nombre dans les
trois cas — jamais « N sur M ». Règle gelée par `test/queue-count-label.test.ts`.

~~Au bord droit, avant les contrôles de fenêtre : **icône de sélection** (Batch, § 11 option 4),
icône seule + infobulle, patron *toolbar* Photos macOS.~~ **Retirée le 2026-08-26** avec son
emplacement `#sift-tb-actions-right` : le commutateur du mode Lot est un **bouton texte en tête de
file**. Ce qui a tranché : Photos place « Sélectionner » dans sa toolbar parce que la grille de
photos EST l'écran — la toolbar y commande la seule chose visible. Dans Revue la file n'est qu'une
colonne sur trois et la barre commande déjà l'écran entier ; le même geste n'a plus la même portée.

**La recherche NE monte PAS dans la barre** — décision du 2026-08-21, **re-challengée et
re-confirmée le 2026-09-03 (issue #57, choix B d'Antoine sur wireframe A/B)** : le champ dans la
colonne dit ce qu'il filtre — « on sait vraiment ce qu'on cherche ». Bibliothèque et Journal
gardent leur recherche en barre ; l'asymétrie est un choix, pas un oubli. Elle reste dans la
colonne file (Zone B′). ~~en **tête** de colonne (décision E, 2026-08-24)~~ → **SOUS la rangée
de filtre** depuis le 2026-08-26, voir § Décisions postérieures. Le
segmenté **Détail / Lot est retiré** (« plus besoin du picker Lot ») : le mode Batch est armé par
le bouton texte de la tête de file, pas par un onglet ni par la barre. Écart assumé à `DESIGN.md`
§ 15, noté là-bas.

### Zone B′ — file

**En tête de la colonne** (patron HIG d'une liste : Notes, Mail compact, Music) : le
**champ de recherche** — loupe (SVG monochrome, jamais un emoji) + placeholder
« Rechercher » + bouton clear `×` quand du texte + anneau d'accent au focus (patron *search
field* du kit, § 03-02). ~~Passée **en tête** le 2026-08-24 (décision E)~~ — elle est **sous la
rangée de filtre** depuis le 2026-08-26 (retour au wireframe § 15) ; elle était en pied avant le
2026-08-24.
L'ordre réel de la colonne, depuis le 2026-08-26 : **rangée de filtre** en tête (le
**bouton de filtre** en pop-up cochable à gauche, le bouton **« Sélectionner »** à droite), puis le
**champ de recherche** contre la liste qu'il interroge, puis la liste virtualisée. Le libellé
« File » et le **compte des pistes visibles** ont quitté cette rangée : le titre de l'écran est
dans la barre, et le compte l'y a suivi (§ Zone A). Il n'y a **plus** de segmenté Détail / Lot, ni
d'icône de barre — le mode Batch s'arme par le bouton texte de cette rangée.

Au **pied** de la colonne il ne reste qu'un contrôle : la **rangée de statut des non
analysées** — compte à gauche (« 1 398 non analysées », `fr-FR`, patron barre de statut du
Finder), bouton **Réanalyser** à droite (grammaire du bouton secondaire, mêmes tokens que
Ré-identifier), filet haut qui borne, rangée entière masquée à zéro. **Refaite le 2026-09-06**
(audit œil-Apple, wireframe validé par Antoine) : la forme précédente était un lien souligné
centré « Réanalyser (N) » — idiome web sans équivalent système, où compte et action étaient
soudés. La bascule « Non analysés uniquement » qui l'accompagnait est retirée (voir
§ Décisions postérieures et la perte de fonction qu'elle emporte).

Une ligne de file porte, dans cet ordre : **pastille de verdict** (`DESIGN.md` § 16,
même rendu qu'en Bibliothèque) · nom de fichier · artiste — titre · **pastille
`DUPLICATE`** au bord droit si la piste est un doublon (rendu hors colonne verdict, cf.
§ 16). **La durée est retirée de la file** (2026-08-21) : inutile ici, et elle mangeait la
place du signal doublon.

**Hauteur : 46 px, constante** — et non `--row-h` (32 px), qui vaut pour une ligne simple. La
rangée de file en porte **deux** (nom de fichier, puis artiste — titre), donc sa hauteur se dérive
des deux interlignes du kit macOS Big Sur (`docs/design-refs/Styleguide.pdf`, § 05 Typography) :
**Callout 12/15** pour le nom, **Caption 1 10/13** pour le sous-texte, plus le gap de 2 et
`2 × --space-8` — soit 15 + 2 + 13 + 16 = 46. Ces interlignes sont **explicites en px** dans
`styles.css` (`.qi`, `.qi-sub`), jamais `normal` : issue #45, mesurée le 2026-08-26 dans la vraie
fenêtre, 46 rangées à 45 px et une à 46 px parce que `line-height:normal` dérive la hauteur des
glyphes réellement présents dans le titre. Ce n'était pas qu'une alternance irrégulière —
`measureQueueRowHeight` met UNE hauteur en cache pour toute la file, donc l'écart d'un pixel
décalait la fenêtre de virtualisation sur 3 000 lignes. Gelé par `test/queue-row-height.test.ts`.

**Quand le mode Batch est armé** (bouton
« Sélectionner » de cette rangée), une **case à cocher** apparaît en tête de chaque ligne ; on coche
track par track.

**Filtre — pop-up à options cochables** (décision du 2026-08-24). Un bouton en tête de file
(« Faux », « Faux + Doublons »…) ouvre un menu à **cases à cocher** : `Lossless`, `MP3`,
`Faux`, `Doublons`, `Non analysés`, chacun avec son compte, un séparateur, puis « Tout
afficher ». `Non analysés` est ajoutée le 2026-08-26 (issue #48) et couvre **toutes** les pistes
sans verdict, y compris celles qui ont épuisé leurs tentatives — périmètre plus large que le
compte du bouton « Réanalyser (N) » du pied, qui les exclut pour pouvoir tomber à zéro. Plusieurs
critères cochés = **union** (Faux *ou* Doublons). « Tout afficher » remet à zéro. Le bouton
résume la combinaison. La recherche (en tête) **gare** le filtre : elle interroge la file
source **entière**, pas le sous-ensemble filtré (patron Finder — saisir passe en mode
résultats — ce qui tue le piège « 0 résultat = bug »). Le filtre isole une catégorie pour la
cocher vite en mode Batch.

Au pied de la colonne : bascule « + N traités » quand la file en contient. Poignée de
redimensionnement à droite, révélée au survol.

### Zone C — surface de travail, mode Détail

**Direction « verdict promu », retenue puis affinée le 2026-08-21** (audit Revue, skill
`sift-macos-ui`). Elle répond au défaut mesuré : la surface était à moitié vide et le
verdict — le signal central — n'était qu'un badge sur un accordéon replié. Elle honore le
patron **Mail** (volet de lecture plein) sans toucher à la grammaire colonne-unique de la
file.

**Largeur de lecture — dynamique, tranché le 2026-08-25.** Les blocs de la zone C (path bar,
en-tête, lecteur, Métadonnées, rail) **suivent la largeur de la fenêtre**, comme tout le reste
du contenu : **pas de plafond de lecture fixe**. Une version antérieure de cette spec les
bornait à une « largeur de lecture unique (~720 px) » — c'était une proposition, jamais
livrée, et elle est abandonnée. Ils gardent en revanche le **même bord de conduite gauche**,
partagé avec la file.

Seul le **spectrogramme** garde une borne, et elle est **technique, pas confortable** :
`--measure-data` (1200 px), **dupliquée côté Rust** dans `analysis::spectrum::MAX_COLS` (cf.
§ Contexte dans le shell). Au-delà, la donnée serait étirée — donc fausse, dans l'app qui
détecte du faux.

Ordre vertical, et il est le parcours de décision :

1. **Path bar** — le chemin d'origine en **fil d'Ariane de tête**, pleine largeur, en
   **segments** (`Dossier › Dossier › fichier`), en **police d'interface** (Inter depuis le
   2026-08-26, Outfit avant), **pas en
   monospace**. Patron *path control* (HIG « Path controls » / `NSPathControl`, la barre
   de chemin du Finder) : segments, troncature **par le milieu** si trop long (garder le
   premier et le dernier). Le mono est un réflexe Terminal, écarté ici.
2. **En-tête piste** — **pochette carrée, à la hauteur du bloc texte** + titre + artiste +
   **format** (petite ligne : `FLAC · 44,1 kHz`). Cette hauteur se **mesure en JS**
   (`sizeCoverToBody` + un `ResizeObserver`) : le pur CSS (`aspect-ratio:1` +
   `align-self:stretch`) rend une largeur **nulle** dans ce contexte flex, mesuré au CDP, et
   une mesure ponctuelle raterait le reflow tardif (chargement de la police d'interface, pose du
   verdict).
   **Conservée le 2026-08-25** contre le *fix 6* du wireframe, qui voulait une taille fixe —
   décision d'Antoine. Le **verdict** est une **pastille
   discrète en haut à droite**, au niveau du titre : **point coloré + mot** (`LOSSLESS`,
   `FAKE`, `À VÉRIFIER`, `—`), **sans capsule**, ~10 px / 500. La couleur (teinte de la
   table, `.sift-lib-v-*`) double le libellé (§ 16, daltonisme). **Dit une seule fois**,
   ici — jamais répété plus bas. Discret par choix : Apple Music tient « Lossless » en
   indicateur neutre parce qu'il n'a qu'un état ; Sift garde la couleur (verdict
   catégoriel : lossless / faux / douteux) mais lâche le poids (fond, majuscules 600).
3. **Lecture** — bouton play **triangle nu** : sans cercle **ni pastille** derrière lui
   (patron Voice Memos / Musique ; reconfirmé le 2026-08-25) ·
   **waveform-overview** fine + pouce rond blanc (bord + ombre) : l'onde tient lieu de
   piste de navigation, exactement le *scrubber* de Voice Memos ; l'inspection fine du
   signal reste au spectrogramme, pas ici · **un seul temps affiché, cliquable** (bascule
   écoulé ↔ restant, patron Musique / Podcasts — jamais les deux à la fois) · **volume =
   capsule du kit, toujours visible** : pilule pleine hauteur, haut-parleur intégré à gauche
   (clic = couper), gros pouce rond, remplissage à gauche du pouce. Le repli-au-survol de la
   décision D (2026-08-24) est **abandonné le 2026-08-25** ; teintes, survol et pressé :
   § Composants — Volume. La waveform ne s'anime pas au chargement. Tempo / key-lock
   **retirés** : le pitch DJ n'est pas voulu sur un écran de décision.
4. **Métadonnées** — **toujours visible**, en **liste d'attributs éditables en place**
   (*text field inline* : texte au repos, champ + anneau d'accent au focus ; patron
   inspecteur Finder « Lire les informations »). Artiste / Titre / Version éditables ;
   Label en lecture seule ; **genres en texte + icône tag** (glyphe *tag* du kit, § 01
   Icons — « Electronic, Synth-pop », **pas de chips** ; décision F, 2026-08-24). Le **seul** bouton **« Identifier »** vit **SOUS la liste
   d'attributs, aligné au bord gauche** (`.sift-meta-actions`) — **décision 1b d'Antoine,
   2026-09-06**, wireframe avant/après : l'ordre de lecture est l'ordre d'usage (on lit les
   valeurs, elles clochent, l'action est là ; patron formulaire de Réglages Système). Il vivait
   au bord droit de l'en-tête, que la largeur du panneau envoyait à ~1300 px du titre. Coût
   assumé : pendant une recherche, les candidats s'insèrent au-dessus des attributs, loin du
   bouton. Le badge **« Prêt CDJ »** qui accompagnait l'ancien emplacement a été **retiré le
   2026-08-25** (demande d'Antoine ; le helper `paintCdjBadge` est supprimé du code).
   « Identifier » lance la recherche Discogs et remplit ces
   mêmes champs, **sans changer de mode** (plus de formulaire à entrer). Quand **plusieurs
   éditions** matchent, elles s'affichent en **liste ouverte inline** (patron Spotlight « Top
   Hit ») : le meilleur match est **pré-appliqué**, les alternatives se permutent d'un clic
   (navigables ↑↓), sans popover ni changement de mode. Le clic sur un match **écrit l'ID3
   immédiatement** (décision datée 2026-08-21, « Entrée = graver »), avec un filet
   **« Rétablir »** inline (+ `Échap`) pour défaire. **La ligne
   « Tags ID3 » est supprimée** (tautologique : « Tags ID3 : ID3 »). Le critère CDJ reste
   défini (`docs/cdj-metadata-formats.md`, WAV exclu) et son recâblage reste ouvert
   ([#46](https://github.com/c0remusic/sift/issues/46)) — il n'a simplement **plus de porteur
   visuel** sur cet écran depuis le retrait du badge.
5. **Diagnostic audio** — **placé sous les Métadonnées** (on identifie plus souvent qu'on
   n'inspecte ; les détails techniques vont en bas du volet). Repliable, **fermé par
   défaut**, en-tête **nu** : « ▸ Diagnostic audio », **sans sous-texte** (un disclosure
   macOS = chevron + titre, rien d'autre). Ouvert : le **spectrogramme domine** (toute la
   largeur disponible jusqu'à sa borne `--measure-data`, la preuve du verdict) ; sous lui,
   **deux pastilles compactes**
   (format · lecture du spectro, ex. `FLAC` · `Pleine bande · 22 kHz`) ; puis **« Détails »**
   qui replie **toutes** les mesures chiffrées (coupure, densité de l'aigu, durée,
   true-peak, phase, écrêtage, canaux, silence…). Le verdict n'est **pas** répété ici, ni
   le format déjà en en-tête. Fermé au repos parce que la grille se recalcule à l'ouverture
   (~631 ms mesurées) et n'est plus stockée : le pas de piste en piste (↑↓) reste
   instantané.

### Zone C, pied — rail d'action

Barre persistante, **à plat** (pas de carte, `.sift-action-rail--flat`), jamais dans le
flux de défilement.

Rangée du haut, réglages : **Destination** (bouton ouvrant l'arbre en **pulldown** — carte
arrondie alignée au bouton, **sans bec**, patron *menu/pulldown* du kit § 05, avec ombre ;
détail ci-dessous) · **Format** (contrôle segmenté : MP3 320 · AIFF 16/44 · WAV 16/44,
options lossless désactivées sur source lossy) · **Nom final** (aperçu, rendu par
`previewFilename`, **jamais** réimplémenté en TS).

**Popover Destination** (décision B, 2026-08-24 ; **bec retiré le 2026-08-25** — un bouton « ▾ »
ouvre un *pulldown* macOS, ancré à un point et **sans bec** ; le bec appartient au NSPopover
détaché, kit § 06-03, cf. `patterns-macos.md` § 8). Une carte alignée, structurée en **sections
façon sidebar Finder** : un en-tête **« Bibliothèque »** coiffe l'arbre, **« Autres »** coiffe les
dossiers externes (style relevé du composant kit `Sidebars/Sidebar-section` via le pont Figma :
10px / poids 600 / tracking 0.12 / casse normale, encre mappée sur token gris). En haut,
l'**arbre** des dossiers de la bibliothèque (chevron + icône dossier + nom, indentés) ; le
dossier courant est **surligné**. Sous le groupe **« Autres »**, les **dossiers custom**
déjà choisis hors bibliothèque (ex. `Sets 2026 · D:\Promo`). Un séparateur, puis **« Ranger
sur place (ne pas déplacer) »** (case). En pied : **« + Nouveau dossier »** — ouvre
l'**explorateur natif** pour choisir où le créer — et **« Choisir un dossier… »** —
sélecteur natif pour un dossier custom existant. Dans les deux cas, **le dossier choisi
entre dans la liste** (groupe « Autres »), disponible ensuite sans re-parcourir l'arbre.

Rangée du bas : **légende clavier** à gauche (retirée le 2026-09-03 puis restaurée le
2026-09-05, voir § pied de boîte) · **actions groupées au bord droit (trailing)** : **Écarter** (secondaire — **gris rempli**, *pas* un ghost à bordure : le kit
ne connaît pas le ghost, le *secondary push button* est un fond gris ; ou **Re-source** si
le verdict est `fake`) puis **Convertir** (primaire, **aplat d'accent bleu + texte blanc**,
la plus à droite, action `Entrée`). Le contraste primaire / secondaire porte seul la
distinction entre les deux issues, comme macOS.

### Zone C — mode Batch

Armé par le **bouton texte « Sélectionner »** de la tête de file (Zone B′), qui devient
« Terminé » une fois armé — patron « Sélectionner » de Photos, mais posé dans la colonne et non
dans la barre (voir § Zone A pour ce qui a tranché). **Pas** par une bascule de vue. Off → file normale, pas de
cases. On → une **case** apparaît par ligne dans la file (Zone B′) et on **coche track par
track** ; il n'y a **pas** de seconde table. La file et le rail restent en place.

La zone C devient le **résumé de la sélection** : compte (« N sélectionnées »), répartition
par verdict (« 2 FAKE »), totaux format / durée (« 1 MP3 320 · 1 FLAC · 6:24 »), puis les
actions **« Écarter la sélection »** (secondaire) et **« Ranger la sélection »** (primaire,
dominante). Le **clic droit** sur la sélection porte les mêmes actions (Ranger · Écarter ·
Changer la destination). Combiné au **filtre** (Zone B′), il isole une catégorie pour la
cocher vite (« Faux » + tout cocher = agir sur une catégorie).

Le rangement d'un lot affiche la **progression** en **sheet non-modale** attachée à la fenêtre
(carte glissant du haut, ombre douce, kit § 06-01), **forme mince** (feuille de copie Finder :
barre déterminée + ligne d'étape + disclosure « Afficher les détails ») ; bouton **« Arrêter »**
(la piste en cours finit, rien n'est annulé). À la fin, **la sheet se transforme en rapport** :
chiffres, puis Rangés / À valider / Échecs dépliables, chaque item « Ouvrir en Détail »
(`batchopen`). Le « pourquoi » par ligne d'échec suppose d'enrichir `BatchResult` côté Rust.

**Confirmation au-delà du seuil** : **alerte modale** du kit (§ 06-02) — carte + titre +
**corps récap structuré** (N prêts → Convertir · M FAKE → Écarter · exclus ignorés ·
destination · format) + boutons **secondaire gris / primaire bleu** + case **« ne plus me
demander »**. Elle **remplace l'armement in-rail** (non ancré Apple). Le « ne plus demander »
vaut pour **la session**, pas un réglage permanent : la confirmation vise un clic non humain,
et le plancher horodaté de 250 ms reste le garde. In-app, **jamais** `window.confirm()`.

## États

| État | Rendu |
|---|---|
| **File vide** | `emptyStateHtml` en zone C, **deux cas** (`state.filedThisSession`) : rangé quelque chose cette session (>0) → « Tout est trié » + compte + **Bibliothèque** ; rien rangé (=0) → « Rien à revoir » + **Accueil** (fix #15 — Revue vide n'est jamais un cul-de-sac) |
| **Aucune piste ouverte** | Zone C en indice sobre, jamais un canevas nu |
| **Analyse en cours** | Ligne de file avec indicateur ; la zone C montre ce qui est déjà connu (tags, nom) et un **squelette statique** pour ce qui manque (placeholder **sans animation** — `DESIGN.md` § 6 « la donnée ne s'anime jamais », précédence `.jrnl-skel` ; jamais un spinner nu) |
| **Analyse échouée** | La ligne se voit **mieux** que les autres, pas moins bien (teinte **danger** rouge, distincte de l'ambre « à vérifier »). Jamais d'atténuation à l'opacité. Bouton Réanalyser dans la ligne |
| **Rangement en cours** | Bouton Convertir en attente, rail verrouillé, file non bloquée |
| **Rangé** | Bandeau **au-dessus du rail**, avec le chemin final et « Annuler ». **Neutre** en permanence ; au rangement il **flashe vert brièvement** puis retombe neutre (`DESIGN.md` § 8 : un état permanent ne se colore pas) |
| **Lot, confirmation** | **Alerte modale** (kit § 06-02), corps **récap structuré** (N prêts → Convertir · M FAKE → Écarter · exclus ignorés · destination · format) + case « Ne plus me demander (**cette session**) ». Remplace l'armement in-rail. Plancher 250 ms horodaté = garde anti-clic. Jamais `window.confirm()` |
| **Lot, progression** | Sheet **non-modale** attachée (forme mince, feuille de copie Finder) : barre déterminée + ligne d'étape + disclosure « Afficher les détails » ; bouton **« Arrêter »** (la piste en cours finit). Puis la sheet **devient le rapport** (chiffres + Rangés / À valider / Échecs dépliables, « Ouvrir en Détail ») |
| **Destination non réglée** | Le popover Destination rend **quand même** « Sur place », le groupe « Autres » (dossiers externes déjà choisis) et « Choisir un dossier… » : ces trois-là n'ont jamais eu besoin d'une racine. **Seule la section arbre** est remplacée par la porte de premier réglage (« Aucune racine de bibliothèque — cet arbre reste vide… » + « Choisir la racine… »). L'échec de lecture de l'arbre se dit **avant** la porte. ⚠️ **Corrigé le 2026-09-02 (issue #54)** : la porte remplaçait le popover ENTIER, donc elle cachait les trois destinations sans racine et laissait Convertir désactivé alors que le backend aurait rangé. Le libellé « choisis ta racine pour commencer à convertir » est faux depuis la même décision |
| **Racine absente, destination visant l'arbre** | Le refus backend `NoLibraryRoot` devient un **toast actionnable** : « Conversion bloquée — aucune racine de bibliothèque. » + bouton **« Choisir la racine »** menant à Réglages (issue #54, 2026-09-02 — le refus nommait un réglage sans y mener, manque relevé par #16). Le mécanisme est le bouton d'action du toast, dont seul le libellé était figé sur « Annuler ». **Ce refus ne peut plus venir que d'un bac de l'arbre** : convertir EN PLACE ou vers un dossier externe n'exige aucune racine (`filing::needs_library_root`) |

## Interactions

### Souris

- **Clic** ligne de file : ouvre la piste dans la zone C.
- **Clic** sur **« Sélectionner »** (tête de file) : arme le mode Batch (cases par ligne) ; le
  bouton devient « Terminé » et désarme. **Clic** sur le **bouton de filtre** (même rangée, à
  gauche) : ouvre le menu cochable.
- **Clic droit** ligne : Ouvrir l'emplacement · Réanalyser · Écarter · Changer la
  destination. En mode Batch, agit sur la sélection : Ranger N · Changer la destination ·
  Écarter N. **Livré et vérifié le 2026-08-26** (`a31ebf2`) — le menu ne servait que le mode
  Batch jusque-là. En mode Détail le clic droit OUVRE d'abord la ligne, comme dans Finder ou
  Mail : « Écarter » et « Changer la destination » agissent sur la zone C, donc sur la piste
  ouverte, et non sur un identifiant.
- **Clic** sur la waveform : déplace la lecture. **Survol** : ghost fill jusqu'au curseur + ligne fine + bulle mm:ss (teinte **transitoire**, distincte du fill de lecture) — patron QuickTime + réticule spectro.
- **Clic** sur le temps : **bascule écoulé / restant** (une seule valeur affichée).
- **Glisser** la poignée : largeur de la file, persistée.
- **Glisser** des fichiers depuis l'OS sur la file : import. Sur le rail d'action :
  nouvelle destination.

### Clavier

Couches 1 et 2 de `DESIGN.md` § 9, plus la couche 3 propre à cet écran :

| Touche | Action |
|---|---|
| `Espace` | Lecture / pause |
| `Entrée` | Convertir (ranger) |
| `⌫` ou `X` | Écarter, ou Re-source si le verdict est `fake` |
| `I` | Identifier |
| `↑` `↓` | Piste précédente / suivante dans la file |

Règles de focus, non négociables : aucun raccourci ne se déclenche dans un `INPUT` ou
un `TEXTAREA` ; un raccourci à une lettre retire le focus du bouton actif avant d'agir
(sinon `Espace` active le bouton focalisé **et** la lecture) ; en mode Batch, les lignes
de sélection possèdent `Espace` et `Entrée` exclusivement. `Échap` dans un champ
Métadonnées **annule l'édition** (revert à la valeur du focus-in) et `stopPropagation` — il
ne remonte pas fermer un popover ou la fenêtre.

### Retour

Le pouce du contrôle segmenté glisse en `--duration-slow`. Le passage d'une piste à
l'autre ne rejoue aucune animation d'entrée : c'est un changement de contenu, pas une
transition d'écran. La forme d'onde ne s'anime pas au chargement. En **fin de piste**, la
lecture s'arrête et le playhead revient à **0** — pas d'auto-avance vers la suivante (la
zone C ne se recompose pas sous l'utilisateur, patron Musique piste isolée).

## Composants — calés sur le kit Big Sur

Relevés sur `docs/design-refs/macOS Big Sur UI Kit (Community).pdf` (vectoriel) ; les
valeurs chiffrées restent des tokens `styles.css`, jamais extraites au pixel. Depuis le
**2026-08-25**, le kit est aussi relevé au **pont Figma REST** (fileKey
`k3ek2XpmIKjqiFUsyn5kCi`), **nœud par nœud** — c'est ce relevé qui a re-raciné l'échelle de
rayons (ci-dessous).

- **Convertir** = *push button* primaire : aplat d'accent (`--color-accent-fill`) + texte
  blanc (`--color-accent-ink`).
- **Écarter** = *push button* **secondaire = gris rempli** (fond `--overlay-selected`, texte
  encre). Le kit **ne connaît pas** le ghost à bordure — à ne pas réintroduire.
- **Recherche** = *search field* : loupe + placeholder + clear `×` + anneau d'accent au
  focus. ~~**En tête** de la colonne file (décision E)~~ → **sous la rangée de filtre**, 2026-08-26.
- **Segmented** (**Format** seul ; le Détail/Lot est **retiré**) = pill blanc **surélevé**
  (ombre) dans un conteneur gris.
- **Icône de sélection** (Batch, barre unifiée) = icône seule + infobulle ; teinte d'accent
  quand armée. Arme les cases par ligne (patron « Sélectionner » Photos, § 11 option 4).
- **Filtre** = *pop-up button* + menu à **cases à cocher** (union multi-critères, compte par
  option, « Tout afficher » remet à zéro).
- **Genres** = **texte + icône tag** (glyphe *tag* du kit, § 01 Icons) ; pas de chips.
- **Champs Métadonnées** = *text field inline* : texte au repos, champ + anneau au focus.
- **Slider** (waveform-overview) = piste fine + **pastille ronde** (bord + ombre) ; portion
  active en accent.
- **Volume** = **capsule macOS Big Sur toujours visible** (kit § 07-Slider Pickers, rangée 1) :
  pilule pleine hauteur, **haut-parleur intégré à gauche** (clic = couper), gros pouce rond,
  remplissage à gauche du pouce, pouce et remplissage **bornés à gauche** pour que l'icône
  reste posée sur le remplissage. Le repli-au-survol (ex-décision D) est **abandonné** —
  « le design n'est pas bon du tout » (Antoine 2026-08-25), on tient le composant du kit.

  **Inversion en thème clair, tranchée le 2026-08-25.** En **sombre**, le remplissage reste
  **clair sur piste sombre**, fidèle au kit. En **clair**, il passe **foncé sur piste claire**.
  La mesure qui a décidé : le contraste remplissage / piste vaut **1,45:1 en clair** contre
  **12,67:1 en sombre** — sous le **3:1** qu'exige WCAG 1.4.11 pour un composant d'interface.
  En clair, le remplissage du kit ne se voyait donc pas du tout ; ce n'est pas une préférence
  de teinte, c'est un seuil franchi. **Survol et pressé** sont ajoutés dans le même geste : le
  kit les donne, la première version ne portait que le repos.
- **Popover** (Destination) = carte arrondie **sans bec** + ombre, ancrée au bouton « ▾ »
  (*pulldown* du kit § 05 ; le bec appartient au `NSPopover` détaché, **retiré le
  2026-08-25**) ; sections façon sidebar Finder — en-tête « Bibliothèque » sur l'arbre,
  « Autres » sur les dossiers custom — puis « Nouveau dossier » / « Choisir un dossier… » →
  sélecteur natif, le dossier choisi **entre dans la liste**.
- **Alerte** (Lot) = carte + titre + message + boutons secondaire/primaire + « ne plus
  demander ».
- **Rail (sidebar)** = item actif **gris arrondi**, **jamais bleu** (le kit montre le bleu ;
  `DESIGN.md` § 14 l'écarte, le bleu est déjà pris — on tient le gris).
- **Rayons** = échelle **re-racinée sur le kit le 2026-08-25** (commit `b973ce3`,
  `DESIGN.md` § 3) : `--border-radius-xs` **4** (case à cocher) · `--border-radius-sm` **5**
  (item de menu, champ texte, segmenté) · `--border-radius-md` **7** (bouton, champ de
  recherche, pulldown, bouton icône) · `--border-radius-lg` **14** (grandes surfaces,
  inchangé, et seul cran qui ne vient pas du kit). L'ancienne dérivation par `calc()` donnait
  8 et 10 : les neuf composants livrés de Revue étaient **tous** décalés d'un cran vers le
  rond, pour cette seule raison. Toujours le token, **jamais un littéral**.
- **Typographie** = 3 tailles effectives (`--text-lg` 15 / `--text-base` 13 / `--text-sm`
  11), hiérarchie par l'**encre**, **monospace réservé** aux chiffres alignés en colonne
  (durées, mesures) + `tabular-nums`. Échelle SF Pro du styleguide confirmée
  (26/22/17/15/13/11/10 pt).

## Conflits code ↔ spec — à corriger à l'implémentation

Relevés par le brainstorm Phase 1 (2026-08-24), **chacun vérifié sur le code**. Le code
contredit une décision figée ; c'est le **code** qui bouge, pas la décision.

1. **Volume — capsule Big Sur, repli abandonné (résolu 2026-08-25)** — le repli-au-survol
   (ex-décision D) a été **retiré** : Antoine l'a rejeté (« le design n'est pas bon du tout »)
   au profit du **composant capsule du kit** (§ 07-Slider Pickers, rangée 1, relevé sur les PDF
   `docs/design-refs/`). `.sift-volume-block`, `.sift-volume-track` et la base partagée
   `.sift-slider-*` (seul autre consommateur, le slider tempo, déjà retiré) sont supprimés ;
   la capsule est désormais le **SVG du kit inliné tel quel** — `.sift-volume`, qui **est**
   le slider (`role="slider"`), le rendu ne pilotant que la largeur du remplissage et le `cx`
   du pouce. Coupure du son câblée (icône intégrée à gauche, clic → `setVolume(0)` ↔ dernier
   volume, glyphe barré à 0). Géométrie mesurée dans les deux thèmes avant livraison.
   ⚠️ `DESIGN.md` § 17 (« le débordement de `.sift-volume-track` n'est pas un défaut »)
   décrivait le mécanisme retiré : **marqué périmé sur place le 2026-08-25**, à ne pas
   invoquer pour restaurer le repli.
2. **Deux temps au lecteur** — `report-view.ts:898-908` affiche écoulé **et** restant (façon
   SoundCloud) ; la spec (Zone C, lecture) veut **un seul temps cliquable**. → un seul.
3. **Bandeau « Rangé » vert permanent** — `styles.css:2438`
   (`background:var(--color-background-success)`) ; `docs/design-system/components.md:193`
   interdit « un aplat vert permanent pour dire "c'est fait" » et `DESIGN.md` § 4 veut un état
   permanent neutre. → fond **neutre + flash** vert bref au rangement (`sift-identified-flash`,
   ne rejoue qu'au rangement, pas à la navigation). Le commentaire daté 2026-08-05 ne portait
   que sur l'**encre**, pas sur le fond.
4. **Rail verrouillé à l'opacité** — `filing-actions.ts:30` (`b.style.opacity = "0.55"`) ;
   `DESIGN.md:262` interdit l'atténuation par opacité (« aucune valeur d'opacité ne franchit
   4,5:1 avant ~0,92 »), le levier est le **token** (encre secondary / disabled). → token,
   pas opacité.
5. **⌘F rate la recherche de Revue** — `focusBarSearch` (`toolbar.ts:188`) cible la barre
   unifiée ; décision E a mis la recherche dans la **colonne file**. → ⌘F doit poser le focus
   dans la recherche de la colonne file en Revue (câblage à confirmer au moment du fix).

Tranché (Phase 1, fork) : « échec » passe de l'ambre au **danger** rouge (`queue-panel.ts:345`),
pour le séparer du doute « à vérifier » (qui garde l'ambre). `DESIGN.md:265` n'imposait que la
**visibilité** de l'échec (déjà acquise) ; la couleur, elle, était ouverte.

## Hors périmètre / questions ouvertes

- **Analyse froide et réactivité.** `analyze_path` est une commande Tauri **synchrone**
  qui appelle `analyse()` en ligne sur cache miss : toute l'IPC est bloquée pendant.
  Le dépôt a l'outil pour chiffrer la durée (`bench_sqlite.rs` via
  `SIFT_BENCH_TRACKS_DIR`, `--ignored`) et ce chiffre n'existe pas. **Décision
  d'architecture, pas de design.**
- **Critère CDJ — établi, badge retiré, code à recâbler.** Le badge **« Prêt CDJ » n'existe
  plus** sur cet écran (retiré le 2026-08-25, `paintCdjBadge` supprimé) : ce qui reste ouvert
  est **backend**, pas visuel. Le WAV est le vrai cas d'échec (tags non affichés fiablement),
  et `tags_cdj_ok` (`analysis/tags.rs`) reste à recâbler sur le format réel via `lofty`
  ([#46](https://github.com/c0remusic/sift/issues/46)). La question « intégrer la contrainte
  codec / génération (FLAC/ALAC ≥ 2016) » ne se rouvrira qu'avec un éventuel nouveau porteur
  visuel.
- **Patron inspecteur des Métadonnées** — repris de Finder « Lire les informations » ; le
  *text field inline* du kit (§ 03-03) le confirme comme composant. La disposition exacte
  (ordre des champs, densité) se cale à l'implémentation.
