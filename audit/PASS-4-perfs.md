# PASS 4 — Performances

> Audit seul, aucune modification de fichier. Méthode détective : chaque problème
> est étayé par une lecture de code précise (fichier:ligne). Deux catégories
> séparées en fin de rapport : preuves de code vs hypothèses à confirmer en
> runtime, avec protocole de mesure proposé pour chacune.

## Axe a — Conversion (FFmpeg, `encode.rs`, `filing.rs`)

### a1. [Low] Pas de problème structurel trouvé sur le squelette de conversion

`src-tauri/src/encode.rs` et `src-tauri/src/filing.rs` sont sains sur les points
audités :
- **Skip de ré-encodage déjà en place** : `encode.rs:91-110` (`is_conformant`)
  compare extension + `sample_rate`/`bit_depth` via `lofty::Probe` **avant**
  d'appeler `encode()` — `filing.rs:298` (`plan_file`) lit ce booléen et
  `execute_file` (`filing.rs:350-393`) prend la branche move-only (`conformant`)
  sans jamais lancer FFmpeg dans ce cas. Pas de reconversion redondante.
- **Pas de fichier chargé entier en mémoire** : `encode()` (`encode.rs:115-158`)
  pilote `ffmpeg-sidecar` en streaming (`child.iter()` sur les événements),
  aucun buffer intermédiaire du PCM côté Rust.
- **Lock DB non tenu pendant l'encode** : `filing.rs:234-249` documente
  explicitement le split 3-phases (plan sous lock → `execute_file` SANS lock →
  commit sous lock) précisément pour ne pas geler les autres utilisateurs de la
  connexion pendant un encode de plusieurs secondes. Confirmé appliqué dans
  `ipc_filing.rs:301-340` (`run_file_batch` : lock DB pris et relâché par
  itération, jamais gardé sur `execute_file`).
- **Batch série, pas parallèle** — voir a2 ci-dessous, catégorie « à confirmer ».

### a2. [Medium] Batch de filing traite les pistes en série, un thread FFmpeg à la fois

**Fichier** : `src-tauri/src/ipc_filing.rs:301-340` (`run_file_batch`)

```rust
for id in track_ids {
    if cancel.0.load(Ordering::SeqCst) { cancelled = true; break; }
    app.emit("file:progress", &FileProgress { ... }).ok();
    let plan = { /* lock DB */ ... };
    // execute_file() lance un process FFmpeg complet, ATTEND sa fin, puis boucle
}
```

**Description** : chaque itération encode un fichier avec FFmpeg (spawn +
attente complète du process, `encode.rs:122-149`) avant de passer au suivant.
Pour un lot de N fichiers non conformes (FLAC→AIFF ou →MP3), c'est N encodages
strictement séquentiels sur un seul thread OS dédié (`ipc_filing.rs:270-273`,
thread nommé `"file-batch"`).

**Impact concret** : sur une machine multi-cœur (le worker d'analyse utilise
déjà `available_parallelism().clamp(1,4)` en `worker.rs:114-117`, preuve que le
poste cible a plusieurs cœurs), le batch de filing n'utilise qu'un seul cœur
pour l'encodage FFmpeg alors que 2-4 encodages FFmpeg indépendants pourraient
tourner en parallèle sans conflit (chaque `encode()` a sa propre paire
source/dest, pas de ressource partagée hors la brève section sous lock DB).
Non mesurable précisément sans profilage (durée FFmpeg dépend du fichier), mais
la structure du code prouve l'absence de parallélisme, pas juste une hypothèse.

**Cause probable** : le split 3-phases a été conçu pour ne pas geler l'UI/DB
(a1), pas pour maximiser le débit. Le passage série était probablement le choix
le plus simple pour garder `cancel` et `progress` cohérents pas-à-pas.

**Proposition** : paralléliser phase 2 (`execute_file`, sans lock) sur un petit
pool borné (2-3 threads, pas plus — l'I/O disque et l'best-effort FFmpeg CPU
saturent vite), en gardant phase 1/3 (lock DB) strictement séquentielles par
fichier comme aujourd'hui. Le flag `cancel` resterait un stop-net "ne plus
soumettre de nouveau travail au pool" plutôt qu'un arrêt immédiat. Compatible
fail-fast : une erreur d'un fichier du pool n'affecte pas les autres (déjà le
cas aujourd'hui, `needs_validation`/`failed` par id).
**Effort** : Medium (gestion de pool + progress agrégée thread-safe).
**Bénéfice attendu** : à confirmer par mesure (protocole ci-dessous) — gain
plausible mais borné par l'I/O disque, pas linéaire avec le nombre de threads.

---

## Axe b — Analyse (Symphonia/lofty/verdict, `analysis/*.rs`, `worker.rs`)

### b1. [High] Un même fichier est ouvert/lu au moins 2 fois par cycle d'analyse complet, jusqu'à 4 fois s'il finit dédoublonné et affiché en Revue avec spectrogramme

**Preuve, ouverture par ouverture** :
1. `analysis/mod.rs:92` → `tags::read(path)` → `lofty::probe::Probe::open(path)`
   (`analysis/tags.rs:39`) : **1er `File::open`**, lecture des tags/propriétés
   sans décodage complet du flux audio, mais un premier handle + parsing
   conteneur.
2. `analysis/mod.rs:97` → `decode::probe(path)` → `open_format(path)`
   (`analysis/decode.rs:30-40`) : **2e `File::open`**, ouvre et sonde le
   conteneur une deuxième fois (juste pour lire `sample_rate`/`channels` du
   header).
3. `analysis/mod.rs:107` → `decode::decode_pcm(path, ...)` → `open_format(path)`
   à nouveau (`analysis/decode.rs:77`) : **3e `File::open`**, cette fois pour le
   décodage complet PCM.
4. Si dédoublonnage déclenché (`dedup.rs:144`, `get_or_compute_fp`, seulement
   si pas déjà en cache `tracks.fingerprint`) → `fingerprint::compute_for_path`
   → `decode::decode_pcm` à nouveau → **4e `File::open`** + décodage complet
   PCM entier redondant avec l'étape 3 (fréquence cible différente : mono vs
   stéréo/mono selon tags, mais même fichier source entièrement redécodé).
5. Si l'utilisateur ouvre l'écran Revue et déplie le spectrogramme
   (`report-view.ts:635`, `analyzePath(r.path, true)`) → `ipc.rs:260` →
   `analysis::analyze(path, true)` → **re-fait les étapes 1+2+3 en entier**
   (5e/6e/7e ouverture), car `ipc.rs:244` ne sert le cache QUE si
   `with_spectrogram == false` (voir b2).

**Impact concret** : pour le chemin le plus commun (analyse worker + dédup,
sans ouverture du spectrogramme), c'est **2 décodages PCM complets** du même
fichier (étape 3 et étape 4) au lieu de 1 — soit ~2× le coût de décodage mesuré
dans `docs/ressources-externes.md` (Évaluation 1 : ~270-710 ms pour un
FLAC/MP3 de 5-8 min). Le `probe()` (étape 2) est un coût marginal (lecture
d'en-tête seule, pas de décodage), mais reste une ouverture de fichier de plus
que nécessaire puisque `decode_pcm` (étape 3) sonde de toute façon le format en
interne.

**Cause probable** : `fingerprint.rs` et `analysis/mod.rs` ont chacun leur
propre appel à `decode::decode_pcm` sans jamais réutiliser le buffer PCM déjà
produit par l'autre — les deux modules ont été écrits indépendamment (M2a vs
M5) sans buffer PCM partagé entre pipeline d'analyse et fingerprinting.

**Proposition** : quand le fingerprint est nécessaire ET que l'analyse tourne
dans le même cycle (le worker analyse puis, plus tard via `dedup.rs`, calcule
le fingerprint), streamer le fingerprint EN MÊME TEMPS que le décodage
d'analyse (`analysis/mod.rs:107`, ajouter un accumulateur `Fingerprinter`
optionnel au même closure `on_block`, comme `dc`/`clip`/`tp`/etc.) plutôt que
deux passes `decode_pcm` séparées à des moments différents. Faisable sans
casser le design actuel (le fingerprint reste caché en DB, calculé une seule
fois) : le vrai gain est de fusionner l'ouverture 3 et 4 en une seule quand les
deux sont demandés ensemble — pas de fusionner analyse et fingerprint quand ils
sont déclenchés à des moments disjoints (dédup à la demande, pas systématique).
**Effort** : Medium (le fingerprint attend du mono 44.1kHz natif alors que
l'analyse peut tourner en stéréo — nécessite un downmix supplémentaire dans le
même closure, gérable).
**Bénéfice attendu** : élimine un décodage complet quand fingerprint et analyse
coïncident — à quantifier par mesure runtime (protocole ci-dessous), car
`get_or_compute_fp` est lazy et pas systématique (seulement si un candidat
homonyme existe, `dedup.rs:110-116`).

### b2. [High] Le spectrogramme "à la demande" ne consulte JAMAIS le cache `report_json`, même si l'analyse de base est déjà en cache

**Fichier** : `src-tauri/src/ipc.rs:225-274` (`analyze_path`)

```rust
if !with_spectrogram {
    let cached: Option<String> = conn.query_row(
        "SELECT report_json FROM tracks WHERE path=?1", ...
    ).ok().flatten();
    if let Some(json) = cached {
        if !json.is_empty() {
            return serde_json::from_str(&json).map_err(|e| e.to_string());
        }
    }
}
let report = crate::analysis::analyze(&path, with_spectrogram)?;
```

**Description** : le commentaire ligne 242-243 du code dit *"Serve the cached
report instantly (no re-decode), except when a spectrogram is requested
(computed on demand, not cached)"* — c'est un choix documenté, pas un bug
silencieux, mais son coût est prouvé par lecture : `with_spectrogram=true`
saute intégralement le cache et relance `analysis::analyze(path, true)` en
entier — donc **tous** les accumulateurs (`dc`, `clip`, `tp`, `sil`, `trunc`,
`pk`, `ph`, en plus de `spec`) sont recalculés depuis un nouveau décodage
complet, alors que **seul** `SpectrumAccumulator` avec `collect_display=true`
(`analysis/spectrum.rs:33-35`) a réellement besoin d'un nouveau passage — les
autres scalaires sont déjà dans `report_json`.

**Impact concret** : chaque clic sur "afficher le spectrogramme" en Revue
(`report-view.ts:622-650`, `wireSpectrogram`) déclenche un décodage complet
supplémentaire du fichier déjà décodé au moins une fois par le worker — 1× le
coût de `decode.rs` (~200-700 ms sur les mesures de l'Évaluation 1) **par
clic**, même pour un fichier déjà entièrement analysé et caché.

**Cause probable** : `report_json` (v5) a été conçu explicitement pour éviter
"un re-décodage à la réouverture" (commentaire de migration, `PASS-0`) mais son
scope n'a jamais couvert le spectrogramme — la colonne DB ne stocke que le
report SANS le spectrogramme (`worker.rs:94`, commentaire : *"spectrogram is
empty here — computed on demand"*), donc il n'y a structurellement rien à
lire dans le cache pour ce champ, pas un oubli d'invalidation.

**Proposition** : soit (a) étendre `report_json` pour inclure le spectrogramme
complet, au prix d'un JSON plus gros persisté pour chaque piste (le
spectrogramme est déjà borné à `MAX_COLS=800 × DISPLAY_BINS=256` octets max,
`spectrum.rs:165-166` — raisonnable, pas un blob énorme), soit (b) séparer le
calcul du spectrogramme du reste du pipeline (nouvelle fonction qui décode
UNIQUEMENT pour alimenter `SpectrumAccumulator`, sans refaire `dc`/`clip`/
`tp`/etc.) pour au moins ne pas payer le coût des 6 autres accumulateurs
inutiles en plus du décodage. L'option (a) est plus simple et cohérente avec
l'intention déjà actée de `report_json` ; l'option (b) réduit le travail par
appel mais ne réduit pas le nombre de décodages complets.
**Effort** : Low pour (a) (ajouter le champ, gérer la taille DB), Medium pour
(b) (nouvelle fonction dédiée dans `analysis/mod.rs`).
**Bénéfice attendu** : option (a) élimine 100% des re-décodages spectrogramme
pour toute piste déjà analysée (majorité des cas d'usage en Revue) — gain net
prouvable par lecture (skip total du décodage), pas juste une hypothèse.

### b3. [Medium] `analysis/mod.rs:110-113` alloue un nouveau `Vec<f32>` à chaque bloc décodé (downmix stéréo→mono)

**Fichier** : `src-tauri/src/analysis/mod.rs:107-120`

```rust
let info = decode::decode_pcm(path, target_ch, |block| {
    if target_ch == 2 {
        ph.push(block);
        let mono: Vec<f32> = block           // <-- allocation neuve à CHAQUE bloc
            .chunks_exact(2)
            .map(|lr| 0.5 * (lr[0] + lr[1]))
            .collect();
        dc.push(&mono); clip.push(&mono); tp.push(&mono);
        sil.push(&mono); trunc.push(&mono); pk.push(&mono); spec.push(&mono);
    } else { ... }
})?;
```

**Description** : le closure passé à `decode_pcm` est appelé une fois par
paquet Symphonia décodé (potentiellement des centaines à des milliers de fois
pour un morceau de plusieurs minutes — la taille de paquet dépend du codec,
typiquement de l'ordre de quelques dizaines de ms audio). Pour chaque appel en
stéréo, `.collect()` alloue un nouveau `Vec<f32>` de la taille du bloc — jamais
réutilisé entre appels.

**Impact concret** : N allocations heap + désallocations où N = nombre de
paquets décodés (proportionnel à la durée du fichier / taille de paquet
Symphonia). Coût par allocation faible individuellement (quelques centaines
d'échantillons f32), mais cumulé sur un scan de bibliothèque de plusieurs
milliers de fichiers, c'est un churn allocateur mesurable en pression GC-like
(Rust n'a pas de GC mais `malloc`/`free` répétés sollicitent l'allocateur
système/le cache du CPU).

**Cause probable** : le pattern `.collect()` est la façon la plus directe
d'écrire un downmix mono en Rust ; aucune réutilisation de buffer n'a été mise
en place car chaque bloc du closure est traité "à la volée" sans état partagé
côté downmix (contrairement aux accumulateurs qui, eux, sont bien réutilisés
d'un appel à l'autre).

**Proposition** : déplacer l'allocation du buffer `mono` HORS du closure
(comme `out` l'est déjà dans `decode_pcm`, `decode.rs:89` — `Vec::with_capacity`
créé une fois, puis `.clear()` + `.extend()` à chaque bloc). Un simple
`Vec<f32>` capturé par la closure `analyze()`, vidé et rempli à chaque appel au
lieu d'être recréé, élimine l'allocation répétée sans changer le comportement
observable.
**Effort** : Low (changement chirurgical, quelques lignes).
**Bénéfice attendu** : réduction mesurable du nombre d'allocations sur le
chemin chaud du décodage (à quantifier par mesure runtime — l'ordre de
grandeur du gain temps CPU dépend du profil de l'allocateur système, cf.
protocole ci-dessous), mais l'allocation redondante elle-même est prouvée par
lecture directe du code (pas une hypothèse).

### b4. [Low] Worker d'analyse : bien conçu, pas de blocage UI prouvé

`worker.rs:220-230` (`worker_loop`) documente et applique correctement :
décodage lourd (`analysis::analyze`) hors du lock DB (`persist_result` reprend
le lock seulement pour l'écriture finale). Le pool est dimensionné par
`available_parallelism().clamp(1,4)` (`worker.rs:114-117`) — parallélisme
réel, pas un thread unique. Pas de problème structurel trouvé ici.

---

## Axe c — Spectrogramme (`analysis/spectrum.rs`, `analysis/peaks.rs`, rendu front)

### c1. [Low] Taille FFT et résolution d'affichage déjà bornées et raisonnables

- FFT = 4096 échantillons (`analysis/mod.rs:79`, `FFT_SIZE`), hop 50%
  (`spectrum.rs:46`) — taille standard pour de l'analyse spectrale audio, pas
  disproportionnée.
- La grille d'affichage est explicitement bornée et poolée AVANT sérialisation
  JSON : `MAX_COLS=800`, `DISPLAY_BINS=256` (`spectrum.rs:165-166`,
  `build_spectrogram`), avec max-pooling en fréquence et sous-échantillonnage
  en temps (`col_stride`, `bin_pool`, `spectrum.rs:173-193`). Le payload JSON
  envoyé au front est donc borné à 800×256 = 204 800 octets `u8` maximum, quel
  que soit la durée du morceau — pas de croissance non bornée pour un long
  fichier.
- La détection de cutoff (`detect_cutoff`, `spectrum.rs:100-150`) tourne sur le
  LTAS pleine résolution (`self.ltas`, taille fixe `bins = fft_size/2 = 2048`),
  indépendamment de la grille d'affichage — cohérent avec le commentaire
  "Cutoff detection is unaffected" (`spectrum.rs:162-163`).

Aucun sur-dimensionnement prouvé ici. Le vrai coût (b2) est le **nombre de
fois** où ce calcul, déjà raisonnable en soi, est relancé — pas sa taille.

### c2. [Medium] Rendu du spectrogramme : `drawSpectrogram` redessine sur canvas à chaque toggle, sans cache de l'ImageData

**Fichier** : `frontend/report-view.ts:88-` (`drawSpectrogram`), appelée depuis
`wireSpectrogram` (`report-view.ts:636`) uniquement au premier "show" (`loaded`
gate ligne 621/631) — donc PAS à chaque re-toggle show/hide (`open`/`loaded`
sont deux flags distincts, le second empêchant un redraw sur un simple
show/hide répété). Ce n'est donc pas un problème de burst UI, seulement lié au
re-décodage back-end de b2 qui, lui, refournit des données à chaque montage de
`report-view` pour une piste donnée (pas de cache front persistant du
spectrogramme brut entre deux ouvertures de la même piste dans la même
session, contrairement à `reportCache`, `report-view.ts:673`, qui NE stocke
QUE le report sans spectrogramme puisque le report initial est chargé avec
`with_spectrogram=false`, `report-view.ts:713`).

**Impact concret** : rouvrir un track déjà consulté dans la même session,
déplier de nouveau le spectrogramme, redéclenche l'appel IPC +
re-décodage complet (b2), car `reportCache` ne mémorise jamais un report avec
spectrogramme rempli — `loaded` (ligne 621) est une variable locale à
`wireSpectrogram`, réinitialisée à chaque montage de `report-view` (donc à
chaque fois qu'on quitte/rouvre la piste), pas un cache par piste.

**Cause probable** : `reportCache` (Map path→report) a été conçu pour le report
de base (déjà quasi-gratuit après le premier cache DB), le spectrogramme étant
volontairement exclu de ce cache in-session comme du cache DB (b2) — donc le
même choix de conception se répercute côté front sans compensation locale.

**Proposition** : une fois b2(a) résolu côté back (spectrogramme inclus dans
`report_json`), ce point se résout de lui-même : le premier `analyzePath`
(avec `with_spectrogram=false` par défaut aujourd'hui, mais le cache DB
contiendrait déjà les données) suffirait, sans appel séparé
`with_spectrogram=true`. Sans toucher au back, une solution front minimale
serait d'ajouter le spectrogramme au `reportCache` existant dès qu'il est
calculé une fois (`reportCache.set(path, {...cached, spectrogram: full.spectrogram})`
après la résolution de la promesse ligne 635), pour au moins éviter le
re-décodage sur un aller-retour de piste DANS la même session.
**Effort** : Low (quelques lignes, si fait indépendamment de b2).
**Bénéfice attendu** : élimine le re-décodage pour le cas "revenir sur une
piste déjà dépliée dans cette session" — cas d'usage plausible en Revue
(comparer plusieurs pistes), à confirmer par mesure de fréquence réelle
(protocole ci-dessous).

---

## Axe d — Fluidité UI (`sift-live.ts`, `library-detail.ts`, `progress-zone.ts`)

### d1. [High] `renderQueue` reconstruit TOUTE la liste (`innerHTML =`) à chaque tick d'analyse, en violation de la règle CLAUDE.md "créer une fois, muter ensuite"

**Fichier** : `frontend/sift-live.ts:166-192` (corps de `renderQueue`),
déclenché par `frontend/sift-live.ts:1385-1396` (`onAnalysisChanged`)

```js
// sift-live.ts:1385-1396
void onAnalysisChanged(() => {
  void import("./report-view").then((m) => m.clearReportCache());
  scheduleAnalyzeRender();   // RAF-coalescé — bon pattern pour la zone de progression
  clearTimeout(t);
  t = setTimeout(() => void renderQueue(false), 300);  // <-- redraw complet, débounce 300ms seulement
});

// sift-live.ts:166-192
ql.innerHTML =
  (items.map((it) => { ... }).join("") || '<div>...</div>');
```

**Fréquence supposée de l'événement déclencheur** : `worker.rs:228`
(`app.emit("analysis:changed", ())`) tire **une fois par piste analysée**,
donc pour un scan initial de plusieurs milliers de fichiers (cas nommé
explicitement dans le code : `sift-live.ts:93` mentionne *"with thousands of
'Prêts' rows"* et `sift-live.ts:1390` mentionne littéralement *"can be dozens
per second during a 4000-track analysis burst"*), l'événement arrive en rafale
dense, dizaines de fois par seconde.

**Description** : le debounce à 300ms (`sift-live.ts:1395`) réduit la
FRÉQUENCE des appels à `renderQueue`, mais chaque appel qui passe reconstruit
la totalité du DOM de la liste via `ql.innerHTML = items.map(...).join("")`
(ligne 166) — détruisant et recréant CHAQUE ligne `.qi` de la file, même
celles dont rien n'a changé. C'est exactement le pattern interdit par la règle
CLAUDE.md ("Jamais d'`innerHTML=` dans un handler appelé en boucle"), même
avec le debounce : sur un scan de 4000 pistes avec un tick réel toutes les
~300ms (au mieux, limité par le debounce, mais peut être plus fréquent si
l'auto-clear analyzeTrailTimer à 350ms retarde différemment), c'est jusqu'à
plusieurs dizaines de reconstructions complètes de liste par minute d'analyse,
chacune proportionnelle au nombre total d'items dans la file (pas seulement
ceux qui ont changé de statut).

**Impact concret** : coût de reconstruction DOM proportionnel à `items.length`
à CHAQUE tick débounce (300ms), pendant toute la durée du scan initial (peut
durer plusieurs minutes pour une grosse bibliothèque). Pour N items dans la
file et T ticks pendant le scan, c'est O(N×T) nœuds DOM créés/détruits au lieu
de O(1) (mise à jour du seul verdict-dot + libellé de la ligne qui vient de
finir son analyse). Le texte du commentaire ligne 1390 ("dozens per second
during a 4000-track analysis burst") montre que les auteurs du code sont déjà
conscients du volume d'événements — mais la protection n'a été appliquée qu'à
la zone de progression (RAF-coalescé, `scheduleAnalyzeRender`), pas à la liste
elle-même.

**Cause probable** : `renderQueue` a été écrit avant que le volume réel
d'événements pendant un scan initial (des milliers de pistes) soit un cas
testé/vécu — le debounce à 300ms a probablement suffi à masquer le problème
lors des tests avec de petits lots, mais ne le résout pas structurellement.

**Proposition** : appliquer le même pattern "create once, mutate in place" que
`progress-zone.ts` (déjà exemplaire dans ce repo, cf. `render()`,
`progress-zone.ts:144-193`) : garder une Map `id → { rowEl, verdictDotEl,
labelEl, wordEl }` entre les rendus, ne créer un nouveau `.qi` DOM que pour un
id absent de la Map, et pour les ids déjà présents, ne muter QUE le
verdict-dot + le mot d'état (`word`/`wordColor`) + l'artiste si disponible —
jamais reconstruire toute la liste. Le tri/ordre des items doit être géré par
réordonnancement des nœuds existants (`insertBefore`) plutôt que par
`innerHTML =`.
**Effort** : Medium (refactor de `renderQueue`, ~30-50 lignes, mais pattern
déjà démontré dans le même repo à copier).
**Bénéfice attendu** : élimine la reconstruction DOM complète pendant un scan
de bibliothèque — gain visible surtout pour les grosses bibliothèques (le cas
explicitement cité en commentaire, "4000-track"), à quantifier par mesure
runtime (protocole ci-dessous) car l'impact perçu dépend du nombre réel
d'items affichés simultanément (la file "pending" seulement, pas toute la
bibliothèque filée).

### d2. [Medium] `list_filed` (bibliothèque) fait une requête SQL par piste pour les genres — N+1

**Fichier** : `src-tauri/src/library.rs:64-151` (`list_filed`) +
`src-tauri/src/genres.rs:23-27` (`get_genres`)

```rust
// library.rs:130-149
for (id, path, format, ...) in rows {
    out.push(LibraryTrack {
        ...
        genres: crate::genres::get_genres(conn, id)?,  // <-- 1 SELECT PAR PISTE
        ...
    });
}
```

```rust
// genres.rs:23-27
pub fn get_genres(conn: &Connection, track_id: i64) -> rusqlite::Result<Vec<String>> {
    let mut stmt = conn.prepare("SELECT genre FROM track_genres WHERE track_id=?1 ORDER BY ord")?;
    ...
}
```

**Description** : `list_filed` charge d'abord toutes les pistes filées en une
seule requête (`library.rs:68-127`, JOIN correct sur `metadata`), puis boucle
sur chaque ligne pour appeler `get_genres`, qui **prépare et exécute une
nouvelle requête SQL par piste**. Pour une bibliothèque de N pistes filées,
c'est 1 + N requêtes au lieu de 2 (la requête principale + un seul agrégat
genres via GROUP_CONCAT ou une jointure groupée).

**Impact concret** : chaque ouverture de l'écran Bibliothèque (ou changement
de filtre — `list_filed` est appelée avec `LibraryFilter`, donc à chaque
changement de facette/recherche côté IPC `list_library`, `ipc.ts:231`) coûte N
allers-retours SQLite en plus de la requête principale. SQLite en local reste
rapide par requête individuelle (pas de latence réseau), mais N requêtes
préparées à chaque filtre change à l'échelle (une bibliothèque DJ typique peut
avoir plusieurs milliers de pistes filées) — le coût croît linéairement avec
la taille de la bibliothèque à CHAQUE filtre appliqué, pas seulement au
premier chargement.

**Cause probable** : `get_genres` a été écrite pour un usage ponctuel
(afficher/éditer les genres d'UNE piste, cf. `filing.rs:227`
`load_tag_extras`, `library-detail.ts` édition), puis réutilisée telle quelle
dans la boucle de listing sans adapter la requête à un contexte multi-lignes.

**Proposition** : remplacer la boucle par une seule requête groupée après le
chargement principal — `SELECT track_id, genre FROM track_genres WHERE
track_id IN (...) ORDER BY track_id, ord`, puis répartir les résultats dans
une `HashMap<i64, Vec<String>>` construite une fois, consultée par id lors de
la construction de chaque `LibraryTrack`. Alternative plus simple : un
`GROUP_CONCAT(genre, ',')` dans la requête principale elle-même (JOIN LEFT sur
`track_genres`), au prix d'un parsing du résultat concaténé côté Rust — à
éviter si l'ordre des genres doit être strictement préservé (le `ORDER BY ord`
actuel), auquel cas la requête groupée séparée est plus sûre.
**Effort** : Low-Medium (isolé à `list_filed`, ne touche pas `get_genres`
utilisée ailleurs pour un seul id).
**Bénéfice attendu** : passe de O(N) requêtes à O(1) requête supplémentaire
pour les genres — gain net prouvable par lecture (élimination structurelle du
N+1), amplitude réelle à mesurer selon la taille de bibliothèque (protocole
ci-dessous).

### d3. [Low] Pas de virtualisation de liste trouvée pour la Bibliothèque — mais le rendu front n'est pas encore câblé

Recherche de `listLibrary`/`libraryFolders` (exportés dans `ipc.ts:231,235`)
dans `frontend/sift-live.ts` et `frontend/library-detail.ts` : aucun appel
trouvé. Le back-end M6b (`library.rs`, `ipc_library.rs`) est prêt et testé,
mais le rendu de la LISTE de la Bibliothèque (par opposition au panneau
détail/édition d'une piste déjà ouverte, que `library-detail.ts` gère bien)
n'est pas encore câblé côté front dans les fichiers audités. Impossible
d'auditer une éventuelle absence de virtualisation sur du code qui n'existe
pas encore — **à surveiller à l'implémentation** : si la liste finit par
utiliser le même pattern `innerHTML = rows.map(...).join("")` que d1 pour
plusieurs milliers de pistes filées, le même problème se reproduira en pire
(liste permanente, pas seulement pendant un scan).

### d4. [Low] Requêtes SQLite : connexion unique `Mutex`-partagée, mais découplée correctement du chemin chaud

Conforme à ce que `PASS-0` avait déjà signalé à vérifier : la connexion
`Mutex<Connection>` unique (`lib.rs`, state partagé) est bien row par row
rapide (SQLite local, pas de round-trip réseau), et les opérations lentes
(scan disque `scanner::reconcile`, encodage FFmpeg) utilisent chacune leur
PROPRE connexion secondaire (`ipc.rs:333`, `spawn_scan` ; le worker
d'analyse ne détient le lock que pour lire un path et écrire le résultat,
`worker.rs:194-218`) — donc le lock partagé n'est jamais tenu pendant une
opération bloquante longue. Le commentaire de migration citant "prep for
moving off the single-connection model" (PASS-0) reste vrai mais n'est pas un
goulot prouvé aujourd'hui : aucune commande Tauri synchrone auditée ne bloque
le thread principal au-delà d'une requête SQL simple.

---

## Coûts prouvés par le code

| # | Axe | Priorité | Résumé |
|---|---|---|---|
| b1 | Analyse | High | Fichier ouvert/décodé 2× (analyse + fingerprint séparés), jusqu'à 3-4× si dédup + spectrogramme demandé |
| b2 | Analyse/Spectrogramme | High | `analyze_path(with_spectrogram=true)` ignore TOUJOURS le cache `report_json`, re-décode tout à chaque clic |
| b3 | Analyse | Medium | Allocation `Vec<f32>` neuve à chaque bloc décodé (downmix stéréo→mono), `analysis/mod.rs:110-113` |
| d1 | UI | High | `renderQueue` fait `innerHTML=` sur toute la liste à chaque tick d'analyse (debounce 300ms, pas de mutation ciblée) — violation directe de la règle CLAUDE.md |
| d2 | UI/DB | Medium | `list_filed` : N+1 requêtes SQL (une par piste) pour charger les genres |
| a2 | Conversion | Medium | Batch de filing 100% série (1 seul thread FFmpeg actif à la fois) — absence de parallélisme prouvée par lecture, gain non quantifié |
| c2 | Spectrogramme | Medium | Pas de cache front du spectrogramme déjà calculé dans la session (dépend de b2) |

**Goulot le plus significatif prouvé** : **b2** — le spectrogramme casse
totalement l'intérêt du cache `report_json` (conçu explicitement pour "éviter
un re-décodage à la réouverture") dès qu'il est demandé, en refaisant un
décodage PCM complet + les 7 accumulateurs alors que seul 1 des 7
(`SpectrumAccumulator`) aurait besoin d'un nouveau passage. Combiné à d1 (qui
peut redéclencher `clearReportCache()` à chaque tick d'analyse,
`sift-live.ts:1387`), c'est le point où le design du cache, déjà bien pensé
pour le cas nominal, a un trou net et démontrable par lecture directe.

---

## À confirmer par mesure runtime (protocole de mesure proposé)

| # | Hypothèse à vérifier | Protocole de mesure concret |
|---|---|---|
| a2 | Le gain de parallélisation du batch FFmpeg est significatif | Chronométrer `run_file_batch` sur un lot fixe de 20 fichiers non-conformes (mix FLAC→AIFF / FLAC→MP3) de tailles réalistes, avant/après un patch qui parallélise `execute_file` sur 2-3 threads. Comparer le temps mur total, pas juste le CPU (le disque peut devenir le vrai goulot). |
| b1 | Le coût du double-décodage (analyse + fingerprint) est perceptible à l'usage | Instrumenter `decode::decode_pcm` avec un compteur d'appels par path (log ou métrique locale), lancer un scan réel d'un dossier de ~500 pistes avec homonymes délibérés (pour déclencher `get_or_compute_fp`), comparer le nombre total d'appels `decode_pcm` au nombre de pistes uniques réellement décodées une fois. |
| b3 | Le coût des allocations `Vec<f32>` par bloc est mesurable au niveau CPU | `cargo bench` (criterion, à ajouter ponctuellement pour CE test seulement — pas un chantier benchmark permanent) sur `analysis::analyze` avec un fixture FLAC stéréo de 5 min, avant/après le fix de b3, en isolant le temps CPU du décodage+DSP (le `log::info!` de `analysis/mod.rs:135-142` donne déjà un chrono grossier `started.elapsed()` — suffisant pour une mesure avant/après sans ajouter de dépendance). |
| c2 / d1 | Fréquence réelle des événements `analysis:changed` en rafale sur un scan réel | Ajouter un compteur temporaire (log) dans le handler `onAnalysisChanged` (`sift-live.ts:1385`) comptant les invocations/seconde pendant un scan réel d'une bibliothèque de plusieurs milliers de fichiers, pour confirmer l'ordre de grandeur "dozens per second" déjà supposé dans le commentaire existant. |
| d1 | Le coût de reconstruction DOM de `renderQueue` est perceptible (jank visible) | DevTools Performance (webview Tauri = Chromium sur Windows) : enregistrer un profil pendant un scan de plusieurs milliers de fichiers, observer le temps cumulé "Recalculate Style"/"Layout"/"Parse HTML" attribuable à `ql.innerHTML =`, comparer au budget de frame (16ms/60fps). |
| d2 | Le N+1 de `list_filed` est perceptible à l'usage | Chronométrer `list_filed` (test Rust dédié avec `std::time::Instant`, hors mesure automatisée du CI) sur une DB de test peuplée avec 5000 pistes filées + genres, avant/après le passage à une requête groupée. |
| b2 | Le re-décodage spectrogramme est visible pour l'utilisateur | Mesurer le délai perçu entre le clic "afficher le spectrogramme" et l'affichage effectif du canvas (`performance.now()` avant l'appel `analyzePath(r.path, true)` et après `drawSpectrogram`, `report-view.ts:635-636`), sur une piste déjà analysée (cache DB rempli), comparer à la même mesure si le cache incluait déjà le spectrogramme. |
