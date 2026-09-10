# Spec — À re-sourcer · Corbeille (ex-Écartés)

## Contexte dans le shell

Deux **destinations du rail**, section « Bibliothèque », à côté de « Rangés » : **À re-sourcer**
et **Corbeille**. L'entrée unique « Écartés » a disparu le **2026-09-08** (déclinaison #24,
troisième écran, maquette faite dans la vraie fenêtre avec les composants de l'app — artefact
« Écartés à la manière d'Apple » — et validée par Antoine : « ok pour tout sauf la boutique »).

Patron macOS : une corbeille est **toujours un item de sidebar**, jamais un filtre — Finder
(Corbeille + « Vider » dans la barre), Photos (« Recently Deleted », section Photos,
`docs/design-refs/02-photos.png`, bandeau de règle + Récupérer / Supprimer dans la barre), Mail
(Indésirables, Corbeille), Notes (Suppressions récentes). « À re-sourcer » est l'Indésirables de
Mail : gardé, à traiter. HIG Sidebars : « navigate between areas of your app or top-level
collections of content ».

Ce que `DESIGN.md` § 15 a mesuré le 2026-08-19 tient : `EcarteItem` porte 8 champs (ni pochette,
ni durée, ni genre, ni année), donc **pas la table de Rangés** — la même *grammaire* de table,
avec les quatre colonnes qui existent. Les sept affordances restent (requeue, restore, trash,
purge, copy-query, store, retry). DESIGN § 15 proposait déjà les deux entrées de rail ; seule la
fusion dans la table de Rangés avait été réfutée.

## Layout

### Zone A — barre unifiée

Titre = le libellé du rail (« À re-sourcer » / « Corbeille », `router.ts::syncNav`) · **compte**
dans `#sift-tb-count` (« N pistes », ce que la table montre) · sur la Corbeille, quand elle n'est
pas vide, **« Vider la corbeille »** en *push button* secondaire danger (`.sift-secondary-trash`,
géométrie du kit `--h-control`), confirmation in-app (`confirmAction`, jamais `window.confirm`),
résultat par toast quand des fichiers résistent. Pas de segmenté, pas de facette. Recherche : hors
périmètre pour l'instant (listes courtes).

### Zone C — table

Au sol (`.sift-library-main`, fond de `.pa`, inset `--space-16`), **aucune carte**. En-tête figé
`.sift-lib-thead`, lignes `.lr` de 32 px (`--row-h`), virtualisées (`createVirtualList`).

| # | Colonne | Rendu |
|---|---|---|
| 1 | **Raison** | pastille 6 px + libellé, une seule forme (DESIGN § 16) : **FAUX** (danger, verdict `fake` — `FAKE` jusqu'au 2026-09-10, le même mot que Revue), **TRONQUÉ** (warning, `truncated`), **À VÉRIFIER** (warning, verdict `grey`), **—** (neutre, sans verdict). Non triable : catégorielle, quatre valeurs |
| 2 | Artiste | triable, tri par défaut ascendant |
| 3 | Titre | triable |
| 4 | Fichier | `--font-mono`, `--text-sm`, tronqué ; triable |
| — | Format | pastille `.pill` en fin de ligne, extension du fichier |

Pas de pochette (le champ n'existe pas), pas de bouton dans la ligne. Clic = ouvre / referme
l'inspecteur sur la piste (`.cur`). Entrée / Espace pareil au clavier.

### Zone D — inspecteur

- **Au repos** : titre de la destination, compte, répartition par raison, la règle en une
  phrase (Corbeille : « Les fichiers restent sur le disque jusqu'au vidage » ; À re-sourcer :
  « Des pistes à racheter : le fichier est faux, tronqué ou douteux »).
- **Une piste ouverte** — les composants de Revue, sans pochette ni lecteur (un fichier écarté
  ne s'écoute pas ici, et `EcarteItem` n'a pas de pochette) :
  1. **En-tête** `.sift-player-header` : titre, pastille de raison + extension, artiste,
     fichier en mono.
  2. **Raison** (titre de fiche `.sift-meta-title`, direction T) : une phrase **factuelle** —
     `EcarteItem` ne porte ni mesure ni date, la phrase ne dit que ce que `verdict` et
     `truncated` attestent.
  3. **Racheter** : **une rangée par boutique** (Beatport, Traxsource, Juno, Bandcamp, Amazon,
     Apple Music), grammaire des rangées de mesure en colonne — le libellé est la boutique, la
     valeur est le bouton « Ouvrir la recherche ». C'est là qu'on voit **dans quelles boutiques**
     aller, le point de l'écran (Antoine : « on veut pouvoir voir DANS QUELLES boutiques il est
     dispo, that's the point »). Puis « Copier le nom ».
     ⚠️ Ce sont des **recherches** (`search?q=`), pas une disponibilité. Les API boutiques ont été
     essayées et **écartées** (« une galère », Antoine, 2026-09-08) : ne pas re-proposer une
     vérification de disponibilité, ni une boutique par défaut dans Réglages (proposée le même
     soir, refusée pour la même raison — le point est de voir toutes les boutiques).
  4. **Actions** : À re-sourcer → « Remettre en file », « Envoyer à la corbeille » ; Corbeille →
     « Restaurer ». Boutons texte seul (`.sift-meta-ident-btn`). Pas de purge par piste.

### Clic droit

Le menu de Rangés adapté (`context-menu.ts`) : Ouvrir l'emplacement · Ouvrir / Masquer le
détail · **Copier le nom** · **Racheter…** (ouvre l'inspecteur sur la fiche Racheter — pas de
sous-menu, HIG Context menus : « aim for a small number of menu items ») · Remettre en file ·
Envoyer à la corbeille (À re-sourcer) / Restaurer (Corbeille). Les actions passent par le délégué
`[data-ec]` de `sift-live.ts` (un bouton fantôme cliqué), qui porte les IPC et leurs erreurs une
seule fois.

## États

| État | Rendu |
|---|---|
| Chargement | squelette statique dans la table (DESIGN § 6), jamais un spinner nu |
| Erreur de chargement | phrase en encre danger + « Réessayer » ; rien n'est dit du contenu |
| Vide | état vide formel (`emptyStateHtml`) : « Rien à re-sourcer » / « La corbeille est vide », retour Revue ; inspecteur fermé, barre sans action |
| Après une action | `sift-live.ts` rappelle `renderEcartes()` : la destination courante se repeint, la piste ouverte reste ouverte si elle est encore là, sinon l'inspecteur revient au repos |

## Vérifié le 2026-09-08 (vraie fenêtre, CDP)

À re-sourcer : titre et compte dans la barre (« 1 piste »), 0 carte, en-têtes Raison ·
Artiste ▴ · Titre · Fichier, ligne « — · Alexander East · I Gravitate 2 U (Alt Mix) · 02 - I
Gravitate 2 U (Alt Mix).flac · FLAC », inspecteur au repos puis, au clic, en-tête + trois fiches,
6 boutons de boutique, 2 actions, aucun débordement. Corbeille vide : « 0 piste », pas de « Vider
la corbeille », état vide, inspecteur fermé. Non exercé sur les vraies données : requeue, trash,
restore, purge (déjà câblés, inchangés dans `sift-live.ts`).

## Hors périmètre / questions ouvertes

- Disponibilité réelle par boutique : écartée (API essayées, galère). Pas de scraping.
- Recherche dans ces deux listes : à ajouter si elles s'allongent.
- `EcarteItem` sans date d'écart ni mesure : la phrase de Raison reste courte tant que le contrat
  ne les porte pas (chantier backend, pas de design).

## Décision — 2026-09-10 : la même rangée que Rangés, lue contre le Finder

Même geste que `bibliotheque.md` § Décision 2026-09-10 (zébrure par parité d'index, ni filet ni
arrondi ni survol, colonne Format en texte). Propre à cet écran : la colonne **Fichier** quitte le
monospace — `DESIGN.md` § 2 réserve `--font-mono` aux chiffres alignés en colonne, un chemin se
rend en `--font-ui`.
