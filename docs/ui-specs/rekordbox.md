# Spec — Rekordbox

## Contexte dans le shell

**Profil Parcours** (`DESIGN.md` § 14). Patron macOS : **Utilitaire de disque** —
cible → action → progression → rapport.

Trois zones : rail · liste des candidats de la section choisie (flexe) · inspecteur du
candidat (`--pane-w`, repliable).

**Le changement structurel de cet écran.** Les quatre sections de synchronisation
(M8 Tier 1/2/3) sont aujourd'hui **quatre cartes empilées verticalement** dans une page
qui défile, chacune avec sa propre action. Quatre cibles et quatre actions dans un même
flux : rien ne dit laquelle on traite. Elles deviennent **quatre entrées de la zone
gauche**, plus une entrée « Tout ». La zone C montre les candidats de celle qui est
choisie. C'est le patron Utilitaire de disque appliqué tel quel : on choisit une cible
avant qu'une action soit disponible.

## Layout

### Zone B′ — sections

Cinq entrées, sélection exclusive, chacune avec son compte en attente :

| Entrée | Contenu |
|---|---|
| **Tout** | Les candidats des quatre sections, colonne Section en plus |
| **Fichiers** | Tier 1 — corrections de chemin détectées au rangement |
| **Playlists** | Tier 2 — doublons `djmdContent` |
| **Métadonnées** | Tier 3 — écarts de tags |
| **Pochettes** | Tier 3 — pochettes manquantes ou divergentes |

Une section dont l'appel IPC a échoué garde son entrée, avec un indicateur d'erreur.
Elle ne disparaît pas — une section absente se lit comme « rien à faire », ce qui est
un mensonge.

### Zone A — barre unifiée

Titre de la section active + compte en attente · **Synchroniser** (action principale,
dominante, portée sur la sélection ou sur toute la section) · recherche à droite.

Le bandeau explicatif du flux (« Sift convertit → l'export fusionne dans le XML lié →
réimporte-le ») descend en tête de zone C, et **seulement quand un XML est lié** : c'est
un rappel de procédure, pas un titre.

### Zone C — table des candidats

| Colonne | Largeur | Rendu |
|---|---|---|
| Case | fixe | Sélection pour la synchronisation |
| **Section** | fixe | Uniquement en mode « Tout » |
| **Piste** | flex 2 | Artiste — titre |
| **Écart** | flex 2 | Ce qui diffère : chemin actuel → chemin corrigé, ou tag avant → après |
| **État** | fixe | En attente · Synchronisé · Échec |

Hauteur `--row-h`. Les lignes remplacent les sept `.sift-ui-card-outline` de
`rekordbox-view.ts`, chacune rembourrée à la main en `padding:10px 12px` — un `10px`
qui n'appartient à aucune échelle (`DESIGN.md` § 5).

### Zone D — inspecteur

Détail du candidat : valeur actuelle dans Rekordbox, valeur proposée par Sift, source de
la proposition, et l'action unitaire. En sélection multiple : compte par section et
l'action de masse.

## États

| État | Rendu |
|---|---|
| **Aucun XML lié** | `emptyStateHtml` en zone C — « Aucun XML Rekordbox lié », action « Lier un fichier XML ». Le rail et la barre restent |
| **Statut indisponible** | Carte d'erreur, bouton Réessayer. Aucune section n'est affichée comme vide |
| **Section en erreur** | L'entrée du rail porte l'indicateur, la zone C porte la carte d'erreur et son motif. Le compte global **compte les sections tombées** — quatre cartes en erreur ne doivent jamais produire un en-tête « à jour » |
| **Rien en attente** | Section affichée, atténuée, libellée « à jour ». Elle ne disparaît pas |
| **Dérive détectée** | Bandeau `warning` persistant en tête de zone C : « Ferme Rekordbox, vérifie la piste, puis relie à nouveau le fichier XML. » Phrase entière, **jamais tronquée** |
| **Synchronisation en cours** | Sheet attachée à la fenêtre, barre déterminée, étape en texte, Annuler présent |
| **Rapport** | Résumé chiffré — synchronisés / échecs — avec accès au détail des échecs et une sortie claire |

## Interactions

### Souris

- **Clic** ligne : sélectionne, remplit l'inspecteur · **⇧+clic** plage ·
  **⌘/Ctrl+clic** ajout · case à cocher pour la sélection de synchronisation.
- **Clic droit** : Synchroniser cette entrée · Ignorer · Voir la piste dans
  Bibliothèque · Ouvrir l'emplacement.
- **Clic** en-tête de colonne : tri.

### Clavier

Couches 1 et 2 de `DESIGN.md` § 9. `Entrée` synchronise la sélection, après
confirmation. **Aucun raccourci à une lettre sur cet écran** : il écrit dans une base
tierce, un accélérateur à une touche y est un piège.

### Retour

La sheet de progression glisse depuis le haut de la fenêtre en `--duration-slow`.
Aucune animation sur l'arrivée ou la disparition d'un candidat.

## Sécurité — non négociable

Cet écran écrit dans un système **live** — la base d'un autre logiciel.

- Le backend refuse d'agir quand Rekordbox tourne
  (`MasterDbError::RekordboxRunning`) ; l'interface le dit avant l'action, pas après
  l'échec.
- Toute écriture en mode `master.db` est précédée d'un backup, et le backup est vérifié
  contre une référence propre **juste avant** l'écriture.
- Aucune écriture ne se déclenche sur la seule foi d'un rapport : l'état est relu
  indépendamment.
- La confirmation est in-app, armée et horodatée. Jamais `window.confirm()`.

## Décision — 2026-09-08 : lu contre Utilitaire de disque (déclinaison #24, cinquième écran)

Référence nommée par cette spec et `DESIGN.md` § 15 : **Utilitaire de disque**. Aucune capture
dans `docs/design-refs/` — deux captures officielles prises sur le guide Apple (macOS Tahoe,
support.apple.com) : la zone centrale d'un volume, et la sheet de confirmation. Elles ne montrent
ni la sidebar ni la toolbar, donc rien n'est affirmé sur celles-ci au-delà de ce qui précède.
Maquette faite dans la vraie fenêtre avec les composants de l'app (v1 réfutée par la référence,
v2 validée par Antoine — artefact « Rekordbox — contre Utilitaire de disque »).

**Ce que la référence dit.** La *cible* (icône, nom en grand, deux sous-lignes) est **en tête de
la zone centrale**, avec à droite un **badge chiffré et sa légende en capitales** (« 2 TB ·
SHARED BY 8 VOLUMES ») ; dessous, la grille d'informations. La sidebar liste les cibles ; la zone
centrale porte l'identité de celle qui est choisie. Ici la seule cible est le XML lié, et ses
sections sont ses « volumes ».

Cinq écarts mesurés dans la vraie fenêtre (CDP 9333), corrigés le jour même :

- **La cible en tête de zone C** (`.rkb-target`) : nom du fichier à `--text-xl`/600, sous-ligne
  « XML Rekordbox lié · N playlists · N pistes » (ou « illisible » en danger), chemin en mono ;
  à droite le badge (`.rkb-badge-n` bordé, `--text-xl`/500) et sa légende en capitales
  (`--text-xs`, `--tracking-wider`). C'était une carte bordée au-dessus de tout, avec ses deux
  boutons dedans.
- **Le badge porte l'état de synchronisation**, trois valeurs et non deux (2026-08-17, impasses
  A13/A14) : `N` + « en attente de synchronisation » · `0` + « à jour » · `?` + « N sections sans
  réponse » (warning) · `—` + « synchronisation indisponible » (warning, la cause nommée sous la
  cible en `.rkb-target-note`). **Le compte quitte la barre** : la référence le met sur la cible.
  La sur-ligne « Synchroniser avec Rekordbox » part avec la carte.
- **Barre = la toolbar d'Utilitaire de disque** : « Réexporter maintenant » (primaire,
  absent tant que le XML est illisible) · « Changer de XML lié », texte seul, géométrie
  `.sift-bar-btn` (`--h-control`, 6/8 — partagée avec « Vider la corbeille »). Le « Synchroniser »
  de la § Zone A ci-dessus appartient à la table des candidats, chantier à part.
- **Colonne B′ au plan de la file** de Revue (`--color-background-queue`, bord à bord, pleine
  hauteur, `.sift-rkb-side`), plus une carte `.sift-ui-card-soft` ; compte par entrée en
  `--text-sm` tertiaire, jamais en opacité. La zone principale défile chez elle (§ 14).
- **Les sections en fiches** (`.rb-fiche`, en-tête `.sift-meta-header` / `.sift-meta-title`
  15/600 comme les fiches de Revue), **sans cadre** : la référence pose une grille d'informations
  sous la cible, pas des boîtes. À droite, le compte ou l'état (« à jour » / « indisponible »,
  atténué par l'encre du titre). **Sessions nommées** comme au Journal — « Session du 29/08/2026
  17h16 » via `session-label.ts`, module pur partagé — à la place de l'identifiant brut.

Tient : le rappel de procédure (sous la cible, seulement lié), le bandeau de dérive en tête,
l'état vide « Aucun XML Rekordbox lié » (sans rappel de procédure), les cinq entrées de B′ et leur
compte, les quatre `data-sift` de dispatch, les lignes `.rb-row` des candidats ambigus et des
doublons de playlist, les confirmations in-app.

**Vérifié dans la vraie fenêtre le 2026-09-08 (CDP 9333)** : plan de la colonne 0.2939 contre
0.2273 du sol ; layout 893 px = `#content` 893 px, aucun défilement de page ; boutons de barre
28 px ; badge « 2 » / « en attente de synchronisation » 10 px capitales ; fiches 15 px/600 sans
bordure ; sessions « Session du 29/08/2026 17h16 » ; plancher typographique 10 px tenu. **Non
exercé** : état non lié, XML illisible, sections en erreur, `masterdb_error` (aucun de ces états
sur la machine de vérification).

**Reste, chantier nommé — pas cette passe** : la § Zone C ci-dessus (table des candidats Case ·
Section · Piste · Écart · État à la place des quatre rendus de section), la § Zone D (inspecteur
du candidat, action de masse), le clic droit. Quatre DTO à ramener à une ligne commune, sur un
écran qui écrit dans une base tierce (§ Sécurité). Ticket à ouvrir.

## Hors périmètre / questions ouvertes

- **Vérification dans le vrai Rekordbox** — manuelle, hors de cette spec.
- **Mode XML contre `master.db`** — le choix vit aujourd'hui dans cet écran. Doit-il
  remonter dans Réglages, où vivent les autres décisions persistantes ? Non tranché.
- **Ignorer un candidat** — l'action est spécifiée dans le menu contextuel ; sa
  persistance (session, ou définitive) ne l'est pas.
