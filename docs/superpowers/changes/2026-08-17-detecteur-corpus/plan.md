# Étape 3 — cross-test Fakin' The Funk

Ouvert le 2026-08-18. `review.md` porte le résultat des étapes 1 et 2 ; ce document est le
protocole de la troisième, et sera complété par son résultat.

## Ce que ce cross-test cherche, et ce qu'il ne cherche pas

La question a changé de nature après l'étape 2. Avant, FTF aurait servi de second avis sur un
détecteur dont on ignorait la qualité — et un accord entre deux outils n'aurait rien prouvé, deux
méthodes voisines pouvant partager un angle mort.

Maintenant la question est précise : **est-ce que FTF détecte les 47 transcodages que Sift laisse
en Ok ?**

- **S'il les détecte** — sa méthode ne repose pas sur la position d'une falaise spectrale, et il
  vaut la peine de comprendre laquelle. C'est le cas qui rapporte.
- **S'il les rate aussi** — le problème est dur pour tout le monde, et ça change ce qu'on peut
  raisonnablement promettre à un DJ. C'est le cas qui recadre.

Dans les deux cas **le juge est le corpus étiqueté, pas l'accord entre deux logiciels.** La vérité
terrain est de notre côté : on sait lesquels sont faux parce qu'on les a fabriqués.

⚠️ FTF n'est PAS une vérité terrain et ne le devient pas en étant d'accord avec nous. Aucune ligne
de ce document ne doit conclure « FTF confirme donc c'est juste ».

## Le corpus

`C:\sift-corpus`, régénérable par :

```
node scripts/make-corpus.mjs "C:\Users\LEETJ\Desktop\MUSIQUE A TRIER\Nouveau dossier (3)" "C:\sift-corpus"
```

- `genuine/` — 10 fichiers, authentiques par **provenance** (achats Beatport, marqueur
  `encoded_by: Encoded by Beatport` + `Purchased at Beatport` dans l'ID3 de fin de fichier).
- `fake/` — 150 fichiers, faux par **construction** (15 variantes d'encodage ré-emballées en FLAC).
- `labels.json` — la vérité terrain, une ligne par fichier.

⚠️ Ne jamais laisser Sift surveiller ce dossier : 160 FLAC dont 150 sont des faux délibérés
entreraient dans la vraie bibliothèque. **Vérifié le 2026-08-18** plutôt que laissé en
garde théorique — la table `sources` ne contient qu'une ligne,
`C:\Users\LEETJ\Documents\Soulseek Downloads\complete`, et `C:\sift-corpus` est en dehors.
À re-vérifier si une source est ajoutée.

## Ce qu'Antoine fait (les seules étapes qui demandent une main humaine)

1. Installer Fakin' The Funk (logiciel commercial Windows, version d'essai suffisante pour 160
   fichiers si elle n'est pas limitée en nombre — à vérifier au lancement).
2. Le pointer sur `C:\sift-corpus` en **récursif**, pour qu'il voie `genuine/` et `fake/`.
3. Exporter le résultat en CSV — FTF propose un export de rapport. Si la version d'essai ne
   l'autorise pas, une capture d'écran de la liste triée par verdict suffit à la main : ce qui
   compte est le couple (nom de fichier, verdict).
4. Déposer le CSV dans `C:\sift-corpus\ftf.csv`.

## Ce que je fais ensuite

1. Mesurer Sift sur le même corpus (`corpus_scan` sur `genuine/` puis `fake/`).
2. Joindre les trois sources — étiquettes, Sift, FTF — sur le nom de fichier.
3. Rendre **deux** matrices de confusion côte à côte, plus la ventilation par encodeur, qui est la
   seule colonne qui répond vraiment : un outil aveugle à une famille entière et parfait sur une
   autre a la même moyenne qu'un outil médiocre partout.

## Le chiffre à battre

Mesuré le **2026-08-18**, une fois la platitude de l'aigu entrée dans `verdict()` :

| | Ok | Douteux | Faux |
|---|---|---|---|
| authentique (10) | 10 | 0 | 0 |
| faux (150 mesurés) | 47 | 62 | 41 |

**0 faux positif, 31,3 % de faux négatifs.** Les 47 ratés sont surtout de l'AAC (`aac256` et
`aacmf256` 9/10 chacun, `aacmf128` 7/10, `aac128` 3/10), du MP3 haut débit (`lame320` 8/10,
`lameV0` 5/10) et quelques isolés.

Ligne de base précédente, pour situer le progrès — même corpus, Sift à `ce97d92`, coupure seule :
102 Ok / 7 Douteux / 40 Faux, soit **68 % de faux négatifs**. Le nombre d'accusations n'a pas bougé
(40 → 41) : tout le gain est passé en zone grise.

## Ce que ce protocole ne mesurera pas

- **10 sources seulement**, toutes de la même famille musicale (house/techno achetée). Le taux de
  faux positifs des deux outils reposera sur dix fichiers — c'est un échantillon, pas une garantie.
- **Aucun transcodage en chaîne** (lossy → lossy → lossless), pourtant courant.
- **Aucun fichier trouvé** : tout le corpus est fabriqué. Ce que les deux outils font sur la vraie
  bibliothèque d'Antoine reste hors de cette mesure.
