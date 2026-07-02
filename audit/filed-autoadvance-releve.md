# Relevé — Filed ↩ : auto-avance + emplacement rail (option C) + détachement file_track mono

> Lecture seule. Aucune modification. À valider avant tout code.
> Décisions produit actées : (1) après un File en mode détail, ouvrir DIRECT le prochain
> pending (zéro clic) ; (2) confirmation Filed = bandeau fin en haut du rail droit (#filfoot),
> contrôles du prochain morceau dessous (option C) ; (3) détacher file_track mono dans la
> même session.

## Le crime
En mode détail, filer un morceau ne fait PAS avancer au suivant : une carte "Filed ✓ ↩ Revert"
s'installe au centre (#mid) et BLOQUE l'auto-avance jusqu'à ce que l'utilisateur reclique un
morceau. + l'encode ffmpeg d'un seul morceau fige la fenêtre (commande synchrone, non détachée).

## Preuves (3 sites)

### A. L'auto-avance EXISTE déjà mais est neutralisée — `syncDetail` (filing.ts:934)
- Commentaire d'intention explicite : « after filing one the next opens automatically ».
- Ligne 937 : `if (state.filedConfirm && mid.querySelector(".sift-filed-confirm")) return null;`
  → dès qu'une confirmation est affichée, syncDetail NE FAIT RIEN (pas d'avance). C'est la
  ligne qui crée la friction.
- Garde-fou à ABSOLUMENT préserver (filing.ts ~946) : si `state.track && paneIsOurs`,
  syncDetail return l'id courant SANS switcher — sinon il détruit le player audio en plein
  chargement. L'auto-avance ne doit s'enclencher QUE quand aucun morceau n'est ouvert.
- Chemin "aucun morceau ouvert" déjà présent (ligne ~952) : `if (items.length) openFilingInto(mid, items[0])`.

### B. La confirmation est au CENTRE, pas dans le rail — `showFiledConfirm` (filing.ts:709)
- Écrit dans `mid` (#mid, le panneau central du rapport) un bloc `.sift-filed-confirm`.
- Vide le rail (`#filfoot`) : `foot.innerHTML = ""`.
- Pose `state.filedConfirm = { batchId, bin }` → c'est ce flag que syncDetail teste pour se bloquer.
- Revert ciblé par batchId via `doRevert` → `revertBatch` (mécanique SOLIDE, ne pas toucher).
- Appelée depuis `doRanger` (filing.ts:692) juste après `fileTrack(...)` réussi.

### C. file_track mono N'EST PAS détaché — `file_track` (ipc_filing.rs:58)
- `#[tauri::command]` SYNCHRONE. Phase 2 (ligne 76) = `execute_file(&plan)` =
  « the multi-second ffmpeg encode + file moves » sur le thread de commande → fige la fenêtre.
- Le patron de fix EXISTE déjà dans le même fichier : `file_batch` (~ligne 95) détache sur un
  thread dédié via `run_file_batch`, lock pris/relâché PAR FICHIER, « never freezes the UI nor
  blocks the analysis worker ». Détacher file_track = appliquer le même geste en version 1-fichier.

## Proposition d'implémentation (à valider — sous-étapes commitées séparément)

### Étape 1 — Front : confirmation rail + auto-avance (option C)
- `showFiledConfirm` : écrire le bandeau fin dans `#filfoot` (haut du rail), PAS dans `#mid`.
  Bandeau = ligne "✓ {bin} · Revert" (border-left 2px success, border-radius 0 — règle CDS
  pour accent un seul côté). Revert garde doRevert/revertBatch inchangé.
- NE PLUS poser le blocage : retirer le `return null` sur filedConfirm dans syncDetail (ligne 937),
  pour que l'auto-avance reprenne. Le bandeau vit dans le rail, indépendant du centre.
- Après File réussi (doRanger) : déclencher l'avance au prochain pending au centre. Réutiliser
  le chemin `openFilingInto(mid, items[0])` existant via le refresh queue:changed → syncDetail.
- Le bandeau persiste tant qu'on n'a pas filé le SUIVANT (puis il se remplace par le nouveau) ou
  reverté. Un seul bandeau à la fois (pas d'empilement pour cette étape — l'historique des filed
  reste un chantier séparé).
- Contrôles du prochain morceau (Destination/Identify/Format/File) rendus SOUS le bandeau dans
  le rail — c'est `renderFoot` qui les pose ; vérifier qu'il coexiste avec le bandeau (le bandeau
  est prepend, les contrôles suivent).
- RISQUE : `renderFoot` réécrit `#filfoot` — il faut que le bandeau survive à un renderFoot, ou
  que renderFoot le re-pose. À cadrer en codant (probable : bandeau = 1er enfant, renderFoot
  écrit après lui / le préserve). Pattern "create once" déjà dans nos conventions.

### Étape 2 — Rust : détacher file_track mono
- Calquer file_batch : phase 1 (plan) synchrone sous lock pour échouer vite sur NoLibraryRoot,
  puis détacher phase 2+3 (execute + commit) sur un thread, émettre un évènement de fin
  (ex. `file:done` mono ou réutiliser le contrat existant) → le front affiche le bandeau à la
  réception, pas en synchrone.
- IMPACT FRONT : `doRanger` ne reçoit plus le `FileResult` en retour direct (batch_id) ; il
  l'obtient via l'évènement. À recâbler proprement (le bandeau a besoin du batch_id pour Revert).
- Garde "une action à la fois" (`acting`) à conserver.

## NE PAS TOUCHER
- Moteur revert (revert_batch, doRevert, revertBatch) — solide, prouvé.
- Garde-fou player de syncDetail (`state.track && paneIsOurs` → pas de switch).
- file_batch (déjà détaché et correct).
- TRASH_PURGE_DAYS (feature stubbée, hors sujet).

## Ordre proposé
Étape 1 d'abord (front, le gain d'usage visible) → test live → STOP → étape 2 (Rust, le freeze)
→ test live. Deux commits séparés. Chaque étape testable seule.


---

## Idée notée (terrain, pote DJ) — « filer sur place » pour les batchs de prep
NON traité maintenant. À reprendre après Filed ↩.

Besoin remonté : pour préparer un set, on veut souvent convertir/nettoyer les morceaux et les
LAISSER dans leur dossier d'origine, pas les ranger dans l'arbre bibliothèque (Library/House/…).

Lecture sûre (à confirmer auprès du pote) : ce n'est PAS un mode destructeur. C'est la même
mécanique de filing qu'aujourd'hui (encode → original au trash → REVERTABLE), sauf que la
destination (`bin_rel`) = le dossier SOURCE au lieu d'un dossier de bibliothèque. Donc une
OPTION DE DESTINATION (« rester ici, mais propre »), pas un nouveau chemin qui détruit l'original.
Cohérent avec « déplacer = encoder + ranger » et le filet revert reste intact.

QUESTION OUVERTE À LEVER avant tout code : le pote veut-il l'original revertable (au trash,
comme la lecture sûre ci-dessus) OU un vrai écrasement sec sans copie de secours ? Si c'est
l'écrasement sec → DRAPEAU ROUGE : casse le revert (plus rien à ramener), frontal avec la
philosophie produit. Ne pas coder ça sans une décision explicite et assumée.

Périmètre probable si lecture sûre confirmée : exposer "dossier source" comme cible de filing
(mono et/ou batch). Petit, sûr, à chiffrer par un relevé dédié le moment venu.


---

## Idée notée (architecture) — corbeille centrale unique vs corbeille same-disk
NON traité. Relevé dédié à faire avant tout code. Décision de conception, pas un quick-fix.

État actuel (preuve, filing.rs:118-126) : `trash_file_fs` déplace l'original vers
`<racine-biblio>/.sift-trash/<track_id>__<nom>` via `std::fs::rename`. Donc la corbeille vit
SOUS la racine bibliothèque → toujours same-disk QUE LA BIBLIO → "jeter" = renommage instantané
tant que la source est sur le disque de la biblio.

Besoin remonté (Antoine) : un SEUL dossier corbeille CENTRAL (pensé interne), pour 3 raisons
cumulées — (1) centraliser tous les originaux jetés au même endroit, (2) ne pas polluer le
disque externe / la biblio, (3) pouvoir tout vider d'un seul coup à un seul endroit.

Contrainte dure (le piège) : les sources à filer viennent de partout, IMPRÉVISIBLE (interne,
externe, USB…). Donc AUCUN emplacement de corbeille n'est same-disk avec toutes les sources à
la fois. `fs::rename` cross-disque échoue → fallback en COPIE d'octets (lent, proportionnel à
la taille). Conséquence : une corbeille centrale rendra une partie des "jeter" lents (ceux dont
la source est sur un autre disque que la corbeille). Aujourd'hui c'est masqué parce que la
corbeille suit la biblio.

TENSION À TRANCHER : corbeille CENTRALE (organisation, besoin d'Antoine) ⊥ corbeille SAME-DISK
(vitesse). Pas conciliables pour toutes les sources simultanément.

Pistes à évaluer dans le relevé dédié (NON décidées) :
- (a) Corbeille centrale interne unique, on ASSUME la copie cross-disque pour les sources non
  internes (simple, mais "jeter" un gros WAV depuis l'externe devient lent + double l'espace le
  temps de la copie).
- (b) Corbeille PAR DISQUE (une `.sift-trash` sur chaque volume source) mais VUE UNIFIÉE dans
  l'UI + purge "tout vider" qui balaie toutes les corbeilles connues → garde le same-disk
  (rapide) ET l'impression d'un endroit unique côté utilisateur. Plus de logique, mais respecte
  vitesse + besoin.
- (c) Corbeille same-source automatique (à côté de chaque original) — rapide mais l'inverse de
  "centralisé", probablement écarté.
- Lier à TRASH_PURGE_DAYS (feature stubbée) : la purge auto/manuelle doit savoir balayer le(s)
  dossier(s) corbeille, quel que soit le modèle retenu.

Impact revert : le revert pointe vers le chemin trash journalisé (actions.record stocke le
`dest`). Tant que le chemin reste connu et stable, le modèle d'emplacement n'affecte PAS la
mécanique revert — à reconfirmer selon la piste retenue.

À FAIRE le moment venu : relevé dédié (carto trash actuel + chiffrage cross-disque réel mesuré
sur la machine d'Antoine) → choisir une piste → STOP → implémenter.


---

## Preuve — l'identification Discogs EST appliquée au fichier (nom + tags). Ne plus jamais re-douter.
Chaîne vérifiée bout en bout (lecture seule) :
1. Clic candidat → `onIdentityApplied` écrit dans `state.canonical` (artist/title/version). filing.ts:372-384.
2. Clic File → `doRanger` appelle `fileTrack(track.id, binRel, target, state.canonical)`. filing.ts:692.
3. Backend `execute_file` → `tagging::write_tags_full(.., &plan.canonical.artist, &plan.canonical.title, ..)`
   écrit les TAGS INTERNES du fichier physique, branche conformant (move) ET branche transcode.
   filing.rs:264-268 et 277-280.
=> Le nom (via previewName) ET les tags du fichier portent l'identité Discogs. Réel, sur disque.

## Diagnostic du vrai inconfort utilisateur
Pas un bug, pas un état manquant : il MANQUE un aperçu fidèle de l'écriture à venir, présenté
comme tel. Aujourd'hui le clic candidat mute une variable mémoire invisible ; l'écran change
(pochette, nom) mais rien ne relie "ce que je vois" à "ce qui sera écrit". Le seul instant où
l'intention devient fait = le File, et c'est déjà accompli. L'utilisateur n'a jamais un moment
"voici exactement ce qui va être écrit" AVANT de s'engager. Le mot "appliqué" est ambigu et à
bannir de l'UI.

Résolution par le redécoupage (déjà décidé) :
- AVANT (centre) : aperçu honnête = "le fichier s'appellera `Artist - Title (Version).aiff` ;
  ses tags porteront cet artiste/titre". Formulé comme une PRÉVISION, pas comme "appliqué".
- APRÈS (droite) : constatation = chemin + nom complet (avec ext) + taille réels. Preuve que
  l'aperçu disait vrai. La confiance se construit en voyant avant==après 2-3 fois.

## DÉCISIONS PRODUIT VERROUILLÉES (cette session)
- Auto-avance : après File en détail, ouvrir DIRECT le prochain pending (zéro clic).
- Confirmation Filed : option C = bandeau fin en haut du rail droit, contrôles du prochain dessous.
- Bandeau : UN seul à la fois (se remplace). Historique empilé = chantier séparé, derrière preuve.
- Redécoupage des rôles centre/droite :
  - CENTRE = le morceau (avant) : rapport/waveform/specs source + Identification Discogs +
    champs éditables (artiste/titre/version) + badge confiance. Identification REPLIÉE par défaut,
    dépliée au besoin (la zone est chargée). + aperçu d'écriture (nom + tags qui seront écrits).
  - DROITE = le résultat (après) : bandeau Filed↩ en haut, puis Chemin → Nom complet (avec ext)
    → Taille (ordre exact demandé). Choix de format + bouton File restent À DROITE (l'action vit
    près de son résultat).
- file_track mono à détacher (calque file_batch) — le freeze hi-res.
- Style (point 5) : fonctions inspirées d'EZ CD Audio Converter, mais PAS son UI "outil pro"
  datée — viser aussi les non-tech. Contrainte de style transverse, pas un ticket.

## PLAN — 3 étapes, 3 commits, 3 tests live (chacune laisse Sift fonctionnel)
### Étape 1 — déplacer Identification + métadonnées vers le CENTRE
Libère le rail droit. Identification/édition rendues au centre (repliable), à côté de la pochette
et du nom qu'elles modifient → le lien cause→effet devient visuel. Ajouter l'aperçu d'écriture
(nom + tags à écrire), bannir "appliqué". Touche : renderFoot (sortir l'éditeur du foot),
doIdentify/onIdentityApplied/renderCandidates/wireCandidateClicks (cibler le centre, pas foot),
updateHeaderName. RISQUE : ces fns ciblent foot/host aujourd'hui ; re-cibler proprement sans
casser le câblage candidats. Test : identifier+éditer marche, au centre, état d'écriture lisible.

### Étape 2 — rail droit = résultat (après) + bandeau Filed + auto-avance
Rail libre → y poser : bandeau Filed↩ (haut, border-left 2px success, radius 0), puis Chemin →
Nom complet → Taille. Format + File restent à droite. Retirer le `return null` de syncDetail:937
qui bloque l'avance ; laisser l'auto-avance reprendre (chemin openFilingInto(mid, items[0]) déjà
présent). Bandeau unique qui se remplace ; survit à renderFoot (1er enfant / re-posé). Test :
filer → résultat à droite → prochain s'ouvre au centre, zéro clic.

### Étape 3 — détacher file_track mono
Calque file_batch : phase 1 (plan) sync sous lock (échoue vite sur NoLibraryRoot), détacher
phase 2+3 sur thread, émettre file:done mono → le front pose le bandeau à réception (plus en
sync). Impact : doRanger n'a plus le batch_id en retour direct → l'obtient par l'événement.
Garde `acting` conservée. Test : filer un gros hi-res sans gel de fenêtre.

Ordre : 1 → test → STOP → 2 → test → STOP → 3 → test. NE PAS toucher : moteur revert, garde-fou
player syncDetail (state.track && paneIsOurs), file_batch, TRASH_PURGE_DAYS.


---

## Étape 1 — visuel VERROUILLÉ (centre = bloc Métadonnées)
Champs (3 catégories, prouvées par ipc.ts Candidate/AppliedIdentity) :
- ÉDITABLES (le cœur de correction d'Antoine) : Artiste, Titre, Version (inputs) + Genres (chips
  ajouter/retirer).
- LECTURE SEULE (viennent de Discogs, jamais corrigés main) : Label, Année.
- PASTILLE de release retenue (descriptif, NON écrit) : Label · Année · Pays · Format, + bouton
  « changer » → ré-affiche la liste candidats depuis la mémoire (pas de nouvel appel Discogs).
  Doublon label/année (pastille + lecture) ASSUMÉ : pastille = quelle édition retenue ; champs =
  ce qui s'écrit. Rôles différents, OK validé.
Vocabulaire VERROUILLÉ : bloc = « Métadonnées ». Action = « Fetch metadata from Discogs ».
Le mot « tags » est INTERDIT ici (réservé Rekordbox, futur, non codé). Jamais « appliqué ».
État par défaut : bloc REPLIÉ (bouton Fetch discret). États : 1 avant recherche → 2 liste de
candidats (pochette·label·année·pays·format) → 3 release choisie repliée + pastille + changer.
Aperçu sous le bloc : UNE ligne « Nom du fichier : Artist - Title (Version).ext » (option 1).
Le nom de fichier ≠ métadonnées (deux écritures distinctes) — l'aperçu ne montre que le nom car
les métadonnées écrites sont déjà les champs au-dessus.
Champs écrits au fichier (write_tags_full, via AppliedIdentity) : artist, title, (version via
canonical), label, year, styles(genres), cover. country/format NE sont PAS écrits (pastille only).

Sources : Discogs seule pour l'instant, mais NE PAS câbler « Discogs » en dur dans l'UI/structure
— nommer par la fonction. Sources multiples = chantier futur dédié (pas d'abstraction prématurée).

PRÊT À CODER. Prochain geste : recâbler doIdentify/renderCandidates/wireCandidateClicks/
onIdentityApplied pour cibler le CENTRE (#mid) au lieu du rail (#filfoot/host), sans casser le
flux candidats. Sortir l'éditeur de renderFoot. Test live étape 1 : identifier+éditer marche au
centre, pastille + changer OK, aperçu nom de fichier à jour, rien cassé.


---

## CHANTIER LAYOUT FLUIDE — cadrage (NON codé, vient APRÈS 1c)

### Déclencheur
Antoine : scroll partiel gênant (le max-height:48% de 1a) + veut une UI qui se redimensionne
dynamiquement, plein écran cohérent. Question soulevée : "panneaux latéraux ne devraient jamais
scroller". Cible retenue = pattern Claude Desktop (scroll par zone indépendant, activé seulement
au débordement ; pieds d'action ancrés ; colonnes élastiques à la fenêtre).

### PREUVE : la fenêtre est DÉJÀ redimensionnable
tauri.conf.json:16-22 → width 1200, height 820, minWidth 920, minHeight 640, **resizable: true**,
decorations false, fullscreen false. La contrainte "hauteurs fixes 169/580px" de l'ancienne
mémoire est PÉRIMÉE (ou concernait un mode strip disparu). Donc le pb n'est pas la fenêtre — c'est
que la mise en page interne ne suit pas la fenêtre (hauteurs en dur type max-height:48%).

### ARCHITECTURE RÉELLE (cartographiée, prouvée) — 4 zones, pas 3
1. **#nav** (menu navigation, extrême gauche) — items de vues en haut (data-view :
   Accueil/Revue/Écartés/Réglages/Bibliothèque, chrome.ts:94), `.nav-foot` ANCRÉ en bas
   (margin-top:auto → Réglages collé en bas, styles.css:63-66). Zone de progression des tâches
   de fond vit dans ce pied (progress-zone.ts foot.prepend(zone)), grandit vers le haut sans
   décaler. = ÉQUIVALENT SIDEBAR CLAUDE DESKTOP. Déjà ancré correctement.
2. **#qcol / .queue** (liste des morceaux à trier) — colonne suivante, redimensionnable par
   drag-handle #qdrag (app.js renderRevue, largeur qw clamp 140-380). = équivalent liste de
   conversations. Peut être longue.
3. **#mid** (centre) — rapport (report-view) + éditeur Métadonnées (.sift-fil-editor depuis 1a).
   Peut être long.
4. **.dest** (colonne droite, app.js:74) — empile : `<div class="col-h">Destination</div>` →
   `<div id="fldz">` (arbre dossiers) → `<div id="filfoot" margin-top:14px>` (rail action :
   éditeur en 1a en a été SORTI ; reste Format/File/Discard). DÉCOUVERTE : #filfoot n'est PAS une
   colonne séparée, il est EMPILÉ sous l'arbre dans .dest.

### CIBLE (pattern Claude Desktop appliqué aux 4 zones)
- #nav : items haut, pied (Réglages+progression) ANCRÉ. Scrolle quasi jamais. DÉJÀ OK.
- #qcol : scrolle elle-même si longue.
- #mid : scrolle lui-même si long.
- .dest : titre "Destination" ANCRÉ en haut → #fldz (arbre) SCROLLE SEUL si long → #filfoot
  (File/Format) ANCRÉ, TOUJOURS VISIBLE (intention explicite d'Antoine : l'arbre scrolle
  indépendamment pour que les actions restent affichées ; risque actuel = arbre long pousse File
  hors écran).
Principe unique partout : chaque zone scrolle indépendamment SEULEMENT si elle déborde ; les
pieds d'action restent ancrés ; les colonnes s'étirent à la taille de fenêtre (plein écran inclus).
RÈGLE latéraux : pas (b) "tronquer" — c'est (a/c) "chaque zone a son propre scroll au
débordement", l'arbre #fldz scrolle, le rail #filfoot reste ancré.

### À FAIRE le moment venu (après 1c)
Relevé d'implémentation : recenser TOUTES les hauteurs/max-height en dur, voir comment .dest est
câblé en hauteur, appliquer flex column + overflow par zone. Tester grand/petit/plein écran.
NE PAS anticiper en 1c (1c ne fait que RETIRER le max-height:48% cassé, pas reconstruire).

### ORDRE GLOBAL VERROUILLÉ
1c cosmétique (EN COURS, test live) → layout fluide → étape 2 (rail=résultat + Final name à
droite + auto-avance) → étape 3 (détacher file_track). Raison : régler le CONTENANT (layout)
avant d'ajouter du CONTENU (rail-résultat étape 2).


---

## CHANTIER MAJEUR (noté, NON codé) — Sift comme CHECKER de compatibilité CDJ
Déclencheur : Antoine tombe sur boothready.app (+ concurrent mixgoat.com). Veut "faire de Sift
un vrai checker compatibilité CDJ". À creuser APRÈS Filed↩ + layout fluide. Repositionne Sift :
"déplacer = encoder + ranger" → "VÉRIFIER + encoder + ranger". Différenciateur vs Boothready/
MixGoat : eux réparent mais ne RANGENT pas ; Sift ferait les deux.

### Le problème central (que Sift adresse déjà en partie)
rekordbox (moteur logiciel permissif) lit des fichiers qui PLANTENT sur un CDJ physique (décodeur
hardware strict). Erreurs E-8305 (DATA FORMAT) / E-8304 (DECODE) / E-8302 (PLAYER, CDJ-3000 MP3
strict) / E-8306 (NO FILE, db≠fichier). "Ça marche sur mon ordi mais pas au club."

### Les 6 familles de pièges CDJ (sources : boothready.app, mixgoat.com, forums Pioneer)
1. **WAV/AIFF 32-bit float** — export par défaut des DAW (Ableton/FL/Logic/Bitwig). CDJ = PCM
   16-bit (et 24-bit sur CDJ-3000) à 44.1/48 kHz uniquement. Identique à un WAV normal à l'œil.
   → fix : convertir 16-bit PCM avec dithering. CONCERNE roommate FL + promos Antoine.
2. **En-tête WAVE_FORMAT_EXTENSIBLE (format tag 0xFFFE au lieu de 0x0001 PCM)** — piège
   Bandcamp/promo. Le fichier peut être 16/44 stéréo VALIDE mais l'en-tête le fait rejeter.
   → fix : ré-encoder / réécrire le format tag (bytes 20-21 → 01 00).
3. **Multicanal (>2 canaux)** — Bandcamp surround/Atmos, stems. Déclenche E-8305 quel que soit
   sample rate/bit depth ; utilise presque toujours l'en-tête EXTENSIBLE. → fix : downmix stéréo
   (ffmpeg -ac 2).
4. **Sample rate hors plage** — 88.2/96 kHz limite, 176.4/192 kHz refusé (hi-res Beatport/
   Qobuz/promo). CDJ = 44.1/48 kHz sûr. → fix : resample.
5. **FLAC/ALAC selon MODÈLE** — FLAC OK seulement CDJ-3000/TOUR1 firmware ≥1.20 ; E-8305 sur
   2000NXS2 et antérieurs. ALAC (.m4a Apple Lossless) : AUCUN CDJ. DRM .m4p : irréparable.
   → règles PAR MODÈLE de player.
6. **Hors-fichier** : USB NTFS non reconnu (CDJ veut FAT32/HFS+, limite 4 Go/fichier en FAT32) ;
   chemin complet > 256 caractères ; caractères invalides ; doublons. → checks structurels.

### LIEN ID3 (ce qu'Antoine pressentait, CONFIRMÉ)
Le diagnostic "Tags" de Sift (cover, ID3 version) n'est PAS qu'informatif. Cas réel forum Pioneer :
un .aif a déclenché E-8305 à cause de frames ID3 défectueuses (frame TDR ID3 v2.2 abandonnée,
frame TCON avec nils résiduels = mauvaises données d'en-tête) ; la réparation = nettoyer les tags.
Donc ID3 corrompu = cause de plantage CDJ. Métadonnées et compatibilité se rejoignent. La section
"Tags" qu'on déplace en 1c pourrait devenir un POINT DE CONTRÔLE, pas qu'un affichage.

### CE QUE SIFT FAIT DÉJÀ vs CE QUI MANQUE (à PROUVER par relevé du code de verdict)
- DÉJÀ (correction) : convertit non-conforme → AIFF/WAV 16/44 ou MP3 320, refuse upscale lossy→
  lossless, tag, range. La conversion règle 32-bit float + sample rate PAR EFFET DE BORD si on
  convertit.
- TROU PROBABLE (détection) : Sift juge sur format/sample rate/bit depth. Il considère
  vraisemblablement un WAV EXTENSIBLE 16/44 stéréo comme "conforme" → il PLANTERA pourtant au club
  (en-tête). Sift ne vérifie probablement PAS : le format tag (0x0001 vs 0xFFFE), le nombre de
  canaux, la validité des frames ID3, ni les règles par modèle de CDJ, ni les checks USB/chemin.
- À FAIRE (relevé dédié, lecture seule) : lire le code de verdict Rust (analysis/probe — où
  conforme/non-conforme est décidé) et établir lesquels des 6 pièges passent à travers
  aujourd'hui. Symphonia + un read du header WAV/AIFF donnent déjà accès au format tag/canaux ;
  lofty lit les ID3. FFmpeg fait déjà les fixs (resample, downmix -ac 2, ré-encode).

### Ampleur / positionnement
Gros chantier, pas une feature. Le faire méthodiquement APRÈS Filed↩ + layout. Décision produit
à trancher le moment venu : Sift "vérifie + corrige + range" en un geste (cohérent ADN) ; option
de cibler un MODÈLE de CDJ (profil) pour les règles FLAC/ALAC ; le check USB/chemin n'a de sens
qu'au moment de l'export Rekordbox/clé (M7, pas encore codé). Ne PAS tout embarquer d'un coup :
commencer par la DÉTECTION des pièges format (32-bit float, EXTENSIBLE, multicanal, sample rate)
qui sont les plus fréquents et déjà à moitié couverts par la conversion.


---

## POSITIONNEMENT / NAMING — réflexion (NON tranchée, pour la phase promo)
Antoine doute que "Sift" soit le meilleur nom ; "Boothready" parle plus (nom = promesse).

NATURE DE SIFT (précisée par Antoine, important) : PAS un gestionnaire de bibliothèque quotidien.
Outil de PRÉPARATION ÉPISODIQUE, orienté tâche, sorti à des moments précis :
- préparer un set,
- corriger des playlists Rekordbox,
- vérifier de nouveaux fichiers téléchargés,
- trier toute la bibliothèque actuelle d'un coup (one-shot).
Toute la valeur (biblio, rangement, check) converge vers UNE promesse : que ça marche sans prise
de tête quand le DJ branche sa clé au club. Donc Sift = mono-promesse, pas multi-fonction.

CONSÉQUENCE NAMING : pour un utilitaire mono-promesse, orienté tâche, qu'on sort ponctuellement,
la DÉCOUVRABILITÉ (nom cherchable type "fix CDJ", SEO sur codes d'erreur) bat souvent la
MÉMORABILITÉ (jolie marque). Les gens cherchent une solution à leur problème, pas une marque. →
"Sift" n'est peut-être PAS optimal ; Antoine a raison de le sentir. MAIS : ne pas copier
Boothready/MixGoat (créneau encombré, Sift est DIFFÉRENT = intégré au workflow, pas réparateur
ponctuel). Cible idéale du nom : (a) dit la promesse DJ/club, (b) capture l'angle tout-en-un /
intégré qui distingue de Boothready, (c) reste possédable (trademark). Peut être descriptif-à-soi
OU marque + tagline-promesse.
NE SE DÉCIDE PAS À CHAUD. Chantier "stratégie produit/marketing", à mener avec la promo YouTube :
finir d'explorer le marché (concurrents, noms, angles libres) → cadrer positionnement → chercher
noms. Lien avec le chantier checker CDJ (la nouveauté = intégration check+encode+range).

NB révision : mon 1er conseil "garde la marque Sift" était trop générique (vaut pour produit large
quotidien type Notion/Figma, pas pour utilitaire mono-promesse cherchable). Réflexion ouverte.


---

## RELEVÉ LAYOUT FLUIDE — cause ISOLÉE (lecture seule, styles.css lu en entier 149 lignes)

### Théorie du crime PROUVÉE : le layout fluide est déjà à 90% en place. Seul .dest dévie.

Cartographie hauteur/scroll des 4 colonnes (styles.css) :
- **.wrap** (racine) : height:100vh; display:flex; flex-direction:column. ✓ socle plein écran.
- **.title/.pitch/.sub** : flex:none (en-tête fixe). ✓
- **.pa** (zone principale, contient les colonnes) : flex:1; min-height:280px; display:flex;
  overflow:hidden. ✓ délègue le scroll aux colonnes, ne scrolle pas elle-même.
- **.sb** (menu/sidebar, = "#nav") : width:200px; flex:none; flex column. .nav-foot{margin-top:
  auto} → Réglages + progression ancrés en bas. ✓ DÉJÀ pattern Claude Desktop.
- **.queue** : flex:none; flex column; overflow:hidden + #ql{flex:1; min-height:0; overflow-y:
  auto}. ✓ la LISTE scrolle seule, titre/pbar restent fixes. DÉJÀ correct.
- **.mid** (centre) : flex:1; min-width:0; flex column; overflow:hidden + .mid-scroll{flex:1;
  min-height:0; overflow-y:auto; flex column}. ✓ DÉJÀ correct (conteneur fixe + zone scroll
  interne). NB : ceci EXISTE — mais le rapport openReportInto/.sift-fil-report peut avoir SON
  PROPRE overflow:auto imbriqué dans .mid-scroll → reste de nesting noté en 1c. À re-vérifier au
  moment du fix : si .mid-scroll suffit, retirer l'overflow interne du rapport.

### LA CAUSE (ligne 134) — .dest scrolle EN ENTIER
.dest{width:260px;flex:none;display:flex;flex-direction:column;padding:13px 12px;...;**overflow-y:auto**}
→ overflow-y:auto est sur la COLONNE ENTIÈRE. Quand l'arbre #fldz est long, TOUTE la colonne
scrolle : titre "Destination" + arbre #fldz + rail #filfoot (File/Format) ensemble. Donc le
bouton File peut sortir de vue. C'est le problème pressenti par Antoine.
Structure HTML (app.js:74) : .dest > [col-h "Destination"] + [#fldz arbre] + [#filfoot rail].
Aucune sous-zone scrollable dédiée (contrairement à .queue/#ql et .mid/.mid-scroll).

### FIX CIBLÉ (chirurgical, PAS une refonte) — à coder ensuite
Reproduire le pattern .queue/.mid sur .dest :
- .dest : overflow-y:auto → **overflow:hidden** (la colonne ne scrolle plus en bloc).
- titre "Destination" (.col-h) : flex:none (ancré haut). Déjà non-flex, OK.
- arbre #fldz : devient la zone scrollable → **flex:1; min-height:0; overflow-y:auto**.
- rail #filfoot : flex:none (ancré bas, toujours visible). margin-top:14px actuel à conserver/
  ajuster.
Résultat : titre + File ancrés, seul l'arbre scrolle s'il déborde. Élastique à toute taille de
fenêtre (resizable:true déjà en place). Tester grand/petit/plein écran + arbre court/long.

### Autres overflow vus (non concernés, RAS) : .home-left/.home-right (vue Accueil) ont déjà
leur propre overflow-y:auto — vue distincte, pas touchée par ce chantier.

### CONCLUSION : le "chantier layout fluide" se réduit à UN fix ciblé sur .dest (+ vérifier le
nesting résiduel du rapport dans .mid-scroll). Beaucoup plus petit que craint. 3 colonnes/4 déjà
conformes.


---

## "FILER SUR PLACE" — cadrage validé (NON codé, chantier séparé APRÈS le layout)
Antoine a confirmé la définition et le périmètre. À coder dans son propre relevé/prompt, pas
mélangé au layout.

DÉFINITION VALIDÉE : "filer sur place" = filer un morceau en pointant la DESTINATION sur son
DOSSIER SOURCE, au lieu d'un dossier de l'arbre bibliothèque. PAS destructeur — c'est la même
mécanique de filing (encode + tag + rename via le plan existant), avec bin = dossier source.
Le filet revert reste INTACT (original → .sift-trash journalisé, comme un filing normal).

PREUVE CODE : la destination de filing est un CHEMIN PHYSIQUE RÉEL, pas une biblio interne.
settings.rs:7 — LIBRARY_ROOT = "chemin absolu de la racine bibliothèque sous laquelle vivent les
bins" ; un bin = sous-dossier réel (bin_rel). Filer dans "House/" = écrire physiquement dans
<LIBRARY_ROOT>/House/. Donc "sur place" = pointer la destination sur le dossier source du fichier.
Confirmé par Antoine : "l'arbre biblio se réfère à un chemin physique réel, pas une biblio
interne de Sift".

VITESSE / REVERT (rappel décision actée) : l'original n'est JAMAIS détruit (revert sûr = principe
non négociable). Détruire l'original ne gagne AUCUNE vitesse (le coût d'un filing = l'encode
ffmpeg ; le déplacement de l'original est un rename instantané same-disk). Donc pas de "mode
écrasement sec". (≠ chantier corbeille centrale vs same-disk, qui reste lui non tranché.)

PÉRIMÈTRE PROBABLE (à chiffrer dans le relevé dédié) : exposer "dossier source" comme cible de
filing (mono et/ou batch), dans l'UI de destination (.dest). Petit et sûr si la lecture ci-dessus
tient. À faire le moment venu : relevé dédié → prompt borné → test live.


---

## CLÔTURE LAYOUT — 2 fix validés en live, 2 commits séparés
Chantier layout bouclé. Le "layout fluide" s'est réduit à 2 fix ciblés (le reste était déjà bon).

FIX 1 (scroll) — styles.css + filing.ts:839 :
- .dest : overflow-y:auto → overflow:hidden. #fldz : flex:1;min-height:0;overflow-y:auto (arbre
  scrolle seul). #filfoot : flex:none (File ancré, toujours visible).
- #mid : wrapper .sift-fil-scroll (flex:1;min-height:0;overflow:auto) enrobant .sift-fil-report
  + .sift-fil-editor ; overflow retiré du seul report (évite double scroll). .sift-fil-dup reste
  flex:none en tête. .sift-fil-report/.sift-fil-editor restent queryables (seq-guard player OK).
- Résultat : colonne droite = arbre scrolle / File visible ; centre scrolle d'un bloc. Validé live.
Commit : fix(layout) scroll par zone.

FIX 2 (bug SPACE) — app.js:311, cause racine PROUVÉE :
- Symptôme : SPACE dans la Revue jouait le son (installFilingKeys/togglePlay OK) MAIS repeignait
  les données de DÉMO du mockup (« Mr. Fingers » = T[0] de app.js).
- Cause : main.ts:5 importe app.js INCONDITIONNELLEMENT → le mockup tourne même en Tauri. Son
  handler keydown (app.js:310, sur #pa) ligne 314 : e.key===' ' → playing=!playing;renderMid()
  réaffichait T (démo). Handler VESTIGE du mockup, doublon d'installFilingKeys (live). Pas de
  stopImmediatePropagation côté live → les deux tiraient. (NB : bug présent depuis toujours,
  jamais déclenché car Antoine n'avait jamais testé SPACE ; PAS causé par le fix scroll — fausse
  piste écartée.)
- Fix : garde `if('__TAURI_INTERNALS__' in window) return;` en tête du handler app.js (même test
  que main.ts:14 inTauri). Hors Tauri (démo Vercel) le mockup garde son clavier complet. Live
  intouché. Validé : SPACE joue le son sans basculer sur la démo ; ↑/↓/Enter/X/I OK.
Commit séparé : fix(revue) garde Tauri sur handler clavier vestige.

ARCHI NOTÉE (utile pour la suite) : app.js (mockup statique, données T factices + son propre
render/handlers) tourne TOUJOURS ; la couche live (sift-live.ts/filing.ts) se superpose en Tauri.
Les handlers/rendus du mockup sont des vestiges potentiels à garder par inTauri si jamais ils
ré-émergent (pattern: `if('__TAURI_INTERNALS__' in window) return;`).

FILE D'ATTENTE (ordre) : "filer sur place" (cadré) → étape 2 (rail=résultat + Final name à
droite + auto-avance, retirer return null syncDetail:937) → étape 3 (détacher file_track) →
checker CDJ.


---

## "FILER SUR PLACE" — UI validée + relevé code (cadrage avant prompt)

### Ligne directrice UI VALIDÉE (Antoine, maquette ok)
"Sur place" n'est PAS une action séparée — c'est une DESTINATION de plus, présentée selon le mode :
- Mode DÉTAIL : une case "Sur place (dossier source)" en bas de la colonne destination .dest,
  sous l'arbre #fldz, au-dessus de File. Cochée → File envoie CE morceau dans son dossier source.
- Mode BATCH : une entrée "Dossier source de chaque morceau" dans le sélecteur de destination du
  lot (binSelectHtml, le dropdown actuel Library root/House/Techno…). Choisie → "Traiter N sur
  place" envoie CHAQUE morceau dans son propre dossier source.
MÊME moteur, MÊME filet (original → corbeille revertable, fichier propre reste sur place). Pas de
logique dupliquée, un seul modèle mental : "je choisis où ça va, puis je file/traite".

### Comportement VALIDÉ (Antoine) : convertir/nettoyer + laisser le résultat dans le dossier
source ; l'original part à la corbeille .sift-trash (revertable). PAS de destruction sèche.

### Relevé code (lecture seule, filing.rs)
- plan_file(conn, root, template, track_id, **bin_rel**, override_target, edited) (filing.rs:198)
  construit la destination ligne 227 : `dest_dir = library::safe_join(root, bin_rel)`. Donc
  destination = root + bin_rel. PREUVE que la destination est un chemin physique réel.
- PIÈGE : safe_join(root, bin_rel) suppose une cible SOUS root. Or "sur place" = dossier du
  SOURCE, qui peut être HORS de root (sources de partout). Donc on NE peut PAS passer par
  safe_join/bin_rel pour "sur place" → il faut un point de bascule : si "sur place", dest_dir =
  dossier parent du `source` directement (Path::new(&source).parent()), en IGNORANT root/bin_rel.
- CAS CONFORMANT vs NON (filing.rs:224-225, prouvé) : un fichier conformant est DÉPLACÉ tel quel
  (garde son extension, pas de transcode) ; un non-conformant est TRANSCODÉ (nouveau fichier).
  Conséquence "sur place" : un conformant filé dans son propre dossier source = quasi no-op
  (renommage au même endroit selon le template) ; un non-conformant = nouveau fichier propre dans
  le dossier source + original → corbeille. Cohérent avec le besoin "nettoyer un dossier de
  téléchargements". ensure_unique gère déjà les collisions de nom.
- Le moteur de filing (phases 1/2/3, FilePlan) reste identique — seul le calcul de dest_dir change
  selon un drapeau "sur place". L'original part déjà à la corbeille via la mécanique existante
  (move_to_trash / trash_file_fs), revert intact.

### À CADRER ENCORE avant prompt (points ouverts du relevé)
1. Front : comment passer le drapeau "sur place" jusqu'à plan_file (mono via fileTrack ; batch via
   file_batch). Probable : une valeur de bin_rel sentinelle (ex. "__SOURCE__") OU un param booléen
   dédié. Décider proprement (pas de fallback ambigu).
2. binSelectHtml (sift-live.ts:384) : ajouter l'entrée "Dossier source de chaque morceau".
3. Colonne .dest détail : ajouter la case (renderFoot/renderEditor ? à localiser).
4. Cas limite conformant no-op : que montrer à l'utilisateur (le morceau est déjà propre et à sa
   place) — message ? rien ? À trancher.
NON codé. Prochaine session "filer sur place" : finir ces 4 points → prompt borné → test live.


---

## "FILER SUR PLACE" — 4 points ouverts TRANCHÉS (prêt pour prompt)

PT1 (front → moteur) TRANCHÉ : SENTINELLE de binRel. fileTrack(trackId, binRel, …) et
fileBatch(trackIds, binRel) passent TOUS deux la destination comme une simple string binRel
(ipc.ts:68 et :85) jusqu'à plan_file. Donc "sur place" = valeur réservée binRel = "__SOURCE__".
Côté Rust plan_file (filing.rs:198) : si bin_rel == "__SOURCE__" → dest_dir = Path::new(&source)
.parent() (dossier du source), au lieu de safe_join(root, bin_rel). UN SEUL point de bascule,
ZÉRO nouveau paramètre à propager (le booléen inPlace toucherait 6 endroits → écarté). Voyage dans
le canal binRel existant. Risque collision nom de dossier "__SOURCE__" ≈ nul (bins viennent de
l'arbre, pas d'une saisie libre).

PT2 (dropdown batch) TRANCHÉ : binSelectHtml (sift-live.ts:384) construit les <option> depuis
batchBins. Ajouter une option en tête value="__SOURCE__" libellé "Dossier source de chaque
morceau". Sélectionnée → batchBin="__SOURCE__" → fileBatch(ids,"__SOURCE__"). Mécaniquement
identique à un dossier normal.

PT3 (case détail) TRANCHÉ sur le principe : case "Sur place (dossier source)" en bas de .dest
sous l'arbre #fldz. Cochée → File appelle fileTrack(id, "__SOURCE__", …) au lieu du binRel choisi
(la case écrit "__SOURCE__" dans la destination courante de l'état filing). Emplacement exact
(renderFoot vs colonne .dest) à confirmer au moment du prompt. Même mécanisme que le batch.

PT4 (cas no-op conformant) TRANCHÉ : PAS de cas spécial. Un morceau déjà conforme filé sur place
= traité comme un filing normal (confirmation Filed habituelle, renommage au même endroit via
template + ensure_unique, original → corbeille). Raffiner SEULEMENT si ça gêne à l'usage. Ne pas
coder de subtilité prématurée.

### PRÊT POUR PROMPT (quand on l'attaque) — périmètre
Rust : plan_file gère bin_rel=="__SOURCE__" → dest_dir = parent du source (ignore root/safe_join).
Vérifier que le reste (conformant move / non-conformant transcode, ensure_unique, original →
trash, phases 1/2/3, revert) marche tel quel avec ce dest_dir. Front : option dropdown batch
(binSelectHtml) + case détail (.dest) écrivant "__SOURCE__". Test live : (a) batch sur place d'un
dossier mixte (conformes + non-conformes) → fichiers propres dans leurs dossiers source, originaux
en corbeille, revert OK ; (b) mono sur place via la case ; (c) un déjà-conforme = filing normal.
Constante "__SOURCE__" définie UNE fois (shared/contracts ou un const partagé), pas en dur
dispersé.


---

## ÉTAPE 2 — relevé (rail=résultat + auto-avance) — théorie PROUVÉE, non codé

### Mécanique actuelle (prouvée, filing.ts)
- doRanger (clic File) : fileTrack(...) ligne 714 → puis showFiledConfirm. Commentaire 715-717 :
  "le panneau ne s'auto-avance plus après filing" (comportement intermédiaire actuel).
- showFiledConfirm (732+) : écrit la carte "Filed ✓ ↩ Revert" dans #mid (LE CENTRE), met
  state.track=null (737), state.filedConfirm=set, et VIDE le rail #filfoot (750). C'est la version
  intermédiaire — PAS la cible.
- syncDetail (964-989) décide quel morceau afficher au refresh de la queue :
  * L.968 : `if (state.filedConfirm && mid.querySelector(".sift-filed-confirm")) return null;`
    ← C'EST LE BLOCAGE de l'auto-avance (commentaire : "ne PAS auto-avancer vers le prochain").
  * Garde-fou player (paneIsOurs + state.track) L.972-977 : si morceau ouvert & panneau intact →
    NE JAMAIS switcher (sinon tue le player en chargement → waveform sans son). Avertissement fort.
  * L.980-987 : si pas de morceau ouvert → openFilingInto(mid, items[0]) = L'AUTO-AVANCE, qui
    EXISTE DÉJÀ et marche. Elle est juste court-circuitée par le return null L.968.

### Théorie étape 2 (limpide)
L'auto-avance n'est pas à construire (elle existe L.985-987). Elle est DÉSACTIVÉE par : (1) le
return null L.968, (2) showFiledConfirm qui prend #mid + met state.track=null + bloque. Pour
l'étape 2, INVERSER le flux :
- showFiledConfirm n'écrit plus dans #mid (centre) mais dans le RAIL (#filfoot/.dest) = "panneau
  résultat/après".
- retirer/conditionner le return null L.968.
- après filing, syncDetail charge le morceau suivant dans #mid (auto-avance existante).

### RISQUE CENTRAL (prouvé, à respecter dans le prompt)
Commentaire filing.ts:973-976 (lettres de sang) : "si un morceau est ouvert et le panneau intact,
ne JAMAIS switcher — sinon détruit le player en plein chargement et coupe l'audio (waveform depuis
peaks mais pas de son)". C'est le bug waveform-sans-son déjà combattu. L'étape 2 marche sur un fil :
auto-avancer APRÈS filing (le morceau filé n'est plus → switcher légitime) SANS casser la règle qui
protège le player PENDANT l'écoute (switcher interdit). Distinguer "morceau filé → avancer" de
"morceau juste analysé → ne rien toucher" = LA subtilité à coder juste.
Aussi : installFilingKeys fait `if(!state.track) return` (911) → après auto-avance, state.track
DOIT pointer le nouveau morceau sinon le clavier (SPACE/Enter/…) meurt. openFilingInto repose
state.track (834), donc ça suit si l'avance passe bien par openFilingInto.

### À CADRER (décisions produit, avant prompt) — NON tranché
1. Où va le bandeau "Filed ↩" dans le rail : en haut de #filfoot ? format exact (option C décidée
   = bandeau fin en haut du rail, contrôles du prochain morceau dessous).
2. UN bandeau à la fois (pas d'historique empilé — chantier séparé déjà noté).
3. Le "Final name" migre-t-il à droite À CE MOMENT (décision actée : oui à l'étape 2) → le rail
   devient résultat/après (Chemin → Nom complet → Taille).
4. Revert depuis le rail : le bouton ↩ doit garder le batch_id (le bandeau en a besoin). Si étape 3
   (file_track détaché) pas encore faite, doRanger a encore le FileResult en retour direct → OK ;
   sinon via événement.
RISQUE = player. À faire à tête reposée. Prompt borné + test live : filer → avance auto au suivant,
bandeau Filed dans le rail, SPACE joue le NOUVEAU morceau (clavier vivant), revert depuis le rail OK.


---

## ÉTAPE 2 — placement du bandeau Filed : DÉCISION = A (pied de rail)

CODÉ (non commité) : étape 2 fonctionne, 6 points testés OK (auto-avance, bandeau rail, SPACE joue
le nouveau morceau avec son, revert ciblé, croix, garde-fou player préservé). Implémentation Claude
Code : doRanger auto-avance via listQueue()→openFilingInto(mid,items[0]) (chemin existant réutilisé,
pas de clone) sinon clearPane ; showFiledConfirm(batchId,bin,filedPath) prepend .sift-filed-banner
dans #filfoot ; syncDetail return-null mort retiré, garde-fou player intact ; renderFoot préserve le
.sift-filed-banner à travers son innerHTML ; doRevert retire le bandeau au lieu de clearPane (ne
yank pas le player du morceau auto-avancé). 2 écarts assumés : (a) pas de taille dans le bandeau
(FileResult = {path,batch_id} seulement, pas d'octets — l'ajouter = étendre FileResult Rust/IPC,
hors périmètre étape 2 ; petite étape séparée si voulu) ; (b) revert silencieux (reste sur le morceau
courant, le fichier reverté revient dans la queue via queue:changed) au lieu de sauter sur le fichier
reverté — choisi pour ne pas casser le player.

PLACEMENT actuel = au-dessus de Format (prepend en tête de #filfoot, donc au-dessus de la pile
Format→File→Discard). Antoine pas satisfait.
DÉCISION = OPTION A : bandeau en PIED de rail, SOUS File + Discard. Contrôles (Format/File/Discard)
gardent leur place habituelle en haut, inchangés. = append dans #filfoot au lieu de prepend, + dans
renderFoot restaurer le bandeau en append (foot.append) au lieu de prepend.
RAISON (Antoine) : cohérence FUTURE avec le mode batch — Antoine anticipe un batch où l'état des
fichiers traités défilera en PIED de rail ; aligner le détail dès maintenant = un seul endroit "ce
qui vient de se passer" dans les deux modes.
PARI ASSUMÉ (relevé honnête) : le batch ACTUEL met son résumé (Selection/Destination/Will encode)
en HAUT de #filfoot (sift-live.ts:393-404) et sa progression part dans la zone de progression du
nav rail GAUCHE (onFileProgress/onFileDone, ~196-200), PAS en pied de #filfoot. Donc "l'état défile
en pied de #filfoot" est une INTENTION future, pas l'existant. Si plus tard le défilé batch va dans
la zone gauche, A ne serait plus aligné — mais re-déplacer (append↔prepend) reste trivial. Pari
conscient, coût quasi nul, on y va.
Options écartées : B (au-dessus de Destination = sortir le bandeau de #filfoot vers #fldz/.dest,
plus de travail) ; C (centre #mid en haut = re-toucher la zone player stabilisée, plus risqué,
jugé plus élégant par Antoine mais sacrifié pour la cohérence batch).


---

## CHANTIER FUTUR — rapatrier l'état/progression BATCH vers le pied du rail droit

INTENTION (Antoine, lecture 2 confirmée) : aujourd'hui l'état + la progression du mode BATCH
vivent dans le NAV RAIL GAUCHE (zone de progression, via onFileProgress/onFileDone, sift-live.ts
~196-217 ; fileNote/#filfoot pour le feedback immédiat du Stop). Antoine l'avait mis à gauche "un
peu par défaut". À TERME il veut le rapatrier en PIED du RAIL DROIT (#filfoot), pour qu'il rejoigne
le bandeau Filed du mode détail (option A, placé en bas du rail droit). But = UN SEUL endroit "ce
qui vient de se passer / ce qui défile" en pied de rail droit, cohérent entre détail et batch.

CONSÉQUENCE sur le micro-prompt "placement A" (déplacement vertical du Filed haut→bas) : NE PAS le
traiter comme une fin en soi isolée. Le placement A n'est que la 1re moitié de ce chantier de
convergence. Penser le déplacement Filed + rapatriement batch ENSEMBLE le jour où on l'attaque,
plutôt que toucher #filfoot deux fois. Tempo laissé à Antoine.

À RELEVER quand on l'attaquera (lecture seule d'abord) : où exactement la progression batch est
rendue à gauche (nav rail / progress-zone.ts : setTask/clearTask/setCancelHandler), ce qui
déclenche son affichage (onFileProgress/onFileDone dans sift-live.ts), et comment la déplacer en
pied de #filfoot SANS casser le Stop-net (fileCancel) ni le compteur figé pendant un encode. Risque
= la zone de progression est aussi utilisée par l'ANALYSE (pas que le filing) — vérifier qu'on ne
déplace que la partie filing/batch, pas l'analyse. À cadrer en maquette avant prompt.

STATUT : noté, NON cadré, NON codé. Le placement vertical A du Filed (micro-prompt déjà rédigé)
attend cette décision de tempo — soit on le fait seul maintenant, soit on l'absorbe dans ce
chantier de convergence.


NAMING — idée "marque + descripteur accolé" (Antoine, à chaud, à mûrir en phase promo)
Antoine : et si on attachait toujours un descripteur au nom, type "Sift, DJ assistant", pour
clarifier ce que fait l'app (Sift seul ne dit rien — un inconnu ne sait pas si c'est un jeu, un
sampler, un plugin) ? Réflexe JUSTE : le motif "Nom — Descripteur" marche précisément pour les
outils au nom abstrait (Notion, Linear, Arc). Il résout la gêne sans renoncer au nom.
NUANCE (le descripteur porte le positionnement) : "DJ assistant" est clair MAIS trop large/muet —
ça évoque un outil de mix ou de reco, pas la promesse précise de Sift (préparation : encoder +
ranger pour que ça marche au club). "Assistant" sous-vend. Or on a acté que pour un utilitaire
mono-promesse, la DÉCOUVRABILITÉ (mots que les DJ tapent quand ils ont le problème) bat la
mémorabilité. Le descripteur est justement le bon endroit pour injecter ces mots ET expliquer.
Donc : pas n'importe quel descripteur — celui qui dit la promesse précise ET attrape la recherche.
Pistes par angle (à départager en promo, PAS à chaud) :
  - rangement : "Sift — DJ library prep" / "prep your DJ library"
  - compatibilité (l'angle CDJ = différenciateur identifié) : "Sift — get your tracks CDJ-ready"
  - geste : "Sift — encode & file your music"
STATUT : idée capturée, cohérente avec la doctrine naming ci-dessus (marque + tagline-promesse).
À trancher en phase promo, avec le nom lui-même, la page tuple.live et le pitch — PAS en fin de
session de dev. Lien direct avec le chantier checker CDJ (le descripteur "CDJ-ready" n'a de poids
que si le check existe).


---

## ÉTAPE 2 — CLÔTURÉE (testée OK, à commiter)

Placement A appliqué et testé : bandeau Filed en BAS de #filfoot (sous Discard). Claude Code :
prepend→append aux 3 endroits (showFiledConfirm + les 2 points de préservation de renderFoot),
margin-bottom→margin-top, ET commentaires périmés "prepended/above"→"appended/below" corrigés.
tsc vert. 3 points live OK : (a) bandeau en bas sous Discard ; (b) clic chip format → bandeau reste
en bas (préservation à travers renderFoot OK) ; (c) ↩ Revert + ✕ marchent depuis le bas.
ÉTAPE 2 COMPLÈTE : auto-avance après filing (centre saute au prochain pending via
listQueue→openFilingInto, chemin réutilisé) + bandeau Filed dans le rail (pas dans #mid) + garde-fou
player préservé (SPACE joue le nouveau morceau avec son, refresh analyse ne switche pas) + revert
ciblé batch_id depuis le bandeau + croix. 2 écarts assumés : pas de taille dans le bandeau
(FileResult={path,batch_id}, l'ajouter = étendre FileResult Rust/IPC, petite étape séparée si voulu)
+ revert silencieux (reste sur le morceau courant, fichier reverté revient via queue:changed, ne
yank pas le player).
COMMITS : feat(filing): étape 2 — auto-advance after filing + Filed banner in rail | puis relevé
séparé docs(audit): étape 2 placement + chantier convergence batch + idée naming.

ÉTAT FILE D'ATTENTE après étape 2 :
- "filer sur place" : 4 points tranchés, prompt PRÊT (non rédigé en entier, périmètre dans le relevé). PROCHAIN candidat code.
- chantier convergence batch (état batch gauche → pied rail droit) : noté, non cadré. Absorbera peut-être un re-look du placement Filed.
- étape 3 (file_track mono détaché façon file_batch) : noté.
- checker CDJ : chantier majeur, après.
- naming + descripteur accolé : phase promo, pas à chaud.


---

## CONCURRENTS / CHECKER — comparatif prouvé par le code (corrige une explication antérieure fausse)

### CORRECTION IMPORTANTE : Sift fait DÉJÀ la fake-detection spectrale
Lecture du code (src-tauri/src/analysis/verdict.rs + worker.rs) : Sift analyse l'AUDIO, pas
seulement le format déclaré. Mon comparatif marketing antérieur ("MLD analyse l'audio, Sift juge le
format déclaré") était FAUX. verdict.rs est dédié à la détection de fraude par cutoff spectral :
- mesure cutoff_hz réel (où le spectre s'arrête) — stocké en base (worker.rs:75).
- LOSSLESS déclaré (FLAC/AIFF/WAV) : cutoff ≥ 20000 Hz (LOSSLESS_OK_HZ) → authentique ; cutoff bas
  → FAKE lossless (MP3 ré-encapsulé) ; entre les deux → zone grise.
- LOSSY déclaré : table du cutoff minimum attendu par bitrate (320→19000, 256→18000, 192→16500,
  160→15500, 128→14500). Déclaré 320 qui coupe à 16k → transcodé up depuis source pourrie → fraude.
- MP3 honnêtement bas (128 coupant à 14.5k) reste Ok (pas puni d'être un vrai 128).
- AUTRES mesures déjà calculées et stockées : clip_runs, clip_pct, true_peak_dbtp, dc_offset,
  phase_correlation, dual_mono, truncated, silence_head/tail_ms, container_ok, codec_error,
  id3_version, has_cover, tags_cdj_ok, spectrogram, peaks. (worker.rs persist_report)
→ La question d'Antoine (départager bons/mauvais rips vinyl) a DÉJÀ une réponse dans Sift : un
mauvais rip / transcode qui coupe bas est flaggé par verdict.rs. Améliorations possibles : (1) mieux
EXPOSER le verdict + le spectre à l'utilisateur (MLD montre un graphe par morceau + score 1–10 ;
vérifier ce que Sift affiche réellement aujourd'hui) ; (2) affiner les seuils (déjà reconfigurables,
"Réglages M2b+" verdict.rs:14).

### Music Library Doctor (musiclibrarydoctor.com) — concurrent sérieux
Fait : Track Matcher (import playlists Spotify/YouTube Music → Rekordbox/Serato/VirtualDJ, complète
les manquants) ; score qualité FFT 1–10 + fake-320/fake-FLAC avec graphe spectral par morceau ;
scan doublons par empreinte Chromaprint (même enregistrement à travers MP3/FLAC/AIFF, bitrates,
renommages, tags sales) ; Folder Library mode (sans logiciel DJ) ; Sound Recognition (identifie +
renomme fichiers mal nommés) ; Smart Source Upgrade (remplace fichiers usés). Intégration NATIVE :
lit master.db Rekordbox (déchiffré), crates Serato, .vdjfolder VirtualDJ — pas d'export XML. ADN
sécurité ~ Sift : rien supprimé auto, copies → corbeille, on approuve chaque suppression, revert.
Prix : free tier (scan complet + conversion playlists gratuite à vie) ; lifetime ~29$ founding ;
mensuel 4.99$. Local-first, Mac+Windows. Très orienté SEO (100+ guides sur codes/erreurs/migrations).
LIGNE DE PARTAGE RÉELLE Sift vs MLD (après lecture code, PAS sur la qualité — les 2 la font) :
- MLD travaille DANS les bases des logiciels DJ ; Sift sur les fichiers + son arbre → c'est l'onglet
  REKORDBOX de Sift (prévu, pas codé). Antoine a raison : Q3 (bases DJ) = chantier Rekordbox.
- MLD a doublons par empreinte Chromaprint sur TOUTE la biblio ; Sift a doublons dans la QUEUE
  (nom/sound-confirmed), pas l'empreinte cross-format sur la biblio → c'est le chantier BIBLIOTHÈQUE
  (prévu). Antoine a raison : Q2 (doublons biblio) = chantier bibliothèque.
- MLD importe Spotify/YouTube → HORS ADN Sift, ne pas suivre.
- DIFFÉRENCIATEUR SIFT QUI RESTE LIBRE : check CDJ PHYSIQUE (32-bit float, en-tête EXTENSIBLE
  0xFFFE, E-8305, multicanal, sample rate hors 44.1/48). MLD vérifie la QUALITÉ de l'audio (fake),
  PAS la COMPATIBILITÉ hardware du conteneur. tags_cdj_ok + container_ok existent déjà dans le report
  Sift → base déjà là pour pousser ce différenciateur.

### Vinyl Rip Quality Checker (vinyl-rip-quality-checker.hub2.day) — PAS un concurrent, complément
N'analyse AUCUN fichier. Checklist MANUELLE/humaine EN AMONT de la capture : on coche Pass/Fail/Skip
à la main pendant qu'on rippe un vinyle, résumé imprimable, données dans le navigateur, no tracking.
Couvre : stylet (propreté, force de lecture, anti-skate), préampli phono + gain staging (pics ~ -6
dB, zéro = clipping irréversible), sample rate ≥ 44.1/16-bit, format (lossless d'abord, MP3 = copie
seulement JAMAIS le master), passe d'écoute complète (wow/flutter, sibilance, balance L/R). 3 tiers
matériel (budget/mid/audiophile). Affiliation Amazon (préamplis/styli).
POSITION : prévention à la CAPTURE (bien ripper) — l'autre bout de la chaîne que Sift/MLD (détecter
après coup). Entièrement complémentaire. Sa doctrine format = celle de Sift (lossless master, MP3
copie). Ce qu'il demande à l'humain de vérifier à l'oreille (clipping, gain), Sift le DÉTECTE déjà
sur le fichier (clip_runs/clip_pct/true_peak_dbtp).

### CARTOGRAPHIE 3 positions sur la même chaîne
1. Vinyl Rip Checker = AVANT/pendant la capture, manuel, humain (bien ripper).
2. Sift + MLD = APRÈS, automatique, sur le fichier (détecter raté/faux).
   Dans cette catégorie : qualité spectrale (Sift ET MLD) | bases DJ + doublons acoustiques (MLD =
   chantiers biblio + Rekordbox de Sift) | check CDJ physique (territoire LIBRE de Sift).
STATUT : aucune action immédiate. Confirme la roadmap existante (biblio, Rekordbox, checker CDJ).
Ne PAS dévier de "filer sur place" (prompt prêt) comme prochain code.


---

## CHANTIER VINYLE — mesures de qualité de rip (cadré, NON codé, À REVOIR)

⚠️ STATUT INCERTAIN : Antoine n'est PAS sûr de vouloir ce chantier. Exploration/cadrage seulement,
à reconsidérer entièrement avant tout engagement. Ne PAS le traiter comme acté ni le faire remonter
comme prochain candidat code. La pertinence même (Sift doit-il aller sur le terrain qualité-de-rip ?)
reste ouverte.

DÉCLENCHEUR : Antoine — le check qualité d'un rip vinyl est aujourd'hui seulement à l'écoute ;
chercher des paramètres OBJECTIFS (mesurables) qui corrèlent avec le jugement subjectif de l'oreille,
pour les défauts PROPRES AU VINYLE que verdict.rs (fake/cutoff) ne capture pas. Un rip peut avoir un
spectre parfait à 20k ET être mauvais (clic, souffle, wow, déséquilibre).

POSTURE PRODUIT TRANCHÉE (importante) : Sift MONTRE LES MESURES, l'HUMAIN JUGE (comme le Vinyl Rip
Checker). PAS de verdict auto bon/mauvais rip → désamorce le risque de faux positif (clic voulu, wow
artistique, bruit de surface intentionnel). Simplifie le DSP : pas de seuil à défendre, juste une
mesure honnête + preuve visuelle. Antoine tranche.

CE QUE SIFT MESURE DÉJÀ et qui s'applique au vinyl (worker.rs report, à EXPOSER) : clip_runs,
clip_pct, true_peak_dbtp (gain trop chaud = écrêtage, défaut n°1 du rip maison) ; dc_offset
(interface mal calibrée) ; phase_correlation, dual_mono (câblage/balance L/R) ; silence_head/tail_ms
(blancs mal coupés). → une partie de la checklist vinyle (gain, balance) est DÉJÀ couverte, juste
pas exposée comme "qualité de rip".

LES 4 AXES VOULUS (multi-select Antoine = les 4), PAR COÛT CROISSANT :
1. EXPOSER L'EXISTANT — coût quasi nul, zéro DSP. Afficher clipping/true-peak/balance/phase/DC/
   silences dans le rapport comme indicateurs de qualité de rip lisibles. PREMIER PAS. Commence par
   un relevé lecture-seule de ce que report-view.ts montre AUJOURD'HUI du verdict + des mesures.
2. CLICS & POPS — coût moyen. Détection de transitoires (discontinuités brutales / dérivée du
   signal), comptage "N clics". En mode montre-la-mesure, aucun jugement requis.
3. SOUFFLE / BRUIT DE SURFACE — coût moyen. Plancher de bruit mesuré sur les passages calmes,
   affiché en dB. Pas de verdict.
4. WOW & FLUTTER — coût ÉLEVÉ, vrai DSP. Pitch qui ondule sur sons tenus ; suivi de pitch ou
   isolation porteuse ~3150 Hz (norme W&F). Le plus distinctif "vinyle" MAIS le plus technique,
   touche le plus le moteur d'analyse Rust stabilisé.
ORDRE LOGIQUE : exposer l'existant → clics → souffle → wow&flutter.

MISES EN GARDE (détective, avant tout code) :
- Faux positifs : sur vinyle, clic/wow/bruit peuvent être INTENTIONNELS (samples, effet, master qui
  imite le vinyle). L'oreille distingue défaut-de-capture vs intention ; un algo non. La posture
  "montre, ne juge pas" neutralise ce risque — la GARDER absolument.
- Touche le moteur d'analyse Rust (worker + analysis/*), cœur stabilisé. Axes 2-4 = vrai DSP, pas
  une session. NE PAS l'attaquer au détriment de "filer sur place" (prompt prêt, mûr).

STATUT : cadré, NON codé. Prochain pas quand on l'attaque = relevé lecture-seule report-view.ts (ce
qui est affiché aujourd'hui). Place dans la file : APRÈS "filer sur place". Lien avec le chantier
checker CDJ (même esprit "vérifier que le fichier tient ce qu'il promet") et avec "mieux exposer le
verdict spectral" déjà noté.
