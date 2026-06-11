# Plan d'implémentation — Sift

> **Sift** — app desktop Windows + Mac de prép sono DJ (nom de travail ; domaine à
> décider plus tard, la marque ≠ le domaine complet). Source de vérité fonctionnelle :
> la spec (`App prépa sons DJ.md`, vault Obsidian). Ce document découpe la construction
> en **jalons livrables** : à chaque jalon l'app est lançable et fait quelque chose de
> réel. La maquette `index.html` actuelle sert de **référence UI** et de base au shell
> frontend (à câbler sur le vrai backend).

## Décisions de cadrage (brainstorm 2026-06)

| Sujet | Décision |
|---|---|
| **Périmètre V1** | Backlog (~15 000 fichiers) **ET** flux Soulseek hebdo. Le pipeline doit encaisser un import massif dès la V1 — pas seulement le flux entrant. |
| **Biblio existante** | Nettoyage **actif** dès la V1 : doublons internes, fakes, tronqués sont scannés **et traités** (pas seulement indexés en lecture seule). ⚠️ Ceci tire une partie de l'ancien M8 dans le MVP — voir garde-fou Rekordbox. |
| **Sécurité Rekordbox** | **Garde-fou d'abord (V1)** : l'app lit le XML/`master.db` en lecture seule, détecte si un fichier est référencé par une playlist et **avertit avant** toute action dessus (suppression/ré-encodage). **Réparation intégrée des chemins (A) repoussée** à une phase ultérieure (met à jour Rekordbox automatiquement). |
| **MP3 < 320 authentique** | **Badge bitrate toujours affiché.** **Seuil configurable** dans Réglages (défaut 320). Sous le seuil → **proposé en re-sourcer par défaut**, mais l'utilisateur garde le choix de ranger quand même (zone grise assumée, pas de blocage). |
| **Diffusion** | Publique et **gratuite** → **code-sign Windows + notarization macOS + auto-update Tauri + site** entrent dans le **périmètre V1** (plus repoussés « avant diffusion »). |
| **Nom** | **Sift** (verbe : tamiser/trier — cœur de l'app). Domaine TBD. |

## Pile (confirmée par la spec)

| Brique | Choix | Rôle |
|---|---|---|
| Shell desktop | **Tauri** (Rust + webview) | léger, Win + Mac, IPC commands |
| Traitement audio | **FFmpeg** (sidecar bundlé) | décodage · peaks · FFT/spectre · conversion |
| Waveform/lecture | **wavesurfer.js** | waveform interactive + player |
| Time-stretch | **SoundTouch.js** (key-lock) / `playbackRate` (varispeed) | fader tempo preview |
| Empreinte | **Chromaprint / AcoustID** | dédup indépendant du nom |
| État | **SQLite** (rusqlite / tauri-plugin-sql) | vus, verdicts, tags, décisions, undo |
| Formatage clé | `diskutil` (Mac) · `fat32format`/diskpart (Win) | utilitaire FAT32, amovible-only |

Deux modules cœur réutilisables, testés isolément : 🔍 **Analyseur** · 🔁 **Encodeur**.

---

## M0 — Scaffolding (socle technique)
**But :** un Tauri qui démarre, FFmpeg embarqué, DB ouverte, CI qui build les 2 OS.
- Init projet Tauri ; remplacer le contenu `index.html` statique par le **shell frontend** (réutiliser le markup/CSS de la maquette : nav, 4 zones, thème clair/sombre).
- Bundler **FFmpeg en sidecar** (binaire par OS) + helper Rust `run_ffmpeg(args)`.
- **SQLite** : ouverture + migrations (schéma initial ci-dessous).
- Squelette **IPC** (commands Rust ↔ front) + types partagés.
- CI GitHub Actions : build Win (.msi) + Mac (.dmg), artefacts non signés.

**Livrable :** fenêtre vide navigable, `ffmpeg -version` appelable depuis le front, DB créée.

## M1 — Watcher + file « à traiter »
**But :** voir ses vrais téléchargements arriver dans la file.
- Surveillance **récursive** des dossiers source (`notify` crate) ; config multi-dossiers (Accueil).
- Détection des fichiers audio dans toute l'arbo (albums Soulseek, dossiers par pseudo) → **éclatement en morceaux individuels**.
- Persistance des « vus » (hash chemin + mtime) → pas de re-traitement ; nettoyage des sous-dossiers vides après tri.
- UI : **Accueil** (dossiers surveillés, compteur) + **file** peuplée en réel.

**Livrable :** déposer un dossier dans la source → la file se remplit toute seule.

## M2 — Analyseur (lecture seule) ⭐ cœur
**But :** voir waveform + spectrogramme + verdict fake + métadonnées RÉELS.

Un seul décodage FFmpeg → **huit sorties** :

| # | Sortie | Signal |
|---|--------|--------|
| 1 | Peaks waveform (JSON) | → wavesurfer |
| 2 | FFT bins fréq/temps | → spectrogramme canvas |
| 3 | Fréquence de coupure | → **verdict fake** (seuil réglable ; zone grise = soumis) |
| 4 | Qualité réelle + **bitrate réel** | → rail lossless vs lossy confirmé ; **badge bitrate toujours affiché**. Vrai MP3 sous le **seuil configurable** (défaut 320, Réglages) → proposé en re-sourcer par défaut, mais rangeable au choix (zone grise assumée). |
| 5 | Écrêtage (clip_runs, clip_pct, true_peak_dBTP) | → **intégrité dynamique** (rips vinyle trop chauds) |
| 6 | **Troncature / fichier incomplet** | → fin abrupte (énergie ne retombe pas) OU erreur décodage FFmpeg en fin de fichier ; durée < attendue |
| 7 | **Silence tête/queue** | → lead-in/run-out parasites → proposer trim |
| 8 | **DC offset** | → moyenne ≠ 0 (fréquent rips cartes son bas gamme) |

Bonus même passe :
- **Compatibilité mono/phase** — corrélation canaux : dual-mono (faux stéréo) + canaux hors phase (destructif en club sur basse sommée mono).
- **Intégrité conteneur/codec** — frames corrompues, header illisible (FFmpeg stderr) → badge « fichier cassé ».
- **Compatibilité tags CDJ** — version ID3, encodage, champs lus par les CDJ (différents selon modèle) → signaler/corriger ce qui ne passe pas au rangement.
- **Pochette embarquée** — présente/absente ; extraite pour l'UI ; ré-embarquée après conversion.

**Transparence du verdict** : la preuve est visible — coupure sur le spectrogramme, marqueurs d'écrêtage sur la waveform, badge explicatif. Le verdict doit être compréhensible, pas subi.

- Lecture **métadonnées/tags** (déclaré vs réel).
- Cache des résultats d'analyse en DB.
- **Tests de caractérisation** sur un jeu de fichiers connus (vrai 320, faux 320 transcodé, AIFF, WAV, FLAC, tronqué, écrêté) → fige le comportement du verdict.

**Livrable :** vue Revue avec vraie waveform, vrai spectrogramme, vrais badges qualité.

## M3 — Player + tempo
- Lecture wavesurfer : seek au clic, **Espace** = play/pause (pas d'autoplay, pas de marqueurs).
- **Fader tempo vertical** ±% : SoundTouch.js (key-lock ON par défaut) + toggle varispeed.

**Livrable :** on écoute et on cale le tempo dans la preview.

## M4 — Encodeur + « déplacer = encoder + ranger » ⭐ boucle complète
**But :** premier flux de bout en bout réellement utile.
- **Encodeur** : conversion 2 rails (MP3 320 / AIFF 16-bit 44,1 par défaut), **jamais d'upscale** — un vrai MP3 reste MP3, on ne fabrique pas du faux lossless depuis du lossy. lossy ≠ lossless. Option 24-bit avertie.
- Ordre strict : ① convertir → ② **tags + nommage sur le fichier CONVERTI** (modèle configurable) → ③ déplacer vers le dossier choisi.
- **Bacs 1-6** (clavier + clic) = ranger ; **« + nouveau »** = dossier à la volée ; bouton **Ranger** + Entrée.
- **Jeter** : libellé adaptatif selon verdict — faux → « ⚠ Re-sourcer » (va dans Écartés), vrai → « Jeter » (corbeille). L'utilisateur voit l'issue avant de cliquer.
- **Journal undo** + **corbeille centralisée auto-purgée** ; **`à-retélécharger.txt`** (copie 1 clic) — format `Artiste Titre` espace simple (Soulseek ne cherche pas avec tiret cadratin) ; aperçu avant action.
- **Mono-emplacement** (zéro doublon physique).

**Livrable :** version « utilisable au quotidien » — la maquette devient réelle.

## M4b — Onglet Écartés

Fichiers qui ne passent pas le tri : faux, tronqués, doublons perdants.

- **Raison visible** par fichier (badge : faux / tronqué / doublon) + nom de fichier brut.
- **Re-sourcer** : export `à-retélécharger.txt` format `Artiste Titre` espace simple (Soulseek). Liens achat par fichier et en batch : **Beatport · Traxsource · Juno · Bandcamp · Amazon · Apple Music**.
- **Corbeille** : envoi vers corbeille système (réversible).
- **Dossier séparé** : déplacer vers `rejeté/` plutôt que supprimer (option).
- Filtre : à re-sourcer / en attente corbeille.

## M5 — Dédup par empreinte (fin du MVP)

### Architecture deux tiers

**Tier 1 — Candidats par nom (gratuit, sans décodage)**
- Module de **normalisation partagé** (réutilisé par identification + renommage) :
  - Supprime : numéro de piste, tokens qualité (320kbps, FLAC, HQ), brackets parasites, noms d'uploaders, underscores.
  - **Conserve** : qualificateurs de version (Original Mix, Remix, Dub, Extended, feat.) — indispensable pour ne pas fusionner Original et Remix.
- Matching **fuzzy token-set** (ratio normalisé) sur les noms nettoyés → liste de candidats par groupe de similarité.

**Tier 2 — Vérification par empreinte (Chromaprint)**
- Chromaprint est PCM-based → **format-agnostic** : compare MP3 / WAV / AIFF / FLAC du même morceau sans faux positifs de format.
- Index inversé sur les sous-empreintes → requête sublinéaire (pas de N² sur 15 000 fichiers).
- Fingerprint initial partagé avec le décodage M2 (one-shot). Incrémental ensuite.
- Confirme « même enregistrement » (pas juste même titre). Anti-fusion Original/Remix si les noms sont ambigus.
- **Filet de sécurité** : les fichiers aux noms illisibles (uploaders, caractères aléatoires) passent directement au tier 2.

### Sélection du gagnant

Rail lossless vs lossy d'abord (jamais croisés sauf pour élimination), puis au sein du rail :

1. **Qualité réelle spectrale** (verdict fake/réel — M2)
2. **Intégrité dynamique** (moins d'écrêtage = clip_pct, true_peak)
3. **Utilisabilité** (AIFF > WAV pour les tags CDJ ; pas de fichier tronqué)
4. **Proximité format cible** (AIFF 16-bit/44,1 kHz par défaut — tous CDJ)

Sur rail lossy uniquement : bitrate comme tiebreaker final.
**Bit-depth ignoré** : la cible est 16-bit/44,1 kHz (tous CDJ) — 24-bit n'est pas un avantage par défaut.

- Détection doublons entre fichiers **et** « **déjà dans ta biblio** ».
- Comparaison N versions → recommande le gagnant → confirmation.

**Nettoyage actif de la biblio existante (décision V1) :** le scan doublons/fakes/tronqués
s'applique aussi aux ~15 000 fichiers **déjà rangés**, pas seulement aux nouveaux. Avant
toute action destructive (suppression/ré-encodage) sur un fichier existant, **garde-fou
Rekordbox** : lecture seule du XML/`master.db`, détection de référence en playlist,
avertissement explicite. La **réparation automatique des chemins** Rekordbox n'est PAS
dans ce jalon (voir M8) — en V1 l'utilisateur est prévenu et décide.

**Livrable :** 🎯 **MVP complet.**

---

## M6 — Identification & Biblio (Phase B)
- **Cascade d'identification** : ① tags fichier → ② **Discogs** (genres/styles + **pochette** + **release_id** stocké) → ③ Beatport/AcoustID → ④ manuel. *Nom final toujours regénéré depuis le modèle.*
- **Onglet Bibliothèque** : mini-lecteur waveform en bas, actions (re-ranger / re-tagger / supprimer), **lien vers la release exacte** (via `release_id`, pas une recherche).
- **Tags custom** (énergie/mood/occasion) + filtres.
- **Tableau de bord** : % lossless vs MP3, doublons restants, fakes à re-sourcer, par genre.
- Panneau métadonnées éditable (pochette + champs Discogs).

## M7 — Rekordbox XML + batch + clé USB (Phase B)
- **Génération playlists Rekordbox via XML** (dossiers + tags → playlists). Rappels (Rekordbox fermé).
- **Vue batch / tableau** : tri (verdict/format/BPM), sélection multiple, action groupée, aperçu.
- **Utilitaire « Formater la clé »** : FAT32 par défaut (contourne limite 32 Go Win), **amovible-only**, double confirmation, exFAT averti.
- Fichiers corrompus/tronqués · clipping.

## M8 — Profond & rétroactif (Phase ultérieure, isolé, risqué)
> Note cadrage : le **scan + traitement** de la biblio existante est remonté en V1 (M5)
> avec garde-fou lecture seule. Ce qui reste ici = la **réparation automatique** qui
> *écrit* dans Rekordbox, plus risquée. **Feature gelée** : on ne fixe pas le design
> tant que des **tests réels sur Rekordbox** (vraies bibliothèques, backup/restore,
> liens cassés) ne l'ont pas validée.
- **Rekordbox `master.db`** (pyrekordbox) : remplacement in-situ, **dédup des playlists existantes**, **réparation/prévention des liens cassés** (chemin change au changement de format) — c'est la bascule garde-fou → **réparation intégrée (option A)**. ⚠️ non-officiel, **backup obligatoire**, Rekordbox fermé.
- **Normalisation loudness** (option, OFF par défaut).

---

## Données (schéma SQLite initial)
- `tracks` — id, path, hash, fingerprint, format, bitrate, duration, declared_fmt, real_quality, verdict (ok/fake/grey), status (pending/filed/resourcing/trash), folder, created_at.
  - Signaux analyseur : clip_runs, clip_pct, true_peak_dbtp, dc_offset, phase_correlation, truncated (bool), silence_head_ms, silence_tail_ms, has_cover (bool), tags_cdj_ok (bool).
- `metadata` — track_id, artist, title, label, year, genre, bpm, cover_path, discogs_release_id, source.
- `custom_tags` — track_id, tag.
- `actions` — id, track_id, type (convert/move/trash/reject), from_path, to_path, ts (pour **undo**).
- `sources` — path, watched (bool).

## Pipeline batch & automatisation (transverse, dès M1)

**Modèle mental :** l'app n'est pas un outil "track par track" — c'est un **pipeline avec queue de décisions**. L'analyse tourne en fond sans intervention ; le DJ ne touche que les décisions ambiguës.

**Décision cadrage — le batch est AUTO par défaut, piloté par des règles fixées à l'avance.**
L'utilisateur configure ses règles une fois (Réglages) ; ensuite le pipeline applique sans
popup et ne remonte en revue que les cas hors-règle / ambigus. La revue manuelle est
l'exception, pas le mode normal. **Invariant dur : un vrai MP3 (≥ seuil, non transcodé)
n'est JAMAIS upscalé** vers lossless — il reste sur son rail lossy, converti seulement si
besoin de conformité CDJ (jamais AIFF/WAV depuis un MP3).

### Worker background (M1+)
- Dès qu'un fichier arrive via le watcher → **analyse auto-déclenchée** (M2) sans clic.
- **Worker Tauri** dédié (thread séparé, non-bloquant UI) avec throttling configurable (ne pas saturer le CPU/disque pendant un set).
- File persistée en DB : reprend après fermeture/crash, pas de double-analyse (hash + mtime).
- Progress global dans la barre de l'app : "X fichiers analysés / Y en attente".

### Routage par confiance
Chaque résultat d'analyse est noté selon la certitude du verdict :

| Confiance | Exemple | Destination |
|-----------|---------|-------------|
| Haute | Fake évident (coupure nette à 16 kHz), doublon identique format-agnostic | → **file d'actions auto** (batch confirmable en 1 clic) |
| Moyenne | Zone grise spectrale, doublon avec versions multiples | → **queue review** (décision groupée) |
| Faible / risqué | Fichier corrompu, ambiguïté nom+empreinte | → **queue review** flaggée |

### Review groupée (pas track par track)
- On ne review **pas les fichiers** — on review les **décisions** regroupées par type :
  - "Ces 14 fichiers sont fake — jeter ?" → un clic.
  - "Ces 3 versions du même morceau — lequel garder ?" → une décision.
  - "47 fichiers à convertir en AIFF 16-bit — lancer ?" → batch.
- Actions groupées : sélection multiple, aperçu diff (avant/après), confirmation unique.

### Règles auto configurables (M4+)
L'utilisateur définit son seuil de confiance requis par type d'action :
- "Fake confirmé (coupure > seuil X) → rejeter automatiquement"
- "Doublon avec winner évident (qualité réelle > 20 dB d'écart) → garder winner sans demander"
- "MP3 < 320 kbps → convertir au rangement"
- "Silence tête > 3 s → trimmer automatiquement"

Les règles auto s'appliquent sans popup ; un journal d'actions (DB `actions`) permet l'undo sur tout.

---

## Décisions UI (issues review — à respecter dès M4)

- **Queue Revue** : n'affiche que les `pending` par défaut ; toggle « + N traités » pour voir tout. Indispensable à l'échelle (15 000 fichiers).
- **Nom de sortie** : toujours sur 2 lignes (word-break), jamais tronqué — c'est l'info validée avant le commit.
- **Bouton jeter** : libellé adaptatif selon verdict — faux → « ⚠ Re-sourcer », vrai → « Jeter ».
- **Ordre onglets nav** : Accueil · Revue · Écartés · Biblio · Rekordbox · Clé USB · Réglages.
- **Undo** : toujours visible après une action (lien « Annuler » ou Ctrl+Z hint).
- **Icône Rekordbox nav** : `ti-playlist`, pas `ti-refresh` (utilisé pour la sync inline).
- **Raccourcis Revue** : 1-5, ↵, X, Espace — tous affichés comme chips dans l'UI.

---

## Transverse (à tenir dès M0)

- **Contrats IPC** typés (Rust ↔ front) versionnés ; le front ne fait jamais d'I/O fichier.
- **Tests** : caractérisation FFmpeg/verdict (M2), équivalence avant/après conversion (M4), fingerprint sur même morceau multi-format (M5).
- **Sécurité fichiers** : toute action passe par le journal `actions` + corbeille réversible ; jamais de suppression sèche.
- **Packaging/signing** : code-sign Windows + notarization macOS, auto-update Tauri — **dans le périmètre V1** (app diffusée gratuitement dès la sortie). Site vitrine inclus.

## Points encore ouverts (à trancher en cours de route)
- **Réparation Rekordbox intégrée (écriture `master.db`/XML)** : feature **gelée tant que
  des tests réels sur Rekordbox** n'ont pas validé le comportement (dédup playlists,
  réparation des liens cassés au changement de chemin, intégrité après backup/restore).
  On ne fixe pas l'API/le flux avant d'avoir mesuré sur de vraies bibliothèques.

**Tranchés au brainstorm (voir Décisions de cadrage) :** nom (Sift) · MP3 < 320 (seuil
configurable, badge, re-sourcer par défaut) · biblio existante (nettoyage actif V1) ·
Rekordbox (garde-fou V1, réparation plus tard, **gelée jusqu'aux tests**) · diffusion
(gratuite, signing + site V1) · **mode batch = auto par règles** (défaut) · **vrai MP3
jamais upscalé**.

## Séquencement / rationale
`M0→M1→M2` posent le socle + le cœur lecture. **M4 clôt la première boucle utile** (on peut s'en servir). **M5 finit le MVP.** Phase B (M6-M7) ajoute confort et Rekordbox sûr. M8 (risqué) reste isolé et optionnel, derrière backups.
