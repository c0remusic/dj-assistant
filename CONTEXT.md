# CONTEXT — sift

Sift = app desktop gratuite (Windows + macOS) de préparation/organisation de
bibliothèque musicale pour DJ : analyse qualité, dédoublonnage, identification,
rangement. Le poste de prépa entre Soulseek et les platines. Principe produit
central : **« déplacer = encoder + ranger »** (`PRD.md`, `PRODUCT.md`,
`docs/design-system/foundations.md`).

Ce fichier fige le **langage de domaine partagé** (concepts, entités, états,
actions) — pas les mécanismes d'implémentation. Chaque terme est ancré sur une
source réelle du repo. Libellés d'interface = source canonique
`docs/design-system/content.md` (l'app est en français ; les identifiants de
code anglais sont notés quand ils divergent du terme métier).

## Glossaire

### Entités et concepts

**Morceau** — un fichier audio traité par Sift (une piste = une ligne `tracks`
en base). L'unité de travail de bout en bout. _Avoid_: « chanson ». Note code :
entité `track`.

**File** — la liste des morceaux « à traiter » (`status='pending'`). Colonne
vertébrale du flux d'entrée. Source : `content.md` (« Liste de traitement →
File »), `queue.rs`. _Avoid_: « queue » (identifiant de code seulement), « file
d'attente » en libellé.

**Revue** — l'écran de décision, un morceau à la fois : écouter, vérifier le
diagnostic, identifier, choisir la destination, convertir ou écarter. Poste
central du produit. Source : `content.md`, `foundations.md`, `report-view.ts`.
_Avoid_: « inspecteur », « détail » comme nom d'écran.

**Bac** — un dossier de destination défini par l'utilisateur, sous la racine de
bibliothèque (ou un chemin absolu de confiance hors racine). C'est là qu'un
morceau est rangé. Source : identifiant de code `bin` / `bin_rel` (`filing.rs`).
_Avoid_: « bin » (code seulement), « dossier cible » comme terme fixe.

**Bibliothèque** — la collection propre déjà convertie et rangée ; surface de
parcours/édition/re-rangement et du régime permanent. Source : `library.rs`,
`PRD.md`. _Avoid_: « collection » quand on parle de l'écran/surface Sift.

**Écartés** — la branche de rejet : morceaux que l'utilisateur ne veut pas,
**parqués, jamais supprimés secs** ; re-sourçables (liens d'achat, copie du nom
pour Soulseek) ou envoyés à la corbeille. Source : `ecartes.rs`, `content.md`.

**Journal** — journal des actions de rangement (surtout par lot), **journalisé
et annulable** (revert). Support concret de la réversibilité. Source :
`journal.ts`, `actions.rs`, `PRD.md`.

**Diagnostic audio** — le résultat d'analyse d'un morceau (verdict qualité +
mesures : clipping, troncature, silence, DC, phase, spectre). Source :
`content.md` (« Analyse audio → Diagnostic audio »), `analysis/`. _Avoid_:
« analyse » comme nom de la section d'écran (réserver « analyser » au verbe).

**Verdict** — le jugement de qualité produit par le diagnostic. Trois valeurs
réelles (`analysis/mod.rs`, enum `Verdict`) :
- **Vrai lossless** (code `Ok`) — lossless authentique.
- **Faux lossless** (code `Fake`) — déclaré FLAC/WAV/AIFF mais contenu lossy
  (cliff spectral), ou mismatch conteneur/contenu. La fraude que Sift traque.
- **Douteux** (code `Grey`) — zone grise, indéterminé.
_Avoid_: « Clean » (jamais une valeur d'enum réelle), « sur-encodé » comme
synonyme de Faux lossless (c'est un état d'affichage distinct).

**name_key** — clé de nom normalisée d'un morceau ; base de la détection de
doublons sur le flux entrant. Terme technique employé tel quel dans le domaine
(`PRD.md`, `dedup.rs`). _Avoid_: « clé normalisée » sans le nom exact.

**Empreinte** — empreinte acoustique locale (Chromaprint) calculée à la demande
pour détecter les **doublons internes** de la Bibliothèque par le contenu.
Source : `fingerprint.rs`, `PRD.md`. _Avoid_: « fingerprint » (code), « hash »
(pas la même chose).

**Doublon** — deux morceaux identifiés comme le même : par `name_key` (flux
entrant) ou par empreinte (interne, à la demande). Sift **propose** de garder le
meilleur (Vrai > Faux lossless > bitrate/format), la décision reste **manuelle**.
Source : `dedup.rs`, `PRD.md`.

**Métadonnées** — identification/tags d'un morceau (titre, artiste, année,
pochette, genre, `release_id`), **proposés par Discogs, confirmés par l'humain**.
Source : `content.md` (« Identification/tags → Métadonnées »), `metadata/`.
_Avoid_: « tags » seul comme nom de la section (garder « Appliquer les tags »
pour l'action d'écriture).

**Match** — un résultat candidat renvoyé par Discogs à identifier. Source :
`content.md` (« Résultat Discogs → Match »).

**Destination** — le dossier (bac) choisi pour ranger un morceau. **Toujours
choisie par l'utilisateur** (pas d'auto-rangement par genre aujourd'hui).
Source : `content.md` (« Choix de dossier → Destination »).

**Format** — le format de sortie du fichier converti (ex. `AIFF`, `WAV`, `FLAC`,
`MP3`). Casse d'origine conservée. Source : `content.md`.

**Nom final** — le nom de fichier calculé après identification et choix du
format, avant rangement. Source : `content.md`, `naming.rs`.

**Source** — un dossier surveillé/importé d'où viennent les morceaux (import
manuel initial, ou watcher en régime permanent). Source : `sources.rs`,
`home-sources.ts`.

**Watcher** — surveillance live des dossiers sources qui prend en charge les
téléchargements futurs au fil de l'eau (régime permanent). Source :
`watcher.rs`, `PRD.md`.

### Intégration Rekordbox / clé USB (satellite export)

**Export Rekordbox** — écriture/synchro vers Rekordbox : export XML et/ou
écriture directe du `master.db`, avec chaîne de sûreté (backup → vérif
round-trip → rollback). Source : `rekordbox_xml.rs`, `rekordbox_masterdb.rs`,
`PRD.md`. _Avoid_: « sauvegarder » / « exporter » pour d'autres actions (réserver
« Exporter » au vrai fichier de sortie externe, cf. `content.md`).

**master.db** — la base Rekordbox chiffrée (SQLCipher) écrite en direct par Sift
(réparation de chemins, dédup playlists, synchro métadonnées/pochette). Terme
employé tel quel. Source : `rekordbox_masterdb.rs`.

**drift_detected** — signal indiquant que l'état XML Rekordbox a divergé de la
bibliothèque Sift. Distinct des réparations `master.db`, jamais fusionné avec
elles. Source : `rekordbox_xml.rs`, spec page Rekordbox.

### Actions (libellés canoniques)

**Convertir** — l'action principale : encoder (au besoin) **et** ranger dans la
destination, en un seul geste. Source : `content.md` (remplace « Ranger » le
2026-07-10, `filing.ts:220`). _Avoid_: **« Ranger »** comme libellé de bouton
(le concept produit « déplacer = encoder + ranger » garde le mot « ranger »,
mais ce n'est plus le libellé affiché).

**Écarter** — envoyer un morceau dans Écartés (rejet récupérable). Source :
`content.md` (remplace « Jeter » le 2026-07-10, `filing.ts:717`). _Avoid_:
**« Jeter »**, « supprimer » (rien n'est supprimé sec par cette action).

**Annuler / revert** — défaire une action de rangement via le Journal
(corbeille/undo). Réversibilité garantie. Source : `actions.rs`, `journal.ts`.

### États d'un morceau (libellés + statut réel en base)

Libellés canoniques (`content.md`) : **Prêt à ranger**, **À finaliser**,
**À vérifier**, **Destination manquante**, **CDJ compatible / incompatible**.

Statuts réels persistés (colonne `tracks.status`, vérifiés en code) :
`pending` (dans la File), `filed` (rangé en Bibliothèque),
`resourcing` (Écartés, en re-sourçage), `trash` (Écartés, corbeille),
`purged`, `excluded` (source exclue). _Avoid_: inventer d'autres noms de
statut ; ce sont les seules valeurs réelles.

## Relations / cardinalités

- Une **Source** (surveillée) alimente 0..N **Morceaux** dans la **File**.
- Un **Morceau** a exactement un **Verdict** (via son **Diagnostic audio**) une
  fois analysé.
- Un **Doublon** relie 2..N **Morceaux** (par `name_key` ou **Empreinte**) ;
  l'utilisateur en garde un, les autres partent en **Écartés**.
- Un **Morceau** identifié porte un jeu de **Métadonnées** issu d'un **Match**
  Discogs confirmé.
- **Convertir** un Morceau le range dans exactement un **Bac** (sa
  **Destination**) et le fait passer `pending` → `filed`.
- **Écarter** fait passer un Morceau `pending` → `resourcing`/`trash` (jamais
  supprimé sec ; original toujours récupérable).
- Chaque action de rangement produit une entrée de **Journal** annulable.
- L'**Export Rekordbox** projette des Morceaux `filed` vers XML et/ou
  `master.db` sans jamais casser cue points / playlists / historique.
