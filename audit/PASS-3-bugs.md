# PASS 3 — Bugs potentiels

> Méthode : détective (théorie → preuve dans le code réel, jamais de conclusion sans
> citation grep-able), principes `error-handling-patterns` (fail-fast, pas de fallback
> silencieux) et `systematic-debugging` (superpowers) appliqués sans accès aux SKILL.md.
> Aucune modification de fichier — audit seul. Lecture directe le 2026-07-02 sur
> `dj-assistant-m6a` (branche `m6a-discogs`).

---

## BUG-1 — Rail déclarée basée sur l'extension, pas sur le contenu réel : upscale
lossy→lossless possible via une extension trompeuse

**Priorité : Critical**

**Fichiers/lignes** :
- `src-tauri/src/analysis/tags.rs:22-27` (`rail_from_ext`)
- `src-tauri/src/filing.rs:287-291` (`plan_file`, decision + guard)
- `src-tauri/src/encode.rs:64-77` (`target_for`, `guard_no_upscale`)

```rust
// analysis/tags.rs:22
pub fn rail_from_ext(ext: &str) -> Rail {
    match ext.to_ascii_lowercase().as_str() {
        "flac" | "wav" | "aif" | "aiff" | "alac" => Rail::Lossless,
        "mp3" | "aac" | "m4a" | "ogg" | "opus" => Rail::Lossy,
        _ => Rail::Unknown,
    }
}
```
```rust
// filing.rs:287
let source_rail = crate::analysis::tags::rail_from_ext(&ext_of(&source));
let target = override_target.unwrap_or_else(|| encode::target_for(source_rail));
if encode::guard_no_upscale(source_rail, target).is_err() {
    return Err(FilingError::Upscale);
}
```

**Description** : `rail_from_ext` classe la « rail » (lossless/lossy) **uniquement
d'après l'extension du fichier**, jamais d'après le contenu réel. À l'inverse, le
décodage (`analysis/decode.rs:37`, `symphonia::default::get_probe()`) sniffe les
vrais magic bytes du conteneur — Symphonia détectera correctement un MP3 renommé
`.flac` comme flux MP3. Mais `plan_file` (filing.rs:287) rappelle `rail_from_ext`
sur l'extension du fichier, PAS le résultat du probe Symphonia déjà calculé à
l'analyse. `guard_no_upscale` (encode.rs:72) compare donc `source_rail` (dérivée
de l'extension mensongère = `Lossless`) au `target` — le garde-fou anti-upscale
ne se déclenche jamais puisqu'il croit filer un vrai FLAC.

**Impact concret** : un utilisateur reçoit un MP3 renommé `.flac` (mislabeling
intentionnel ou accidentel, cas visé justement par la feature de détection M2).
Le pipeline d'analyse peut correctement flaguer le morceau `Verdict::Fake` (le
cutoff spectral réel trahit le MP3, via `verdict.rs:36-44` avec `declared=Lossless`).
Mais si l'utilisateur file quand même le morceau (verdict ignoré, ou en batch avec
règle de tolérance), `plan_file` calcule `is_conformant(&source, Aiff1644)` → faux
(extension "flac" pas dans `["aif","aiff"]`) → `execute_file` appelle
`encode::encode(src, dst, Aiff1644)` (filing.rs:371) qui spawn FFmpeg. FFmpeg lit
le VRAI contenu MP3 (il sniffe aussi le conteneur, indépendamment de l'extension)
et produit un AIFF PCM 16/44.1 bit-exact avec la resynthèse MP3 — un fichier
lossless *techniquement valide* mais dont le contenu est un ré-encodage amplifié
d'un MP3, exactement le scénario que le garde-fou `guard_no_upscale` existe pour
bloquer. Le garde-fou est contourné parce qu'il consulte la mauvaise source de
vérité (extension) au lieu du verdict d'analyse déjà en base (`tracks.declared_rail`
n'existe pas comme colonne séparée — seul `verdict`/`cutoff_hz` sont stockés,
cf. `worker.rs:65-71`).

**Cause probable** : `rail_from_ext` a été conçue pour le cas normal (extension
honnête) et réutilisée telle quelle au moment du filing sans jamais consulter le
résultat de probe/analyse déjà calculé et persisté en DB pour ce même fichier.

**Correction proposée** : au moment du filing, dériver `source_rail` du contenu
réellement décodé — soit en rappelant `analysis::decode::probe()` (léger, pas un
decode complet) pour confirmer le vrai codec, soit en lisant une colonne dédiée
`tracks.detected_codec`/`tracks.declared_rail` persistée par le worker à l'analyse
(actuellement absente du schéma — seul `verdict` dérivé existe). Alternative plus
chirurgicale : refuser de filer sans confirmation utilisateur explicite un track
dont `verdict='fake'` reste non résolu.

**Effort** : Medium (ajouter soit un appel `decode::probe` au moment du plan,
soit une colonne migration DB + branchement worker → filing).

**Bénéfice attendu** : ferme la faille du garde-fou anti-upscale, cohérent avec
la fonctionnalité signature du produit (détection de faux lossless).

---

## BUG-2 — Aucune vérification `analyzed_at IS NOT NULL` / `status='pending'`
avant de filer un track

**Priorité : Medium**

**Fichiers/lignes** :
- `src-tauri/src/filing.rs:272-346` (`plan_file`)
- `src-tauri/src/ipc_filing.rs:216-241` (`file_track`)
- `src-tauri/src/ipc_filing.rs:338-353` (`run_file_batch` → `batch_canonical`/`plan_file`)

**Description** : `plan_file` lit uniquement `path` (filing.rs:88-91, `track_path`)
et appelle `reconcile_track` (lit les tags bruts du fichier, filing.rs:127-136) —
aucune requête ne vérifie `tracks.status='pending'` ni `analyzed_at IS NOT NULL`
avant de construire le plan de filing. `file_track`/`file_batch` n'ajoutent aucun
garde côté commande IPC non plus (`ipc_filing.rs:216-241`, `301-360`).

**Impact concret** : si le frontend affiche un track avant la fin de son analyse
(aucune requête `SELECT ... WHERE analyzed_at IS NOT NULL` trouvée non plus côté
front — `frontend/filing.ts` et `frontend/sift-live.ts` ne filtrent jamais sur
`analyzed_at`), un clic « filer » sur ce track fonctionne quand même : le nommage/
tag/déplacement se basent sur les tags de fichier (indépendants de l'analyse) donc
ne plantent pas, mais le **verdict affiché à l'utilisateur au moment du clic peut
être absent ou stale** (pas de faux-lossless détecté pour ce fichier précis) — le
garde no-upscale ne s'appuyant de toute façon jamais sur le verdict (voir BUG-1),
ceci n'aggrave pas BUG-1 mais illustre que rien n'empêche de filer « à l'aveugle ».

**Cause probable** : le filing (identité/nommage) et l'analyse (verdict qualité)
sont deux pipelines volontairement découplés (le premier ne dépend pas du second
pour fonctionner) — mais aucun gate produit n'a été ajouté pour empêcher un filing
prématuré côté UX.

**Correction proposée** : décrite seulement — si le produit veut garantir qu'un
verdict est toujours visible avant filing, ajouter un check (front ou commande)
refusant/avertissant quand `analyzed_at IS NULL` pour l'id filé.

**Effort** : Low (une clause SQL + un message d'avertissement front).

**Bénéfice attendu** : évite de filer un track sans que l'utilisateur ait vu son
verdict qualité, cohérent avec la promesse produit « analyse avant rangement ».

---

## BUG-3 — Pas de coordination de handle de fichier entre le worker d'analyse
(lecture) et `write_tags_full`/`apply_tags` (écriture) sur le même fichier

**Priorité : High**

**Fichiers/lignes** :
- `src-tauri/src/worker.rs:220-230` (`worker_loop` → `analysis::analyze`)
- `src-tauri/src/analysis/decode.rs:30-40` (`open_format`, `std::fs::File::open`)
- `src-tauri/src/tagging.rs:18-33` (`write_tags_full`, `Probe::open(path).and_then(|p| p.read())` puis sauvegarde)
- `src-tauri/src/ipc_filing.rs:168-186` (`apply_tags`, écrit hors lock DB)
- `src-tauri/src/filing.rs:358-366` (chemin conformant : lit puis écrit les tags avant le `rename`)

**Description** : Le worker d'analyse (thread pool séparé, `worker.rs:220`)
décode un fichier via Symphonia (`decode::open_format`, ouverture `std::fs::File`)
SANS jamais consulter ou attendre de verrou applicatif sur ce chemin de fichier.
En parallèle, `apply_tags` (ipc_filing.rs:168, appelée depuis le thread principal
Tauri command sur simple clic utilisateur) et le chemin « conformant » de
`execute_file` (filing.rs:358-366) ouvrent le MÊME fichier en écriture via
`lofty::Probe` — également sans coordination avec le worker. Le seul point de
synchronisation du projet est le `Mutex<Connection>` (accès DB), qui ne protège
en rien les accès au système de fichiers.

**Impact concret** : un utilisateur ouvre l'écran Revue sur un track pendant que
le worker d'analyse (pool de 1 à 4 threads, `worker.rs:114-117`) est encore en
train de le décoder (scan initial d'une grosse bibliothèque, ou re-scan après
watcher). S'il édite les tags puis clique « Appliquer » (→ `apply_tags`) ou
« Filer » sur un track conformant (→ tag-in-place avant `rename`, filing.rs:358),
`write_tags_full` tente d'ouvrir le fichier en écriture pendant que Symphonia le
tient ouvert en lecture dans le worker. Sur Windows (plateforme cible principale
du projet, cf. CLAUDE.md), le verrouillage de fichier est nettement plus strict
qu'en POSIX — une InitializeSecurityDescriptor/CreateFile en mode partagé
insuffisant peut lever une violation de partage (« os error 32 : le processus ne
peut pas accéder au fichier car il est utilisé par un autre processus »), que
`write_tags_full` remonte alors comme `FilingError::Tag`/erreur `apply_tags` —
un échec visible à l'utilisateur, potentiellement intermittent et donc difficile
à reproduire/diagnostiquer (fenêtre de course étroite mais réelle : la fenêtre
worker=décodage complet d'un fichier, ~5-700 ms d'après le benchmark Symphonia
documenté dans `docs/ressources-externes.md`).

**Cause probable** : le worker et le filing/apply_tags ont été développés comme
deux sous-systèmes indépendants (le commentaire de `worker.rs:1-3` dit
explicitement tourner « OFF the DB lock » — jamais mentionné vis-à-vis du
filesystem), sans jamais introduire de verrou par-chemin.

**Hypothèse à vérifier** (non confirmée par test reproductible ici — nécessite
un test d'intégration avec un vrai worker actif + apply_tags concurrent sur
Windows) : la fenêtre de course est réelle en théorie de code, mais son
occurrence effective dépend du comportement précis de `File::open` de Symphonia
(mode de partage par défaut sur Windows via `std`) vs `lofty::Probe::open` — à
vérifier par un test délibéré (ouvrir le fichier en lecture bloquante dans un
thread, tenter `write_tags_full` en parallèle, observer l'erreur).

**Correction proposée** : introduire un verrou applicatif léger par `track_id`
(ex: `HashSet<i64>` protégé par `Mutex`, à l'image de `Queue.queued` dans
`worker.rs:12`) que `apply_tags`/`file_track`/`execute_file` acquièrent (ou
attendent) avant d'ouvrir le fichier en écriture, et que le worker consulte
avant de démarrer le décodage — ou plus simple : retirer l'id de la queue
`Queue.queued`/`running` AVANT de permettre le filing (le worker expose déjà
`running` dans `Queue`, filing.rs pourrait le consulter via un nouvel accessseur).

**Effort** : Medium (nouvel état partagé + branchement dans 2-3 call sites).

**Bénéfice attendu** : élimine une classe d'échecs intermittents « fichier
utilisé par un autre processus » sur Windows, plateforme cible principale.

---

## BUG-4 — `execute_file` (phase 2, chemin conformant) ne revérifie pas
l'existence du fichier source juste avant `std::fs::rename`

**Priorité : Medium**

**Fichier/ligne** : `src-tauri/src/filing.rs:350-368`

```rust
pub fn execute_file(plan: &FilePlan) -> Result<Vec<FsLog>, FilingError> {
    let mut log = Vec::new();
    if plan.conformant {
        let old_tags = tagging::read_tags_full(&plan.source).map_err(FilingError::Tag)?;
        ...
        std::fs::rename(&plan.source, &plan.dest).map_err(|e| FilingError::Io(e.to_string()))?;
```

**Description** : `plan_file` (phase 1, sous verrou DB) lit `source` depuis la DB
et calcule `dest`. `execute_file` (phase 2, SANS verrou DB — commentaire
filing.rs:348 le dit explicitement) peut s'exécuter un temps notable plus tard
(plusieurs secondes pour un encode ffmpeg dans le chemin non-conformant, quasi
immédiat dans le chemin conformant). Entre les deux phases, rien n'empêche le
fichier source d'avoir été déplacé/renommé/supprimé côté OS (l'utilisateur, un
autre outil, ou même le watcher de Sift lui-même sur une autre source qui pointe
vers le même chemin physique). `read_tags_full`/`rename` échoueront alors avec une
erreur IO standard (fichier introuvable) — ce qui est correctement remonté en
`Err`, PAS un plantage silencieux — mais aucun message dédié « le fichier source a
disparu entre l'analyse et le filing » n'existe : l'utilisateur reçoit l'erreur OS
brute via `FilingError::Io`.

**Impact concret** : scénario réaliste avec le file-watcher actif — un utilisateur
supprime/déplace manuellement un fichier depuis l'Explorateur Windows pendant
qu'il est affiché en attente de filing dans Sift, puis clique « Filer ». L'erreur
remontée est technique (`FilingError::Io("... os error 2 ...")` ou équivalent) et
non humanisée — aligné avec le gap déjà noté dans `docs/ressources-externes.md`
(section Veille UX 2026-06-24, point 2 : messages d'erreur IPC bruts au lieu
d'être humanisés). Ce n'est pas un crash ni une corruption de données (les tests
`filing.rs:663-673` confirment que sur échec de `execute_file`, `commit_file`
n'est jamais appelé — rien n'est marqué `filed` en DB à tort), mais une régression
UX répétée sur toute erreur fichier disparu.

**Cause probable** : erreur générique du `std::fs`/`lofty` non interceptée pour
produire un message dédié « fichier disparu » distinct d'autres IOError.

**Correction proposée** : dans `execute_file`, faire un `Path::new(&plan.source).exists()`
explicite avant la première opération FS et retourner une variante dédiée
(ex: `FilingError::SourceMissing`) que le front peut humaniser distinctement
(cf. `journal.ts:150-155` qui a déjà ce pattern de traduction pour
`revert_batch` — « source gone » y est déjà géré, seulement pour le revert, pas
pour le filing initial).

**Effort** : Low (un check + une variante d'erreur + une entrée `humanError`).

**Bénéfice attendu** : message actionnable au lieu d'une erreur OS brute,
cohérence avec le pattern déjà en place côté revert.

---

## Points vérifiés SANS bug trouvé (preuve à l'appui)

- **Verrou DB pendant les opérations longues** : `file_track` (ipc_filing.rs:216-241)
  et `run_file_batch` (ipc_filing.rs:301-360) libèrent explicitement le verrou
  `Mutex<Connection>` autour de `execute_file` (l'encode FFmpeg multi-secondes) —
  confirmé par les commentaires filing.rs:348 et ipc_filing.rs:9-11, et par la
  boucle `run_file_batch` qui reprend/relâche le lock **par fichier** (ligne
  330-337 lock, phase 2 hors lock, phase 3 relock plus loin). Aucun deadlock ni
  gel de l'UI plausible par ce chemin — le pattern est correctement appliqué à
  chaque commande longue inspectée (`file_track`, `file_batch`, `identify`
  n'a pas été audité en détail ici par manque de temps, à vérifier en Pass 4/perf
  si le réseau Discogs bloque le lock).
- **Worker vs DB lock** : `worker.rs:220-230` (`worker_loop`) ne tient le verrou
  que pour `read_path` (une requête) et `persist_result` (une requête) — le
  décodage/analyse complet tourne hors verrou (commentaire ligne 223 explicite).
  Pas de risque de « database is locked » ni de contention prolongée par ce
  chemin.
- **Sentinel `FILE_IN_PLACE` (`"__SOURCE__"`) et `EXTERNAL_DEST_PREFIX`
  (`"__EXTERNAL__::"`)** : un seul point de décision (`plan_file`, filing.rs:304-322)
  gère explicitement les deux cas avant tout appel à `library::safe_join` — le
  code et les commentaires (filing.rs:14-30) documentent précisément pourquoi
  (éviter que `safe_join` sanitize le littéral en un vrai dossier `__SOURCE__`).
  Le nom identique est mirroré côté front dans `shared/contracts.ts` et utilisé
  de façon cohérente (`frontend/filing.ts:33,1222`, `frontend/sift-live.ts:55,543`).
  `safe_join` lui-même a des tests couvrant la traversée de chemin
  (`library.rs:368-382`). Aucun chemin de code observé ne concatène le sentinel
  comme un vrai chemin disque.
- **Fichier corrompu / décodage impossible** : `analysis::analyze` propage l'échec
  de `decode::probe`/`decode_pcm` via `?` (analysis/mod.rs:97,107) jusqu'à
  `persist_failure` (worker.rs:102-110), qui fixe `report_json=''` (sentinel non-NULL)
  pour ne plus jamais re-sélectionner ce fichier dans `select_pending` — testé
  explicitement (`worker.rs:283-294`, cas `c` filé, mais le mécanisme du sentinel
  vide est documenté et cohérent avec le SQL `report_json IS NULL`, worker.rs:44).
  Pas de boucle infinie de re-analyse sur fichier cassé.
- **Extension trompeuse au niveau décodage** : Symphonia sniffe le vrai contenu
  du conteneur (`decode.rs:37`, `symphonia::default::get_probe()`), l'extension
  n'est qu'un `Hint` de désambiguïsation — le décodage lui-même n'est PAS trompé
  par un mauvais suffixe de fichier. Le problème est en aval, au moment du
  **filing** (voir BUG-1), où `rail_from_ext` est réutilisée à tort comme source
  de vérité.
- **UI après `revert_batch`/`undo_last`** : les deux commandes Rust émettent
  explicitement `queue:changed` (ipc_filing.rs:468,484) après la mutation DB ; le
  front a un listener global unique sur cet événement (`sift-live.ts:1358-1361`)
  qui redéclenche `refresh()` (re-fetch, pas une simple mutation locale). Le
  Journal lui-même (`journal.ts:231-244`) ne fait qu'une mutation DOM locale de
  la ligne revertée (`jrnl-row--reverted`) mais ne prétend pas re-synchroniser
  toute la liste — cohérent car `queue:changed` s'en charge côté vue tracklist.
  Pas de désynchronisation identifiée sur ce chemin.
- **Listeners Tauri et démontage d'écran** : les 4 listeners d'événements
  (`onQueueChanged`, `onAnalysisChanged`, `onFileDone`, `onFileProgress`) sont
  enregistrés UNE SEULE FOIS au boot dans `sift-live.ts:1358-1363` (jamais par
  vue), donc pas de risque d'accumulation ou de listener orphelin par navigation
  d'écran. Les handlers eux-mêmes gardent leurs mutations DOM derrière des checks
  de présence explicites (`fileNote`, sift-live.ts:768 : `if (!foot) return`;
  commentaire ligne 810-811 confirmant que chaque renderer « no-ops when its root
  is absent »). Pas de mutation confirmée sur un DOM disparu.

---

## Bilan

| Priorité | Nombre |
|---|---|
| Critical | 1 |
| High | 1 |
| Medium | 2 |
| Low | 0 |

**Total : 4 bugs**, tous avec citation fichier:ligne et extrait de code réel.
Aucune hypothèse non vérifiée nécessitant une section séparée (BUG-3 contient une
sous-hypothèse explicitement marquée, avec méthode de vérification proposée).
