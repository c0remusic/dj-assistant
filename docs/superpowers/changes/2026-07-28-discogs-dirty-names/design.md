# Recherche Discogs sur noms de fichiers sales — design

Date : 2026-07-28
Branche : `perf-mi-fixes` (ou branche dédiée à décider au lancement)
Statut : design, non implémenté

## 1. Le problème, mesuré

Toutes les mesures ci-dessous ont été prises le 2026-07-28 sur la vraie base
`%APPDATA%\com.sift.app\sift.db` (2 714 pistes, 3 923 Mo), en rejouant la logique
exacte de `naming.rs` en Python et en lisant les tags ID3v2/FLAC directement sur
disque (ni mutagen ni tinytag présents sur la machine — lecteur minimal écrit
pour la mesure, scripts sous le scratchpad de session).

**Statut du corpus (NG77)** : ce sont les 2 714 pistes réellement scannées par
Sift chez Antoine, pas un échantillon arbitraire. C'est la population cible,
mais d'un seul utilisateur — les proportions ci-dessous décrivent SA
bibliothèque, pas « les DJ » en général.

| Mesure | Valeur |
|---|---|
| Pistes totales | 2 714 |
| `parse_filename` échoue | 1 355 (49,9 %) |
| … dont tags VIDES (éch. 300) | 79,3 % |
| … dont tags propres qui sauvent le cas | 18,7 % |
| → tombent en branche `(false, None)` | ≈ 1 100 |
| `parse_filename` réussit | 1 359 (50,1 %) |
| … dont artiste = numéro de piste / face vinyle | 435 (32,0 % du bucket) |
| … de ces 435, tags vides (éch. 200) | 84,0 % |
| → envoient `artist="01"` à Discogs | ≈ 365 |
| **Total requêtes structurellement cassées** | **≈ 1 465 / 2 714 (54 %)** |
| Version perdue alors qu'elle est dans le nom | 330 |
| `tracks.duration` renseignée | 2 711 / 2 714 |
| `tracks.fingerprint` renseignée | 22 / 2 714 |

### 1.0 Plafond du nettoyage par chaîne : 99,7 %

Mesure ajoutée après une première conclusion FAUSSE (voir §10, « Croyances
révisées ») : en dégrossissant chaque nom (retrait des crochets, du numéro de
piste, du code de face, des suffixes scène/hash/URL/débit) puis en cherchant un
séparateur exploitable dans le nom OU dans le dossier :

| | Pistes | Part |
|---|---|---|
| Structure artiste/titre récupérable | 2 705 | **99,7 %** |
| … séparateur dans le nom dégrossi | 2 167 | 79,8 % |
| … séparateur seulement dans le dossier | 243 | 9,0 % |
| … pas de séparateur mais titre exploitable | 295 | 10,9 % |
| **Vraiment sans espoir** | **9** | **0,3 %** |

Les 9 : `001_Untitled`, `002_Untitled`, `[DRAGON002] A1..B2`, `B2 - Untitled`,
`10. -ism`, `A8`.

**9 est un MAJORANT, pas un compte exact.** Au moins un de ces cas est un faux
positif du dégrossisseur approximatif utilisé pour l'estimation : `10. -ism` a
été mangé par la règle « suffixe scène `-[a-z]{2,4}$` », qui a pris le titre
`-ism` pour un tag de groupe de release. Le vrai titre est `-ism`. Ce cas est
entré au corpus T1 comme piège explicite. Le compte exact ne sera connu qu'après
T2, mesuré par le corpus et non par une approximation.

« Récupérable » signifie ici qu'une structure artiste/titre plausible SURVIT au
dégrossissement — c'est le plafond de ce qu'un nettoyeur peut viser, pas une
garantie d'extraction correcte. C'est le corpus T1 qui mesurera le réel.

### 1.1 Les trois défauts qui se composent

**D1 — `JUNK_TOKENS` est un portail de rejet, pas un nettoyeur.**
`naming.rs:28-31` liste `"["`, `"]"`, `"_"`, `"320"`, `"rip"`… et `has_junk`
(`naming.rs:34`) teste la présence en SOUS-CHAÎNE. Un seul `[` dans le nom et le
nom entier est jeté. Le seul vrai nettoyeur, `clean_stem` (`naming.rs:145-193`),
ne tourne qu'APRÈS le rejet, et uniquement pour préremplir un champ éditable —
jamais pour construire la requête.

**D2 — La branche d'échec efface l'artiste et jette la version.**
```rust
(false, None) => Canonical {
    artist: String::new(),   // naming.rs:133
    title: clean_stem(stem),
    version: None,           // naming.rs:135
    confidence: Confidence::Yellow,
},
```
Contrairement à la branche `(true, None)` qui, elle, appelle
`extract_version_hint` (`naming.rs:121`). 330 pistes perdent ainsi un
`(Original Mix)` / `(Deep Mix)` / `(luke fair mix)` écrit dans le nom — or la
version pèse ×3 au scoring de tracklist (`discogs.rs:210-215`).

**D3 — Le seul repli dégradé exige un artiste non vide.**
```rust
if (cands.is_empty() || best_primary <= 0)
    && !q.artist.trim().is_empty()      // discogs.rs:405
    && !q.title.trim().is_empty()
```
D2 force `artist=""` exactement dans le cas le plus sale. **Le pire cas est
précisément celui à qui aucun repli n'est accordé.** Il n'existe aucune autre
dégradation : pas de retry sans version, pas de retry par tokens.

### 1.2 La cause racine architecturale

`Canonical` porte DEUX contrats incompatibles :

- **identité affichée et écrite** — préremplit les champs éditables et le badge
  vert/jaune (`ipc_filing.rs:61-75`), sert de base au nom de sortie
  (`render_filename`, `naming.rs:223`) et aux tags écrits (`tag_title`,
  `naming.rs:237`), et de nom de repli dans Écartés (`ecartes.rs:105`) ;
- **requête de recherche** — `ipc_identify.rs:24-33` construit `Query { artist,
  title, version }` directement depuis lui.

Le portail de rejet existe pour protéger le PREMIER contrat : mieux vaut un champ
vide qu'un mauvais nom de fichier écrit sur le disque. C'est défendable. Mais le
second contrat n'a pas les mêmes exigences — une requête est jetable, elle ne
touche jamais le disque — et il paie le prix fort.

**C'est la séparation à faire.** Une requête peut être agressive parce qu'elle ne
coûte rien quand elle se trompe ; une identité écrite doit rester prudente.

## 2. Périmètre retenu

Antoine a tranché le 2026-07-28 : **axes 1 et 3**, dans cet ordre de départ.

- **Axe 1 — nettoyeur + cascade de replis** (cœur du chantier).
- **Axe 3 — signaux gratuits** : dossier parent comme source d'artiste, durée
  comme départage des candidats.

Hors périmètre, explicitement (pas écartés, différés) :

- **Axe 2 — les corrections tapées pilotent la requête** (`identify` ne
  transmet que le `track_id`, `ipc.ts:268`). Reste le contournement du jour :
  cliquer « Appliquer » avant de relancer. Déclencheur de réouverture : dès que
  l'axe 1 est mesuré et qu'il reste un volume significatif de cas où Antoine
  connaît la réponse mais ne peut pas la donner à la recherche.
- **Axe 4 — empreinte acoustique / AcoustID.** Seul chantier capable de toucher
  les noms sans aucune structure — mais ils ne sont que **9 sur 2 714** (§1.0),
  pas 524 comme estimé au premier jet. L'axe perd donc sa justification de
  nécessité structurelle : il ajoute une API externe, un jeton et du calcul sur
  toute la bibliothèque pour 0,3 % des pistes. Déclencheur de réouverture : une
  bibliothèque dont la part de noms sans structure dépasse quelques pour cent,
  ou un besoin de VÉRIFIER une identification plutôt que de la deviner (usage
  différent, non évalué ici).

## 3. Décision structurante : séparer la requête de l'identité

On introduit un type et un module distincts pour ce qui part sur le réseau.

```
naming.rs          — INCHANGÉ dans son contrat : Canonical reste l'identité
                     prudente (affichée, écrite, nommée). Le portail de rejet
                     conserve son rôle protecteur.
search_terms.rs    — NOUVEAU module profond. Interface étroite :
                       pub fn build(input: TermsInput) -> Terms
                     Implémentation riche : nettoyage, minage du dossier,
                     génération de la cascade. Jamais écrit sur disque.
discogs.rs         — consomme la cascade au lieu de son unique retry gardé.
```

### 3.1 Pourquoi un module séparé plutôt qu'assouplir `naming.rs`

Assouplir `is_clean` reviendrait à laisser des noms agressivement nettoyés
atteindre `render_filename` et les tags écrits. Le module séparé donne une
frontière de test unique (`build` est une fonction pure : entrées texte →
cascade), et permet d'être agressif sans risque de contaminer le disque.
Anti-pattern évité : un troisième module d'orchestration par-dessus les deux —
`discogs.rs` appelle `build` directement.

### 3.2 Interface visée

```rust
pub struct TermsInput<'a> {
    pub canonical: &'a Canonical,   // ce que reconcile a déjà décidé
    pub stem: &'a str,              // nom de fichier sans extension
    pub folder: &'a str,            // nom du dossier parent (jamais le chemin complet)
    pub duration_s: Option<f64>,    // tracks.duration, pour le départage
}

pub struct Terms {
    pub artist: String,             // JAMAIS vide si un signal quelconque en porte un
    pub title: String,
    pub version: Option<String>,
    pub duration_s: Option<f64>,
    pub ladder: Vec<Attempt>,       // du plus spécifique au plus dégradé
}

pub struct Attempt {
    pub q: String,                  // la chaîne envoyée en `q=`
    pub label: &'static str,        // pour le log et les tests
}
```

`Terms.ladder` est calculée en une fois, sans I/O. `discogs.rs` itère dessus et
s'arrête au premier essai qui rend un score de tracklist > 0.

## 4. Ce que le nettoyeur doit traiter

Chaque motif ci-dessous a été observé dans la vraie bibliothèque (échantillons
dans les mesures de §1). Ce sont les cas de test à figer, pas une liste
spéculative.

| Motif | Exemple réel observé | Traitement |
|---|---|---|
| Séparateur `_-_` | `01_dj_hal_and_jay_thomas_-_dont_stop_(tony_thomas_remix)` | souligné → espace AVANT de chercher le séparateur |
| Séparateur `--` | `02-maetrik--force_feeling_(decomposed_subsonic_remix)-dh` | reconnu comme séparateur |
| Séparateur sans espaces | `A1-Good Fortune (DJ Hal's Lunar Love Mix)` | code de face vinyle détecté puis retiré |
| Tiret demi-cadratin | `Ashtar Afterhours – Body Music` | normalisé en `-` |
| Numéro de piste en tête | `01 Awaken Abyss`, `01-01 Snares Snare`, `001_Untitled` | retiré, 1 à 3 chiffres + séparateur |
| Code de face vinyle | `A1.`, `B2`, `C1-`, `D2` | retiré en tête, JAMAIS pris pour l'artiste |
| Crochets catalogue | `[FAR11 - 2005]`, `[12GOTEL002]`, `[AGE 302]` | retiré (aujourd'hui : rejette tout le nom) |
| Crochets bruit | `[Free Download]`, `[DJ Uploader]`, `[original]` | retiré |
| Débit accolé | `-320kbps-`, `[320]`, `320k`, `V0` | retiré (aujourd'hui : filtre par mot EXACT, `naming.rs:189`, donc rate tout) |
| URL | `www.somesite.com` | retiré (aujourd'hui : absent de `DROP`, finit dans la requête) |
| Suffixe de groupe de release | `-ccat`, `-idc`, `-sq`, `-dh`, `-dL`, `-DEFENESTRATE` | retiré en fin de chaîne |
| Hash hexadécimal | `-7d468690`, `-bcc65623` | retiré |
| `feat.` / `ft.` | non traité nulle part aujourd'hui (`naming.rs:415` assert l'inverse) | conservé dans le titre, retiré de l'artiste pour un essai dégradé |
| Accents | `Béatrice`, `Déjà vu` | `fold_char` existe déjà (`naming.rs:243`) mais n'est appelé que par `dedup.rs:277` — le réutiliser pour un essai dégradé |

## 5. Le dossier parent comme source d'artiste

Mesuré sur les 1 355 noms qui échouent : 91 dossiers distincts, médiane 4 pistes
par dossier.

| Forme du dossier | Part | Exemple réel |
|---|---|---|
| contient ` - ` | 11,8 % | `1996 - The Way - The Deep (EP)` |
| sans ` - ` mais rejeté par `has_junk` | 64,6 % | `Floppy_Sounds-Downtime-(SLIPCD40)-2CD-FLAC-1995-dL` |
| champ unique | 23,6 % | `(SOMA 21) Slam-Snapshots` |

Les 64,6 % « junky » ne sont pas pauvres : ils sont riches et rejetés par le même
portail que les noms de fichiers. `Floppy_Sounds-Downtime-(SLIPCD40)-2CD-FLAC-1995-dL`
porte l'artiste ET l'album. Le même nettoyeur les rend exploitables.

**Garde-fou dur.** Les dossiers les plus peuplés ne portent RIEN : `2_040924`
(524 pistes), `complete` (138), `rever` (18), `All Track Part Four` (62). Un
dossier n'est retenu comme source d'artiste que s'il produit une décomposition
plausible ; sinon il est ignoré en silence. Un dossier ne doit jamais injecter
un faux artiste dans 524 requêtes d'un coup.

**Confiance.** Un artiste dérivé du dossier ou d'un nettoyage agressif est
`Yellow`, jamais `Green` — il préremplit le champ éditable, l'utilisateur le voit
avant tout rangement. C'est le seul point où ce chantier touche l'identité
écrite, et il la touche vers plus de prudence affichée, pas moins.

## 6. La cascade de replis

Remplace l'unique retry gardé de `discogs.rs:404-422`. La garde
`!q.artist.trim().is_empty()` **disparaît** : elle exclut exactement la population
qui a le plus besoin d'un repli.

| # | Essai | Condition |
|---|---|---|
| 1 | `artiste titre` nettoyés | artiste ET titre non vides |
| 2 | `artiste titre` sans la version | version présente et essai 1 infructueux |
| 3 | `titre` seul | titre non vide |
| 4 | `titre` sans accents ni ponctuation (`fold_char`) | titre contient du non-ASCII ou de la ponctuation |
| 5 | `artiste dossier` (artiste + album miné du dossier) | dossier exploitable |

Arrêt au premier essai dont le meilleur score de tracklist est > 0. Budget
réseau : voir §8.

## 7. La durée au scoring

`tracks.duration` est renseignée sur 2 711 pistes sur 2 714 et n'est ni envoyée
ni comparée. La tracklist Discogs porte une durée par piste (`discogs.rs:324-351`
la récupère déjà pour le scoring de titre).

Ajout à `track_match_score` (`discogs.rs:194-226`) : si les deux durées sont
connues, écart ≤ 3 s → `+3` ; écart > 15 s → `-3` ; entre les deux → 0. Les
valeurs exactes sont à caler sur le corpus, pas à décréter ici.

C'est le désambiguïsateur le plus fiable disponible : deux mixes du même titre
sur la même release diffèrent presque toujours en durée, là où le recouvrement de
tokens les confond.

## 8. Contraintes à respecter

- **Débit Discogs.** Aucune limitation côté client aujourd'hui, et le
  `Retry-After` réel n'est pas lu (`retry_after_s: 60` en dur, `discogs.rs:23`).
  Le pire cas actuel est déjà de 14 requêtes par clic (1 recherche + 6
  tracklists + 1 repli + 6 tracklists, `discogs.rs:17`, `394-395`, `413-414`)
  contre 60/min autorisées. **Une cascade à 5 essais ne doit pas multiplier ça
  par 5.** Contrainte dure : plafond de requêtes par clic inchangé ou meilleur.
  Levier : les essais dégradés ne sondent pas 6 tracklists chacun.
- **Aucune nouvelle dépendance.** Tout est faisable en `std` + ce qui est déjà
  dans l'arbre.
- **MSRV 1.77.2**, pas d'async, `unwrap`/`expect` interdits hors tests
  (`.claude/rules/rust.md`).
- **`cargo fmt --check` propre** avant tout commit (dette déjà payée une fois
  sur ce chantier, commit `920f552`).

## 9. Tranches

Verticales, chacune démontrable seule.

- **T1 — Corpus de noms sales.** Extraire de la vraie bibliothèque un corpus
  représentatif (~120 noms couvrant tous les motifs de §4 et §5), avec pour
  chacun l'attendu `(artiste, titre, version)`. Fixture committée + test qui
  mesure le taux de réussite. **C'est l'étalon** : sans lui, aucun changement
  ultérieur n'est falsifiable. Le test doit d'abord ÉCHOUER contre le code
  actuel avec un taux mesuré (NG81 : un témoin qui passe ne prouve rien tant
  qu'on n'a pas vérifié qu'il pouvait échouer). Bloque T2 et T3.
- **T2 — `search_terms.rs` : nettoyeur + extraction.** Le module profond de
  §3.2, sans le dossier. Démontrable : le taux du corpus T1 monte, chiffre
  avant/après.
- **T3 — Dossier parent.** Câbler `folder` jusqu'à `build` (`reconcile_path`
  ne reçoit aujourd'hui que le chemin, `filing.rs:151-159` ; `tracks.folder`
  existe en base). Minage + garde-fou §5. Démontrable : sous-ensemble du corpus
  portant un dossier.
- **T4 — Cascade de replis.** Retrait de la garde `discogs.rs:405`, boucle sur
  `Terms.ladder`, plafond réseau de §8. Démontrable : test unitaire sur la
  génération de la cascade + **premier test bout-en-bout de `Discogs::search`**
  (aucun n'existe : les 15 tests de `discogs.rs:427-719` couvrent des fonctions
  pures, il n'y a aucun mock HTTP dans l'arbre).
- **T5 — Durée au scoring.** Porter `duration_s` jusqu'à `Query` puis dans
  `track_match_score`. Indépendant de T2/T3. Démontrable : tests unitaires.

Ordre : T1 → (T2, T3) → T4 ; T5 en parallèle de T2/T3.

### État au 2026-07-28

| Tranche | État | Preuve |
|---|---|---|
| T1 corpus | livré | `search_corpus.rs`, 77 cas, base 5/77 |
| T2 nettoyeur | livré | 75/77 exacts (97,4 %) |
| T3 dossier | livré, **fusionné dans T2** | idem — voir ci-dessous |
| T4 cascade | livré | garde `discogs.rs:405` retirée, budget réseau testé |
| T5 durée | à faire | — |

**T2 et T3 ont été fusionnés.** Le minage du dossier réutilise intégralement le
nettoyeur du nom (`normalise` → `strip_groups` → `strip_loose_noise` →
`strip_scene_suffix` → `split_fields`) : les livrer séparément aurait signifié
écrire ce pipeline deux fois, puis le dédupliquer. La démontrabilité est
préservée — les cas du corpus qui dépendent du dossier (`A1-Stepback` →
`Slam`) sont testés nommément.

Résultat mesuré sur le corpus, `naming::reconcile` → `search_terms::build` :

| | base | après | |
|---|---|---|---|
| exacts | 5 (6,5 %) | **75 (97,4 %)** | ×15 |
| artiste | 15 (19,5 %) | 75 (97,4 %) | ×5 |
| titre | 17 (22,1 %) | 75 (97,4 %) | ×4,4 |
| version | 44 (57,1 %) | 77 (100 %) | — |

Deux cas non résolus, délibérément : `2-Gunne-What-I-Like--Fi-LOPZUP` et
`02-Retiro_An-2_Fluent_Remix_`. Détail et raison dans les constantes
`TERMS_*` de `search_corpus.rs`.

**Budget réseau tenu** (§8) : pire cas passé de 14 requêtes par clic
(principal + repli, 2 × (1 + 6)) à 13 (1 + 6, puis 2 × (1 + 2)). Vérifié par
un test qui compare les deux formules, pas par un commentaire.

**Décision d'implémentation non prévue au design** : `ipc_identify::identify`
n'appelle plus `filing::reconcile_track`. Il lit les tags et le chemin
lui-même, puis fait primer les tags propres sur `search_terms`. Motif : passer
par `Canonical` réintroduisait le portail de rejet dans le chemin de requête,
c'est-à-dire exactement le défaut corrigé. Les deux chemins sont désormais
distincts — `ipc_filing::reconcile` continue seul d'alimenter les champs
éditables et le badge de confiance, inchangé.

## 10. Ce que ce design ne garantit pas

- **9 pistes resteront non identifiables** (§1.0) : aucun nettoyage de chaîne ne
  peut inventer un artiste absent du nom, du dossier et des tags. C'est l'axe 4,
  hors périmètre, et il ne vaut plus le détour pour ce volume.

### Croyances révisées

- **Croyance** : « 524 pistes du dossier `2_040924` sont nommées `001_Untitled`
  et resteront non identifiables ; 19 % de la bibliothèque exige l'empreinte
  acoustique. »
  **Réfutée par** : extraction des noms complets du dossier, 2026-07-28 — ils
  sont du type `[BU 002] DJ Gregory - Freeze`, `[0012] QA 0-127 - Fiction`,
  `[BR 95004] A1 Baron Noir - Paris`. Parfaitement structurés, rejetés en bloc
  par le seul `[` de `JUNK_TOKENS` (`naming.rs:29`). Les `001_Untitled` sont
  dans un autre dossier (`02 [2015]`), et il y en a 2. Les deux constats
  avaient été confondus parce que le dossier le plus peuplé et le motif
  `Untitled` avaient été relevés dans deux mesures séparées, puis rapprochés
  sans vérification.
  **Ce que ça change** : le plafond du nettoyage par chaîne passe de ~81 % à
  99,7 % ; l'axe 4 (AcoustID) perd sa justification et passe de « seule réponse
  possible pour 19 % » à « 0,3 % » ; et la valeur de l'axe 1 augmente d'autant.
- **Aucune mesure du taux de réussite RÉEL après coup n'est possible
  aujourd'hui.** Rien en base n'enregistre une tentative d'identification :
  ni la requête envoyée, ni le nombre de résultats, ni le rang du candidat
  retenu. Le seul signal est `discogs_release_id IS NOT NULL` (4 pistes sur
  2 714 au 2026-07-28), qui confond « recherche ratée » et « jamais tentée ».
  Le corpus T1 mesure la qualité de l'EXTRACTION, pas celle des résultats
  Discogs. Instrumenter les tentatives est un chantier séparé, non tranché.
- **Les deux lignes de log existantes sont mortes en production** :
  `discogs.rs:393` et `discogs.rs:410` n'émettent rien car `tauri_plugin_log`
  n'est enregistré que sous `cfg!(debug_assertions)` (`lib.rs:146-153`).
- **Le corpus vient d'une seule bibliothèque.** Les proportions de §1 décrivent
  celle d'Antoine. Les MOTIFS sont probablement généraux (scène Soulseek /
  vinyle rippé), les POIDS ne le sont pas.
