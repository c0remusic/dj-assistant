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
