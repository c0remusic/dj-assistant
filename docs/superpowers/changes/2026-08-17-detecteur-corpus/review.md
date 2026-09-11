# Le détecteur de faux lossless, mesuré sur de la vraie musique

Ouvert le 2026-08-17 sur une question d'Antoine : **« est-ce qu'on est sûrs que le détecteur de
faux fonctionne ? »** La réponse est non, et ce document dit de combien.

Trois étapes, décidées ensemble dans cet ordre : corriger ce qui est prouvé faux, construire un
corpus étalonné, puis seulement confronter à un second logiciel.

## Étape 0 — ce que les tests prouvaient, et ce qu'ils ne prouvaient pas

`verdict()` est une table de seuils, couverte par 11 tests unitaires : solide, mais ce n'est pas là
qu'est le risque. Tout le risque est dans `detect_cutoff` (`analysis/spectrum.rs`), et le corpus
qui le gardait descend **entièrement d'un seul signal** :
`aevalsrc=0.3*sin(2*PI*(300+20000*t/10)*t)` — un balayage sinusoïdal (`make-fixtures.mjs:15`), pas
de la musique, passé par **un** encodeur à **deux** débits.

Un sweep a de l'énergie pleine à chaque fréquence qu'il traverse : c'est l'entrée la plus facile
possible pour un détecteur de falaise. Les deux ancres « authentiques » prévues
(`anchor_real_lossless.flac`, `anchor_real_320.mp3`) **ne sont pas sur le disque**, et les tests qui
les utilisent passent en sautant.

## Étape 1 — deux faux positifs prouvés, corrigés (`ce97d92`)

Trouvés en interrogeant la base de production (2705 pistes analysées), pas par lecture de code.

**Le pied de basse rendu comme une coupure.** 10 fichiers portaient un cutoff entre 571 et 1367 Hz,
dont 4 exactement à 571,0 — le bin 53, c'est-à-dire `guard`, le plus bas que la boucle pouvait
tester. Un morceau house de 5 minutes n'a pas zéro contenu au-dessus de 571 Hz : les 10 étaient
marqués FAKE à tort.

La sonde `spectrum::tests::ltas_probe`, ajoutée pour l'occasion, a montré le mécanisme — et il n'est
pas celui qu'on supposait. **Il n'y a aucune falaise dans ces fichiers** : spectre lisse jusqu'à
21 kHz. La seule chute de 18 dB sur 500 Hz s'y trouve au passage grave→médium (+24,5 dB à 120 Hz,
−0,1 dB à 800 Hz), et rien au-dessus ne remonte à moins de `RECOVERY_TOL` du niveau des **graves** —
donc `recovers` est faux et la boucle rend son propre plancher. Comparé au témoin sain mesuré le
même jour, la seule différence est **4 dB de pente** : 17 dB de chute contre 21.

Correctif : `SEARCH_FLOOR_HZ = 2000`, une borne physique et non un seuil calibré — aucun passe-bas
d'encodeur ne descend là. Contrôle sur la base réelle : **zéro fichier** n'a de cutoff entre 1400 et
8400 Hz, donc la borne retire les 10 faux positifs sans déplacer aucune autre mesure.

**Un décodage vide lu comme une mesure.** 2 MP3 de plus de six minutes, déclarés 320 kbps,
`codec_error` NULL, portaient `cutoff_hz = 0` — que le verdict lisait comme « coupe à 0 Hz, très en
dessous du plancher de 19 000 pour du 320 » et marquait FAKE. `NO_MEASUREMENT_HZ` est maintenant
nommé et rend Grey, **après** le désaccord de conteneur, qui est une fraude établie sans le spectre.

Migration **v21** : sans elle le correctif ne toucherait pas les données. `verdict` et `cutoff_hz` ne
sont couverts par aucune version de cache — c'est le défaut « une seule des trois sorties d'étape est
versionnée » relevé sur la map #6.

## Étape 2 — le corpus étalonné, et ce qu'il mesure

10 fichiers lossless de provenance connue (achats, dossier fourni par Antoine) × 15 variantes
d'encodage, ré-emballées en FLAC. **160 fichiers étiquetés, vérité terrain par construction** : les
originaux sont authentiques par provenance, les transcodages sont faux parce qu'on les a fabriqués.

Reproductible : `scripts/make-corpus.mjs` (fabrication) + `analysis::corpus::corpus_scan` (mesure) +
`scripts/score-corpus.mjs` (matrice). Le corpus lui-même — 17 Go de dérivé — n'est pas versionné.

### Résultat

```
verite \ verdict        Ok    Grey    Fake   total
genuine                10       0       0      10
fake                  102       7      40     149

FAUX POSITIFS (authentique -> Fake) : 0/10  = 0,0 %
FAUX NEGATIFS (faux -> Ok)          : 102/150 = 68,0 %
  rattrapes en Grey                 : 7/150
```

**68 % des transcodages passent pour du lossless authentique.** Et la moyenne ment : la ventilation
par encodeur montre que le détecteur n'est pas « moyennement bon », il est **aveugle à des familles
entières**.

```
variante        n   detecte  rate   Grey   cutoff min..max
aac128          10        0     10      0   22050..22050  <-- RATE
aac256          10        0     10      0   22050..22050  <-- RATE
aacmf128        10        0     10      0   22050..22050  <-- RATE
aacmf256        10        0     10      0   22050..22050  <-- RATE
lame128         10        9      1      0   16817..22050  <-- RATE
lame160         10       10      0      0   17582..18002
lame192         10       10      0      0   18906..19391
lame256         10        2      1      7   19488..20004  <-- RATE
lame320         10        0     10      0   20177..20704  <-- RATE
lameV0          10        0     10      0   22050..22050  <-- RATE
mfmp3_128       10        9      0      0   15999..16699
mfmp3_320       10        0     10      0   22050..22050  <-- RATE
opus128         10        0     10      0   20215..24000  <-- RATE
vorbisq5        10        0     10      0   20941..22050  <-- RATE
wma192          10        0     10      0   20134..22050  <-- RATE
```

### Ce que la ventilation dit

**Ce qui marche : le MP3 à passe-bas dur, et rien d'autre.** LAME 128/160/192 et MediaFoundation 128
sont détectés à 38/40. C'est le cas d'école — l'encodeur brickwalle entre 16 et 19,4 kHz et la
falaise est franche.

**Ce qui passe en entier, et pourquoi :**

- **Tout l'AAC (40 fichiers, 4 variantes, 100 % raté)** — cutoff mesuré à 22050, c'est-à-dire
  *aucune falaise détectée*. Les encodeurs AAC modernes ne brickwallent pas comme le MP3 des années
  2000. Il n'y a rien à voir pour un détecteur de coupure.
- **LAME 320 (100 % raté)** — cutoff 20177 à 20704, donc **au-dessus** de `LOSSLESS_OK_HZ` = 20000.
  Le détecteur voit la falaise et la juge normale. C'était le cas dur prévu, il est confirmé.
- **LAME V0 et MediaFoundation 320 (100 % ratés)** — 22050, pas de passe-bas du tout.
- **Opus, Vorbis, WMA (100 % ratés)** — 20134 à 24000, au-dessus du seuil ou sans coupure.

**Le seul endroit où la zone grise travaille** est LAME 256 : 7 des 10 y tombent
(19488–20004 chevauche exactement `LOSSY_CLIFF_HZ` 19500 et `LOSSLESS_OK_HZ` 20000). C'est le
comportement voulu de la bande grise, et c'est la seule variante où elle sert.

**Zéro faux positif** sur les 10 authentiques — le correctif de l'étape 1 tient sur ce corpus.

### Ce que cette mesure ne dit pas

- **10 sources seulement**, toutes de la même famille musicale (house/techno achetée). Le taux de
  faux positifs à 0/10 n'est pas une garantie : c'est un échantillon de dix.
- **Aucun transcodage en chaîne** (lossy → lossy → lossless), qui est pourtant courant dans la vraie
  vie et probablement plus visible.
- **Rien sur les fichiers déjà en bibliothèque** : le corpus est fabriqué, pas trouvé.
- Une ligne du corpus a dû être écartée du dénominateur : `src02_mfmp3_128.flac` était un **fichier
  de 0 octet** laissé par un run avorté, que le `existsSync` nu du script faisait sauter au run
  suivant. Corrigé (`done()` exige une taille non nulle). Le compte d'erreurs du harnais est ce qui
  l'a rattrapé — un artefact vide était étiqueté comme un vrai faux.

### Un biais vérifié, et écarté

Les transcodages sortaient en **s32 (24 bit)** et les authentiques en s16, parce qu'un décodeur
lossy rend du flottant. Le corpus devenait séparable par la profondeur de bits seule — un artefact
du pipeline, pas de la fraude.

Vérifié plutôt que supposé : quatre variantes ré-emballées en s16 donnent des cutoffs **identiques**
(22050 / 16860 / 22050) ou à 12 Hz près pour Opus (20215 contre 20227, sous la résolution d'un bin),
et des verdicts identiques. Le biais ne portait pas la mesure. `-sample_fmt s16` est quand même posé
dans le script, pour que le corpus n'ait qu'une seule variable.

## Ce que ça implique — non tranché

Le détecteur repose sur **un seul signal** : la position d'une falaise spectrale. Ce signal existe
dans le MP3 à passe-bas dur et n'existe pas ailleurs. Aucun réglage de seuil ne rattrape ça : à
22050 Hz il n'y a rien à seuiller.

Rendre l'AAC et le haut débit détectables demande un **autre signal**, ce qui est une décision de
conception et pas un correctif. Pistes connues, non évaluées ici : la finesse de structure du haut
du spectre (les trous de quantification MDCT survivent au ré-encodage), ou l'énergie résiduelle par
bande au-dessus de 16 kHz plutôt que la position d'une coupure.

### Une piste écartée par la mesure — le taux de compression FLAC

Une version antérieure de ce document annonçait comme piste prometteuse le rapport taille FLAC /
durée : le ré-emballage d'un transcodage semblait compresser beaucoup moins bien que l'original
(45 Mo contre 78–106 Mo pour la même piste), le signal lossy n'ayant plus la redondance bit-à-bit
d'un master.

**C'était mesuré sur les fichiers contaminés par le biais s32.** Les transcodages étaient en 24 bit
et l'original en 16 : la moitié de l'écart annoncé était de la profondeur de bits, pas de
l'entropie. À profondeur égale (s16), sur la même piste :

| fichier | taille |
|---|---|
| authentique | 45,2 Mo |
| aac128 | **43,7 Mo** |
| opus128 | 46,1 Mo |
| lame128 | 70,7 Mo |
| lameV0 | 71,0 Mo |

Le signal existe pour le MP3 — mais le MP3 est justement la famille que le détecteur voit déjà.
Pour l'AAC il pointe **dans le mauvais sens** (le transcodage compresse *mieux* que l'original), et
pour Opus il ne dit rien. Autrement dit il est absent là où le trou est, et redondant là où il n'y
en a pas. La piste est écartée, et c'est la mesure qui l'écarte, pas une intuition.

## Élargir le corpus — ce que la provenance permet vraiment (2026-08-18)

Le côté authentique reposait sur 10 achats. Le chiffre fragile de l'étape 2 est le **0/10 en faux
positifs** : c'est lui qui protège Antoine de re-sourcer un bon fichier, et dix, c'est peu. D'où la
recherche de sources supplémentaires **de provenance établie**.

### Le piège du marqueur trop lâche

Les achats Beatport portent un marqueur écrit par le magasin. Premier essai : grep de la chaîne
`Beatport` n'importe où dans le fichier. Sur `BACKUP USB` (676 fichiers lossless, l'export Rekordbox
d'Antoine), **116 correspondances** — trop beau.

Le cas qui l'a démasqué, `Kyoto (Ariane Blank Remix).wav`, 77 Mo, `pcm_s16le` :

```
encoded_by : dBpoweramp 2024-05-30      <- pas Beatport
cutoff     : 16031 Hz  ->  FAKE
```

Acheté sur Beatport à l'origine — le commentaire `Purchased at Beatport` est encore là — puis
**converti localement en WAV**, et le cutoff à 16 kHz dit que la source de cette conversion était un
MP3 128–160 kbps. Le tag d'achat survit à la conversion ; il atteste l'origine du morceau, pas
l'intégrité du fichier.

Second essai, marqueur strict (`Encoded by Beatport` exact) : **rate 5 des 10 achats connus**, parce
que Beatport écrit tantôt `Beatport`, tantôt `Encoded by Beatport` dans le même champ.

Ce qui marche est de lire le **champ** `encoded_by`, pas de grepper les octets.

### Ce que `BACKUP USB` contient réellement

676 fichiers lossless, champ `encoded_by` lu un par un :

| `encoded_by` | fichiers | lecture |
|---|---|---|
| *(absent)* | 520 (77 %) | provenance inconnue |
| `dBpoweramp` (3 versions) | 123 | converti localement, source inconnue |
| `Lavf60.3.100` | 17 | converti localement (ffmpeg) |
| `Beatport` / `Encoded by Beatport` | **13** | fichier de magasin |
| autres convertisseurs | 3 | — |

**12 titres uniques** utilisables, tous `Ok` (cutoff 20241 à 22050). Le corpus passe donc de 10 à
22 sources authentiques — l'échantillon qui porte le taux de faux positifs plus que double.

### Le chiffre inconfortable, et ce qu'il ne prouve pas

Sur les 123 fichiers convertis au dBpoweramp, Sift en flague **3**. Ce n'est pas rassurant : c'est
un **plancher**. Le détecteur rate 68 % des transcodages de notre corpus et ne voit rien de l'AAC ni
du haut débit, donc les 89 « Ok » de cette population sont exactement les fichiers dont on sait
qu'ils ont été reconvertis depuis quelque chose, et dont le détecteur actuel ne peut pas dire quoi.

⚠️ On ne peut PAS en tirer un nombre de faux cachés. Extrapoler 3 avec le taux de détection du
corpus supposerait que les conversions d'Antoine ont le même mélange d'encodeurs sources que nos 15
variantes, ce que rien n'établit. Le seul énoncé soutenable est qualitatif : **le nombre réel est
supérieur à 3, d'un facteur inconnu.**

## Comment FTF décide, et pourquoi il plafonne au même endroit que nous (2026-08-18)

Établi depuis **ses propres fichiers de langue et sa base de réglages**, pas par désassemblage :
son algorithme n'a pas été lu et n'a pas été copié.

Son unique critère de faux, verbatim (`Languages\en-US.txt`) :

```
MSG_FAKE_FILE = Mr. Funk says: FAKE! Actual bitrate (%d) is lower than stated bitrate (%d)
```

Et le « débit réel » sort de la coupure : `Settings:CutoffLevl = 19600` (l'UI l'appelle
« Allow cutoffs above: »), placeholders `$frequency` et `$realbitrate` côte à côte, log
`Analyzing frequency (aggressive) for %s`. C'est **exactement notre branche lossy**, avec un seuil
au lieu de deux. Son mode « agressif » est vague dans sa propre interface : *« might detect more
fakes »*.

**Ce qu'il fait et pas nous** — un seul point compte : `Actual duration != stated duration`
(colonnes `Duration`/`ActualDuration`, peuplées et distinctes dans sa base). Nous prenons
`duration_sec` de l'en-tête et ne la comparons **jamais** au décodé (`analysis/mod.rs`), alors
qu'on décode déjà tout le fichier. C'est gratuit et ce n'est pas fait. Il a aussi une classe
CORROMPU distincte (silence long en plein morceau) et un `SupportFHGEncoder`.

**Ce que nous faisons et pas lui** : le désaccord de conteneur (un MP3 renommé `.flac` — rien dans
son schéma ni ses messages), la zone grise (il est binaire), phase/dual-mono/true-peak.

**Sa colonne `hasHole`** — la piste qui semblait la plus prometteuse — vaut **0 sur les 14 fichiers
de sa base**, dont de vrais MP3, et aucune chaîne d'interface ne la mentionne. Elle a l'air
vestigiale.

Son seuil de 19600 appliqué à **nos coupures déjà mesurées** donne 32 à 48 détections sur 150.
Nous : 40. **Les deux sont dans le bruit l'un de l'autre**, parce que c'est le même signal.

## Un signal qui double la détection — la platitude spectrale de l'aigu

Le plafond n'est pas un réglage, c'est le choix du signal. Deux candidats testés et **réfutés par la
mesure** avant celui qui marche :

- **Alignement sur la grille de trames du codec** (MP3 1152, AAC 1024) : les longueurs décodées sont
  **identiques** entre l'authentique et ses 7 transcodages (17 722 908 échantillons). ffmpeg honore
  les infos gapless dans les deux sens, la grille est effacée. Mort sur un corpus fabriqué
  proprement.
- **Corrélation des enveloppes aigu/médium** (hypothèse : un aigu resynthétisé par SBR suit la bande
  basse de trop près) : authentiques 0,48–0,65, faux 0,31–0,66. Se chevauchent. Mort.

**Ce qui marche.** Un encodeur lossy ne supprime pas l'aigu : il ne garde que ses coefficients les
plus forts et met le reste à zéro. L'aigu devient **clairsemé et pointu**, là où un master porte un
plancher de bruit continu. Ça se mesure par la platitude spectrale (moyenne géométrique / moyenne
arithmétique) de la bande 16-20 kHz, médiane sur les trames. Le sens est l'inverse de l'intuition
de départ : les transcodages sont **moins** plats, pas plus.

| | détecte | angle mort |
|---|---|---|
| coupure (Sift **et** FTF) | 40/150 = 27 % | AAC, LAME 320, V0, Opus, Vorbis, WMA |
| **platitude de l'aigu** | **91/150 = 61 %** | Opus seul (0/10) |

Elle attrape ce que la coupure rate entièrement : mfmp3_320 9/10, vorbisq5 8/10, wma192 8/10,
aac128 6/10, lameV0 5/10, lame320 3/10 — tous à 0/10 en coupure. Et elle garde 10/10 sur les LAME
128/160/192/256 que la coupure attrape déjà : **elle la domine sur ce corpus**, elle ne la complète
pas.

### La validation, parce que le seuil est ajusté

Le seuil (−5,4 dB) est le **minimum des 10 authentiques** — donc ajusté sur eux par construction, et
61 % serait un chiffre creux sans épreuve indépendante.

Épreuve : les **10 authentiques de `BACKUP USB`**, achats Beatport d'une autre provenance, qui n'ont
pas servi à fixer le seuil. Résultat : **−4,7 à −2,8 dB, zéro faux positif**. Les 20 authentiques
des deux jeux tiennent dans [−5,4 ; −2,6] ; les transcodages descendent à −43,8.

### Par trame contre LTAS — l'agrégation fait les deux tiers du signal

Le choix d'agréger la platitude **par trame** (médiane) plutôt que de la calculer sur le LTAS était
justifié dans le code par la seule robustesse aux trames de silence, avec la mention explicite que
le gain de séparation **n'avait pas été mesuré**. Il l'est maintenant — même corpus, même règle de
seuil (le plancher des authentiques) :

| agrégation | seuil | détection |
|---|---|---|
| **par trame, médiane** | −5,4 dB | **91/150 = 61 %** |
| LTAS, moyenne puis platitude | −3,1 dB | 56/150 = 37 % |

Et l'écart est concentré exactement sur les familles que la coupure rate :

| | par trame | LTAS |
|---|---|---|
| aac128 / aacmf128 | 6 / 3 | 1 / 0 |
| lame320 / lameV0 | 3 / 5 | 0 / 0 |
| mfmp3_320 | 9 | 4 |
| vorbisq5 / wma192 | 8 / 8 | 2 / 4 |
| lame 128/160/192/256 | 10 chacun | 10 chacun |

Sur les LAME que la coupure attrape déjà, **les deux formes font 10/10** : le par-trame n'apporte
rien là où le signal est franc, et tout là où il ne l'est pas.

Ça confirme le mécanisme supposé : la moyenne long terme détruit la structure temporelle qui trahit
l'encodeur — un aigu présent sur 10 % des trames et absent ailleurs y ressemble à un aigu faible
mais continu. C'est la même raison qui rend `detect_cutoff`, calculée sur le LTAS, aveugle à l'AAC.

**Ce que ça ne répare pas** : la marge du seuil est de **0,12 dB** entre le plus bas authentique et
le suivant, dans les DEUX formes. La fragilité du seuil n'est pas un problème d'agrégation.

### Épreuve sur matériel musicalement différent (2026-08-18)

La réserve principale était que 20 authentiques house/techno ne disent rien du classique, de
l'ambient ou d'un master acoustique — le matériel dont l'aigu est naturellement clairsemé, donc là
où cette feature ferait ses faux positifs. Antoine a fourni **29 fichiers de son disque**, dont un
album de broken beat acoustique entier (Kaidi Tatham, 15 titres) et deux titres ambient.

**24 passent, 5 flaguent.** Et les 5 sont invisibles pour la coupure : `Ok`, cutoff 22050 partout.

| matériel | platitude |
|---|---|
| Kaidi Tatham, 15 titres acoustiques | −2,5 à −5,3 dB — tous ok |
| Nova Tekk ambient ×2 | −2,9 / −3,1 dB — ok |
| divers house/électro (8) | −2,8 à −5,3 dB — ok |
| **Sheeq — Just B4** | **−6,5 dB** |
| **Alex Neri, EP entier (4 titres)** | **−10,8 à −12,7 dB** |

**Le cas redouté n'a pas cassé** : l'album acoustique passe en entier, l'ambient aussi.

Forme spectrale de l'EP Alex Neri, mesurée : décroissance régulière jusqu'à −47 dB à 19 kHz puis un
**plateau plat à −49 dB** de 19 à 20,5 kHz. Ce plateau est un plancher de bruit, pas du contenu — il
n'y a plus rien de musical au-dessus de ~18 kHz, dans un fichier déclaré **24 bits** (2117 kbps).
Témoin passant (Kaidi Tatham) sur la même mesure : −17 à −23 dB de vrai contenu jusqu'à 20 kHz.
La coupure rend 22050 sur les deux, parce que la pente ne fait jamais 18 dB sur 500 Hz.

**Deux réserves qui restent ouvertes :**

1. **La marge est trop mince pour livrer.** Plusieurs authentiques sont à −4,8 / −5,0 / −5,3 pour un
   seuil à −5,4. Sur du matériel encore plus varié les faux positifs commenceraient juste en
   dessous. Le seuil ne peut pas rester le minimum observé ; il faut soit une marge décidée, soit
   une bande grise comme pour la coupure.
2. **Rien ne PROUVE que les 5 flagués sont des faux.** La mesure dit que leur aigu s'arrête tôt sur
   un plancher — indication forte, pas preuve. Seule la provenance trancherait, et elle n'est pas
   établie pour ces fichiers.

### La référence consolidée — 44 fichiers, et une distribution bimodale

La réserve qui restait était la fragilité du seuil : « 0,12 dB entre le plus bas authentique et le
suivant ». Consolidation de tout ce qui a été mesuré, trois provenances d'achat indépendantes et
trois familles musicales :

| jeu | n | plage |
|---|---|---|
| corpus Beatport | 10 | −5,4 à −2,6 |
| corpus Beatport clé USB | 10 | −4,7 à −2,8 |
| fournis par Antoine, ceux qui passent | 24 | −5,3 à −2,5 |
| **référence élargie** | **44** | **−5,4 à −2,5** |
| les 5 écartés | 5 | −12,7 à −6,5 |

**La lecture de la « marge » était fausse.** La borne basse n'est pas un point isolé : trois
fichiers y convergent (−5,4 / −5,3 / −5,3). Un minimum unique serait un accident d'échantillon ;
un amas est une frontière. Et la distribution est **bimodale** — 44 fichiers d'un côté, **un vide
de 1,1 dB**, puis 5 fichiers entre −12,7 et −6,5. Il n'y a rien entre −6,5 et −5,4.

Les 44 couvrent house/techno acheté, ambient (Nova Tekk), et un album de broken beat acoustique
entier (Kaidi Tatham, 15 titres) — le matériel dont on craignait qu'il fasse des faux positifs.

**L'exclusion des 5 est un jugement, pas une mesure**, et il faut le lire comme tel. Sa raison :
le vide qui les sépare, plus l'inspection spectrale de l'un d'eux (contenu s'arrêtant à ~18 kHz sur
un plancher plat à −49 dB). Deux populations, pas une queue de distribution. Mais **ils sont
achetés eux aussi** — donc ces bornes décrivent « un master à bande pleine », pas « un fichier
légitime ». Un master volontairement sombre tombera dessous sans être fautif, et c'est exactement
pourquoi l'affichage n'accuse pas.

### Ce que ça n'établit toujours pas

- **20 fichiers authentiques**, tous house/techno achetée. Du classique, du jazz, de l'ambient, un
  master analogique ancien — tout ce dont l'aigu est naturellement clairsemé — n'a **pas** été
  testé, et c'est exactement là que cette feature ferait des faux positifs.
- **Opus reste invisible** (0/10) : il ne creuse pas l'aigu.
- Rien sur les transcodages en chaîne, ni sur les fichiers réels de la bibliothèque.
- La sonde est `scripts/hf-flatness-probe.mjs` ; **rien n'est branché dans le détecteur**. C'est un
  candidat mesuré, pas une décision.

## L'angle mort Opus, fermé par une seconde bande (2026-08-18)

Opus était invisible aux deux signaux : 0/10 en coupure, 0/10 en platitude. Diagnostic mesuré : les
fichiers Opus sont à **48 kHz** — le codec force ce taux — donc la bande fixe 16-20 kHz y tombe à
**33-42 % du Nyquist**, en pleine bande passante, là où Opus a encore tout son contenu.

**Un piège écarté avant de conclure.** Séparer Opus des authentiques pouvait n'être que séparer
48 kHz de 44,1 kHz, tous les authentiques du corpus étant à 44,1. Trois témoins 48 kHz **sans
codec** ont donc été fabriqués (rééchantillonnage des sources), pour que la comparaison porte sur
le codec et pas sur le taux :

| bande | authentique 44k | témoin 48k sans codec | opus128 48k |
|---|---|---|---|
| 16-20 kHz (fixe) | −5,1 / −4,4 / −5,1 | −5,0 / −4,4 / −5,1 | −4,9 / −3,2 / −2,9 |
| 0,80-0,98 × Nyquist | −3,2 / −4,1 / −10,8 | −4,4 / −6,3 / −14,1 | **−21,9 / −31,7 / −29,3** |

### Les deux bandes sont complémentaires, pas redondantes

Sur les 150 transcodages, seuil = plancher des authentiques pour chaque bande :

| | fixe | relative | union |
|---|---|---|---|
| opus128 | 0/10 | **10/10** | 10/10 |
| lame320 | 3/10 | **10/10** | 10/10 |
| aac128 | 6/10 | **9/10** | 9/10 |
| lame128 / lame160 / mfmp3_128 | **10 / 10 / 6** | 0 / 0 / 0 | inchangé |
| vorbisq5 / wma192 | 8 / 8 | 8 / 8 | **10 / 9** |
| **total** | **61 %** | 50 % | **77 %** |

Le mécanisme est symétrique et explicable : sur un MP3 128 qui coupe à 16,8 kHz, la bande relative
(17,6-21,6 kHz à 44,1 kHz) tombe **entièrement au-dessus de la coupure**, sur un plancher résiduel
uniforme — donc parfaitement plat, donc « rien à signaler ». Chaque bande est aveugle là où l'autre
voit. Un test l'épingle, mesuré par mutation : rendre les deux bandes identiques le fait tomber.

### Bilan des trois signaux

| | détection sur 150 transcodages | angle mort |
|---|---|---|
| coupure spectrale (Sift **et** FTF) | 27 % | AAC, 320, V0, Opus, Vorbis, WMA |
| + platitude bande fixe | 61 % | Opus, une partie de l'AAC |
| + platitude bande relative | **77 %** | ce qui reste n'est plus une famille entière |

Faux positifs : **0** sur 44 authentiques pour la bande fixe, 0 sur 20 pour la relative — par
construction, les seuils étant posés au plancher observé.

### Ce que ça n'établit pas

- La référence de la bande **relative** ne s'appuie que sur **20 fichiers** (contre 44 pour la
  fixe), et l'un d'eux tire son plancher à −10,9. C'est pourquoi cette mesure vit dans les détails
  techniques et pas dans les lignes principales.
- Les 23 % restants n'ont pas été caractérisés : on sait combien passent, pas lesquels ni pourquoi.
- Aucun transcodage en chaîne, toujours.

## CORRECTION — le 77 % était payé par des faux positifs non mesurés (2026-08-18)

Les chiffres de la section précédente reposaient sur un seuil de bande relative à **−10,9 dB**,
tiré des **10** authentiques du corpus. Le jeu authentique a été élargi à **32 fichiers** de trois
familles musicales, et le seuil honnête — celui qui ne déclenche sur aucun d'eux — tombe à
**−23,8 dB**.

**Ce qui le fixe : deux morceaux ambient.** Nova Tekk, *Chill Out Vol. 6* : −23,8 et −21,9 sur la
bande relative, alors qu'ils sont parfaitement normaux sur la bande fixe (−3,1 et −2,9). **Le haut
du spectre d'un master ambient est légitimement clairsemé.** Avec l'ancienne borne, ces deux
fichiers achetés auraient été annoncés « sous la plage » — un faux positif sur du matériel
authentique, dans un affichage censé ne rien accuser.

| | seuils 10 authentiques (−5,4 / −10,9) | **seuils 32 authentiques (−5,3 / −23,8)** |
|---|---|---|
| bande fixe seule | 91/150 = 61 % | 94/150 = 63 % |
| bande relative seule | 75/150 = 50 % | **25/150 = 17 %** |
| **union** | **116/150 = 77 %** | **102/150 = 68 %** |

Par encodeur, aux seuils honnêtes :

| variante | fixe | relative | union |
|---|---|---|---|
| opus128 | 0/10 | **6/10** | 6/10 |
| wma192 | 8/10 | 7/10 | **9/10** |
| lame192 / lame256 | 10/10 | 5/10 | 10/10 |
| lame128 / lame160 | 10/10 | 0/10 | 10/10 |
| aac256 / aacmf256 | 2/10 | 0/10 | 2/10 |
| lame320 | 4/10 | 0/10 | 4/10 |

**La bande relative garde une utilité mais bien moindre qu'annoncé** : elle reste la seule à voir
Opus (0 → 6/10) et elle ajoute wma192. Elle ne « ferme » pas l'angle mort Opus, elle l'entame.

**Ce que cet épisode dit de la méthode**, et c'est la vraie leçon : le 77 % n'était pas une erreur
de calcul, c'était un chiffre mesuré sur un jeu authentique trop étroit. Un seuil posé au plancher
de 10 fichiers d'une seule famille musicale n'est pas un seuil, c'est une propriété de
l'échantillon. Les deux morceaux ambient qui l'ont démenti étaient dans le lot fourni par Antoine
depuis le début — ils n'avaient simplement jamais été passés sur la bande relative.

## Ce qu'on rate encore, caractérisé (2026-08-18)

⚠️ **Chiffres refaits le 2026-08-18 par le chemin Rust**, au plancher honnête de −5,8 (voir la
section suivante). La version précédente de ce tableau annonçait 34 ratés à partir de mesures de la
sonde JS : elle comparait un détecteur à des seuils qui n'étaient pas les siens.

Les **52 transcodages (34,7 %)** que l'union des deux bandes laisse passer :

| variante | ratés | distance au seuil le plus proche |
|---|---|---|
| aac256 | 9/10 | 0,07 à 3,15 dB |
| aacmf256 | 9/10 | 0,38 à 3,08 dB |
| lame320 | 8/10 | 0,08 à 2,63 dB |
| aacmf128 | 7/10 | 0,07 à 3,11 dB |
| lameV0 | 5/10 | 0,76 à 2,99 dB |
| mfmp3_128 | 4/10 | 0,01 à 2,64 dB |
| aac128 | 3/10 | 0,12 à 1,59 dB |
| opus128 | 3/10 | 0,24 à 2,59 dB |
| lame128, mfmp3_320, vorbisq5, wma192 | 1 chacun | 0,64 à 2,91 dB |

**Aucun n'est hors de portée.** 16 sur 52 sont à moins d'1 dB d'un seuil, 35 à moins de 2, et
**aucun au-delà de 3,15 dB**. Ce n'est pas un trou de capacité mais une marge — et 25 des 52 sont
de l'AAC, qui reste le noyau dur.

Se rejoue par :

```
SIFT_CORPUS_DIR=C:\sift-corpus\fake cargo test --manifest-path src-tauri/Cargo.toml --release \
  corpus_scan -- --ignored --nocapture > scan-fake.txt
node scripts/score-corpus.mjs C:\sift-corpus\labels.json scan.csv
```

### Une meilleure règle de décision ne suffit pas — mesuré, cinq règles testées

Les seuils sont déjà posés au plancher des authentiques : les baisser créerait des faux positifs.
L'espoir restant était que les deux mesures, **prises ensemble**, séparent là où chacune seule
échoue. Testé :

Refait par le chemin Rust le 2026-08-18 (`scripts/score-corpus.mjs`, seuils lus dans `verdict.rs`) :

| règle | détection | faux positifs |
|---|---|---|
| **OU des deux seuils (actuel)** | **65,3 %** | **0/10** |
| somme des deux axes < min authentique | 70,0 % | 0/10 |
| OU des trois (seuils + somme) | 78,0 % | 0/10 |
| z-score minimal < min authentique | 75,3 % | 0/10 |
| les deux sous la médiane authentique | 56,0 % | **4/10** |
| OU des seuils, OU les deux sous la médiane | 80,7 % | **4/10** |

**Ce tableau ne dit PAS que trois règles battent le OU actuel**, et la version précédente le lisait
mal en concluant l'inverse avec la même sérénité. Les règles 2 à 6 tirent leur référence des **10
authentiques qu'on est en train de scorer** : leur colonne « faux positifs » est nulle *par
construction*, pas par mesure. Le OU actuel, lui, applique un plancher venu d'un autre jeu de
fichiers — c'est la seule ligne dont le 0/10 soit une mesure. Comparer les six colonnes revient à
comparer une note d'examen à une note qu'on s'est donnée soi-même.

Ce que le tableau dit vraiment : **une règle calibrée sur les fichiers qu'elle juge gagne 13
points**, et c'est la mesure de ce qu'un seuil auto-référentiel s'offre gratuitement. Les deux
lignes à 4/10 sont les seules dont la référence ne suffit pas à se protéger — elles échouent même
à leur propre examen.

Conclusion soutenable, inchangée : dans cet espace à deux dimensions, la population authentique et
les transcodages ratés **se chevauchent réellement**. Il ne manque pas une meilleure règle, il
manque une **troisième mesure indépendante**.

## Le troisième signal : ce que dit la littérature, et pourquoi notre approximation échoue

L'AAC à débit élevé reste le noyau dur (aac256 et aacmf256 à 2/10). La piste classique en
forensique audio est la **structure de quantification MDCT** : un encodeur met à zéro des bandes
entières de coefficients, et cette structure survit au décodage.

La littérature publique le confirme et donne la forme de la solution — notamment
[*Detection of Genuine Lossless Audio Files: Application to the MPEG-AAC Codec*](https://www.researchgate.net/publication/331400801_Detection_of_Genuine_Lossless_Audio_Files_Application_to_the_MPEG-AAC_Codec),
qui distingue un lossless authentique d'un ré-encodage **sans apprentissage automatique**, par
détection des erreurs de quantification dans le domaine temps-fréquence ; et
[*AAC Audio Compression Detection Based on QMDCT Coefficient*](https://link.springer.com/chapter/10.1007/978-3-030-00021-9_32),
qui exploite la distribution des coefficients MDCT nuls.

### Trois formulations testées sur notre FFT — toutes réfutées

Bande 12-18 kHz, mesures relatives à la médiane de chaque trame :

1. **fraction de bins très creux** (> 25 dB sous la médiane de leur trame) ;
2. **plus longue plage CONTIGUE** de bins creux — les bandes de facteur d'échelle sont contiguës ;
3. **écart p50−p10** de la distribution en dB, comme indice de bimodalité.

Les trois montrent le bon ordre sur `src01` (authentique 2,31 % → aacmf256 3,95 % → aac256 4,85 %
→ aac128 9,01 %) et **rien du tout** sur `src05` (0,36 % contre 0,46 % et 0,38 %). Les valeurs
absolues sont pilotées par le MATÉRIAU, pas par le codec.

Taux mesuré sur les 10 sources, critère « le faux dépasse de 50 % l'authentique **de la même
source** » :

| | séparé |
|---|---|
| fraction de bins creux, aac256 **et** aacmf256 | **1/10** |

Et ce critère est déjà **trop favorable** : il compare chaque fichier à son propre original, ce
qu'on n'a jamais dans la vraie vie. La source 09 va même dans le mauvais sens (authentique 5,53 %
contre 4,49 % pour le transcodage).

### Ce que ça établit, et ce que ça n'établit pas

**Établi** : une statistique de « trous » calculée sur une FFT Hann 4096 à 50 % de recouvrement ne
sépare pas l'AAC haut débit d'un master. Inutile d'y revenir sous cette forme.

**Non établi, et c'est important** : que l'idée soit fausse. La cause probable est que notre FFT
**n'est pas la transformée du codec**. L'AAC quantifie dans une MDCT à fenêtres 2048/256 avec
commutation, à un décalage de trame inconnu ; une FFT d'une autre base, d'une autre fenêtre et d'un
autre alignement étale cette structure jusqu'à l'effacer. Les méthodes publiées reproduisent la
transformée du codec **et cherchent le décalage de trame**.

C'est donc un chantier d'implémentation — MDCT, commutation de fenêtres, recherche d'alignement —
et pas une sonde. Il n'a pas été entrepris, et rien ici ne dit qu'il aboutirait.

## Le seuil et son juge n'étaient pas mesurés par le même code (2026-08-18)

Tous les taux publiés plus haut sur les bandes de platitude — 61 %, 63 %, 17 %, 68 %, 77 % —
venaient de `scripts/hf-flatness-probe.mjs` : décodage forcé en **mono 44,1 kHz**, DFT naïve, 200
trames échantillonnées, 150 s au plus. Le code qui analyse un fichier dans l'app mesure au **taux
natif**, sur tout le fichier, par la FFT de `spectrum.rs`. Personne n'avait comparé les deux.

`corpus_scan` imprime maintenant les deux colonnes de platitude, donc le corpus se mesure enfin par
le chemin qui juge. Les 10 authentiques :

| | sonde JS (publié) | chemin Rust (mesuré) |
|---|---|---|
| plage bande fixe | −5,4 à −2,6 | **−5,79** à −2,63 |

**Un seul fichier fait toute la différence, et c'est un achat** : `src09`
(*Paco & The Julia Set — The Deep Wire*, .wav) mesure −5,79 par le chemin Rust, la sonde le situait
à −5,4. Au seuil livré de −5,4, ce fichier acheté basculait en **Douteux**.

Ça touche aussi une conclusion de méthode publiée plus haut : la « distribution bimodale » avec un
vide entre −6,5 et −5,4. Le vide n'est vide que pour la sonde ; par le chemin Rust, `src09` tombe
dedans. **L'amas de trois fichiers à la borne basse (−5,4 / −5,3 / −5,3) reste à re-mesurer** — il
n'a jamais été passé par le juge.

### Ce que coûte le plancher, mesuré

| plancher bande fixe | union détecte | authentiques touchés |
|---|---|---|
| −5,0 | 110/150 | 2/10 |
| **−5,4** (livré jusqu'ici) | 105/150 | **1/10** |
| −5,6 | 104/150 | 1/10 |
| **−5,8** (retenu) | **98/150** | **0/10** |
| −6,0 | 96/150 | 0/10 |

Sept détections payées pour zéro faux positif. Le dépôt a une contrainte permanente à zéro faux
positif ; elle tranche. `HF_FIXED_FLOOR_DB = -5.8` — arrondi vers le bas depuis −5,79, parce
qu'une borne posée sur la valeur exacte d'un fichier réel dépend de son troisième chiffre.

⚠️ **Ce plancher repose sur 10 fichiers**, et c'est le défaut que ce chantier a déjà corrigé une
fois. Ce qui le lèverait : repasser les 44 authentiques de la référence élargie par le chemin
Rust. Ils ne sont pas sur cette machine.

### La platitude entre dans le verdict — Douteux, pas Faux

`verdict()` lit désormais les deux bandes, mais **seulement quand la coupure n'a plus rien à dire**
(bande pleine, ≥ 20 kHz). Elle ne dégrade jamais un verdict déjà négatif ; elle rattrape ce que la
falaise ne peut pas voir. Le résultat est **Douteux**, jamais Faux : la plage de référence tient sur
44 fichiers dont deux ambient s'approchent légitimement du plancher, et accuser sur cette base
produirait des faux positifs sur du matériel acheté.

Matrice complète, corpus entier, seuils du juge :

| | Ok | Douteux | Faux | |
|---|---|---|---|---|
| authentique (10) | **10** | 0 | 0 | 0 faux positif |
| faux (150) | 47 | 62 | 41 | **31,3 % de faux négatifs** |

Contre la ligne de base du 2026-08-17 (102 Ok / 7 Douteux / 40 Faux, soit 68 % de faux négatifs) :
**les faux négatifs passent de 102 à 47.** Le nombre d'accusations, lui, ne bouge pas (40 → 41) —
tout le gain est en zone grise, ce qui est exactement le contrat : Sift signale davantage sans
accuser davantage.

Angles morts restants, tous en AAC ou en MP3 haut débit : `aac256` et `aacmf256` 9/10 ratés,
`lame320` 8/10, `aacmf128` 7/10, `lameV0` 5/10.

## La grille du codec se retrouve — mécanisme établi, détecteur non (2026-08-18)

Reprise de la piste MDCT, celle que la section précédente laissait comme « chantier
d'implémentation ». `src-tauri/src/analysis/mdct.rs` porte la transformée (fenêtre sinus, blocs
longs AAC : 2048 échantillons → 1024 coefficients) et une sonde d'alignement.

### Ce qui change par rapport aux trois formulations réfutées

Elles mesuraient une sparsité **absolue**, et la conclusion était que « les valeurs absolues sont
pilotées par le matériau, pas par le codec ». La sonde mesure un **contraste** : fraction de
coefficients creux au meilleur décalage de trame, divisée par la même fraction au décalage médian.

Le raisonnement, et il est vérifiable indépendamment du taux : un master n'a aucune raison d'avoir
un alignement privilégié, quel que soit son matériau. Un fichier passé par un encodeur AAC a été
quantifié sur UNE grille. Le rapport élimine le niveau absolu, donc le matériau.

### Le résultat qui compte n'est pas le taux, c'est le décalage retenu

L'entrée est décalée de **17 échantillons** avant analyse (`SIFT_MDCT_SKIP`), pour que
l'alignement vrai ne soit pas celui de nos fichiers fabriqués depuis l'échantillon 0. L'alignement
vrai devient donc 1024 − 17 = **1007**.

Décalage retenu sur les 20 faux (10 sources × aac256, aacmf256) :

```
22 1007 154 17 792 592 599 1007 776 1007 91 1007 1007 1007 1007 1007 122 161 1007 1007
```

**1007 exactement, sur 10 des 20.** Les 10 authentiques, eux, pointent n'importe où : 70, 217,
939, 582, 324, 43, 942, 1014, 56, 713.

La grille de quantification du codec est retrouvée **à l'échantillon près**, sur la moitié des
faux, et jamais sur un master. C'est une preuve de mécanisme plus forte qu'un taux de détection :
un artefact de protocole ne tombe pas sur le seul décalage qui a un sens physique.

### Comme détecteur, ce n'est pas livrable

| | authentiques | non appariée | appariée |
|---|---|---|---|
| grille de 32, aligné | 1,022–1,048 | 17/20 | 19/20 |
| grille de 32, décalé de 17 | 1,022–1,063 | 5/20 | 12/20 |
| pas de 1, décalé de 17 | 0,988–1,056 | **10/20** | 16/20 |

Trois choses à lire, et deux sont des mises en garde :

1. **Le pic fait moins de ±16 échantillons de large.** La ligne 1 contre la ligne 2 le dit : sur
   une grille grossière, le taux s'effondre dès que le fichier est rogné. Le 17/20 de la première
   ligne était une propriété de la façon dont on fabrique le corpus, pas du signal.
2. **Le seuil est posé au maximum des 10 authentiques** — auto-référentiel, exactement le défaut
   corrigé plus haut sur la platitude. Le 0/10 de faux positifs est une construction.
3. Le balayage au pas de 1 rattrape (5/20 → 10/20) sans revenir au niveau aligné, parce qu'un
   maximum sur 1024 candidats est gonflé **pour les deux populations** — le plus haut authentique
   monte de 1,048 à 1,056.

### Le levier suivant, nommé par la mesure

Le maximum sur les décalages est une statistique bruitée, et le repérage ne dispose que de 8
trames. Ce que le tableau des décalages suggère est meilleur : **faire voter les trames**. Un
fichier AAC doit voir toutes ses trames désigner le MÊME décalage ; un master doit les voir se
disperser. C'est une mesure d'accord, insensible au niveau absolu comme au gonflement du maximum.

Rien de tout ça n'est branché sur `verdict()`, et le module le dit en tête.

## Étape 3 — le cross-test Fakin' The Funk

Pas encore fait. Sa valeur a changé : avant, il aurait servi de second avis sur un détecteur dont on
ignorait la qualité. Maintenant, la question est précise et bien meilleure — **est-ce que FTF
détecte les 47 que Sift laisse en Ok ?**

- S'il les détecte, sa méthode ne repose pas sur la position d'une coupure, et il vaut la peine de
  comprendre laquelle.
- S'il les rate aussi, le problème est dur pour tout le monde, et ça change ce qu'on peut
  raisonnablement promettre à un DJ.

Dans les deux cas c'est le corpus étiqueté qui sert de juge, pas l'accord entre deux logiciels : la
vérité terrain est de notre côté maintenant.

Le corpus se régénère par `node scripts/make-corpus.mjs <dossier-source> <dossier-sortie>`.

## Audit de provenance des 10 authentiques (2026-09-11)

Déclencheur : Antoine ne reconnaissait pas « Mezmotized » (301 kbps en FLAC, quasi mono) et n'a
pas accès à son historique Beatport pour trancher. Les originaux ont été retrouvés sur disque et
inspectés au conteneur (chunks, tags, taille des données PCM contre la durée) — pas au signal, qui
est justement ce que la référence sert à juger.

| fichier | conteneur | verdict de provenance |
|---|---|---|
| src01 Brix 'n' Mortar (`D:\MUSIQUE`) | AIFF, `encoded_by = Beatport`, ISRC | intact |
| src02..src08 (7 AIFF, `D:\sift-backup-djermusique\Contents`) | `Encoded by Beatport` strict, `COMM` puis `SSND`, PCM exact à l'octet, pochette 500×500 | intacts |
| src09 Paco & The Julia Set, The Deep Wire (.wav) | chunk `LIST INFO` avec `ISFT = Lavf58.20.100`, tags Beatport recopiés en champs INFO, pas de chunk `id3 ` | **réécrit par ffmpeg 4.1 après l'achat**, origine indéterminable (AIFF Beatport ou MP3 Beatport) |
| src10 Peter Munch, Golden Pieces In Space | original absent de tous les disques ; la copie FLAC du corpus porte les tags Beatport mais `make-corpus.mjs` ne recopie pas `encoded_by` | **non vérifiable** |

Faits utiles au passage : Beatport écrit `TFLT = MPG/3` sur ses AIFF (9/9 ici), ce cadre n'indique
donc PAS une origine MP3 ; le commentaire « Purchased at Beatport » voyage avec les partages et ne
vaut rien seul ; le débit d'un AIFF/WAV est toujours 1 411 kbps (PCM) et celui d'un FLAC dépend du
mixage (301 kbps pour un quasi-mono authentique, corrélation des canaux 0,998).

Décision d'Antoine : garder les 8 conteneurs intacts, sortir src09 et src10 de la référence
(`labels.json` réécrit, sauvegarde `labels.2026-08-18.avant-audit-provenance.json`). Leurs 30
transcodages restent des faux : la transformation est connue, même si la source ne l'est plus.

Ce que ça change : src09 était le fichier à −5,79 de platitude qui avait fait reculer le seuil le
2026-08-18 (`HF_REF_LO`, aujourd'hui une borne d'affichage seulement) ; le plancher livré (−12,
8ac3a23) vient d'une autre référence (23 sources ACID assainies + 114) et n'est pas touché. Les
« 0/10 faux positifs » deviennent 0/8.

Matrice du détecteur livré (scan du 2026-09-10, binaire courant, 8 authentiques / 150 faux) :
authentiques 8 Ok / 0 Grey / 0 Fake ; faux 59 Ok / 26 Grey / 65 Fake. Par famille : MP3 128-256
49/50 Fake ; LAME 320 et V0 20/20 **Ok** ; AAC 256 5/10 et 9/10 Fake ; AAC 128 1/10 et 1/10 ;
MediaFoundation 320, Opus, Vorbis, WMA : Grey ou Ok, jamais Fake. Le cross-test FTF (étape 3)
n'est toujours pas fait ; Antoine a FTF.

## La fenêtre LAME 320 (2026-09-11) — ce que la référence ACID contient entre 20 000 et 20 750 Hz

Décision d'Antoine (« go pour les deux ») : rouvrir une fenêtre entre la falaise (20 000) et la bande
pleine, en `Grey`, pour attraper LAME 320 (coupure 20 177-20 704 sur les 20 fichiers du corpus,
20/20 jugés `Ok` en v0.1.1). Avant de figer la borne haute à 20 750, scan de `D:\MUSIQUE\ACID`
(546 lossless déclarés + 33 lossy) avec le binaire v0.1.1 :

| coupure | lossless ACID |
|---|---|
| ≤ 20 000 (Faux) | 9 |
| 20 000-20 750 (la fenêtre) | **21** |
| 20 750-21 999 | 28 |
| ≥ 22 000 | 488 |

Les 21 de la fenêtre, mesurés au spectre moyen (chute d'énergie entre 500 Hz sous la coupure et
500 Hz au-dessus ; témoins : `lame320` du corpus à 41,5 et 48,8 dB) : **19 ont un mur d'au moins
20 dB** (de 20,1 à 53,3 dB — les cinq titres de l'EP « Occibel - TKL CONFINED EDITION » entre 42 et
53, « Bassam - Slave To The Rave » en .aif ET en .wav à 43,6), 2 sont intermédiaires (12,8 et
15,7 dB), aucun n'est un roll-off doux. Ce sont très probablement des MP3 320 réencapsulés, dans un
dossier tenu pour sûr — la famille exacte que le détecteur v0.1.1 ne voyait pas, et que le
2026-09-01 n'avait pas relue (l'assainissement portait sur les murs de 19,4-20,4 kHz).

Coût de la fenêtre côté authentiques : aucun connu. Les 8 achats vérifiés sont à 22 050 ; les
28 fichiers ACID entre 20 750 et 22 000 ne sont pas touchés. Borne gelée à 20 750 (46 Hz au-dessus
du maximum mesuré). Réserve : le témoin « roll-off ACID » utilisé ici (trois fichiers à 21,4 kHz)
chute aussi de 25 à 38 dB à ±500 Hz — la pente seule ne sépare pas parfaitement mur et roll-off
raide ; c'est pour ça que la fenêtre rend `Grey` et non `Fake`.

Matrice mesurée avec le binaire à fenêtre (re-scan complet du corpus, 2026-09-11, 8 authentiques /
150 faux) : authentiques 8 Ok / 0 Grey / 0 Fake ; faux **45 Ok (30 %) / 40 Grey / 65 Fake**. Contre
v0.1.1 : 59 → 45 Ok, 26 → 40 Grey, Fake inchangé à 65. Par famille, ce qui bouge : lame320 10/10
Grey (était 10/10 Ok), opus128 9 Grey / 1 Ok (était 7 / 3), wma192 10 Grey (était 8 / 2). Identique
à l'estimation faite depuis le scan v0.1.1 avec la règle rejouée en Python : la règle est bien celle
qu'on croyait.

## Le banc MP3 (#63) — première mesure (2026-09-11)

`analysis/mp3_bank.rs` : dual exact de la chaîne de synthèse de symphonia (papillons d'aliasing
inversés, MDCT 36 fenêtrée, inversion de fréquence, analyse polyphase `C = D/32`), puis
`quant_trace::frame_likelihood` sur les 576 coefficients avec les bandes B.8, fenêtre sfb 13-20.
Reconstruction analyse→synthèse vérifiée (retard 481, gain 1, résidu −84 dB ; `|D|/32` sans les
signes rend une corrélation de 0,17, donc les signes de la table comptent). Balayage 32 phases de
sous-bande × 18 phases de granule, 0,6 s par fichier.

`L_mp3` seul, corpus complet (`mp3_scan`, réglage 8×8, λ = 0,18) :

| famille | min | médiane | max | > λ |
|---|---|---|---|---|
| authentiques vérifiés (8) | 0,062 | 0,078 | **0,094** | 0/8 |
| lame320 | 0,531 | 0,922 | 0,984 | 10/10 |
| lameV0 | 0,641 | 0,875 | 0,969 | 10/10 |
| lame256 / 192 / 160 | 0,578 / 0,453 / 0,203 | 0,844 / 0,609 / 0,375 | 1,000 / 0,844 / 0,641 | 10/10 chacune |
| lame128 | 0,109 | 0,188 | 0,359 | 7/10 |
| mfmp3_320 / mfmp3_128 | 0,484 / 0,188 | 0,922 / 0,453 | 1,000 / 0,688 | 10/10 chacune |
| aacmf256 | 0,078 | 0,219 | 0,234 | 6/10 (des faux quand même) |
| aac128, aac256, aacmf128, opus, vorbis, wma | 0,062 | 0,078 | ≤ 0,219 | 0-1/10 |

**`src09_genuine` (The Deep Wire, le WAV réécrit par ffmpeg, sorti de la référence le matin même
sur le seul conteneur) : `L_mp3 = 0,797`, décalage 545, canal G.** Le signal confirme ce que le
conteneur suggérait : c'est un MP3 transcodé. La référence de 10 en contenait un, et le banc AAC ne
le voyait pas. Les 8 vérifiés au conteneur plafonnent à 0,094, 1,9× sous λ.

Intégration : `analyze()` prend le maximum des deux bancs comme `quant_likelihood`, même λ.
`REPORT_CACHE_VERSION` 10 → 11 (reprise de la bibliothèque rangée par le pool, #59).

### Le banc MP3 sur ACID, croisé avec la coupure (2026-09-11)

`mp3_scan` sur les 546 lossless déclarés d'ACID (dossier MIXTE : achats et téléchargements, Antoine
le redit — il ne borne PAS les faux positifs) : **11 fichiers portent une grille MP3** (2,0 %) :
1 sous la falaise, 5 dans la fenêtre 20 000-20 750, 5 à bande pleine (22 050 Hz, invisibles à la
coupure : le cas V0). Dans la fenêtre, sur les 21 fichiers, 5 portent la grille (Innershades ×2 à
1,000 et 0,781, « Sans Bateaux » 0,953, « rhythm invention » 0,812, « Babyloop » 0,641) et **16 ne
la portent pas** (L 0,078-0,094) tout en ayant un mur de 20 à 53 dB à 20,4-20,7 kHz — les cinq
Occibel, Bassam, Bluefish, Daïf, Severed Heads, Acid Jerks… Le banc AAC ne les voit pas non plus.

Deux lectures possibles, non départagées : un master avec un passe-bas raide de mastering (ça
existe : suréchantillonnage de limiteur, EQ de fin de chaîne), ou un transcodage dont la chaîne a
cassé l'alignement de grille (ré-échantillonnage, normalisation avec écrêtage). C'est exactement
pour ça que la fenêtre rend `Grey` et pas `Fake` : le mur seul ne suffit pas, et le banc, lui,
tranche quand il trouve la grille. Le corpus, fabriqué proprement (décodage → FLAC, sans gain ni
ré-échantillonnage), ne reproduit pas ce cas ; à fabriquer un jour (mp3 → gain → AIFF) pour
mesurer ce que la grille supporte.

### Le banc MP3 sur 411 lossless taggés magasin (2026-09-11)

Référence construite par marqueurs de magasin sur toute la bibliothèque (`store-markers.csv`) :
185 Bandcamp, 219 « Purchased at Beatport » en commentaire, 7 Beatport strict (encoded_by +
ordre COMM/SSND + taille PCM exacte). Un tag de magasin dit d'où vient le morceau, pas le
format acheté ni ce qu'il a subi depuis — la référence borne donc le taux de faux positifs par
le HAUT, pas par le bas.

`mp3_scan` (`mp3-ref.log`) : **12 / 411 au-dessus de λ = 0,18 (2,9 %)**, tous à L ≥ 0,50 ;
392 sous 0,10, 7 entre 0,10 et 0,18, **aucun entre 0,18 et 0,50**. La mesure est bimodale :
la grille est là ou n'y est pas, λ ne coupe pas dans une pente. Les 7 strict sont tous sous
0,10. Les 12 sont 9 titres (Deep Wire, Klank, Can I Eat en double) : 9 en commentaire
Beatport, 3 Bandcamp — dont Deep Wire, déjà identifié comme transcodage (WAV réécrit par
ffmpeg, retiré de la référence authentique le 2026-09-10). Le commentaire « Purchased at
Beatport » voyage avec les partages : ces 9 ne sont pas une preuve d'achat.

Détecteur intégré (`corpus_scan`, `corpus-ref.log`) sur les mêmes 411 : **382 Ok (93 %),
19 Fake, 10 Grey**. Les 19 Fake se partagent en deux causes nettes : 9 par coupure sous
20 000 Hz (15,4 à 19,9 kHz — Bakked « Structure EP » ×3 à 18,4 kHz, Toothpick à 15,4 kHz,
Lonewolf, Habersham, Aquanauts, Spectrum : un master lossy mis en vente tel quel, Bandcamp et
Beatport publient ce que le label envoie) et 10 par grille MP3 (les 9 titres ci-dessus, dont
Jay Tripwire n° 7 coupé à 20 833 Hz — AU-DESSUS de la fenêtre, invisible à la coupure — et
Aphasia, Deep Wire, Can I Eat à bande pleine). Dans la fenêtre 20 000-20 750 : 8 fichiers,
4 Fake par la grille, 4 Grey sans grille (Supreme ×2, 3rd Hustle, Magic Tonight). Les 6 Grey
restants sont à 22 050 Hz par la platitude sous le plancher master (Dav ×5, KOKO.IT) —
règle antérieure au chantier, intacte.

Ce que ça dit : les 7 Beatport strict rendent Ok, et sur les 404 autres — dont rien ne
garantit le format acheté — 3 titres à bande pleine (Aphasia, Can I Eat, Deep Wire) n'ont que
la grille pour anomalie. Deep Wire est déjà connu (WAV réécrit par ffmpeg). Aphasia et Can I
Eat sont les deux seuls cas où le banc seul décide sur cette référence : faux positif ou
transcodage V0, on ne peut pas trancher sans le fichier d'origine — Antoine a reconnu ne pas
avoir acheté tout ce qui porte un tag de magasin. Borne haute du taux de faux positifs du banc
sur du lossless taggé magasin : 2 / 411 (0,5 %), à condition que ces deux-là soient
authentiques, ce qui n'est pas établi.

### Matrice v4 — fenêtre + banc MP3 intégré (2026-09-11, `corpus-scan-v3.csv`)

`node scripts/score-corpus.mjs C:/sift-corpus/labels.json corpus-scan-v3.csv`, 8 authentiques :

| vérité \ verdict | Ok | Grey | Fake |
|---|---|---|---|
| authentique (8) | **8** | 0 | 0 |
| faux (150) | 31 | 27 | **92** |

Contre la matrice v3 (fenêtre seule, 2bc7044) : faux Ok 45 → **31**, Grey 40 → **27**, Fake
65 → **92**. Par famille : lame160/192/256, mfmp3_128 10/10 ; lame320, lameV0, lame128,
mfmp3_320 9/10 ; aacmf256 9/10 ; aac256 5/10 ; aac128 et aacmf128 1/10 ; opus, vorbis,
wma 0 Fake (Grey 9, 3, 10). Le raté de chaque famille MP3 est **le même fichier, src08** : 10 min
53 s stéréo 44,1 kHz, au-delà du plafond de rétention PCM de 9,07 min (`QUANT_MAX_PCM_SAMPLES`,
#52) — les deux bancs rendent `None`, le verdict retombe sur la coupure. Ce n'est pas le banc
qui rate, c'est la sonde qui n'a pas de signal ; le banc seul (`mp3-corpus.log`) donne 0,953
sur `src08_lame320.flac`. Lever le plafond (sonder les 9 premières minutes au lieu de renoncer)
est le prochain gain le moins cher : il rend 3 Fake de plus sans toucher au seuil.

Ce qui reste hors de portée après #63 : les AAC à bas et moyen débit (aac128, aacmf128 : 1/10)
et les trois codecs sans banc (Opus, Vorbis, WMA), qui restent Grey par la platitude quand ils
le sont. Ce sont les 31 Ok restants, à 3 exceptions près (src08).

### Le plafond levé : src08 re-sondé sur ses 9 premières minutes (2026-09-11, `corpus-scan-v4.csv`)

La rétention ne renonce plus au-delà de `QUANT_MAX_PCM_SAMPLES`, elle garde le début
(`retenir_pour_sonde`, coupe sur une trame entière). Re-scan des 16 fichiers src08 seuls
(`corpus-src08.log`), fusionné dans la matrice v3 :

| vérité \ verdict | Ok | Grey | Fake |
|---|---|---|---|
| authentique (8) | **8** | 0 | 0 |
| faux (150) | 27 | 24 | **99** |

src08 rend maintenant lame320 0,938, lameV0 0,813, mfmp3_320 0,844, lame128 0,188 — et, gain non
prévu, aac256 0,203, aacmf128 0,219, aacmf256 0,266 : sept Fake de plus (31 → 27 Ok, 27 → 24
Grey, 92 → 99 Fake). `src08_genuine` reste à 0,078, Ok. **Les huit familles MP3 sont à 10/10**,
aacmf256 aussi. L'ancien motif du renoncement (« une mesure qui n'est plus celle sur laquelle λ a
été calibré ») est réfuté par la mesure : neuf minutes d'un morceau de onze portent la même grille.

Reste : aac128 1/10, aacmf128 2/10, aac256 6/10 ; Opus, Vorbis, WMA sans banc (0 Fake, Grey par
la platitude quand ils le sont).

## Les AAC bas débit (2026-09-11)

État de départ (matrice v4) : aac128 1/10, aacmf128 2/10, aac256 6/10, aacmf256 10/10.

### La fenêtre : ffmpeg encode ses blocs longs en KBD, l'analyse ne regardait qu'en sinus

`aacpsy.c`, modèle `psy_lame_window` (celui par défaut de l'encodeur `aac` de ffmpeg) :
`window_shape = 1` (Kaiser-Bessel dérivée) pour `ONLY_LONG` et `LONG_STOP`, `0` (sinus) pour
`LONG_START` et les huit blocs courts. L'annulation du repliement temporel exige la même fenêtre
des deux côtés : une MDCT sinus d'un flux synthétisé en KBD ne rend PAS les coefficients du codec
(`mdct::tests::une_synthese_kbd_ne_se_reanalyse_quen_kbd`, écart relatif > 1 % contre < 1e-4 en
forme appariée). C'est l'explication du constat de `quant_trace` du 2026-09-02, « la grille des
blocs longs est essentiellement absente après décodage » : elle était regardée à travers la
mauvaise fenêtre.

Mesure, `quant_scan` sur 56 fichiers (40 AAC du corpus, 6 LAME témoins, 10 authentiques dont les
deux retirés), `SIFT_QUANT_SKIP=17`, une résolution et une forme à la fois (moyenne de L / nombre
au-dessus de λ = 0,18) :

| famille | long sinus | long KBD | court sinus | court KBD |
|---|---|---|---|---|
| aac256 | 0,087 / 0 | **0,350 / 8** | 0,228 / 6 | 0,056 / 0 |
| aacmf256 | **0,409 / 10** | 0,258 / 6 | 0,250 / 6 | 0,053 / 0 |
| aac128 | 0,083 / 0 | 0,087 / 0 | 0,113 / 1 | 0,059 / 0 |
| aacmf128 | 0,081 / 0 | 0,080 / 0 | 0,119 / 2 | 0,053 / 0 |
| authentiques (10) | 0,084 / 0 | 0,087 / 0 | 0,062 / 0 | 0,059 / 0 |
| lame320 / lameV0 (6) | ≤ 0,089 / 0 | ≤ 0,089 / 0 | ≤ 0,062 / 0 | ≤ 0,057 / 0 |

Trois faits :

1. **ffmpeg `aac` : 0/10 en long sinus, 8/10 en long KBD**, et neuf fichiers sur dix convergent
   sur le MÊME décalage, 1007 — la signature de grille. La forme était le verrou.
2. **Media Foundation (`aac_mf`) encode ses longs en sinus** : aacmf256 10/10 en sinus (décalages
   154 ×6, 1007 ×4), 6/10 en KBD. Deux encodeurs, deux formes : le balayage doit essayer les deux
   sur les blocs longs, et garder le max, comme il le fait pour les canaux.
3. **Les blocs courts sont en sinus chez les deux** : court KBD ne rend rien (≤ 0,078 partout).
   Inutile d'y dépenser un balayage.

Les 128 kbps restent aveugles dans les quatre configurations. Mais en court sinus, 7 des 10
aac128 convergent quand même sur le décalage 47, avec un L de 0,05 à 0,28 : la grille est là,
ce sont les BANDES qui manquent — diagnostic par bande à suivre (`diagnostic::quant_bandes`).

### Diagnostic par bande : la fenêtre 40-47 était fausse pour tout le monde

`diagnostic::quant_bandes` (décalage forcé, résolution, forme, canal), sur `src02`, long KBD,
décalage 1007, canal M, bandes sous `τ` sur 64 trames :

| bandes | kHz | aac256 | aac128 | authentique |
|---|---|---|---|---|
| 17-32 | 2,1-9,6 | 12-24 | 9-21 | 0-2 |
| 33-44 | 10-17,9 | 21-26 | 0-4 | 0-2 |
| 45-47 | 17,9-20 | 7-15 | 0 | 0-2 |

La grille d'un aac128 vit entre 2 et 10 kHz, celle d'un aac256 entre 2 et 18 kHz. La fenêtre
jugée (40-47, 14,5-20 kHz) tombait là où elle s'éteint — aac256 n'était détecté que par le bord.
Les blocs courts, eux, sont quasi vides sur de la musique stationnaire (0-4/64) : ffmpeg n'en
émet qu'aux transitoires, et « le signal est dans les blocs courts » (2026-09-02) n'était vrai
que parce que les longs étaient regardés en sinus.

Fenêtre large 17-44 (28 bandes, 224 cellules par groupe) plutôt qu'un maximum glissant sur 8 :
plus de cellules resserre la loi nulle, un max sur des fenêtres l'élargirait. `quant_scan` long
seul, saut 17 :

| famille | L min-max | > 0,085 |
|---|---|---|
| authentiques (10) | 0,036-0,045 | 0 |
| LAME 320/V0 témoins (6) | 0,040-0,045 | 0 |
| référence magasin (411) | max 0,076 | 0 (deux à 0,076, cinq > 0,06) |
| ACID lossless (546) | max 0,062 | 0 |
| aac128 | 0,089-0,371 | **10/10**, tous au décalage 1007, KBD |
| aacmf128 | 0,054-0,228 | 7/10, tous en sinus |
| aac256 | 0,357-0,571 | 10/10 |
| aacmf256 | 0,357-0,857 | 10/10 |

`λ_long = 0,085` : au-dessus du maximum des 957 lossless réels (0,076, marge 12 %), sous le
minimum aac128 (0,089). Les blocs courts gardent leur échelle (8 bandes, 64 cellules) et leur
0,18, le banc MP3 son 0,18 : `quant_likelihood` voyage désormais en RAPPORT au seuil de son banc
(`max(L/λ)`, jugé contre 1), parce qu'un max de `L` bruts aurait été dominé par l'échelle la plus
bruyante. `REPORT_CACHE_VERSION` 13.

### Matrice v5 — KBD + fenêtre longue 17-44 + rapport au seuil (2026-09-11, `corpus-scan-v5.csv`)

| vérité \ verdict | Ok | Grey | Fake |
|---|---|---|---|
| authentique (8) | **8** | 0 | 0 |
| faux (150) | 10 | 22 | **118** |

Contre v4 (99 Fake) : faux Ok 27 → **10**, Grey 24 → 22, Fake 99 → **118**. aac128 1 → **10/10**,
aac256 6 → **10/10**, aacmf128 2 → 8/10, aacmf256 10/10 ; les huit familles MP3 restent à 10/10.
Les 10 Ok restants : 2 aacmf128 (0,054 et 0,080 en long, sous 0,085), 1 opus, 7 vorbis. Opus,
Vorbis et WMA restent les trois codecs sans banc (0 Fake, Grey par la platitude quand ils le sont).

Référence magasin (411), détecteur intégré : **382 Ok, 19 Fake, 10 Grey — identique à v4**. Les
10 Fake par grille sont les mêmes dix fichiers, tous par le banc MP3 (rapports 3,1 à 5,6) ; le banc
AAC long n'en ajoute aucun, ni ne retire rien. Zéro faux positif nouveau sur 411 + 546 lossless réels.
