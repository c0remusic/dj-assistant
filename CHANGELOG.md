# Journal des versions

Ce fichier est la **source** des notes de version : `release.yml` extrait la section du tag
publié et la passe à `releaseBody`. Ce texte part à deux endroits — la page GitHub de la
release, et le champ `notes` de `latest.json`, que chaque installation existante télécharge en
vérifiant les mises à jour. Il s'écrit donc pour un utilisateur, pas pour un développeur : les
détails techniques vivent dans les messages de commit.

Une section manquante fait **échouer** le build de release plutôt que publier des notes vides.
Le titre de section doit être exactement `## vX.Y.Z` pour que l'extraction le trouve.

## v0.1.0

### Une interface refaite, écran par écran

Sift a été relu contre les applications système de macOS — Finder, Musique, Mail, Photos,
Utilitaire de disque, Réglages Système — et redessiné sur leur grammaire, puis adapté aux
conventions Windows (contrôles de fenêtre à droite, `Ctrl`, clic droit partout).

- **Une barre unique** porte le titre de l'écran, son compte et ses actions. Le rail de
  navigation est groupé en sections, repliable, et accueille désormais les **dossiers
  surveillés** (l'écran Accueil a disparu : ses sources vivent dans le rail, avec leur
  couleur, leur compte et leur état de surveillance).
- **Revue**, l'écran de décision : lecteur façon Apple (temps cliquable, survol de l'onde,
  volume en capsule), verdict dit en un mot — **VRAI**, **FAUX** ou **À VÉRIFIER** —, fiche
  Métadonnées éditable en place et gravée au blur, identification Discogs en liste ouverte,
  Diagnostic audio dans l'inspecteur, filtre par facettes et recherche en tête de la file,
  menu contextuel sur chaque piste, raccourcis clavier en infobulle.
- **Mode Lot** : on coche dans la file, la zone centrale résume la sélection (verdicts,
  formats, durée), une alerte récapitule avant un rangement de masse, une feuille de
  progression non modale montre l'avancement puis le rapport. Menu **Sélection** : tout,
  aucune, seulement une catégorie (Lossless, MP3, Faux, Doublons), sans les faux ;
  `Ctrl+A` / `Ctrl+Maj+A`.
- **Rangés** (ex-Bibliothèque) : table à fonds alternés comme le Finder, colonnes
  redimensionnables et réordonnables (mémorisées), tri sur chaque colonne, sélection
  multiple avec inspecteur agrégé, menu contextuel (ouvrir l'emplacement, réanalyser,
  écarter, corbeille), pochette-bouton de lecture, inspecteur qui parle Revue.
- **À re-sourcer** et **Corbeille** deviennent deux destinations du rail, dans la même
  table.
- **Journal** : une vraie table, groupée par session et par jour ; une action annulée reste
  marquée annulée après un redémarrage.
- **Rekordbox** : une seule synchronisation — « Synchroniser la sélection » ou « Tout
  synchroniser » —, les candidats groupés par type et nommés « Artiste — Titre », les
  faits du fichier lié en tête.
- **Clé USB** : colonne des disques, puis pour le disque choisi la barre d'occupation par
  format, une grille de faits (point de montage, format, capacité, libre, fichiers,
  modèle, périphérique, santé) et les actions Formater, Éjecter, Relire.
- **Réglages** : catégories à gauche, panneau à droite, application immédiate — plus
  aucun bouton Enregistrer, le modèle de nommage se grave à la frappe.
- Police **Inter**, gris système d'Apple, accent bleu système, coins arrondis de fenêtre
  sur Windows 11, une seule hauteur de rangée, deux familles de boutons, et plus aucun
  survol sur un bouton plein.

### Une détection des faux lossless plus fiable

- Deux signaux s'ajoutent à la coupure spectrale : la **platitude de l'aigu** et la
  **vraisemblance de quantification** (la trace que laisse un codec dans les échantillons).
  Mesuré sur la référence du dépôt : les faux négatifs passent de 68 % à 31 %.
- La durée décodée est mesurée, plus lue dans l'en-tête ; l'absence de mesure ne produit
  plus de verdict ; les seuils ont été re-dérivés sur une référence assainie.
- Un changement de moteur ne perd plus la bibliothèque rangée : le verdict est rejoué au
  démarrage sur les mesures stockées, et les pistes rangées qui ne peuvent pas l'être sont
  reprises par l'analyse de fond, la file d'abord. Un compteur le dit dans le journal.
- Le badge « Prêt CDJ » juge le porteur du tag (un WAV en RIFF n'est pas lisible par la
  platine), plus sa seule présence.

### Rangement

- La racine de bibliothèque n'est plus exigée pour convertir : elle ne conditionne que
  l'arbre de destinations. Le rappel vit dans le rail, sous les sources.
- Le popover Destination est en sections (Bibliothèque / Autres), avec un pied natif.
- Un dossier surveillé sans fichier audio reconnu porte un badge « 0 audio ».

### Clé USB

- L'écriture du FAT32 au-delà de 32 Go part par blocs de 1 Mio au lieu de secteur par
  secteur, et l'étape affiche les Mo écrits : plus d'écran qui semble figé.
- Le type de partition FAT32 est posé correctement ; l'éjection d'un SSD USB vu comme
  disque fixe passe par son périphérique parent ; plus de demande d'élévation quand la
  partition existante convient.

### Vitesse

- Les rapports d'analyse en cache sont 21 fois plus petits (la base réelle est passée de
  4,1 Go à 119 Mo), la grille du spectrogramme se recalcule à l'ouverture.
- Le dédoublonnage ne recalcule plus tout quand une seule piste est rangée, et sa fenêtre
  de durée énumère au lieu de filtrer.
- Un rendu d'écran en retard ne repeint plus l'écran courant ; le rail des sources se met
  à jour en place.

### Sous le capot

- Build macOS Intel ajouté (en plus d'Apple Silicon) ; FFmpeg macOS compilé depuis les
  sources, sans composant GPL.
- Les chaînes françaises ont retrouvé leurs accents, avec des gardes qui empêchent le
  retour du problème.

## v0.0.3

### L'écran Clé USB fonctionne

Il n'avait jamais fonctionné depuis son introduction : aucune clé n'apparaissait, quelle
qu'elle soit. Six causes distinctes, toutes corrigées.

- Les disques sont désormais énumérés au niveau **physique**, donc une clé neuve ou sans
  système de fichiers apparaît elle aussi — c'est précisément celle qu'on veut formater.
- Le type de bus est lu là où il est exact. Les boîtiers USB modernes se déclarent en interne,
  et Sift les écartait donc à tort.
- Le formatage demande l'élévation quand il en a besoin, au lieu d'échouer en silence.
- Le nom du volume est demandé au système, plus seulement lu.

**Ce qui n'est pas encore prouvé** : le formatage FAT32 n'a jamais abouti sur un vrai disque,
et l'éjection n'a jamais été exécutée de bout en bout. Les corrections sont écrites, pas
confirmées par l'usage. À utiliser avec prudence, sur un disque dont vous avez une sauvegarde.

### Occupation du disque, par format

Un graphique montre la répartition de ce qui occupe un volume — lossless, MP3, le reste — sur
l'écran Clé USB comme sur la Bibliothèque, avec la place libre et l'état de santé du volume.

### Formater en FAT32 au-delà de 32 Go

Windows refuse de créer un FAT32 de plus de 32 Go. Sift écrit désormais le système de fichiers
lui-même, ce qui lève cette limite pour les clés et SSD de grande capacité. macOS n'était pas
concerné : il n'a jamais eu ce plafond.

### Vitesse

- L'ouverture de la Revue passe de ~1,8 s à ~15 ms.
- Trois causes de gel de l'interface supprimées, dont le rangement, qui ne bloque plus.
- Le formatage ne fige plus la fenêtre pendant son exécution.

### Sous le capot

- FFmpeg passe au build LGPL sur Windows. Sift n'utilise que l'encodeur MP3 et du PCM, donc les
  composants sous licence GPL n'apportaient rien et n'imposaient que leurs contraintes.
- Rescan manuel par source rebranché sur l'écran Accueil.

## v0.0.2

**Aucun changement fonctionnel.** Cette version ne contient qu'un commit de plus que la
v0.0.1 : la montée de numéro elle-même.

Elle a été publiée quinze minutes après la v0.0.1 pour vérifier la mise à jour automatique en
conditions réelles — l'app installée ne cherche une mise à jour que s'il en existe une plus
récente que la sienne, donc la seule façon d'éprouver ce chemin était de publier une version
qui ne change rien d'autre.

Si vous êtes en v0.0.1, cette mise à jour ne vous apporte rien. Passez directement à la
v0.0.3.

## v0.0.1

Première version publiée. Elle représente le projet complet de son premier commit
(2026-06-11) à sa publication : 896 commits, et les neuf jalons de la V1.

- **Analyse** — décodage natif, détection de faux lossless au spectrogramme, clipping,
  troncature, silence, phase. C'est la raison d'être de Sift : un MP3 ré-encodé en FLAC ne se
  voit pas dans un tag, seulement dans son spectre.
- **Écoute** — lecture des fichiers avec forme d'onde, verrouillage de tonalité, réglage de
  tempo.
- **Ranger, c'est encoder** — deux rails de sortie, refus de sur-encoder, tags et nommage
  automatiques, bacs de destination, corbeille et annulation.
- **Écartés** — re-sourcer une piste rejetée, liens d'achat, copie vers Soulseek.
- **Doublons** — par nom puis confirmation à l'oreille, par empreinte acoustique.
- **Identification** — Discogs : pochette, genres, métadonnées.
- **Bibliothèque** — parcourir, éditer, re-ranger, tableau de bord de statistiques.
- **Rekordbox** — export XML dont les playlists survivent à un renommage ou un déplacement,
  puis écriture directe dans `master.db` : réparation de chemins, dédoublonnage de playlists,
  synchronisation des métadonnées et des pochettes. Chaîne de sûreté complète — sauvegarde,
  vérification aller-retour, retour arrière — éprouvée sur une vraie bibliothèque de
  2828 pistes.
- **Clé USB** — formatage FAT32/exFAT sur Windows et macOS.
- **Mise à jour automatique**, sans certificat de signature.
