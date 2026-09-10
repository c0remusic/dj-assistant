# Spec — Bibliothèque

> Écran canonique. Les cinq autres specs héritent de sa table (`DESIGN.md` § 16) et
> ne redécrivent que leurs écarts. Toute décision prise ici et contredite ailleurs
> est un défaut, pas une variante.

## Contexte dans le shell

**Profil Parcours** (`DESIGN.md` § 14). Patron macOS : **Finder / Music** — sidebar de
sources, toolbar unifiée, table dense, inspecteur.

Trois zones : rail (`--rail-w`, fixe) · table (flexe) · inspecteur (`--pane-w`, fixe,
repliable). La page ne défile pas ; la table défile chez elle, en-têtes de colonne
figés.

Cet écran absorbe **Écartés** (`DESIGN.md` § 15, fusion 2) : « À re-sourcer » et
« Corbeille » sont deux entrées de la section Bibliothèque du rail, rendues par la
même table.

## Layout

### Zone B — rail, section « Bibliothèque »

Trois entrées, sélection exclusive, fond plein arrondi
(`--color-background-secondary`) sur l'active :

| Entrée | Contenu de la table | Compte |
|---|---|---|
| **Rangés** | Pistes converties et rangées | oui |
| **À re-sourcer** | Écartées avec un motif récupérable | oui, ton `warning` si > 0 |
| **Corbeille** | En attente de purge | oui, ton neutre |

Sous ces trois entrées, la **facette active** se choisit dans la barre unifiée, pas
dans le rail — le rail porte des *sources*, pas des filtres.

### Zone A — barre unifiée

De gauche à droite après les contrôles de fenêtre :

1. **Titre** — le nom de la source active (« Rangés », « À re-sourcer », « Corbeille »),
   suivi du compte en `--color-text-tertiary`.
2. **Facette** — contrôle segmenté à trois options : Dossiers · Genres · Artistes.
   Le choix pilote le contenu du sélecteur de facette (ci-dessous).
3. **Qualité** — trois chips : Tous · Lossless · MP3. Grammaire chip (filtre non
   exclusif de la facette), pas contrôle segmenté.
4. **Doublons** — action, pas filtre. Lance le scan sur toute la bibliothèque.
5. **Mode de vue** — contrôle segmenté, deux icônes : Table · Grille.
6. **Recherche** — à droite, toujours. ⌘/Ctrl+F y place le focus.

Le sélecteur de facette (liste des dossiers / genres / artistes avec leurs comptes)
descend en **tête de la table**, replié par défaut en un bouton portant la valeur
active. Il ne prend plus une colonne à lui : il filtrait, et un filtre appartient à la
barre. Ce geste supprime `.sift-library-side` et sa hauteur
`calc(100dvh - 210px)`, qui encodait la hauteur du bloc au-dessus.

**Livré le 2026-08-19.** Le bouton porte le type en encre secondaire et la valeur active
en encre primaire — la hiérarchie d'un chemin de Finder : on lit d'abord où on est.
Le panneau s'ouvre **sous** le bouton (`anchoredBelowPosition`, jumelle de la géométrie
du popover de destination : celle-là sert un bouton en bas de fenêtre, celle-ci un bouton
en tête de table). Une facette sans valeur le dit — « Aucun dossier pour l'instant. » —
au lieu d'ouvrir ses types sur du vide, ce qui se lit comme un défaut de chargement.
`.sift-library-layout` est partie avec `.sift-library-side`, et avec elle les **quatre**
rattrapages de largeur qu'elle documentait (150 → 190 → 245 → 272).

#### Le composant est un pop-up button, pas une carte flottante

Première version : une carte de `--pane-w` contenant un **contrôle segmenté** à trois
onglets empilé au-dessus d'une liste. Refaite le jour même sur retour d'Antoine (« le
panneau est placé bizarrement, regarde comment fait Apple Music »), contre deux sources.

- **HIG « Pop-up buttons »** : « A pop-up button displays a menu of mutually exclusive
  options », et « the button can update its content to indicate the current selection ».
  La même page renvoie au **pull-down button** dès qu'il faut un sous-menu — or empiler un
  sélecteur de type au-dessus d'une liste de valeurs, c'est exactement un sous-menu déguisé.
- **`docs/design-refs/macos-big-sur-ui-kit.png`**, § 04-Pickers / **02-Menu Pickers**,
  dark mode : le menu ouvert est une liste d'items compacts, largeur prise sur le contenu,
  **coche à gauche sur le seul item sélectionné** avec gouttière réservée pour les autres.
  Les **Segmented Pickers** y sont un composant *voisin et distinct* — c'est celui qu'on
  avait mis à l'intérieur.

Conséquences appliquées : plus de contrôle segmenté (le type devient la première **section**
du menu, séparée par un filet), largeur `max-content` bornée 180–320 px au lieu de
`--pane-w`, items à `role="menuitemradio"`, et la coche à l'**encre du texte** — relevée
blanche dans le kit, pas à l'accent.

⚠️ **Aucune dimension n'est reprise de cette planche.** Son échelle d'export n'est pas
établie (5129 × 32768 px, lue par bandes), donc seule sa **structure** fait autorité, comme
le demande la doctrine.

#### Le placement se recalcule, il ne se mémorise pas

Le panneau est rouvert par `renderBiblioLive` après un rebuild complet de `#content` — donc
au milieu d'un cycle de mise en page où la barre unifiée et la liste virtualisée ne sont pas
encore montées. L'ancrage se fait donc au **second frame** (un `requestAnimationFrame`
s'exécute avant le recalcul de style : même leçon que `playFadeIn` dans `confirm-modal.ts`),
et défiler ou redimensionner **ferme** le menu au lieu de courir après sa géométrie — c'est
déjà la règle de `context-menu.ts`, et une seule règle pour deux surfaces flottantes vaut
mieux que deux comportements à retenir.

### Zone C — table

Colonnes, largeurs, tri, densité, sélection, menu contextuel : **`DESIGN.md` § 16 sans
écart**. Rappel des deux ajouts : **BPM** et **Durée**, colonnes fixes en `--font-mono`
avec `tabular-nums`, alignées à droite.

En-tête de table figé pendant le défilement. **Livré le 2026-08-19** : `position:sticky`
sur `.sift-lib-thead`, le conteneur de défilement étant `#content` et non la table — aucun
nombre n'a donc à connaître la hauteur de ce qui précède, même raisonnement que le retrait
de `calc(100dvh - 210px)`. Fond **opaque** obligatoire, sinon les lignes défilent visiblement
sous l'en-tête et les deux textes se superposent. Mesuré : l'en-tête passe de 116 px à 68 =
`contentTop (44) + paddingTop (24)`, et la première ligne se cale à 85, donc dessous.

Ni shadcn ni ui-thing n'exposent de composant d'en-tête figé (cherché le 2026-08-19, zéro
résultat des deux côtés) — la référence ici est le mécanisme CSS, pas un composant, et c'est
dit comme tel plutôt que d'inventer une source.

Au-dessus de la table, une seule ligne : bouton de facette · nom de la valeur active ·
compte de pistes. Les cartes de statistiques d'aujourd'hui (`statsCardsHtml`) quittent
le haut de l'écran — elles poussaient tout le contenu vers le bas et forçaient la
constante `210`. Leur donnée remonte dans l'inspecteur en état « aucune sélection ».

**Livré le 2026-08-19**, et la mesure chiffre le geste : la première ligne de la table
commençait à **396 px** du haut de la fenêtre, elle commence à **138**. `statsCardsHtml`
est supprimée, et `library_stats` ne fait plus partie des appels de l'écran — un
aller-retour IPC de moins à chaque frappe de recherche.

La surface de la table remplit désormais la zone C (`min-height:100%`) : avec deux pistes
affichées elle mesurait 152 px dans une zone de 864, et le fond s'interrompait au milieu de
l'écran comme si le contenu était coupé. Dans Finder les **lignes** s'arrêtent, la surface
non.

**Mode Grille** : mêmes données, tuiles de pochette. Il hérite du tri de la table et
n'a pas de contrôle de tri propre. `LIBRARY_GRID_TILES_PER_ROW` reste piloté par la
largeur réelle de la zone, pas par une constante.

Virtualisation obligatoire dans les deux modes — la zone C est le conteneur de
défilement, plus `#content`.

### Zone D — inspecteur

- **Aucune sélection** — résumé de la source active : nombre de pistes, répartition par
  format, occupation disque. C'est là que vont les cartes de statistiques retirées de
  la zone C.

  ⚠️ **La répartition se calcule sur ce que la table montre, jamais sur `library_stats`.**
  Mesuré le 2026-08-19 : la première version reprenait les compteurs globaux, et sous un
  titre « TECH HOUSE · 2 pistes » on lisait « Lossless 2 · MP3 1 » — trois pistes sous un
  titre qui en annonce deux. C'est exactement le défaut que sortir ces cartes de la zone C
  devait supprimer, pas déplacer. Le graphique d'occupation, lui, reste global et garde son
  cache : il décrit la bibliothèque entière et le refaire à chaque frappe coûterait un
  aller-retour pour un résultat identique.
- **Une piste** — pochette, artiste, titre, version · lecture et forme d'onde ·
  verdict avec son détail · métadonnées (label, année, genres, BPM, durée, format,
  débit) · chemin du fichier · actions : Identifier, Fiche Discogs, Réanalyser.
- **Plusieurs pistes** — résumé agrégé : compte, formats présents, durée totale, et les
  seules actions applicables en masse.

Sections repliables, en-têtes discrets, libellé à gauche en `--color-text-secondary`,
valeur à droite en `--color-text-primary`.

### Section doublons

Aujourd'hui rendue dans le flux, sous la table. Elle devient un **mode de la zone C** :
lancer le scan remplace la table par la liste des groupes, avec un retour explicite.
Un scan est un résultat, pas un appendice de liste.

**Livré le 2026-08-19.** La ligne d'en-tête change de contenu avec le mode : le bouton de
facette cède la place à « ← Retour à la table », au même endroit — ce qui pilote la zone C
reste à la même place d'un mode à l'autre. Le retour est une porte **nommée** : sans elle,
on sort d'un résultat en devinant quel contrôle le referme.

## États

| État | Rendu |
|---|---|
| **Vide, sans filtre** | `emptyStateHtml` — « Bibliothèque vide », note vers Revue, une action. Impasse assumée : le rail et la barre restent |
| **Vide, avec filtre** | « Aucun résultat » + « Réinitialiser les filtres » s'affichent **sous l'en-tête de colonnes**, qui reste. Réf. shadcn `data-table-demo`, dont l'état vide est une ligne du corps (`colSpan`, texte centré) : remplacer la table emporterait les en-têtes, donc les contrôles de tri — on retirerait les commandes qui pourraient défaire le filtre. Barre et recherche restent aussi |
| **Chargement, premier rendu** | Squelette de lignes dans la structure finale. Jamais un écran blanc |
| **Chargement, re-rendu** | Les données valides restent affichées. Un rendu déclenché par une frappe, un clic de facette ou un changement de tri **ne blanchit jamais** l'écran |
| **Recherche en cours** | Indicateur discret dans la barre unifiée, à droite du champ. Débounce 250 ms |
| **Scan de doublons en cours** | Zone C en mode scan, progression déterminée, bouton Annuler présent |
| **Erreur de scan** | Carte douce, encre `danger`, bouton Réessayer. **Rien n'est dit du contenu** : ne jamais afficher « aucun doublon » après un scan échoué |
| **Sélection** | `--color-background-secondary` sur les lignes, inspecteur en résumé agrégé |

## Interactions

### Souris

- **Clic** ligne : sélectionne, remplit l'inspecteur.
- **⇧+clic** : étend la plage · **⌘/Ctrl+clic** : ajoute ou retire.
- **Double-clic** : ouvre l'emplacement du fichier.
- **Clic** sur le bouton lecture de la ligne : écoute. Seul bouton restant dans la ligne.
- **Clic droit** : Ouvrir l'emplacement · Identifier · Fiche Discogs · Réanalyser ·
  Écarter · (sur À re-sourcer et Corbeille) Restaurer, Purger.
- **Glisser** sur un séparateur d'en-tête : largeur de colonne, mémorisée.
- **Glisser** un en-tête : ordre des colonnes, mémorisé.
- **Glisser** un dossier depuis l'OS : ajout aux sources surveillées.

### Clavier

Couche 1 et couche 2 de `DESIGN.md` § 9 intégralement. Rien de propre à cet écran, et
c'est voulu — une table se pilote partout de la même façon.

`⌘/Ctrl+F` place le focus dans la recherche · `Échap` la vide puis rend le focus à la
table.

### Retour

Changement de facette et de tri : instantané, aucune animation sur les lignes.
Le pouce du contrôle segmenté glisse en `--duration-slow`.
Aucune animation sur le défilement ni sur le tri.

## Décision — 2026-09-08 : Rangés lu contre Revue — la table au sol, la barre pilote

**Contexte.** Déclinaison de la direction retenue sur Revue (#24), premier écran : Rangés.
Méthode : vraie fenêtre (1561 × 941), zones mesurées contre celles de Revue, sept constats
sourcés, trois directions dessinées sur la grille et les **plans mesurés** de Revue — barre,
rail, zone C et inspecteur partagent UN fond (`--color-background-primary`, 0.2273 en
sombre), la file de Revue est la seule zone levée (0.2939). Rangés peignait sa zone C de la
couleur de cette file (carte `.sift-ui-card`) plus un cran pour l'en-tête (0.3492) : trois
plans là où Revue en a deux. Sources motif : `docs/design-refs/03-mail.png` (filtre en tête
de liste), `02-photos.png` (pop-up de portée au centre de la toolbar), `finder-window.png`
(contenu bord à bord) ; règles : HIG Toolbars (trois emplacements, trois groupes maximum,
bord arrière = recherche + bouton d'inspecteur), Sidebars (collections dans la sidebar),
Split views (volet tertiaire repliable par plusieurs chemins).

**Tranché par Antoine : « F mais avec la barre haute de M », tête de liste supprimée.**

- **Zone C au sol.** `.sift-library-main` perd `sift-ui-card sift-ui-card-pad` ; inset
  horizontal `--space-16` comme la zone C de Revue, rien en haut. L'en-tête figé prend le fond
  du sol (`--color-background-primary`), opaque parce que sticky.
- **La barre pilote tout.** Bouton de facette « Dossiers · Tous ⌄ » (pop-up button, popover
  inchangé, ancré sur `[data-bib="facetpop"]`) en tête des actions ; **compte dans
  `#sift-tb-count`** à côté du titre, comme la file de Revue ; en mode doublons, « ← Retour
  à la table » prend la place du bouton de facette (ce qui pilote la zone C reste au même
  endroit) et le compte dit « Doublons — toute la bibliothèque ». La rangée
  `.sift-bib-headline` disparaît.
- **Colonnes : Artiste · Titre · Durée · Genre · Année.** **Verdict retirée** (« pas besoin de
  mettre le verdict ») — le verdict se lit dans l'inspecteur à l'ouverture ; le tri par rang
  catégoriel part avec elle, `verdictView` et la story `library-verdict.stories.ts` aussi.
  **BPM retirée** : aucun écrivain côté Rust (`metadata.bpm` lue, jamais remplie — grep du
  2026-09-08) ; elle reviendra avec l'analyse qui l'écrira. Un stockage `sift-libcols-v1` qui
  porte encore ces deux champs est filtré (gelé par `test/library-columns.test.ts`).
  ⚠️ DESIGN.md § 16 (« colonne 1 = Verdict », « BPM… en base et n'atteint pas l'écran ») est
  **en retard** de cette décision et faux sur BPM : à recaler.
- **Ligne : pochette = bouton de lecture** (patron Musique) — le triangle apparaît au survol
  de la ligne par-dessus la vignette (`.sift-lib-play`, voile `--overlay-scrim`), au focus, et
  sur la piste ouverte. Le bouton lecture séparé (22 px) et l'**icône Discogs / loupe** de fin
  de ligne sont retirés (DESIGN § 16 : les actions secondaires vivent au clic droit et dans
  l'inspecteur). Espaceurs d'en-tête : 24 et 40.
- **Inspecteur idle inchangé** (« ▶ Voir le détail complet » est un disclosure, pas un CTA à
  icône).

Vérifié dans la vraie fenêtre (CDP) : `.sift-library-main` sans carte, padding `0 16px`,
fond transparent sur `.pa` 0.2273, en-tête 0.2273 ; barre « Dossiers Tous · Tous Lossless MP3
Doublons · vues · recherche », compte « 15 pistes » ; en-têtes 24 / 269 / 269 / 52 / 192 / 52 /
40 = cellules ; 0 `.lk-icon`, 15 glyphes de lecture ; survol réel : triangle sur la pochette ;
popover de facette ancré sous son bouton dans la barre ; mode doublons : « Retour à la table »
dans la barre, compte renommé, retour = 15 lignes et bouton de facette de retour.

**Inspecteur à l'ouverture — « ok pour le proposé » (même nuit, wireframe « Rangés —
inspecteur ouvert », colonne réelle 1:1 contre la colonne en composants de Revue).** Livré
en deux temps :

- **Livré le 2026-09-08 (5, 1, 2, 4 du wireframe)** — toutes règles scopées `#sift-aside`,
  donc valables aussi pour la zone D de Revue : rangée de mesure **libellé au-dessus de la
  valeur** (le rail de 150 laissait ~60 px à la valeur, « Pleine bande · coupure 22050 Hz »
  sur quatre lignes) ; **en-tête de Revue** — titre et artiste dans `.sift-report-name` /
  `.sift-report-sub` (plus le nom de fichier en 15/600 sur trois lignes), pochette **fixe 56**
  en colonne (`COVER_COLUMN_PX`, la piste « pochette fixe 56px » de la spec Revue que l'en-tête
  B avait écartée en surface large), verdict qui passe à la ligne au lieu de se couper ;
  **lecteur simple de l'app, sa rangée telle quelle, mise à l'échelle de la colonne** — « réutilise
  les composants déjà utilisés, scale-les à la taille de la fenêtre » (Antoine) : gaps à
  `--space-12`, capsule compressible (90 au repos, 56 au plancher), onde à plancher 64 ; dans
  245 px l'onde tombait à 0 (une version à deux rangées, capsule étirée à 225, a vécu une heure
  et n'était plus le bouton de volume de l'app). Mesuré après : play 28 · onde 65 · temps 24 ·
  haut-parleur 22 · capsule 90 dans 277 ; **plus aucune carte** — la boîte
  de lecture ne peint rien en colonne, la carte « Piste ouverte » et son chevron sont partis
  (fermer = re-cliquer la ligne), l'éditeur perd son cadre. Mesuré après : pochette 56 × 56,
  titre deux lignes, sous-titre = artiste, onde 177 × 24, capsule 225, rangées en colonne de
  38 px, 0 carte, 1333 px de défilement (1546 avant, 3292 la veille).
  Écart au wireframe, assumé : le verdict se lit entre le titre et l'artiste (la rangée de titre
  de Revue le porte, l'artiste est un frère de cette rangée — pas de réordonnancement CSS
  possible sans toucher au markup partagé).
- **Livré le 2026-09-08 aussi (3 du wireframe, « point 3 puis les autres écrans »)** : la
  fiche Métadonnées de Revue dans la colonne, à sa place — en-tête · audition · **Métadonnées**
  · Diagnostic (le Diagnostic prend un slot à lui, `.lib-diag`, 5ᵉ argument d'`openReportInto`,
  même mécanisme que la zone D de Revue). Même grammaire, mêmes classes (`.sift-meta-*`,
  `.sift-attr-*`, `.sift-cands-host`, `chosenRowHtml`) : titre « Métadonnées », release choisie
  en ligne inerte (label · année — pays et format n'existent pas sur `LibraryTrack`), bouton
  Identifier / Ré-identifier (liste ouverte, fork F ; sans badge « I », le raccourci est celui
  de Revue), attributs en place Artiste · Titre · Label · Année · Genres — **gravés au blur /
  Entrée, Échap annule** (`update_metadata`, toast « Enregistré » annulable). Partis : champs
  bordés, « Enregistrer », « Voir la release » (clic droit « Fiche Discogs »), « Supprimer »
  (clic droit « Envoyer à la corbeille »), « changer » sur la pochette (clic droit **« Changer la
  pochette… »**, nouvelle entrée, active sur la piste dont la fiche est ouverte, gravée aussitôt).
  La pochette monte dans l'en-tête à l'ouverture et à l'identification (`.sift-report-cover`,
  comme Revue). Ce qui est partagé avec Revue : le rendu des candidats et de la ligne choisie,
  le rapport, les classes CSS ; le câblage reste propre à chaque écran (Revue lit `RevueState`
  et grave par `write_tags_full`, ici un `LibraryTrack` par `update_metadata`) — partager le
  markup par des classes et non par une fonction commune est un choix, expliqué en tête de
  `library-detail.ts`. Mesuré : audition 237–278, fiche 314–619, Diagnostic 631–1351 ; 5 champs
  sans bordure ni fond au repos, rail de libellé 72 px, ligne choisie « Groove Armada — … ·
  Ragbull · 2004 », 0 carte, pochette d'en-tête 56 px peinte à l'ouverture, aucun débordement
  horizontal. Genres restent éditables ici (Revue les montre en lecture seule, décision F) ;
  l'autocomplétion sur les genres connus est conservée.

- **Zone D redimensionnable** (« il faut aussi pouvoir redimensionner le panneau », même nuit) :
  la poignée de la file de Revue (`.sift-qresize`, 16 px de prise à cheval sur le filet) posée sur
  le bord gauche de l'inspecteur (`#sift-aside-resize`, index.html ; `toolbar.ts::
  installAsideResize`, câblée une fois au boot). Tirer vers la gauche élargit. Bornes **280–480**
  (280 = la colonne mesurée avec son padding, sous quoi l'onde n'a plus de place ; 480 = le
  plafond de la file), persistée en `localStorage` `sift-aside-w` comme `sift-qcol-w`, réappliquée
  à chaque `openAside`, poignée masquée avec la colonne. Vaut pour tous les écrans qui ouvrent la
  zone D. HIG Split views (« dividers… support dragging to resize »). ⚠️ DESIGN.md § 14 écrit
  encore « D fixe » — en retard. Mesuré : 320 → 420 au glisser de 100 px, bornes 280 / 480
  atteintes, valeur relue à la réouverture, poignée cachée avec le panneau.

Décision S (dossiers dans le rail, canon HIG) écartée ce jour au profit de F + barre.
« Doublons » reste un chip du segmenté de filtres alors que c'est une action (spec § Zone A) :
à sortir. Gap de 24 px entre la barre et l'en-tête figé (padding-top de `#content`) : Finder
n'en a pas, à mesurer.

## Décisions du 2026-08-19

Les trois questions ouvertes de cette spec sont tranchées ci-dessous, chacune par une
mesure du dépôt et non par arbitrage de goût.

### Actions de masse — trois, et le contrat IPC les nomme

La question était : « quelles actions s'appliquent réellement à N pistes **déjà
rangées** ? » Le mode Lot de Revue n'en offre que deux, Convertir et Écarter
(`batch-panel.ts::actionButtonHtml`), et la première n'a pas d'objet ici : une piste de
la Bibliothèque est par définition déjà convertie et rangée. Il restait donc à mesurer
ce que le contrat permet, action par action.

| Action | En masse ? | Pourquoi |
|---|---|---|
| **Réanalyser** | oui | `reanalyzeTracks(trackIds: number[])` prend déjà un tableau (`ipc.ts:51`). Non destructive |
| **Écarter** | oui | `rejectBatch(trackIds)` existe et rend `{rejected, failed[]}` (`ipc.ts:233`) — c'est l'IPC du mode Lot |
| **Corbeille** | oui | `trashTrack(trackId)` est unitaire : la boucle se fait côté frontend, séquentiellement |
| Ouvrir le détail | non | Singulier par définition — un inspecteur montre une piste |
| Fiche Discogs | non | Ouvrirait N pages de navigateur |
| Identifier | non | Réseau Discogs, et chaque identification demande de **choisir** un candidat. Une identification de masse serait un choix pris à la place de l'utilisateur |

Le menu contextuel garde donc **la même liste d'entrées à la même position**, que la
sélection porte une piste ou mille — règle de `context-menu.ts` : ce qui ne s'applique
pas est **désactivé, jamais retiré**. Seuls les libellés portent le compte au-delà de
une.

**Confirmation.** Écarter et Corbeille au-delà de `BATCH_CONFIRM_THRESHOLD` (10, la
constante du mode Lot) passent par `confirmAction()` — modale in-app armée, jamais
`window.confirm()`. Le motif n'est pas la réversibilité (les deux sont annulables) mais
le clic qui n'est pas humain, et `⌘/Ctrl+A` sur une liste virtualisée sélectionne
précisément ce qu'on ne voit pas.

**Compte-rendu.** `rejectBatch` rend `failed[]` : le toast dit le nombre réellement
traité et nomme les échecs. Un compte seul se lirait comme un succès plus petit.

### Colonne Label — inspecteur seul

`LibraryTrack.label` existe et reste **hors des colonnes**. Une colonne optionnelle
supposerait un mécanisme de colonnes activables qui n'existe nulle part : `SORT_COLUMNS`
(`library-views.ts:62`) est une liste fixe de six entrées et les largeurs sont des
règles CSS (`.sift-lib-col-*`, `styles.css:945-949`). Construire ce mécanisme pour un
seul champ dont personne ne trie une bibliothèque de DJ est disproportionné. Le label
reste éditable dans l'inspecteur (`library-detail.ts:89`), là où il est déjà.

À rouvrir si le mécanisme de colonnes optionnelles arrive par ailleurs — il est déjà
demandé par `DESIGN.md` § 16 (largeurs et ordre mémorisés), et ce jour-là Label est le
premier candidat.

### Menu contextuel — la liste réelle

Ordre figé, positions stables, libellés qui portent le compte au-delà d'une piste :

| # | Entrée | Active quand |
|---|---|---|
| 1 | **Ouvrir l'emplacement** | une seule piste — révéler N fichiers ouvrirait N fenêtres |
| 2 | **Ouvrir le détail** / **Masquer le détail** | une seule piste. Le libellé suit l'état : `openBiblioDetail` bascule |
| 3 | **Fiche Discogs** | une seule piste, et identifiée |
| 4 | **Réanalyser** | toujours |
| 5 | **Écarter** | toujours — pas `danger` : réversible depuis Écartés, et DESIGN.md § 4 réserve le rouge au risque réel (corrigé le 2026-08-19) |
| 6 | **Envoyer à la corbeille** | toujours · `danger` |

**« Identifier » n'a pas d'entrée**, et ce n'est pas un oubli : le bouton `identify` de la ligne
ouvre déjà le détail (`sift-live.ts`, `act === "identify"` → `openBiblioDetail`), donc une entrée du
même nom ferait exactement ce que fait « Ouvrir le détail » — deux libellés pour une action. Le
choix d'un candidat Discogs vit dans l'inspecteur, et il ne peut pas en sortir : identifier demande
de **choisir**.

### Persistance des largeurs de colonnes — `localStorage`, et l'argument d'origine était faux

La question opposait `settings` à `localStorage` au motif que « le premier survit à un
changement de machine ». **C'est faux** : la base vit dans `app_data_dir()`
(`src-tauri/src/lib.rs:222`), donc dans le profil utilisateur de la machine, exactement
comme `localStorage`. Ni l'un ni l'autre ne suit l'utilisateur ailleurs.

La ligne de partage réelle est celle que le dépôt applique déjà :

- `localStorage` — **état d'affichage de la fenêtre**, que le backend ne lit jamais.
  Précédent : le repli du rail (`chrome.ts:377`).
- `settings` (SQLite) — **configuration produit** que Rust lit aussi : racine de
  bibliothèque, token Discogs, thème, gabarit de nom de fichier.

Une largeur de colonne est de la première catégorie. Décision : `localStorage`, avec le
`try/catch` du rail — un stockage refusé ne doit pas casser la table.

**Livré le 2026-08-19** (`frontend/library-columns.ts`). Le geste n'existait pas quand la décision
a été prise — `col-resize` n'apparaissait que sur la poignée de la file de Revue. Règles :

- Le séparateur est un **enfant** de son en-tête, sur son bord droit : il suit la colonne quand
  celle-ci change de largeur ou de place. Zone de prise 7 px, comme `.qdrag`.
- Une colonne **non touchée garde sa règle CSS** et continue donc de s'adapter à la largeur de la
  zone. Draguée, elle se **fige en px** — c'est le sens du geste.
- Bornes 48–600 px. Le plancher n'est pas cosmétique : sous 48 px l'en-tête ne montre plus son
  libellé ni sa flèche, et la colonne devient impossible à réélargir puisque sa propre poignée n'a
  plus de prise.
- **Clic droit sur l'en-tête** (patron Finder) : « Réinitialiser les colonnes », désactivée tant que
  la disposition est d'origine. C'est la porte de sortie obligatoire.
- Un ordre mémorisé est **filtré** contre les colonnes connues et **complété** par les manquantes :
  une entrée inconnue peindrait une cellule vide par ligne, une entrée absente ferait disparaître
  une donnée en silence. Gelé par `test/library-columns.test.ts`.

### Ce qu'un en-tête doit faire des deux gestes qu'il porte

Il est **à la fois** bouton de tri et poignée de déplacement. Un seuil de 5 px sépare les deux :
en dessous, le geste reste un clic et trie ; au-delà, il déplace et le clic qui suit est neutralisé.
Sans ce garde, tout réordonnancement trierait aussi la table — deux effets pour un geste.

## Écarts entre cette spec et le code — état au 2026-08-19

- ~~**« Ouvrir l'emplacement » n'est appelable par rien**~~ — **corrigé**. Commande Rust
  `reveal_track` (`ipc_filing.rs`) : prend un `track_id`, jamais un chemin, et résout depuis la base
  — avec un chemin fourni par le front, la branche Windows serait un moyen de pointer Explorer
  n'importe où. Un fichier absent **échoue** au lieu d'ouvrir son dossier parent : ouvrir le dossier
  d'un fichier qui n'y est pas ressemblerait à un succès et ne dirait rien de la question qu'on
  vient de poser. Câblée sur l'entrée 1 du menu **et** sur le double-clic.
- **« Restaurer » et « Purger » au clic droit** supposent que « À re-sourcer » et « Corbeille »
  soient rendues par cette table — c'est la fusion 2, **réfutée par la mesure** (`DESIGN.md` § 15).
  Ces deux actions restent chez Écartés. Rien à corriger ici.
- ~~**Largeurs et ordre de colonnes au glisser**~~ — **livrés**, voir ci-dessus.

## Hors périmètre

- **Tonalité et énergie** — absentes du modèle (`shared/contracts.ts`, `db.rs`, vérifié
  le 2026-08-19). Aucune colonne n'est spécifiée pour elles. Les ajouter est un
  chantier d'analyse Rust, pas de design.

## Décision — 2026-09-10 : la rangée lue contre le Finder

Antoine : « pas fan du design de la liste, comment ferait Apple ? Comment fait la ref ? » Mesuré
dans la vraie fenêtre : rangée de 33 px avec un filet SOUS CHAQUE rangée et un arrondi de 7 px,
survol au plan de sélection (`--color-row-active` = `--color-background-secondary`, celui de
`.sel`), `translateY(1px)` à la pression, format en pastille à fond secondaire en bout de rangée,
Artiste et Titre à flex égal. Les refs locales n'ont pas de table (Mail et Notes sont des listes
multilignes, le kit Big Sur n'a aucun composant table) ; la référence est la présentation par
liste du Finder, et les HIG « Lists and tables » § macOS : « *Consider using alternating row
colors in a multicolumn table* ». Deux maquettes CSS sur les vraies données — A Finder zébré, B
Mail à filets — **A retenue, sans survol de rangée** (macOS n'en a pas).

Livré : `.lr` sans filet, sans arrondi, sans transition ; zébrure `--overlay-alt` (token neuf,
un cran au-dessus de `--overlay-hover`, clair .045 / sombre .05) par parité d'index posée par
`createVirtualList` (`libraryTableRowHtml(…, alt)`) — jamais `:nth-child`, la fenêtre recyclée
changerait de parité au défilement ; aucun `:hover` de rangée, le triangle de lecture se révèle
au survol de la pochette ; en-tête en encre secondaire ; colonne **Format** en texte
(`.sift-lib-col-fmt`, en-tête « Format »), la pastille `.pill` quitte la table. `DESIGN.md` § 16
(« pas d'alternance ») renversé le même jour. Non retenu pour l'instant : l'ordre de Musique
(Titre d'abord).
