# Sift, le manuel

Ce manuel s'adresse à quelqu'un qui vient d'installer Sift et veut s'en servir. Il ne suppose
aucune connaissance technique. Le vocabulaire du code et de l'architecture vit ailleurs, dans
`CONTEXT.md` et `docs/developpement.md`.

Le même manuel, dans le design de l'application : [sift-music.vercel.app/manuel.html](https://sift-music.vercel.app/manuel.html),
aussi [en PDF](https://sift-music.vercel.app/manuel.pdf). Les installeurs sont sur
[la page des versions](https://github.com/c0remusic/sift/releases/latest).

Sift prépare une bibliothèque de DJ. Il repère les faux fichiers lossless, trouve les doublons,
range chaque morceau au bon format et au bon endroit, puis envoie la bibliothèque vers Rekordbox
et sur une clé USB. Une règle gouverne tout le reste : **déplacer un morceau, c'est le convertir
et le ranger dans le même geste**. Sift ne supprime rien sans que vous le demandiez, et il ne
modifie jamais un dossier qu'il surveille.

---

## Installer

Un seul fichier à télécharger, selon votre machine. Tout le reste de la page de version sert à la
mise à jour automatique et ne s'installe pas : `.app.tar.gz`, `.msi`, les fichiers `.sig`,
`latest.json`. Le `.app.tar.gz` est le piège classique : il pèse plus lourd que le `.dmg`, donc il a
l'air d'être le bon.

| Votre machine | Le fichier à prendre |
|---|---|
| Windows | `Sift_<version>_x64-setup.exe` |
| Mac avec puce Apple (M1 et suivants) | `Sift_<version>_aarch64.dmg` |
| Mac Intel | `Sift_<version>_x64.dmg` |

Les installeurs ne sont pas signés, faute de certificat Apple ou Microsoft. Votre système affiche
donc un avertissement au premier lancement, une seule fois.

**Sur Windows**, SmartScreen annonce « Windows a protégé votre ordinateur ». Cliquez sur
**Informations complémentaires**, puis sur **Exécuter quand même**.

**Sur Mac**, ouvrez le `.dmg`, glissez Sift dans Applications, puis selon le message :

- « développeur non identifié » : ouvrez **Réglages Système › Confidentialité et sécurité**,
  descendez jusqu'au message qui concerne Sift et cliquez sur **Ouvrir quand même** ;
- « Sift is damaged and can't be opened » : ce message n'offre aucun bouton. Ouvrez le Terminal et
  tapez la commande ci-dessous. Le `-r` est indispensable, une application Mac est un dossier.

```
xattr -dr com.apple.quarantine /Applications/Sift.app
```

Si le message persiste : `codesign --force --deep --sign - /Applications/Sift.app`

Les versions suivantes s'installent d'elles-mêmes, depuis l'application.

---

## Trois mots à connaître

**Faux lossless.** Un fichier FLAC, WAV ou AIFF fabriqué à partir d'un MP3, d'un AAC ou d'un Opus.
L'étiquette dit « sans perte », mais le son a déjà été compressé, et cette qualité ne reviendra
pas. Ces fichiers se vendent, circulent, et à l'oreille la différence ne s'entend pas toujours.
C'est ce que Sift cherche.

**Verdict.** Ce que Sift conclut d'un fichier, en un mot :

| Verdict | Ce que ça veut dire |
|---|---|
| **VRAI** | Rien dans le signal ne contredit un vrai fichier lossless. |
| **FAUX** | La compression est mesurable. Le fichier est mis de côté pour être racheté ailleurs, il n'est pas supprimé. |
| **À VÉRIFIER** | Quelque chose d'inhabituel, sans preuve. Sift le signale et vous laisse décider, spectrogramme à l'appui. |

**Écarter.** Sortir un fichier du circuit sans le supprimer. Deux destinations : **À re-sourcer**
pour un faux ou un fichier tronqué, à racheter ailleurs, et **Corbeille** pour un doublon ou une
erreur. Rien n'est effacé tant que vous ne videz pas la corbeille vous-même.

### Les mots anglais gardés tels quels

`LOSSLESS`, `DUPLICATE`, `MATCH`, `CHECK MATCH`, `kbps`, `kHz`, `MP3`, `AIFF`, `WAV` restent en
anglais dans les filtres, les étiquettes et les badges, parce que c'est sous cette forme qu'ils
apparaissent partout ailleurs : dans Rekordbox, sur les boutiques, sur les forums. Le verdict, lui,
est en français.

---

## Le rail, de haut en bas

La colonne de gauche a deux parties. En haut, vos **sources** : les dossiers que Sift surveille,
chacun avec sa couleur, le nombre de fichiers en attente et son état. Cliquer sur une source
limite la liste de Revue à ses fichiers. Un dossier sans aucun fichier audio reconnu porte la
mention « 0 audio ».

En dessous, huit **écrans**, dans l'ordre d'une session de travail. `Ctrl+1` à `Ctrl+8` y mènent
directement (`⌘` sur Mac), et `Ctrl+B` replie la colonne.

### Revue

L'écran principal : un morceau à la fois, on écoute, puis on décide. La liste des fichiers est à
gauche, le morceau ouvert au centre, le diagnostic à droite. La liste se filtre par catégorie
(Lossless, MP3, Faux, Doublons…) et par recherche, `Ctrl+F` y va directement.

Pour chaque morceau, vous avez le lecteur, le verdict, une fiche de métadonnées modifiable sur
place, l'identification Discogs et le diagnostic audio complet. Le son passe avant le verdict :
on écoute, puis on tranche.

- **Convertir** (`Entrée`). Le fichier part vers sa destination, converti en MP3, WAV ou AIFF selon
  votre choix, avec un nom de fichier construit depuis les tags. Le menu Destination propose
  l'arborescence de votre bibliothèque et vos autres dossiers.
- **Écarter** (`Retour` ou `X`). Vers À re-sourcer si le verdict est FAUX ou si le fichier est
  tronqué, vers la Corbeille sinon. Un doublon est signalé comme tel, avec le morceau d'origine.
- **Identifier** (`I`). Sift interroge Discogs, propose des correspondances et écrit dans le fichier
  les tags que vous retenez : artiste, titre, label, année, genres, pochette.

**Le mode Lot.** Cochez des morceaux dans la liste : la zone centrale résume la sélection
(verdicts, formats, durée totale) et un seul bouton range tout. Le menu **Sélection** permet de
tout prendre, de ne rien prendre, de prendre une seule catégorie, ou tout sauf les faux ; `Ctrl+A`
et `Ctrl+Maj+A` font de même. Au-delà d'un certain nombre de fichiers, Sift récapitule avant de
partir, puis affiche la progression et un rapport.

### Journal

L'historique complet de ce que Sift a fait, regroupé par session et par jour : conversions,
fichiers écartés, identifications, synchronisations. Chaque ligne se défait d'un clic. Un
rangement de masse parti au mauvais endroit revient d'ici, même après avoir redémarré
l'application.

### Rangés

Votre bibliothèque, telle qu'elle est sur le disque, dans une table à la Finder : colonnes
personnalisables et mémorisées, tri, recherche, sélection multiple avec un panneau récapitulatif.
La pochette sert de bouton de lecture. Le clic droit ouvre l'emplacement du fichier, relance
l'analyse, déplace ou écarte. Une barre montre l'espace occupé par chaque format.

### À re-sourcer

Les fichiers faux ou tronqués, chacun avec la raison. Considérez cette liste comme une liste de
courses : ces morceaux attendent d'être rachetés ou retrouvés ailleurs. Le bouton **Restaurer**
remet un morceau dans la liste de Revue si le verdict vous semble trop prudent.

### Corbeille

Les doublons et les erreurs. Rien n'est effacé du disque tant que vous ne videz pas la corbeille,
et Sift demande confirmation avant de le faire. Un morceau se restaure de la même façon que depuis
À re-sourcer.

### Rekordbox

Une seule action pour tout synchroniser : **Synchroniser la sélection** ou **Tout synchroniser**.
Sift compare votre bibliothèque rangée avec celle de Rekordbox, regroupe ce qu'il propose par type
(nouveaux morceaux, métadonnées, pochettes, fichiers liés à réparer) et nomme chaque élément
« Artiste, Titre ». Deux façons d'écrire : un fichier XML que vous importez dans Rekordbox, ou
l'écriture directe dans sa base de données.

**Rekordbox doit être fermé pendant l'écriture directe.** Sift sauvegarde la base avant d'y
toucher, vérifie la sauvegarde, et refuse d'agir si le logiciel est ouvert.

### Clé USB

La colonne de gauche liste les disques amovibles. Pour le disque choisi, vous voyez l'espace
occupé par format, ses caractéristiques (point de montage, format, capacité, espace libre,
nombre de fichiers, modèle, état de santé) et trois actions : **Formater…**, **Éjecter**,
**Relire le disque**.

Le formatage écrit du FAT32, le format que lisent les platines, même au-delà des 32 Go que Windows
refuse. Il avance par blocs et affiche les mégaoctets écrits.

**Formater efface tout le contenu de la clé.** Sift le demande deux fois en nommant le volume, et
refuse si le disque a changé entre-temps.

### Réglages

Les catégories à gauche, le panneau à droite. Chaque réglage s'applique immédiatement, il n'y a
pas de bouton Enregistrer.

- **Général** : le dossier racine de votre bibliothèque. Il définit l'arborescence de rangement,
  pas la conversion.
- **Nommage** : le modèle de nom de fichier, construit à partir des tags, avec un aperçu en direct.
- **Identification** : votre jeton Discogs.
- **Apparence** : le thème, automatique, clair ou sombre.

`Ctrl+,` ouvre les Réglages depuis n'importe quel écran.

---

## Le clavier

| Touche | Où | Effet |
|---|---|---|
| `Espace` | Revue | écouter, mettre en pause |
| `Entrée` | Revue | convertir le morceau ouvert |
| `Retour` ou `X` | Revue | écarter |
| `↑` `↓` | Revue, tables | morceau précédent, morceau suivant |
| `I` | Revue | identifier sur Discogs |
| `Ctrl+Z` | partout | annuler la dernière action |
| `Ctrl+A` / `Ctrl+Maj+A` | Revue en mode Lot, tables | tout sélectionner, ne rien sélectionner |
| `Ctrl+F` | écrans avec une recherche | aller à la recherche |
| `Ctrl+1` à `Ctrl+8` | partout | l'écran correspondant, dans l'ordre du rail |
| `Ctrl+B` | partout | replier ou déplier le rail |
| `Ctrl+,` | partout | Réglages |
| `Échap` | partout | fermer le menu ou la fenêtre ouverte |

Sur Mac, `⌘` remplace `Ctrl`.

---

## Ce que Sift ne fera jamais sans vous

- **Supprimer un fichier.** Écarter un morceau le déplace. Seul le vidage de la corbeille efface,
  et il demande confirmation.
- **Modifier vos dossiers de téléchargement.** Sift les lit, il ne les réécrit jamais.
- **Écrire dans Rekordbox** sans avoir d'abord sauvegardé et vérifié sa base.
- **Rendre une action définitive.** Tout ce que Sift fait s'annule depuis le Journal. Sur un lot
  important, la confirmation s'arme quelques instants pour qu'un double clic ne la traverse pas.

---

## Ce que la détection ne voit pas

Aucun détecteur n'est parfait. Sift mesure trois choses dans le signal : la fréquence où le spectre
s'arrête, la densité des aigus, et la trace que laisse un codec dans les échantillons. Sur son jeu
de test (8 achats vérifiés, 150 transcodages fabriqués depuis ces achats), il n'a accusé aucun
fichier authentique : c'est la règle qui prime, il vaut mieux laisser passer un faux que vous faire
racheter un bon fichier. En contrepartie, une partie des transcodages haut débit passe encore,
surtout les MP3 320 et V0 et l'AAC 128, qui ne coupent pas les aigus. Un chantier est ouvert pour
les attraper.

En clair : un verdict **FAUX** est fiable. Un verdict **VRAI** signifie que rien de mesurable ne
contredit le fichier, pas qu'il est garanti authentique.

---

## Quand quelque chose ne va pas

Les erreurs s'affichent dans l'application avec leur cause. Si un fichier refuse de s'analyser,
son message reste visible sur sa ligne. Une mise à jour disponible se propose au démarrage.

Pour signaler un problème : [github.com/c0remusic/sift/issues](https://github.com/c0remusic/sift/issues).
Indiquez la version, votre machine, et ce que vous attendiez.
