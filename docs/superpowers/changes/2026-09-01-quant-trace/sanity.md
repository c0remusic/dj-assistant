# Sanity du prototype `quant_trace` — mesures brutes et commande de rejeu

Prototype de l'issue #52 (vraisemblance de transcodage, méthode d'Olivier Derrien). Ce fichier
existe parce que la preuve du prototype vivait en prose dans l'en-tête de
`src-tauri/src/analysis/quant_trace.rs` : un tableau de chiffres sans la commande qui les produit
n'est pas rejouable, donc pas vérifiable.

**Ce document ne conclut rien.** Il enregistre huit fichiers mesurés à deux sauts. Huit fichiers ne
calibrent aucun seuil, et `λ` reste NON CALIBRÉ.

## Corpus

`C:\sift-corpus`, le corpus du 2026-08-17 (celui de #52, en dossier temporaire de session, avait
disparu du disque). Huit fichiers : cinq `fake/src0N_aac256.flac` et trois
`genuine/src0N_genuine.flac`, tous FLAC 44,1 kHz stéréo, 6-7 minutes.

Le harnais parcourt un DOSSIER. Rassembler les huit dans un dossier de travail (liens durs, même
volume — pas de copie de 360 Mo) :

```powershell
$d = "$env:TEMP\sift-sanity8"
New-Item -ItemType Directory -Force $d | Out-Null
1..5 | ForEach-Object { New-Item -ItemType HardLink -Path "$d\src0${_}_aac256.flac"  -Target "C:\sift-corpus\fake\src0${_}_aac256.flac" }
1..3 | ForEach-Object { New-Item -ItemType HardLink -Path "$d\src0${_}_genuine.flac" -Target "C:\sift-corpus\genuine\src0${_}_genuine.flac" }
```

## Commande exacte

Deux exécutions, identiques au saut près. `--release` est obligatoire (le balayage compte
1024 décalages × 8 groupes × 8 trames × 4 canaux × 2 résolutions par fichier) ; `--ignored` parce
que le harnais ne tourne pas dans la suite normale.

```powershell
$env:SIFT_QUANT_DIR = "$env:TEMP\sift-sanity8"
$env:SIFT_QUANT_SKIP = "17"   # puis "29" pour la seconde passe
cargo test --manifest-path src-tauri\Cargo.toml --release --lib quant_scan -- --ignored --nocapture
```

Sous une session concurrente qui tient `target/`, passer par `scripts/cargo-isolated.sh` avec les
mêmes arguments (c'est ainsi que les lignes ci-dessous ont été produites, le 2026-09-02).

`SIFT_QUANT_SKIP` n'est pas une commodité : les faux du corpus sont encodés depuis l'échantillon 0,
donc sans saut la mesure porterait sur la fabrication du corpus et non sur le signal. C'est le piège
nommé par le review du 2026-08-18.

## Passe 1 — `SIFT_QUANT_SKIP=17`

Sortie CSV verbatim (`L;decalage;canal;resolution;secondes;fichier`) :

```
0.25000;47;S;court;0.4;src01_aac256.flac
0.09375;95;D;long;0.3;src01_genuine.flac
0.20312;47;S;court;0.3;src02_aac256.flac
0.07812;613;D;long;0.3;src02_genuine.flac
0.12500;47;S;court;0.3;src03_aac256.flac
0.09375;502;G;long;0.3;src03_genuine.flac
0.45312;47;M;court;0.3;src04_aac256.flac
0.25000;47;S;court;0.3;src05_aac256.flac
-- 8 fichiers parcourus, 0 sans mesure (saut=17)
```

Les cinq faux tombent sur le même décalage 47, en blocs courts. Les trois authentiques pointent
ailleurs chacun, en blocs longs.

## Passe 2 — `SIFT_QUANT_SKIP=29` (le contrôle H4)

**Prédiction écrite avant la mesure.** L'amorçage d'un encodeur AAC LC vaut 2112 échantillons et le
pas des blocs courts vaut 128, donc le décalage attendu est `(2112 − saut) mod 128 = (64 − saut) mod
128` puisque `2112 = 16×128 + 64`. À `saut = 17` cela donne `64 − 17 = 47` ✔ (la passe 1) ; à
`saut = 29`, `64 − 29 = 35`, soit un déplacement de `−(29 − 17) = −12`.

Sortie CSV verbatim :

```
0.25000;35;S;court;0.3;src01_aac256.flac
0.09375;83;D;long;0.3;src01_genuine.flac
0.20312;35;S;court;0.3;src02_aac256.flac
0.07812;601;D;long;0.3;src02_genuine.flac
0.12500;35;S;court;0.3;src03_aac256.flac
0.09375;490;G;long;0.3;src03_genuine.flac
0.45312;35;M;court;0.3;src04_aac256.flac
0.25000;35;S;court;0.3;src05_aac256.flac
-- 8 fichiers parcourus, 0 sans mesure (saut=29)
```

Les cinq décalages demandés par le contrôle : **35, 35, 35, 35, 35** (src01 à src05). Prédiction
tenue. Les `L` sont inchangés à la cinquième décimale, ce qui était attendu : le saut ne change pas
le contenu, seulement l'origine des indices.

## Ce que le contrôle prouve, et ce qu'il ne prouve pas

⚠️ **Le déplacement de −12 n'est PAS en soi une preuve.** L'optimum d'une recherche déterministe sur
un signal rogné de 12 échantillons supplémentaires se déplace de −12 quel que soit le signal, et les
trois authentiques le font aussi : 95 → 83, 613 → 601, 502 → 490. Prendre ce déplacement pour la
signature du codec serait exactement l'erreur que le contrôle cherchait à écarter.

Ce que le contrôle prouve : la **convergence**. Cinq fichiers de sources musicales indépendantes
tombent tous sur UNE valeur, aux deux sauts, et cette valeur est celle que l'arithmétique
d'amorçage prédit sans rien ajuster. Les trois authentiques, eux, se dispersent sur trois valeurs
sans rapport entre elles. C'est la grille du codec, retrouvée à l'échantillon près.

## Limites, non levées par ces mesures

- **Huit fichiers ne calibrent rien.** `LAMBDA_PROVISOIRE = 0,031` est trop BAS : il classerait les
  trois authentiques (0,078 à 0,094) en faux. La bande qui sépare ces huit-là est
  `0,094 < λ ≤ 0,125`. La valeur se dérive sur le corpus entier, par le code qui l'applique.
- **Un seul encodeur, un seul débit.** Cinq `aac256` produits par `ffmpeg aac`. Rien ici ne dit ce
  que rendent `aac_mf`, un `aac128` (dont la fenêtre de bandes est réglée pour ≥ 192 kbps) ou un
  MP3.
- **Les blocs longs ne séparent rien**, et c'est mesuré ailleurs (voir l'en-tête du module) : la
  fenêtre `BANDE_DEBUT_LONG` est à revoir, ou les blocs longs à abandonner.
- **Rien de tout cela n'est branché sur `verdict()`.** C'est un prototype.
