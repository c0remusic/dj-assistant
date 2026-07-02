# PASS 6 — Logique produit

> Audit seul, aucune modification de code. Méthode détective : chaque constat
> cite fichier:ligne. Grille d'analyse : "the no list" (Steve Jobs — trancher
> le scope, chaque feature doit se justifier) et "build less" (37signals —
> ne pas construire ce qui n'apporte pas de valeur nette). Skills invoquées
> dans le raisonnement : `steve-jobs-design-review`, `37signals-way`.
>
> Contexte lu : `README.md`, `docs/ressources-externes.md` (différenciateur
> CDJ + section "Écarté"), `audit/PASS-0-carte.md`.

---

## Constat 1 — Le différenciateur CDJ est réel dans le moteur, quasi invisible dans l'UI

**Priorité** : High
**Fichier:ligne** : `frontend/filing.ts:1036-1043` (label "Compatibilité CDJ" +
"Version ID3", rangés dans la carte Identification, sous Genres) ;
`frontend/report-view.ts:276` (commentaire confirmant le déplacement hors de
la "spectral-proof box") ; `README.md:6-8` (promesse produit : "convertit au
format CDJ au moment du rangement").

**Description** : le README et `docs/ressources-externes.md` présentent la
compatibilité hardware CDJ (32-bit float, header EXTENSIBLE, erreur E-8305)
comme LE différenciateur du produit face à Rekordbox/MediaMonkey. Mais dans
l'UI réelle, "Compatibilité CDJ" est une simple ligne `row("Compatibilité
CDJ", yn(report.tags_cdj_ok))` — un badge oui/non au même niveau visuel que
"Version ID3" — noyée dans la carte Identification, après Genres. Rien dans
le vocabulaire de l'écran Revue (labels, titres de carte, verdict) ne nomme
explicitement "CDJ" comme raison d'être de l'app.

**Impact concret** : un utilisateur qui découvre l'app dans son écran
principal (Revue) ne voit jamais le mot "CDJ" avant d'ouvrir le panneau
d'identification et de faire défiler jusqu'à Genres. Le différenciateur
produit revendiqué en doc n'est pas celui que l'utilisateur perçoit en
premier — il perçoit un détecteur de faux lossless générique + un gestionnaire
de bibliothèque, features qui existent ailleurs.

**Cause probable** : le déplacement documenté au commentaire
(`report-view.ts:276`, "moved to the Identification card... alongside
Label/Année/Genre") a suivi la logique de groupement de la maquette, pas une
hiérarchie de message produit — le refactor UI (2026-07-02, cf. commits
"remaining-time pill", "waveform hover-scrub") a porté sur le player, pas sur
la mise en avant du différenciateur.

**Proposition** : remonter "Compatibilité CDJ" (et ce qu'elle implique
concrètement — 32-bit float refusé, header EXTENSIBLE, etc.) dans le verdict
principal ou juste en dessous, avec un langage qui nomme le CDJ, plutôt que
de le laisser en ligne de tableau secondaire.

**Effort** : Petit (déplacement UI + reformulation d'un label).
**Bénéfice attendu** : le message produit réel (pourquoi Sift existe plutôt
que d'utiliser Rekordbox/MediaMonkey) devient visible à l'endroit où
l'utilisateur passe le plus de temps (Revue).

---

## Constat 2 — Le verdict ok/fake/grey explique le POURQUOI, contrairement à l'hypothèse initiale

**Priorité** : Low (constat positif, à ne pas casser)
**Fichier:ligne** : `frontend/report-view.ts:61-86` (`realQuality`,
`spectroCaption`), `report-view.ts:108-118` (`drawSpectrogram`, ligne de
cutoff annotée sur le spectrogramme), `report-view.ts:226-229`
(`verdictCardHtml`, labels "Sur-encodé — à re-sourcer" / "À vérifier
d'abord").

**Description** : contrairement à une UI de statut opaque, le verdict est
accompagné d'un débit MP3 estimé depuis `cutoff_hz` ("MP3 ≈ 192 kbps"), d'une
légende explicite ("coupure nette = transcodage probable") et d'un
spectrogramme annoté avec la fréquence de coupure en kHz dessinée dessus. Le
mécanisme (cutoff spectral) est donc bien exposé, en langage DJ
compréhensible (kbps estimé) plutôt qu'en jargon (Hz bruts uniquement).

**Impact concret** : aucun — c'est un point fort à préserver, pas un défaut.

**Cause probable** : n/a.

**Proposition** : n/a — à citer comme référence de qualité UX quand on
retravaille le Constat 1 (même niveau de pédagogie à appliquer au badge CDJ).

**Effort** : n/a. **Bénéfice attendu** : n/a.

---

## Constat 3 — La feature "Export Rekordbox / Clé USB" est une simulation frontend pure, sans aucun backend

**Priorité** : Critical
**Fichier:ligne** : `frontend/sift-live.ts:300-327` (`startExportSim` — commentaire
explicite : *"No Rekordbox/USB backend exists yet... the work itself is
simulated: a fake per-track tick"*) ; `frontend/sift-live.ts:346-360`
(`runNavExport` — récupère le vrai compte de pistes via `listLibrary()` puis
lance uniquement la simulation) ; `frontend/progress-zone.ts:11,30,38` (task
kind "export" avec icône USB et label "Export" — infrastructure générique
réutilisée) ; `frontend/styles.css:99` (commentaire : *"Export (Rekordbox /
Clé USB): visually secondary — not built yet"*) ; `index.html:20-22` (items
nav "Rekordbox" / "Clé USB" bien présents dans le rail) ; aucune commande
Tauri correspondante dans `src-tauri/src/ipc*.rs` (grep négatif sur
`file_track`-like patterns pour "export" — 0 résultat) ; confirmé absent du
README (état des jalons, M7 "Rekordbox XML + batch + clé USB" listé "à
venir", `README.md:23`) et de `docs/ressources-externes.md` (section
Rekordbox listée comme candidats non intégrés : `rbox`, `rekordcrate`).

**Description** : cliquer sur "Rekordbox" ou "Clé USB" dans le rail de
navigation déclenche `runNavExport`, qui lit le nombre réel de pistes filées
(`listLibrary()`) puis lance un `setInterval` de 450ms par "piste" avec une
barre de progression dans la zone de progression globale — sans qu'aucun
fichier XML Rekordbox ne soit écrit, sans qu'aucune clé USB ne soit touchée.
Le code le documente lui-même sans ambiguïté ("the work itself is
simulated"). C'est une maquette animée, pas une feature.

**Impact concret** : un utilisateur qui clique "Rekordbox" verra une
progression crédible (icône USB, compte réel de pistes, barre qui avance,
état "done" après ~450ms/piste) puis... rien ne s'est produit sur disque. Ceci
est trompeur si exposé sans garde-fou en dehors du dev — un utilisateur qui
croit avoir exporté sa bibliothèque vers Rekordbox et se rend au club
découvre l'absence réelle de l'export sur le terrain, au pire moment possible.

**Cause probable** : la nav rail vient de la maquette `app.js` d'origine
(item déjà présent visuellement) ; le wiring live a suivi le principe
"mockup-first, improve" mais seulement pour l'AFFORDANCE (compte réel de
pistes, intégration dans la zone de progression générique), pas pour la
fonction réelle — cohérent avec le README qui place ce jalon (M7/M8) "à
venir, gelé".

**Proposition** : soit masquer complètement les entrées nav "Rekordbox" /
"Clé USB" tant que le backend n'existe pas (cohérent avec le "no list" —
mieux vaut une fonctionnalité absente qu'une fonctionnalité qui ment sur son
propre état), soit les remplacer par un état "Bientôt disponible" non
cliquable/désactivé, au lieu d'une simulation qui imite un vrai traitement.

**Effort** : Petit (retirer/désactiver l'entrée nav + `runNavExport`,
`startExportSim` restent en `sift-live.ts` gelés ou supprimés).
**Bénéfice attendu** : élimine un risque de confiance utilisateur cassée sur
le terrain (DJ en club sans sa bibliothèque exportée) — coût de correction
très inférieur au coût de la confusion en usage réel.

---

## Constat 4 — Réglages exposés dans `settings.rs` que l'utilisateur DJ ne verra/comprendra jamais directement

**Priorité** : Medium
**Fichier:ligne** : `src-tauri/src/settings.rs:7-17`.

**Description** : la table `settings` expose 5 clés :
- `library_root` (`LIBRARY_ROOT`) — nécessaire, l'utilisateur le choisit
  explicitement (Réglages).
- `filename_template` (`FILENAME_TEMPLATE`) — un DSL de masks
  (`naming.rs`, 379 lignes) avec placeholders `{artist} {title} {version}` ;
  la doc (`docs/ressources-externes.md`, section "Veille concurrente —
  MediaMonkey") vise un DSL plus riche façon MediaMonkey (`<Track#:2>`,
  `$If`, `$Replace`...). Le défaut (`{artist} - {title}{version}`) couvre le
  besoin DJ standard ; la personnalisation avancée du masque est une feature
  de power-user bibliothèque générale (photo/musique manager), pas un besoin
  DJ typique.
- `trash_purge_days` (`TRASH_PURGE_DAYS`) — durée de rétention de la
  corbeille (`.sift-trash`). Utile mais un réglage "combien de jours avant
  suppression définitive" est le genre de préférence qu'un DJ configurera une
  fois puis oubliera — pas un axe de valeur produit.
- `discogs_token` (`DISCOGS_TOKEN`) — nécessaire (Discogs impose un token
  pour un quota correct, cf. `docs/ressources-externes.md` section
  "Renommage Discogs" : 60 req/min authentifié vs 25 anonyme), mais demande à
  l'utilisateur d'aller créer une app développeur sur discogs.com pour
  obtenir un token — friction d'onboarding non triviale pour un DJ non
  technique.
- `current_session_id` — n'est PAS un réglage utilisateur (commentaire
  ligne 15-17 : "Written once at startup" pour grouper les actions du
  journal) ; sa présence dans le même module `settings.rs`/table `settings`
  que les vraies préférences utilisateur mélange deux concepts (préférence
  vs état interne d'infra).

**Impact concret** : `filename_template` et `trash_purge_days` sont des
réglages que la quasi-totalité des utilisateurs DJ laisseront à leur valeur
par défaut sans jamais les toucher consciemment — leur exposition dans
Réglages (à vérifier dans l'UI Réglages elle-même, hors scope de cette passe
frontend ciblée) ajoute de la surface cognitive sans bénéfice pour le
segment cible. `current_session_id` n'étant pas une préférence, son mélange
dans la même table que les vrais settings est un signal d'architecture (pas
un problème produit direct) mais complique la lecture du module pour
quiconque cherche "les réglages visibles à l'utilisateur".

**Cause probable** : `settings.rs` fait office de table clé/valeur
générique pour TOUT état persistant simple (préférences ET état interne),
choix d'architecture pragmatique (une seule table, migration v4) qui n'a pas
été scindé en "préférences utilisateur" vs "état applicatif interne".

**Proposition** : côté produit (pas archi) — vérifier si `filename_template`
et `trash_purge_days` sont vraiment exposés dans l'écran Réglages ; si oui,
les reléguer dans une section "Avancé" repliée plutôt qu'au même niveau que
`library_root`/`discogs_token`, cohérent avec le principe de divulgation
progressive déjà noté dans `docs/ressources-externes.md` (section
"Écarté — vykee.co", qui acte explicitement vouloir la progressive disclosure
nativement).

**Effort** : Petit (UI Réglages seulement, aucun changement de schéma).
**Bénéfice attendu** : réduit la charge cognitive de l'écran Réglages sans
retirer aucune capacité.

---

## Constat 5 — Parcours importer → analyser → filer : trace du chemin réel, peu de friction évitable trouvée

**Priorité** : Low (constat majoritairement positif)
**Fichier:ligne** : `frontend/home-sources.ts:101-113` (`pickAndAddFolder`,
un seul appel `addSource` + refresh) ; `frontend/sift-live.ts` (wiring
Revue, décrit par `report-view.ts` et `filing.ts`) ; `frontend/filing.ts:1047`
(bouton "Appliquer les tags ID3", optionnel, distinct de "Ranger") ;
`frontend/journal.ts:186,258` (les 2 seuls `window.confirm` du repo, sur des
actions bulk destructrices) ; `frontend/filing.ts` keyboardHintsHtml
(`filing.ts:137-144`, raccourcis SPACE/ENTER/BKSP/HAUT-BAS).

**Description** : le chemin réel tracé est : Accueil (ajouter un dossier
surveillé, un clic + picker OS) → le watcher scanne automatiquement (pas
d'action utilisateur) → Revue (la piste apparaît en file, son-d'abord) →
écoute (SPACE) → décision : Ranger (ENTER) ou Écarter (BKSP). L'identification
Discogs et l'application des tags ID3 sont des actions EXPLICITEMENT
optionnelles avant le rangement (bouton séparé "Récupérer les métadonnées
Discogs", bouton séparé "Appliquer les tags ID3" — `filing.ts:1022,1047`),
pas des étapes obligatoires insérées dans le flux. Les seules confirmations
modales (`window.confirm`) protègent des actions bulk irréversibles (accepter
route un article) : accepter/écarter en masse (`journal.ts:186`) et annuler
un batch de plus de 10 morceaux (`journal.ts:258`) — ce sont des garde-fous
proportionnés, pas de la friction gratuite.

**Impact concret** : aucune étape intermédiaire inutile identifiée sur le
chemin nominal (écouter → ranger). Le flux respecte le principe affiché en
CLAUDE.md ("Un seul geste par morceau : écouter → ranger ou écarter").

**Cause probable** : n/a — c'est le résultat d'un design déjà itéré (Phases
0-2 de la refonte Revue, cf. mémoire "Refonte Revue").

**Proposition** : rien à trancher ici ; à re-vérifier seulement si un futur
écran ajoute une étape obligatoire entre Revue et Ranger.

**Effort** : n/a. **Bénéfice attendu** : n/a.

---

## Constat 6 — `reject_track` (à re-sourcer) vs `trash_track` (corbeille) : distinction réelle et suffisamment claire

**Priorité** : Low (constat de vérification, pas un problème)
**Fichier:ligne** : `src-tauri/src/ecartes.rs:43-64` (`requeue_track`,
inverse de reject) ; `src-tauri/src/ecartes.rs:99-` (`restore` depuis
`.sift-trash`) ; `frontend/ecartes-view.ts:15-24` (`ecReason` — badges
distincts "tronqué" / "faux" / "à re-sourcer" pour les rejets, un badge
séparé "Purger la corbeille" pour la corbeille, `ecartes-view.ts:104-107`).

**Description** : `reject` place le morceau en `status='resourcing'` (le
fichier original reste en place, en attente d'un meilleur fichier à
retélécharger — le morceau réapparaît listé "à re-sourcer" avec liens
Soulseek/boutiques, `ecartes-view.ts:32-49`). `trash_track` déplace
physiquement le fichier vers `.sift-trash` (suppression physique différée,
purge après `trash_purge_days`). L'UI distingue bien les deux sections
("À re-sourcer" vs "Corbeille") avec des actions différentes (Restaurer =
remettre en file pour resourcing ; Restaurer = remettre le fichier en place
pour trash). Le chevauchement fonctionnel existe (un item "à re-sourcer" peut
lui-même être envoyé à la corbeille via le bouton trash à `ecartes-view.ts:78`)
mais c'est une transition d'état voulue (rejet → si jamais retrouvé plus tard →
corbeille), pas une duplication de fonctionnalité.

**Impact concret** : aucun problème de confusion utilisateur identifié dans
le code — les deux sections sont visuellement et fonctionnellement séparées.

**Cause probable** : n/a.
**Proposition** : n/a.
**Effort** : n/a. **Bénéfice attendu** : n/a.

---

## Constat 7 — `undo_last` est un raccourci de `revert_batch`, pas une fonctionnalité parallèle — pas de duplication réelle

**Priorité** : Low (constat de vérification)
**Fichier:ligne** : `src-tauri/src/actions.rs:256-276` (`undo_last`
interroge simplement le batch le plus récent non annulé puis appelle
`revert_batch(conn, &b)` directement, ligne 271).

**Description** : `undo_last` n'est pas une seconde implémentation
concurrente de l'annulation — c'est `revert_batch` appliqué au batch le plus
récent (LIFO). Aucune divergence de comportement entre les deux : la logique
métier (garde LIFO, réversion FS newest-first, gestion `tag_edit`-only,
restauration du statut `pending`) vit à un seul endroit
(`revert_batch`, lignes 179-252). C'est un wrapper de commodité, pas un
chevauchement fonctionnel problématique.

**Impact concret** : aucun — hypothèse de départ (chevauchement) invalidée
par la lecture du code.

**Cause probable** : n/a. **Proposition** : n/a.
**Effort** : n/a. **Bénéfice attendu** : n/a.

---

## HYPOTHÈSES NON VÉRIFIÉES

- L'écran Réglages lui-même (rendu de `filename_template`/`trash_purge_days`
  dans l'UI) n'a pas été lu directement dans cette passe (hors du set de
  fichiers listé pour Pass 6) — le Constat 4 se limite à ce que `settings.rs`
  expose comme surface possible, pas à sa présentation réelle à l'écran.
- Le contenu exact du DSL de masks de `naming.rs` (les tokens/fonctions
  réellement implémentés vs la veille MediaMonkey citée en doc) n'a pas été
  vérifié ligne à ligne — seule l'existence du setting `filename_template` et
  son défaut sont confirmés.
- L'onboarding réel du token Discogs (écran, message d'erreur `NO_TOKEN`
  mentionné dans `docs/ressources-externes.md`) n'a pas été inspecté dans
  cette passe — l'affirmation de friction d'onboarding (Constat 4) repose sur
  la nature du token (créer une app développeur Discogs), pas sur le wording
  UI observé.

---

## Synthèse des priorités

| Priorité | Nombre |
|---|---|
| Critical | 1 (Constat 3 — export Rekordbox/USB simulé) |
| High | 1 (Constat 1 — différenciateur CDJ noyé) |
| Medium | 1 (Constat 4 — réglages jamais touchés par l'utilisateur cible) |
| Low | 4 (Constats 2, 5, 6, 7 — vérifications/points positifs) |

**Verdict "no list" (Steve Jobs) / "build less" (37signals)** : la feature la
plus urgente à trancher n'est pas d'en ajouter une, mais de retirer ou geler
visiblement l'export Rekordbox/Clé USB tant qu'aucun backend n'existe — une
fausse promesse de fonctionnalité coûte plus cher en confiance qu'une case
vide honnête. Le second axe est de resserrer le message produit autour du
vrai différenciateur (compatibilité CDJ) au lieu de le laisser dilué dans une
liste de champs de métadonnées.
