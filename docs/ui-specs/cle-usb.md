# Spec — Clé USB

## Contexte dans le shell

**Profil Parcours** (`DESIGN.md` § 14). Patron macOS : **Utilitaire de disque**, et ici
littéralement — c'est la même tâche, sur le même objet.

Trois zones : rail · liste des disques amovibles (flexe) · détail du disque choisi
(`--pane-w`, repliable).

L'écran quitte le plafond `.sift-settings-stack{max-width:560px}` qu'il partage
aujourd'hui avec Réglages : une **liste de disques** n'est pas un formulaire. Sur une
fenêtre de 1200 px ce plafond laissait 44 % de la zone vide.

## Layout

### Zone A — barre unifiée

Titre « Clé USB » · compte de disques détectés · **Actualiser** · pas de recherche
(la liste tient à l'écran par construction).

### Zone C — liste des disques

Une ligne par volume amovible. Hauteur supérieure à `--row-h` assumée et **nommée** :
la ligne porte un graphique d'occupation, pas seulement du texte. C'est la seule
dérogation à la hauteur unique de `DESIGN.md` § 16, et elle vaut pour cet écran seul.

Contenu d'une ligne : icône de bus · nom du volume · capacité · système de fichiers ·
barre d'occupation par format · espace libre en `--font-mono` avec `tabular-nums`.

**Un disque non amovible n'apparaît jamais.** Le bus se lit sur `MSFT_Disk.BusType`,
jamais sur `InterfaceType`, qui ment sur les boîtiers UASP.

### Zone D — détail du disque

Graphique d'occupation détaillé par format · liste des dossiers de premier niveau avec
leur poids · chemin de montage · et les deux actions :

- **Formater** — ouvre la sheet de formatage.
- **Éjecter** — action directe, avec son état d'échec explicite.

## Décision — 2026-09-09 : lu contre Utilitaire de disque (déclinaison #24, sixième écran)

Trois maquettes dans la vraie fenêtre, avec les composants de l'app et un disque fictif (l'écran
n'avait aucun disque branché) : v1, actions Éjecter · Formater… dans la barre, comme la toolbar de
la référence ; v2, actions dans la section, faits en libellés alignés à droite (grammaire Finder ›
appareil, celle de Rekordbox) ; v3, v2 corrigée. Antoine : « plutôt v2 mais le layout est bizarre
avec le texte et la barre d'affichage de l'espace n'est arrondie que d'un côté » — mesuré : deux
grammaires se mélangeaient (la barre au bord gauche, le texte à 150 px), et le segment Libre
peignait `--color-background-primary`, invisible au sol. **v3 retenue (« go v3 »).**

Ce que la référence donne (guide Utilitaire de disque, « Intro », « Obtenir des informations
détaillées » : « la barre latérale affiche chaque périphérique de stockage… », « des informations
sur celui-ci s'affichent à droite », le bouton Info pour « emplacement, format, capacité, espace
disponible, point de montage, état S.M.A.R.T. ») et ce que l'écran en fait :

- **Colonne B′** (`.sift-usb-side`, `--pane-w`, plan de la file, bord à bord — la même règle CSS
  que la colonne des sections de Rekordbox, par co-sélecteur) : « Disques amovibles », une entrée
  `.fld` par disque (`usbEntryHtml`) — glyphe, nom (`driveDisplayName`), capacité à droite, « vide »
  pour un lecteur sans média. Sélection exclusive, `Entrée`/`Espace` au clavier, clic droit.
- **Zone C** (`.sift-usb-main-inner`, bornée à `--measure-data`), le disque choisi :
  1. **Tête** `.sift-usage-head` (la même que Rekordbox) : glyphe, nom du VOLUME (sinon lettre ou
     « Disque N »), ligne de faits « Disque USB externe · FAT32 · E: · modèle », capacité encadrée.
  2. **Occupation** : `renderUsageChart({ plain: true })` — barre + légende + « Voir le détail
     complet », sans carte (la zone C ne peint rien). Segment Libre en `--color-border-secondary`.
  3. **Faits** (`.sift-usb-facts`, grille 4 colonnes, libellé au-dessus) : Point de montage · Format
     · Capacité · Libre · Fichiers · Modèle · Périphérique · Santé (en `warning` si ≠ OK). Pas de
     « Connexion » : le bus n'est pas remonté par `RemovableDrive`, on n'invente pas.
  4. **Actions** (`.sift-usb-actions`) : Formater… (sheet) · Éjecter · Relire le disque. Un refus
     d'éjection s'écrit sur place (`humanizeEject`, `role="status"`).
- **Barre** : Actualiser seul (l'énumération WMI est à la demande) ; compte « N disques ».
- **Pas de zone D** : le disque est la zone C. La § Layout ci-dessus (liste en C, détail en D)
  est **remplacée** par ceci ; `DESIGN.md` § 15 recalé le même jour.
- Partis : la carte `.sift-settings-stack` de 560 px, le paragraphe d'explication, la ligne
  « lettre · modèle · taille · Formater… » et la carte d'occupation empilée sous chaque ligne
  (`.sift-usb-list`, `.sift-usb-row*` retirés de `styles.css`).

### Reste

- **Ouvrir dans l'explorateur** (clic droit, § Interactions) : aucune commande IPC ne l'expose —
  omis, pas simulé.
- **Vérification avec un disque branché** : l'état vide et le squelette se vérifient dans la vraie
  fenêtre ; les états avec disque (occupation, faits, Éjecter, sheet) demandent une clé — manuelle,
  par Antoine.

## États

| État | Rendu |
|---|---|
| **Aucun disque** | `emptyStateHtml` — « Aucun disque amovible détecté », note sur le lecteur de cartes vide et les disques internes, bouton Actualiser |
| **Détection en cours** | Squelette dans la structure finale : colonne + zone C |
| **Lecture d'occupation en cours** | La tête est déjà là (nom, capacité) ; le corps dit « Analyse de l'occupation… » |
| **Disque inaccessible** | Entrée présente ; en zone C, « Occupation indisponible » et la cause en encre `danger`. Jamais masquée |
| **Disque non formaté** | Entrée « Disque N » ; zone C : tête, « Aucun volume monté — rien à parcourir », faits avec « — », Formater… seul |
| **Lecteur sans média** | Entrée « vide » ; zone C : tête seule et la phrase qui explique la lettre fantôme de l'explorateur |
| **Formatage — confirmation** | Sheet attachée. Nom du volume à retaper, confirmation **armée et horodatée**. Jamais `window.confirm()` |
| **Formatage en cours** | Sheet bloquante mais **annulable**, barre déterminée, étape en texte. Depuis le 2026-09-09 l'étape d'écriture du FAT32 dit les Mo écrits (tous les 16 Mio) — l'écriture elle-même passe de plusieurs minutes à quelques secondes sur 500 Go (`sector_io.rs`, fenêtre de 1 Mio au lieu d'un secteur). **Non tenu encore** : ni barre déterminée, ni annulation — `formatDrive` n'a pas de chemin d'annulation |
| **Formatage — rapport** | Résumé : système de fichiers produit, capacité utile, sortie claire |
| **Élévation refusée** | Message explicite : ce qui a été refusé et ce que l'utilisateur peut faire. Sentinelle `ELEVATION_DECLINED` |
| **Disque disparu en cours d'opération** | Sentinelle `DRIVE_VANISHED` — l'opération s'arrête, l'état est dit, rien n'est supposé |
| **Identité incohérente** | Sentinelle `IDENTITY_MISMATCH` — le volume n'est plus celui qui a été ciblé. L'opération est refusée, pas retentée |
| **Éjection refusée** | Sentinelle `EJECT_BUSY` — nommer ce qui tient le volume si l'information est disponible |

Les cinq sentinelles (`DRIVE_VANISHED`, `IDENTITY_MISMATCH`, `ELEVATION_DECLINED`,
`EJECT_BUSY`, et le préfixe de destination externe) sont des **littéraux partagés** avec
le Rust et épinglés par des tests. Leur rupture est silencieuse côté interface : ne
jamais les réécrire côté TS.

## Interactions

### Souris

- **Clic** ligne : sélectionne, remplit le détail.
- **Clic droit** : Formater · Éjecter · Ouvrir dans l'explorateur · Actualiser.
- **Clic** sur un segment du graphique d'occupation : filtre le détail sur ce format.

### Clavier

Couches 1 et 2 de `DESIGN.md` § 9. `Entrée` ouvre le détail.

**Ni `⌫` ni raccourci à une lettre sur cet écran.** Le formatage efface un disque : il
n'a pas d'accélérateur clavier, il passe par la sheet et sa confirmation armée.

### Retour

La sheet glisse depuis le haut en `--duration-slow`. La barre d'occupation ne s'anime
pas à la lecture — c'est une donnée mesurée, elle s'affiche.

## Sécurité — non négociable

- **Volumes amovibles uniquement.** Un disque fixe n'entre jamais dans la liste.
- Le nom du volume doit être **retapé** pour armer le formatage. La confirmation est
  in-app, armée et horodatée.
- L'identité du volume est revérifiée **juste avant** l'écriture, pas seulement à la
  sélection.
- FAT32 au-delà du plafond de 32 Go de Windows passe par `fatfs` (MIT). Sur macOS,
  `diskutil eraseDisk` ignore ce plafond et le binaire ne lie pas `fatfs`.

## Hors périmètre / questions ouvertes

- **Message de refus de formatage** — reste à finir : il doit nommer *ce qui* a été
  refusé et *quoi faire*, pas seulement échouer.
- **Éjection d'un SSD USB vu comme fixe** — le verbe shell est absent dans ce cas ; il
  faut viser le devnode parent. Comportement à re-vérifier avant de le spécifier comme
  acquis.
- **Copie vers la clé** — hors périmètre de cet écran aujourd'hui. Si elle y entre un
  jour, c'est une opération longue de plus, même patron.
