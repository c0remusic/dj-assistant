# Sift — manuel

Pour quelqu'un qui vient de recevoir le lien et veut s'en servir. Pas pour un développeur : le
vocabulaire technique du dépôt vit dans `CONTEXT.md`, et l'installation dans
[`install-non-signe.md`](install-non-signe.md).

La même chose, dans le design de l'app : [`manuel.html`](manuel.html) (à ouvrir dans un
navigateur — GitHub ne le rend pas).

Sift prépare une bibliothèque de DJ : il détecte les faux fichiers lossless, trouve les doublons,
range, exporte vers Rekordbox et prépare une clé pour les platines. Son principe tient en une
phrase — **déplacer, c'est encoder et ranger** : un fichier ne bouge jamais sans être mis au bon
format et au bon endroit en même temps.

---

## Trois mots

**Faux lossless** — un fichier qui se présente en FLAC, WAV ou AIFF alors que son contenu est
passé par du MP3, de l'AAC ou de l'Opus. Le contenant est sans perte, le contenu ne l'est plus, et
la qualité perdue ne revient pas. C'est ce que Sift traque.

**Verdict** — ce que Sift dit d'un fichier, en un mot et pas plus :

| verdict | ce que ça veut dire |
|---|---|
| **VRAI** | rien de mesuré ne contredit un master |
| **FAUX** | preuve mesurée : le spectre s'arrête là où un encodeur lossy coupe, l'aigu est plat, ou les échantillons portent la trace d'un codec |
| **À VÉRIFIER** | quelque chose sort de l'ordinaire sans être une preuve — Sift le signale, il n'accuse pas |

**Écarter** — sortir un fichier du flux sans le supprimer. Deux destinations : **À re-sourcer**
pour un faux ou un fichier tronqué à racheter ailleurs, **Corbeille** pour un doublon ou une
erreur. Rien ne s'efface tant que la corbeille n'est pas vidée, à la main.

### L'anglais est gardé exprès

`LOSSLESS`, `DUPLICATE`, `MATCH`, `CHECK MATCH`, `kbps`, `kHz`, `MP3`, `AIFF`, `WAV` restent en
anglais dans les filtres, les chips et les badges, parce que c'est sous cette forme qu'ils
apparaissent partout ailleurs : Rekordbox, boutiques, forums. Le verdict, lui, est en français.

---

## Le rail, de haut en bas

Le rail de gauche a deux parties. En haut, les **sources** : les dossiers que Sift surveille,
chacun avec sa couleur, son compte de fichiers en attente et son état. Cliquer une source filtre
la file de Revue sur ses fichiers. Un dossier sans audio reconnu porte le badge « 0 audio ».

Dessous, huit **écrans**, accessibles par `Ctrl+1`…`8` dans l'ordre affiché (`⌘` sur Mac).
`Ctrl+B` replie le rail. Ils suivent le trajet d'un fichier.

### Revue

Une file à gauche, un fichier au centre, l'inspecteur à droite. La file se filtre par facettes
(Lossless, MP3, Faux, Doublons…) et par recherche ; `Ctrl+F` y va directement.

Pour chaque morceau : le lecteur, le verdict en un mot, la fiche Métadonnées éditable en place,
l'identification Discogs, et le Diagnostic audio dans l'inspecteur. Le son passe **avant** le
verdict — on écoute, puis on tranche.

- **Convertir** (`Entrée`) — le fichier part vers sa destination, encodé au format choisi (MP3,
  WAV ou AIFF). Le popover Destination propose l'arbre de la bibliothèque et les autres dossiers.
- **Écarter** (`Retour` ou `X`) — vers À re-sourcer si le verdict est FAUX ou le fichier tronqué,
  vers la Corbeille sinon. Un doublon détecté est nommé comme tel, avec l'original.
- **Identifier** (`I`) — cherche la piste sur Discogs, propose les candidats, applique les tags.

**Mode Lot** : cocher des pistes dans la file passe en mode Lot — la zone centrale résume la
sélection (verdicts, formats, durée) et un seul bouton range tout. Le menu **Sélection** propose
tout, aucune, seulement une catégorie, ou tout sauf les faux ; `Ctrl+A` / `Ctrl+Maj+A`. Sur un
lot important, une alerte récapitule avant de partir et une feuille de progression montre
l'avancement puis le rapport.

### Journal

Tout ce que Sift a fait, en table groupée par session et par jour. Chaque action se défait depuis
sa ligne — un rangement de masse parti au mauvais endroit revient d'ici. Une action annulée reste
marquée annulée après un redémarrage.

### Rangés

La bibliothèque, en table à fonds alternés comme le Finder : colonnes redimensionnables,
réordonnables et mémorisées, tri sur chaque colonne, sélection multiple avec inspecteur agrégé,
pochette-bouton pour écouter, clic droit (ouvrir l'emplacement, réanalyser, écarter, corbeille).
La barre d'occupation par format dit combien pèse chaque famille.

### À re-sourcer

Les fichiers faux ou tronqués, avec la raison. Une liste de courses : les morceaux y attendent
d'être rachetés ou retrouvés ailleurs. **Restaurer** remet une piste dans la file.

### Corbeille

Doublons et erreurs. Rien n'y est supprimé du disque tant que la corbeille n'est pas vidée, et le
vidage se confirme dans l'app.

### Rekordbox

Une seule synchronisation : **Synchroniser la sélection** ou **Tout synchroniser**. Sift compare
la bibliothèque rangée avec celle de Rekordbox, groupe les candidats par type et les nomme
« Artiste — Titre ». Deux chemins d'écriture : un XML à importer, ou l'écriture directe dans la
base de Rekordbox.

⚠️ **Rekordbox doit être fermé** pendant l'écriture directe. Sift sauvegarde la base avant de la
toucher et refuse d'agir si le logiciel tourne.

### Clé USB

La colonne des disques amovibles, puis pour le disque choisi : la barre d'occupation par format,
une grille de faits (point de montage, format, capacité, libre, fichiers, modèle, périphérique,
santé) et trois actions — **Formater…**, **Éjecter**, **Relire le disque**. Le formatage écrit du
FAT32 même au-delà de 32 Go, par blocs d'un mégaoctet, et l'étape affiche les Mo écrits.

⚠️ **Formater efface la clé.** Sift le demande deux fois, en nommant le volume, et refuse si le
disque a changé entre-temps.

### Réglages

Catégories à gauche, panneau à droite, application immédiate — pas de bouton Enregistrer.
**Général** (racine de la bibliothèque, qui ne conditionne que l'arbre de destinations),
**Nommage** (modèle de nom de fichier, aperçu à la frappe), **Identification** (jeton Discogs),
**Apparence** (Auto, Clair, Sombre). `Ctrl+,` depuis n'importe où.

---

## Clavier

| touche | où | effet |
|---|---|---|
| `Espace` | Revue | écouter / pause |
| `Entrée` | Revue | convertir la piste ouverte |
| `Retour` ou `X` | Revue | écarter |
| `↑` `↓` | Revue, tables | piste précédente / suivante |
| `I` | Revue | identifier sur Discogs |
| `Ctrl+Z` | partout | annuler la dernière action |
| `Ctrl+A` / `Ctrl+Maj+A` | Revue en Lot, tables | tout sélectionner / rien |
| `Ctrl+F` | écrans avec recherche | aller à la recherche |
| `Ctrl+1`…`8` | partout | l'écran du rail, dans l'ordre affiché |
| `Ctrl+B` | partout | replier / déplier le rail |
| `Ctrl+,` | partout | Réglages |
| `Échap` | partout | fermer le menu ou le popover ouvert |

Sur Mac, `⌘` remplace `Ctrl`.

---

## Ce que Sift ne fait jamais tout seul

- **Rien n'est supprimé** sans une demande explicite. Écarter déplace, ça n'efface pas ; seul le
  vidage de la corbeille efface.
- **Aucune source n'est modifiée** : un dossier surveillé est lu, jamais réécrit.
- **Rien n'est écrit dans Rekordbox** sans sauvegarde préalable vérifiée.
- Les actions coûteuses ou irréversibles se confirment dans l'app — et sur un lot important, la
  confirmation s'arme quelques instants pour qu'un double-clic ne la traverse pas.

---

## Ce que la détection attrape, et ce qu'elle laisse passer

Un manuel qui promet un détecteur parfait ment. Sift mesure trois choses : la coupure du spectre,
la platitude de l'aigu, et la trace qu'un codec laisse dans les échantillons. Sur la référence du
dépôt (150 transcodages fabriqués, 10 achats vérifiés) :

- **Aucun fichier authentique n'a été accusé.** C'est la contrainte qui prime sur tout le reste :
  mieux vaut rater un faux que faire re-racheter un bon fichier.
- **Environ un tiers des transcodages passent encore pour authentiques** (31 % de faux négatifs,
  contre 68 % avant la v0.1.1). Le noyau dur est l'AAC à débit élevé et le MP3 320 : ces
  encodeurs ne coupent pas le haut du spectre, donc la trace la plus visible n'existe pas chez eux.

Autrement dit : un verdict **FAUX** est fiable, un verdict **VRAI** veut dire « rien de mesuré ne
le contredit », pas « garanti ».

---

## Quand quelque chose ne va pas

Les erreurs s'affichent dans l'app avec leur cause. Si un fichier refuse de s'analyser, son message
est conservé et visible sur la ligne du morceau — ce n'est pas un silence. Une mise à jour
disponible se propose au démarrage.

Pour signaler un problème : [github.com/c0remusic/sift/issues](https://github.com/c0remusic/sift/issues).
