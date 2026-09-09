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

## Décision — 2026-09-08 : lu contre Finder › appareil, Photos › Importer et Utilitaire de disque (déclinaison #24, cinquième écran)

Une première passe le même soir (`27a3ff6`) avait mis la cible en tête de zone C, la colonne B′ au
plan de la file et les sections en fiches sans cadre. Retour d'Antoine : « le texte semble posé au
hasard » — mesuré, la zone principale faisait 2332 px et les compléments (« à jour », « 1 »,
« Tout sélectionner ») étaient poussés à son bord droit, à 1900 px de leur titre — « et pourquoi on
ne peut pas tout synchroniser d'un coup ? ». Puis : « regarde la ref et comment Apple ferait »,
« d'autres pages peut-être ? ». Trois apps Apple font le geste de cet écran — **des changements en
attente, à appliquer sur un autre système** — et leurs figures officielles disent la même chose :

- **Finder › appareil** (guide macOS Big Sur, « Overview of syncing ») : l'appareil dans la
  sidebar ; en tête l'icône, le nom, une ligne de faits ; une barre de boutons par type de contenu
  (General · Music · Movies…) ; des sections à **libellé aligné à droite** (Software: / Backups: /
  Options:), la valeur en face, les **actions secondaires dans la section** (Check for Update,
  Restore iPhone… sous Software:) ; en bas, **un seul Apply**.
- **Photos › Importer** (guide macOS Tahoe) : l'appareil dans la sidebar ; barre **« Import
  Selected »** (inactif sans sélection) · **« Import All New Photos »** (primaire) ; les éléments
  en **groupe nommé avec son compte** (« New Photos (15 photos) »), sans boîte.
- **Utilitaire de disque** (guide macOS Tahoe) : la cible en tête, une action de toolbar = une
  sheet. Pas de liste d'attente.

Quatre maquettes dans la vraie fenêtre avec les composants de l'app (v1 réfutée par la référence,
v2 table de la § Zone C, v3 carte de Clé USB, v4 Finder + Photos — artefact « Rekordbox — comment
Apple le ferait ») ; **v4 retenue, « moins de prose »** (Antoine, « go v4 »).

### Ce qui est livré

- **Barre unifiée** = Photos › Importer : **« Synchroniser la sélection »** (`data-sift="rkbsyncsel"`,
  inactif sans sélection, compte entre parenthèses sinon) · **« Tout synchroniser (N) »**
  (`rkbsyncall`, primaire). N est borné à la section active de B′ (« Tout » = les quatre) — choisir
  une section, c'est le bouton de contenu du Finder. Les deux sont inactifs si la synchronisation
  est indisponible ou en cours. Plus de compte dans la barre ni de badge.
- **Zone C bornée à `--measure-data`** (`.rkb-main`), grammaire du Finder › appareil :
  1. **Tête** (`.sift-usage-head`, la même que Clé USB) : icône, nom du fichier, une ligne de
     faits « XML Rekordbox lié · N playlists · N pistes · N en attente de synchronisation » (ou
     « à jour », ou warning : « synchronisation indisponible », « N sections sans réponse »).
  2. **Fichier :** chemin en mono, puis **Réexporter maintenant** (absent tant que le XML est
     illisible) · **Changer de XML lié…** — les actions du XML près du XML, comme Check for Update
     sous Software:.
  3. **master.db :** « Lisible » ou la cause (`masterdb_error`, warning) ; « Dérive : aucune » ou
     la phrase entière en warning (elle était un bandeau).
  4. **En attente :** un groupe par section (`.rkb-group-hd` « Métadonnées (1) », « · N à
     choisir » si des ambigus attendent), **une rangée par candidat** (`.rkb-cand`, `--row-h`) :
     case, piste, écart à droite en encre secondaire ; cochée = `--overlay-hover`. Rangée ambiguë
     sans case, écart « à choisir », une piste candidate par bouton. Erreur d'application sous la
     rangée. Rien en attente : « Rien — le XML lié est à jour. »
  Libellés à droite (150 px, 600), filet en retrait entre les sections. Pas de rappel de procédure,
  pas de règle master.db en prose — les faits seuls.
- **Colonne B′** inchangée (plan de la file, cinq entrées, compte ou « — » si la section n'a pas
  répondu).
- **Synchroniser** (`rekordbox-plan.ts`, pur, gelé par `test/rekordbox-plan.test.ts`) : le plan
  = tiers dans l'ordre 1 → 3 (métadonnées) → 3 (pochettes) → 2 (playlists), sélection ∩ en attente,
  borné à la section active ; une confirmation in-app (« Synchroniser N entrées avec Rekordbox ?
  Ferme Rekordbox avant de continuer. »), les quatre IPC enchaînés — chacun garde son backup et son
  refus si Rekordbox tourne —, un rapport par toast, l'échec inscrit sur sa rangée.
- **Ignorer** : clic droit sur une rangée (pas sur un doublon de playlist, rien n'y est persisté).
- Partis : la carte du XML, la sur-ligne, les quatre fiches, les groupes de session repliés et
  leur « Tout sélectionner », les quatre boutons « Appliquer la sélection », « Dédupliquer » par
  groupe, le bandeau de dérive, le rappel de procédure. `.rb-row`, `.rb-session-*`, `.bx-row`
  retirés de `styles.css`.

### Vérifié dans la vraie fenêtre le 2026-09-08 (CDP 9333)

Zone C 1200 px de large ; barre « Synchroniser la sélection » inactif · « Tout synchroniser (2) »
28 px ; libellés « Fichier : / master.db : / En attente : » alignés à droite, 150 px ; groupes
« Métadonnées (1) », « Pochettes (1) » ; rangées 32 px. Clic sur une rangée : `aria-checked`,
fond `--overlay-hover`, barre « Synchroniser la sélection (1) » actif ; second clic : retour.
« Tout synchroniser » : confirmation « Synchroniser 2 entrées avec Rekordbox ? Ferme Rekordbox
avant de continuer. » puis **Annuler** — aucune écriture, les deux candidats toujours en attente.
Plancher typographique 10 px tenu, aucun défilement de page. **Non exercés** : la synchronisation
effective (écriture dans `master.db`, manuelle par Antoine), état non lié, XML illisible, sections
en erreur, ambigus, doublons de playlist, `masterdb_error`, dérive.

### Reste

- **Rangée Pochettes / Fichiers** : dit le fichier, pas « Artiste — Titre » — `PendingArtworkSync`
  et `PendingMasterdbRepair` ne portent que des chemins ; à enrichir côté Rust (miroir
  `contracts.ts`, test de contrat).
- **Inspecteur** (§ Zone D) : non fait — un candidat se lit sur sa rangée.
- La § Zone C ci-dessus (table Case · Section · Piste · Écart · État) est **remplacée** par les
  groupes de rangées de Photos : à 200 candidats, la table de Rangés redeviendrait la bonne forme.

## Hors périmètre / questions ouvertes

- **Vérification dans le vrai Rekordbox** — manuelle, hors de cette spec.
- **Mode XML contre `master.db`** — le choix vit aujourd'hui dans cet écran. Doit-il
  remonter dans Réglages, où vivent les autres décisions persistantes ? Non tranché.
- **Ignorer un candidat** — l'action est spécifiée dans le menu contextuel ; sa
  persistance (session, ou définitive) ne l'est pas.
