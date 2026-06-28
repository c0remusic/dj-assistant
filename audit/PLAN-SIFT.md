# PLAN D'IMPLÉMENTATION — SIFT (source unique)

> Écrit le 2026-06-28. Consolide : RAPPORT-direction.md (25/06), filing-detach-releve.md,
> filed-autoadvance-releve.md, + chantiers connus. But : UNE source ordonnée pour arrêter les
> microfix dispersés. Règle : un chantier à la fois, testé live + commité avant le suivant.
> Méthode : détective (théorie→preuve→fix), fail-fast, pas de fallback, changements chirurgicaux.
> ⚠️ Ce plan concerne SIFT uniquement. Tuple (device M4L, design carbon, Tuple Controller APC,
> campagne promo) = roadmap SÉPARÉE, autre dépôt, autres fichiers.

---

# SÉQUENÇAGE EXÉCUTABLE (étape par étape — l'ordre fait foi)

Logique d'ordre, en une phrase : fermer ce qui flotte (A) → finir ce que l'étape 2 a ouvert (B, C)
→ réparer le pilotage qui cause les microfix (D) → boucler l'étape 3 (E) → grosses briques par
dépendances (F). Chaque étape = testable + commitable seule. Un chantier à la fois.

## PHASE A — fermer le code en suspens (ce soir) ✅ FAIT
- **A1. ✅ "Filer sur place" TESTÉ (4/4) + à commiter.** 4 tests OK sur même disque : (a) non-conforme
  sur place garde-fou anti-corruption OK ; (b) conforme déjà nommé pas de " (2)" ; (c) batch source ;
  (d) filing normal inchangé. Commit : feat(filing): filer sur place (sentinelle __SOURCE__).
  NB : cas CROSS-DISQUE pas encore couvert (échouera au trash rename) → débloqué par Phase C-bis.

## PHASE B — finir l'éditeur de la Revue détail (LE chantier "un bloc" anti-microfix)
- **B1. ✅ FAIT (diagnostic 28/06) — c'est de l'AFFICHAGE FRONT PUR, rien côté Rust.**
  Chaîne de données prouvée intacte de bout en bout : Discogs renvoie label/year → parse_search les
  met dans Candidate (discogs.rs:107-108) → apply_identity_cmd passe le Candidate complet →
  apply_identity PERSISTE label+year dans table metadata (mod.rs:90-97) ET les renvoie au front dans
  AppliedIdentity{label,year} (mod.rs:74-75) → plan_file relit metadata et écrit label/year dans les
  tags du fichier filé. TOUT marche côté données. Le seul trou : onIdentityApplied (filing.ts) ne lit
  que applied.canonical (artist/title/version) et IGNORE applied.label/applied.year, et renderEditor
  n'a aucun champ pour les montrer. → B2 réduit à 2 changements front, ZÉRO Rust.
- **B2. Implémentation (FRONT SEUL).** (1) onIdentityApplied : lire aussi applied.label + applied.year
  (déjà dans le retour). (2) renderEditor : afficher label + année. DÉCISION ANTOINE : LECTURE SEULE
  (display only), PAS éditables. Raison : le choix de la RELEASE parmi les candidats Discogs fixe déjà
  le bon label/année (pressage original vs réédition) → éditer serait redondant. → vraiment 2 lignes
  front, zéro Rust, zéro extension de Canonical.
- **B3. "Final name" à droite** (= 1.1). Déplacer le preview centre→rail, refreshPreview suit.
  Maquette d'abord. APRÈS B2 (le label/année peut changer ce que le rail affiche).
  → B1+B2+B3 pensés ENSEMBLE, commités séparément. Réponse directe aux microfix.
- **B4. Action "Apply ID3 tags" (NOUVELLE, revertable).** Besoin Antoine : corriger les métadonnées
  d'un fichier SANS le filer — écrire les tags ID3 (artiste/titre/label/année/genre/pochette depuis
  Discogs ou champs édités) SUR PLACE, sans encode, sans déplacement, sans changement de statut
  'filed'. Action EN PLUS à côté de File/Discard. Marche sur TOUT fichier (conforme OU non-conforme).
  Distincte de "filer sur place" qui, lui, CONVERTIT un non-conforme (encode) — c'est justement ce
  qu'on ne veut pas ici. DÉCISION : REVERTABLE → nouveau type d'action dans le journal `actions` qui
  SAUVEGARDE les anciens tags avant d'écrire, et un revert qui les restaure (le revert actuel gère
  des déplacements move/convert/trash, PAS des modifs de contenu de tags → vrai nouveau mécanisme,
  c'est le coût). Réutilise write_tags_full (existe). À cadrer : relevé du journal actions + comment
  lire/restaurer les anciens tags avant prompt.
- **B5. ✅ FAIT + TESTÉ + à commiter — bug réouverture identité (28/06).** apply_identity écrivait artist/title/
  label/year/discogs_release_id en base mais openFilingInto relisait via reconcile (tags fichier) →
  identité Discogs perdue à la réouverture. Corrigé : track_release élargi renvoie artist/title/
  version/label/year/identified ; openFilingInto prend metadata si identified, sinon reconcile.
  CORRECTION IMPORTANTE : la colonne `version` EXISTE (migration v4, db.rs:83 — l'affirmation "pas de
  colonne version" était FAUSSE, Claude Code l'a prouvé en lisant le schéma) → le remix est splitté
  du titre et persisté sans migration. Testé OK (remix survit close/reopen + cold start). RESTE à
  vérifier le piège parenthèses-non-remix (titre légitime "(Come On In)" ne doit pas être amputé).
- **B6. ✅ FAIT + TESTÉ + à commiter — Ré-afficher la release SÉLECTIONNÉE à la réouverture.**
  Besoin Antoine : aujourd'hui choisir une release remplace la zone candidats par une ligne
  "Identified: Artiste — Titre" + pochette + bouton "change" (onIdentityApplied, filing.ts:487-497).
  À la réouverture d'un morceau déjà identifié cette ligne disparaît (retour au bouton Fetch nu) —
  on ne voit plus QUELLE release était retenue. FIX : si identified (déjà détecté via track_release),
  afficher cette ligne "Identified" au lieu du bouton Fetch nu. Reconstruction depuis la base :
  artist/title (track_release) + cover_path (metadata) suffisent — la ligne n'affiche que pochette +
  artist — title, PAS besoin de format/pays/liste candidats. Zéro réseau à l'ouverture, zéro stockage
  en plus. DÉCISION ANTOINE : le bouton "change" à froid RELANCE un Fetch Discogs (la liste des
  candidats d'origine n'est plus en mémoire à froid) — logique et simple, pas de stockage en plus.
- **B7. BUG cohérence chip/nom (trouvé 28/06 en testant B3).** previewName() (filing.ts:338) retombe
  sur "mp3_320" EN DUR quand state.target est null, MAIS la chip surlignée (filing.ts:641) retombe sur
  defaultTarget(rail) (= aiff si lossless, mp3 si lossy). Deux défauts DIFFÉRENTS quand target=null →
  le nom peut afficher .mp3 alors que la chip AIFF est allumée (rail lossless). FIX : une seule source
  de vérité pour le défaut — previewName doit utiliser le MÊME défaut que la chip (state.target ??
  defaultTarget(rail)), pas "mp3_320" en dur. Front pur. (Révélé par B3 qui met nom+chips côte à côte.)
- **B8. (FUTUR) Option UPSCALE / NEVER UPSCALE.** Idée Antoine : un toggle dans le menu. Aujourd'hui
  le code grise DÉJÀ AIFF/WAV en dur pour une source lossy (renderFoot:637-640 "can't upscale a lossy
  file"). B8 = rendre ce verrou OPTIONNEL : NEVER UPSCALE (défaut actuel, chips lossless grisées/lock
  pour un mp3) vs UPSCALE (autorise quand même). Cohérent ADN Sift (pas de faux lossless). Distinct
  de B7 (B7 = bug, B8 = feature). À cadrer plus tard.

## CHANTIER TRANSVERSAL — TAGS CDJ-SAFE (lié au checker CDJ F3)
DÉCLENCHEUR (Antoine, à propos de B4) : "s'assurer que les tags écrits soient compatibles CDJ".
PRÉCISION détective : le souci CDJ n'est PAS le contenu des champs (artiste/année/label — le CDJ s'en
fiche) mais la VALIDITÉ TECHNIQUE de l'ID3 : en-tête corrompu, version ID3 non supportée (ID3v2.4 vs
v2.3 — les CDJ Pioneer aiment v2.3, v2.4 peut faire E-8305), pochette trop lourde / format non digéré
→ le CDJ REFUSE de charger (E-8305). TRANSVERSAL : write_tags_full est utilisé par le filing normal
ET par B4 → si lofty n'écrit pas du CDJ-safe, TOUT l'écriture de tags de Sift est concernée, pas juste
B4. Le régler "juste pour B4" serait incohérent. À PROUVER d'abord (relevé lecture seule) : ce que
lofty/write_tags_full écrit AUJOURD'HUI (version ID3 ? gestion pochette ?), puis écarts avec ce que
les CDJ exigent, puis corriger UNE FOIS pour tout l'écriture de tags. Rattaché au Bloc F3 (checker CDJ,
le différenciateur). NE PAS mélanger avec B4 : coder B4 d'abord, ce chantier à part.

## PHASE C — convergence batch
- **C1. Vérifier ce que le batch reverte déjà** (lecture seule : revert_batch existe, quel
  déclencheur côté batch ?).
- **C2. Bandeau Filed + revert dans le rail batch** + rapatrier l'état batch (nav rail GAUCHE →
  pied rail DROIT). Design à cadrer (batch = N morceaux). Après C1. (= 2.1)

## PHASE C-bis — TRASH CENTRALISÉ + onglet "Jetés" (décision archi 28/06)
PROBLÈME prouvé (filing.rs:309) : trash_file_fs utilise TOUJOURS plan.root (racine de la biblio
configurée). Or Sift importe des dossiers de PARTOUT (c'est le but). Filer sur place un fichier venu
d'un autre disque (ex. clé E:) enverrait l'original dans D:\Musique\.sift-trash → rename CROSS-DISQUE
qui ÉCHOUE sous Windows ("cannot move across volumes"). Hypothèse "tout sous une racine unique" =
PÉRIMÉE.
DÉCISION ANTOINE : centraliser ACCÈS **ET** STOCKAGE. Dossier fixe unique = `Documents\Sift\Trash`
(nom neutre). Argument décisif : 200 fichiers sur 3 disques, on se rend compte APRÈS d'une connerie →
devoir fouiller 3 disques = la prise de tête que Sift doit supprimer. Un seul endroit pour tout
récupérer.
SÛRETÉ (objection cross-disque levée par Antoine, juste) : pour un NON-conforme, encode CRÉE le
converti AVANT de trasher l'original (filing.rs:303-309) → jamais de perte. Pour le déplacement vers
le trash central cross-disque : discipline COPY → VERIFY → DELETE (copier vers le trash, vérifier
l'intégrité, SEULEMENT ENSUITE supprimer l'original) → un fichier valide à tout instant. rename rapide
en optim quand c'est le même disque.
PÉRIMÈTRE (prouvé par lecture ecartes.rs:101-130) — symétrique et localisé :
1. trash_file_fs (filing.rs) : cible = Documents\Sift\Trash (résolu via dirs/known-folder), nom
   collision-free déjà géré (ensure_unique + préfixe track_id__). rename si même disque, sinon
   copy→verify→delete.
2. restore_track (ecartes.rs:124) : le std::fs::rename(&to,&from) devient le MÊME mouvement
   copy→verify→delete dans l'autre sens (origine peut être sur un autre disque que le trash). Gardes
   déjà présents (refuse si trashé disparu L.116 / si origine occupée L.119) → garder.
3. purge_trash (ecartes.rs:135+) : fait déjà remove_file(to_path) → marche quel que soit l'emplacement,
   RIEN à changer.
BONUS GRATUIT : le journal `actions` stocke déjà from_path/to_path en chemins absolus → AUCUN
changement de schéma. L'onglet "Jetés" (la vraie bonne idée d'Antoine) se construit par-dessus :
lister `actions WHERE type='trash' AND undone=0` (= la requête que purge_trash fait déjà), bouton
Restaurer = appelle restore_track (existe). Vue centralisée unique, quel que soit le disque d'origine.
LIEN : "filer sur place" (0.1) RÉVÈLE ce problème (fichier hors-biblio → racine incohérente). À traiter
AVANT de compter sur le trash en usage multi-disque réel. Place : après convergence batch, ou remonter
si le multi-disque devient bloquant à l'usage.

## PHASE D — assainir le pilotage (corrections direction, fort levier)
- **D1. P-2 réconcilier README + CLAUDE.md** (1-2h). = CAUSE RACINE des microfix. Meilleur ROI, tôt.
- **D2. P-1 échelle typo+espacement tokenisée.** Règle la douleur "pas pro". APRÈS B/C (sinon migrer
  l'UI deux fois).
- **D3. P-3 split sift-live.ts** (1144 l → 3 extractions) avant que M6b le regrossisse. Add-ons :
  P-4 assertions DOM, P-5 retirer dead_code.

## PHASE E — finir l'étape 3 (PRESQUE FAITE — voir correction ci-dessous)
- **E1. Détacher file_track MONO** (reste synchrone, gèle sur 1 fichier). Patron de file_batch en
  version 1-fichier. Petit.
- **E2. (différé) Undo du batch entier** (batch_id partagé au lieu de par-fichier). Optionnel.

## PHASE F — grandes briques (ordre par dépendances)
- **F1. Bibliothèque** (dont doublons par empreinte acoustique Chromaprint). [Q2]
- **F2. Onglet Rekordbox** (bases DJ + check USB/chemin). [Q3] — a besoin de F1.
- **F3. Checker CDJ** (32-bit float / EXTENSIBLE / multicanal / sample rate / ID3→E-8305 ;
  tags_cdj_ok+container_ok déjà là). — son check USB a son sens à l'export Rekordbox (F2).

## À REVOIR / hors-code
- ⚠️ 6.1 VINYLE (pas sûr — ne pas coder sans redécision). 6.2 auto-update niveau 1 (petit, quand).
- Naming/promo = phase promo, pas à chaud.

## ⚠️ CORRECTION IMPORTANTE — ÉTAPE 3 DÉJÀ FAITE À ~80% (prouvé par ipc_filing.rs, 28/06)
Le relevé filing-detach-releve.md décrivait l'état AVANT travaux. Le code RÉEL montre que c'est fait :
- file_batch DÉJÀ détaché : run_file_batch sur thread dédié "file-batch", lock PAR FICHIER (3 phases
  plan/execute/commit), émet file:done(BatchResult). → gel UI + blocage worker du batch = RÉSOLU.
- Annulation stop-net DÉJÀ là : file_cancel + FilingCancel(Arc<AtomicBool>), flag vérifié ENTRE
  fichiers. → commit "cancel" du relevé = FAIT.
- Progression par fichier DÉJÀ là : struct FileProgress{done,total} pour la zone globale kind="file".
RESTE de l'étape 3 = E1 (file_track mono, encore SYNCHRONE lignes 58-85, gèle 1 fichier) + E2 (undo
batch entier, différé). Le Bloc 3 originel est donc quasi vide → ne PAS le replanifier comme gros.

---


## RAPPEL DIRECTION (RAPPORT-direction.md, prouvé)
M0→M6a livrés, M6b en cours (pas "M2 à venir" = README périmé). Direction SAINE : PAS de refonte,
juste des corrections ciblées. Le vrai risque = divergence doc↔code qui fausse le pilotage (= la
cause des microfix). À PRÉSERVER (ne pas casser) : sécurité fichiers (journal actions + corbeille
réversible), invariant 2 rails / jamais d'upscale, hybride Symphonia+FFmpeg, discipline async front
(openSeq/currentWs/invalidation cache), tokens couleur, tests (150 lib + clippy + tsc), binding
honnête UI↔fonctions réelles.

---

## BLOC 0 — FERMER CE QUI FLOTTE (priorité absolue)

### 0.1 — "Filer sur place" — CODÉ, NON TESTÉ, NON COMMITÉ
Sentinelle FILE_IN_PLACE="__SOURCE__" (un seul canal binRel, miroir Rust↔contracts.ts). plan_file :
si bin_rel==sentinelle → dest_dir=source.parent() (jamais via safe_join). ensure_unique(path,
ignore:Option) self-ignore SEULEMENT sur chemin conformant/move (None pour encode, sinon FFmpeg
écraserait sa source = corruption — garde-fou ajouté par Claude Code, juste). Front : option dropdown
batch + case détail + bandeau "Filed → source folder".
→ ACTION : rebuild Rust via tauri dev, puis 4 tests : (a) non-conforme sur place → converti dans
dossier source, original corbeille, revert OK [TEST DU GARDE-FOU] ; (b) conforme déjà nommé → PAS de
" (2)" parasite ; (c) batch "Dossier source" lot mixte ; (d) filing normal INCHANGÉ. Puis commit.

---

## BLOC 1 — FINIR L'ÉDITEUR DE LA REVUE DÉTAIL (traiter EN UN, pas en microfix)
Ces deux trous apparaissent à l'usage de l'étape 2. Les faire ensemble = "finir l'éditeur détail".

### 1.1 — "Final name" à droite
Aujourd'hui : rendu au CENTRE par renderEditor (filing.ts ~631), volontairement collé aux champs
artist/title/version qui le modifient (refreshPreview temps réel). Antoine le VEUT à droite (rail).
Tension à assumer : sépare le preview (effet) des champs (cause). À cadrer : maquette + où exactement
dans le rail, et refreshPreview doit suivre.

### 1.2 — Label + année après identification Discogs
Preuve : type Canonical (contracts.ts) ne porte QUE artist/title/version/confidence — PAS label/year.
onIdentityApplied applique donc seulement ces 3 champs. label/year existent dans LibraryTrack +
MetadataEdit + sont écrits dans les tags par plan_file (TagExtras), mais NI affichés NI éditables
dans la Revue, et l'identification ne les applique pas. → DIAGNOSTIC RUST À FAIRE AVANT DE CODER :
la commande d'identification Discogs récupère-t-elle réellement label/year ? (cas: récupéré-mais-
jeté vs pas-récupéré-du-tout). Décide l'ampleur. Lecture seule d'abord.

---

## BLOC 2 — CONVERGENCE BATCH (aligner le batch sur le détail)

### 2.1 — Bandeau Filed + revert dans le rail BATCH
Constat : le bandeau Filed + revert est une feature du mode DÉTAIL seulement (showFiledConfirm,
filing.ts). Le batch (renderBatchRail, sift-live.ts) montre un RÉSUMÉ de sélection (Selection/
Destination/Will encode/Excluded), pas de bandeau Filed ni revert par morceau. PAS un bug = feature
absente. Design à cadrer : un batch file N morceaux → le bandeau par-morceau du détail ne se
transpose pas tel quel. Lié au chantier convergence (rapatrier l'état batch du nav rail GAUCHE vers
le pied du rail DROIT, pour UN seul endroit "ce qui vient de se passer" dans les 2 modes).
ATTENTION : vérifier d'abord ce que le batch reverte déjà (revert_batch existe) et via quel
déclencheur.

---

## BLOC 3 — DÉTACHER LE FILING (étape 3) — vérifier l'état d'abord
⚠️ DIVERGENCE À LEVER : filing-detach-releve.md présente file_batch comme À détacher ; mais
filed-autoadvance-releve.md (preuve C) dit que file_batch est DÉJÀ détaché (run_file_batch, lock
par fichier, "never freezes"). → LIRE ipc_filing.rs pour trancher ce qui est fait avant de planifier.
Si file_batch déjà détaché, il reste :
- 3.1 Progression par fichier dans la zone globale (event kind="file" + wiring progress-zone.ts).
- 3.2 Annulation stop-net (Arc<AtomicBool> + commande file_cancel + bouton Stop + résumé partiel).
- 3.3 (différé/optionnel) détacher file_track MONO (sync → gèle 1 fichier) ; undo du batch entier
  (batch_id partagé au lieu de par-fichier).
Journalisation source→dest = GRATUITE (commit_file écrit déjà dans actions par fichier ; moteur
revert_batch/undo_last/list_journal déjà là et testé).

---

## BLOC 4 — CORRECTIONS CIBLÉES DE DIRECTION (P-1..P-5, vérifier si déjà faites)
Du RAPPORT-direction (25/06), ROI décroissant. VÉRIFIER lesquelles sont encore en attente :
- P-1 Échelle typo + espacement tokenisée (preuve : 144 font-size inline, 0 échelle). 0,5j +
  ~0,25j/écran. → règle direct la douleur "pas pro", arrête de deviner les tailles. FORT LEVIER.
- P-2 Réconcilier README + CLAUDE.md (README dit "M2 à venir"). 1-2h, risque nul. → supprime la
  source des mauvaises décisions de cap (LA cause racine des microfix).
- P-3 Split sûr de sift-live.ts (1144 l → 3 extractions nommées). 0,5-1j, AVANT que M6b regrossisse.
- P-4 (add-on) assertions DOM (fail-fast), 2-3h. P-5 retirer #![allow(dead_code)], 2-4h.

---

## BLOC 5 — GRANDES BRIQUES FONCTIONNELLES (chantiers majeurs, pas des fix)
- 5.1 BIBLIOTHÈQUE — dont doublons par EMPREINTE ACOUSTIQUE (Chromaprint cross-format, comme MLD ;
  Sift ne fait aujourd'hui que la dédup dans la QUEUE par nom). [= Q2 d'Antoine]
- 5.2 ONGLET REKORDBOX — lecture/écriture bases DJ. C'est là que le check USB/chemin du checker CDJ
  prend son sens (export clé). [= Q3 d'Antoine]
- 5.3 CHECKER COMPATIBILITÉ CDJ — différenciateur. Détecter les pièges hardware : 32-bit float WAV,
  en-tête WAVE_FORMAT_EXTENSIBLE 0xFFFE, multicanal >2ch, sample rate hors 44.1/48, validité ID3
  (→ E-8305). Base DÉJÀ là : tags_cdj_ok + container_ok dans le report. MLD ne fait PAS ce check
  (lui = qualité audio/fake ; Sift = compatibilité hardware du conteneur). Trou à prouver : Sift
  juge format/sample-rate/bit-depth mais pas format-tag/canaux/validité-ID3 → à compléter.

---

## BLOC 6 — INCERTAIN / À REVOIR (NE PAS coder sans redécision d'Antoine)
- 6.1 ⚠️ CHANTIER VINYLE (wow&flutter, clics, souffle, exposer l'existant). Antoine PAS SÛR. Posture
  si jamais : Sift MONTRE les mesures, l'humain JUGE (désamorce faux positifs sur intention
  artistique). Note : Sift fait DÉJÀ la fake-detection spectrale (verdict.rs cutoff_hz) + mesure
  déjà clip/true-peak/phase/dual-mono/dc/silences (worker.rs) — une partie de la checklist vinyle
  est déjà là, juste pas exposée. À revoir : Sift doit-il aller sur le terrain qualité-de-rip ?
- 6.2 Auto-update niveau 1 (détecter+notifier via version.json, sans télécharger). Scopé, petit,
  insérable quand voulu.

---

## HORS-CODE — PHASE PROMO (pas à chaud, pas maintenant)
- Naming / descripteur accolé ("Sift — DJ library prep" / "CDJ-ready" / "encode & file"). "DJ
  assistant" trop muet (sous-vend). Découvrabilité > mémorabilité pour utilitaire mono-promesse.
  À trancher avec le nom, la page, le pitch — en phase promo. Lié au checker CDJ (le descripteur
  "CDJ-ready" n'a de poids que si le check existe).

---

## POURQUOI LES MICROFIX (diagnostic)
Les trous des blocs 1-2 (Final name, label/année, bandeau batch) ne sont PAS des oublis : ce sont
les finitions naturelles qui apparaissent en UTILISANT l'étape 2. La cause racine = ce que dit le
RAPPORT-direction : divergence doc↔code, pas de plan unique à jour. CE FICHIER est la réponse.
Règle anti-microfix : traiter un BLOC entier comme un chantier, pas corriger les trous un par un
quand on les croise à l'usage.
