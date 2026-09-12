# PRD — Performance et micro-interactions

Chantier ouvert le 2026-07-27. Cadre les corrections de lenteur et de feedback issues de deux
audits menés le même jour : une boucle de recherche en 3 tours sur la performance et
l'architecture, et un catalogue de micro-interactions. Les deux ont été relus en contexte frais
par un modèle tiers.

Ce document dit le QUOI et les critères d'acceptation. Le COMMENT vit dans le plan
d'implémentation qui l'accompagnera.

---

## 1. Le problème, en une phrase

L'application ralentit et gèle sur des gestes ordinaires, et une partie de ses actions ne
produisent aucun retour visible — sans qu'aucun chiffre n'ait jamais été mesuré pour dire où ni
combien.

## 2. Ce qui a été mesuré (2026-07-27)

Premières mesures réelles du projet depuis la Phase 3 du 2026-07-14. Prises en lecture seule sur
la base de production (`%APPDATA%/com.sift.app/sift.db`) et sur les logs d'un `tauri dev` en cours
d'analyse.

| Grandeur | Valeur mesurée |
|---|---|
| Pistes en base | 3 907 (3 906 `pending`, 1 `filed`) |
| Fichier `sift.db` | 6 520,9 Mo (+ 45,5 Mo de WAL) |
| Cumul `report_json` | 6 323,3 Mo, soit **97 % du fichier** |
| Moyenne par rapport | 1 657 Ko — maximum 3 191 Ko |
| `actions.meta` | 176,5 Mo pour **58 lignes**, dont 176,5 Mo sur 26 `tag_edit` |
| Plus grosse pochette journalisée | 55 710 Ko de JSON pour une seule image |
| Analyse (decode + DSP) | médiane 985 ms, p90 1 417 ms, max 4 159 ms, sur 1 581 pistes |
| Débit d'analyse | 1 s d'audio traitée en 2,4 ms |

## 3. La cause racine, unique

**`serde` sérialise un `Vec<u8>` en tableau d'entiers décimaux JSON**, soit environ quatre
caractères par octet. Ce seul défaut explique les deux postes ci-dessus :

- le spectrogramme d'affichage est correctement borné en mémoire — `MAX_COLS = 1200` colonnes en
  `u8`, `src-tauri/src/analysis/spectrum.rs:226` et `:253`, de l'ordre de 300 Ko — mais atteint
  1 657 Ko une fois écrit dans `report_json` ;
- la pochette d'origine conservée pour l'annulation d'un `tag_edit` (`meta.cover.bytes`) suit
  exactement le même chemin.

Conséquence directe : ce n'est **pas** un arbitrage entre latence et stockage. Le cache du
spectrogramme existe pour que le clic sur la Revue ne redécode pas le fichier, et ce redécodage
coûterait 985 ms en médiane et jusqu'à 4,2 s. Le cache est justifié ; c'est son encodage qui est
fautif.

## 4. Décisions prises (2026-07-27)

**D1 — Cible de volume : 15 000 pistes.** Reprend la cible V1 déjà écrite dans
`docs/superpowers/plans/2026-07-14-phase3-decision.md`. Tout ce qui tient à 15 000 part ; ce qui
ne casse qu'à 100 000 est différé avec un déclencheur de réouverture nommé, jamais renvoyé
vaguement à plus tard.

**D2 — Le snapshot d'annulation reste self-contained.** L'annulation d'un `tag_edit` doit rester
fidèle : si une pochette Discogs a été appliquée, l'ancienne n'existe plus nulle part ailleurs, le
fichier audio ayant été réécrit. Elle reste donc embarquée dans `actions.meta`, comme aujourd'hui.
Seul son encodage change (voir P2).

*Cette décision a été prise, puis renversée le jour même au checkpoint.* La première version
sortait la pochette dans un fichier à côté, `meta` ne gardant qu'un chemin. Le grilling a montré
que ce choix avait déjà été fait et écarté, avec sa raison écrite dans le code
(`src-tauri/src/tagging.rs:114-116`) : *« self-contained means a revert can never be orphaned by a
missing backup, at the cost of a larger journal row for the rare tag edit »*. Le motif tient sur
les chiffres — 26 `tag_edit` pour 3 907 pistes — et le risque qu'il écarte est exactement celui
que la version sortie du fichier réintroduisait. Le réencodage seul ramène `actions.meta` de
176,5 Mo à environ 44 Mo, sans sous-système à écrire, sans orphelin possible et sans renverser une
décision existante.

**D3 — Budget de latence, avec un palier renforcé sur le geste répété.**

| Classe | Budget | Gestes concernés |
|---|---|---|
| Boucle de rangement | **< 50 ms** | clic sur un bac, aperçu du nom final, passage à la piste suivante, accusé du clic sur Convertir |
| Perçu instantané | < 100 ms | survol, sélection, frappe dans un filtre, retour sur un écran déjà visité |
| Fil de pensée | < 1 s | premier chargement d'un écran, ouverture d'un rapport, recherche Discogs |
| Long, avec indicateur | > 1 s | analyse, conversion, écriture master.db, scan de dossier |
| Transversal | 60 fps tenus | défilement de toute liste, sans exception |

Aucune opération, quelle que soit sa classe, ne bloque l'interface.

Deux conséquences assumées de D3.

**Le verrou SQLite global devient bloquant** pour ce PRD, et non simplement souhaitable. Les gestes
de la première ligne traversent des commandes qui le prennent pendant de l'I/O disque ; 50 ms est
hors d'atteinte tant qu'il n'est pas découpé.

**Le rangement devient asynchrone.** Constat du checkpoint : aujourd'hui `frontend/filing-actions.ts:51`
fait `await fileTrack(...)` et `src-tauri/src/filing.rs:522` appelle `encode::encode(...)` de façon
synchrone — le clic sur Convertir attend donc la fin de l'encodage FFmpeg, et aucun découpage de
verrou n'y changerait quoi que ce soit. Le budget porte sur l'**accusé** du clic, la conversion
continuant en tâche de fond avec progression. C'est un changement de parcours, pas seulement de
performance : il est acté ici en connaissance de cause, avec son traitement de l'échec en D5.
L'encodage est déjà hors du verrou DB pour une raison voisine, documentée en `filing.rs:297`.

**D4 — Rétention du journal : 30 jours glissants.** Constat du checkpoint : il n'existe aujourd'hui
**aucune** rétention, aucun `DELETE FROM actions` dans tout `src-tauri/src/actions.rs`. Le journal
croît indéfiniment. La borne est en temps et non en volume, ce qui est un choix assumé : elle est
lisible pour l'utilisateur (« vous avez un mois pour annuler ») mais ne plafonne pas le disque —
un mois intensif d'éditions de tags peut peser lourd. À rouvrir si le cas se présente.

Deux exceptions assumées à cette règle, écrites ici pour qu'elles ne soient pas découvertes plus
tard comme un bug : un lot **à demi annulé** (annulation interrompue en cours, encore reprenable) et
un lot **épinglé par une opération master.db vivante** (`pending` ou `ambiguous`, que l'utilisateur
n'a ni appliquée ni écartée — la ligne `rekordbox_masterdb_*` serait supprimée en cascade avec lui)
**n'expirent jamais, à aucun âge**. La correction passe avant la place. Conséquence : « les lignes
de plus de 30 jours disparaissent » n'est pas universellement vrai, et ce sont justement les lots à
demi annulés qui peuvent porter les gros `meta`. Ils partent quand l'utilisateur termine
l'annulation ou tranche l'opération Rekordbox.

**D5 — Échec tardif d'une conversion : retour dans la file, avec marqueur persistant.** Conséquence
de l'asynchrone acté en D3. La piste revient en `needs_validation` — la notion existe déjà et le
mode Lot s'en sert (`frontend/batch-tracklist.ts:79`) — avec un marqueur qui survit à la
navigation. Le toast accompagne, il ne porte jamais l'information à lui seul : un signal qui passe
pendant que le regard est ailleurs n'informe personne, et c'est précisément le motif qui a fait
couper trois micro-interactions du catalogue. Reste à trancher plus tard : où la piste réapparaît
dans l'ordre de la file.

## 5. Périmètre

### 5.1 Déjà livré, non committé (branche `perf-mi-fixes`)

Neuf fichiers, 133 insertions, 59 suppressions. `npx tsc --noEmit` propre, `cargo test` 396 passés
0 échec. Vérification manuelle en 31 points non encore passée.

- Spinner : suppression de la déclaration concurrente injectée au runtime, qui faisait passer tous
  les spinners de 0,7 s à 1 s dès qu'un rapport avait été ouvert une fois dans la session.
- Toast : deux implémentations concurrentes unifiées, remplacement par mutation en place.
- Dépôt de fichiers : `setDropActive` rendue idempotente, et accusé de réception qui compte ce que
  le backend a réellement importé plutôt que le nombre de chemins lâchés.
- Annulation depuis le toast : retour visible au succès comme à l'échec, qui manquait entièrement.
- File de Revue : plus de parcours récursif complet de la bibliothèque à chaque tick d'analyse ;
  défilement qui survit à un aller-retour de navigation.
- Commentaire de `src-tauri/src/worker.rs` réaligné sur la mesure.

### 5.2 Tranches à faire, dans l'ordre

**P1 — Instrumenter avant de corriger.** Sans mesure avant/après, aucun fix de ce PRD ne peut être
déclaré fini au sens de D3. Étendre `src-tauri/src/bench_volume.rs`, qui existe déjà et tourne à
la demande, aux commandes de la boucle de rangement. Done : un chiffre avant, reproductible, pour
chaque geste de la classe « boucle de rangement » et « perçu instantané ».

**P2 — Encodage binaire des `Vec<u8>`.** La cause racine du §3, traitée en un seul endroit :
`report_json` et `actions.meta`. Done : le cumul `report_json` **divisé par au moins 3,4** à volume
de pistes égal, spectrogramme toujours affiché à l'identique, aucune latence ajoutée à l'ouverture
d'un rapport. Preuve : `SELECT SUM(length(report_json))` avant / après sur la même base, plus
vérification visuelle du spectrogramme.

*Le critère était « ÷4 », il a été ramené à ÷3,4 après mesure.* Sur 24 rapports réels, base85 sur
`mag_db` seul donne ÷2,08, et ÷3,40 en plafonnant aussi l'enveloppe `peaks`. Les deux routes qui
atteignaient ÷4 ont été écartées en connaissance de cause : compresser en amont du base85 (÷4,82)
exige que le front décompresse via `DecompressionStream`, dont la présence dans WebView2 n'a pas
été vérifiée et dont l'absence ferait disparaître le spectrogramme ; réduire `DISPLAY_BINS` de 384
à 192 (~÷6) dégrade la précision verticale du spectrogramme, donc la lisibilité d'une coupure de
faux-lossless, c'est-à-dire le geste que le produit existe pour servir. Arbitré sur un rendu
comparatif des deux résolutions, généré depuis une piste réelle de la base. Le chiffre est amendé
plutôt qu'arrondi : une cible ratée qu'on réécrit en douce est pire qu'une cible revue.

Point relevé au checkpoint, à ne pas manquer : `REPORT_CACHE_VERSION` vaut 5
(`src-tauri/src/analysis/mod.rs:87`) et `src-tauri/src/ipc.rs:296` traite toute autre version comme
un cache miss qui se répare seul — le bump est donc sûr, aucune migration à écrire. **Mais un cache
miss ne supprime rien** : les 6,3 Go existants resteraient en base jusqu'à ce que chaque piste soit
rouverte une par une. Le gain ne se matérialise pas tout seul, il faut un vidage explicite de la
colonne au moment du bump.

**P3 — Rétention du journal (D4).** 30 jours glissants sur `actions`, la colonne `ts` existant
déjà. Done : les lignes plus anciennes que 30 jours ne sont plus en base, et l'annulation d'une
action encore dans la fenêtre reste fidèle, pochette comprise.

Ce que le checkpoint a écarté ici : `list_journal` ne charge **pas** `meta` — sa requête
(`src-tauri/src/actions.rs:872-878`) projette huit colonnes et exclut même `type NOT IN ('tag_edit')`.
Le seul chemin qui lit `meta` est `revert_batch` (`:726`), au moment d'annuler, sur le seul lot
concerné. Le poids du journal est donc un problème de disque, jamais de latence. Aucune projection
SQL à écrire.

**P4 — Le verrou SQLite ne couvre plus d'I/O disque.** Bloquant par D3. Le patron existe déjà dans
le repo et est documenté : `apply_tags` (`src-tauri/src/ipc_filing.rs:218-280`) et
`track_file_tags` (`:168-178`) font le découpage ; il n'a pas été propagé. Sites confirmés site par
site par deux audits indépendants, dont la lecture de tags, la réécriture de fichier, la copie vers
la corbeille, le parcours de bibliothèque, le parse XML et le décryptage master.db.
Done : la boucle de rangement passe sous 50 ms mesurés (P1), et aucune commande ne tient le verrou
pendant une opération de fichier. Risque à surveiller : correction, pas performance — toute
régression de cohérence des données annule la tranche.

**P5 — Rangement asynchrone (D3, D5).** L'accusé du clic sur Convertir revient sous 50 ms, la
conversion continue en tâche de fond avec progression, un échec ramène la piste en
`needs_validation` avec un marqueur persistant. À faire APRÈS P4 : tant que le verrou est pris
pendant de l'I/O, un accusé rapide masquerait un gel au lieu de le supprimer. Done : le budget de
50 ms tenu sur la boucle complète, mesuré par P1, et un échec provoqué délibérément qui remonte
visiblement alors que l'utilisateur est déjà deux pistes plus loin.

**P6 — Micro-interactions restantes.** Les quatre du catalogue qui ne sont pas encore livrées :
confirmation destructive désarmée par `disabled` plus garde horodatée, fondu du toast, jauge des
5 s du mode Lot indexée sur l'échéance, état occupé des écritures master.db.
Contrainte technique établie en revue : le retrait d'une classe « au rAF suivant » ne déclenche
aucune transition dans WebView2, le style à `opacity:0` n'étant jamais calculé. Double rAF ou
reflow forcé obligatoire, sans quoi les deux seuls fondus du lot seront livrés sans avoir jamais
joué. Done : la trace DevTools confirme que le mouvement ne touche que le compositing.

### 5.3 Conditionné à une mesure préalable — MESURES FAITES le 2026-07-31

Ces deux tranches ne partaient pas tant que le chiffre n'existait pas. Les deux mesures ont été
prises le 2026-07-31 ; elles tranchent en sens opposés.

- **Rekordbox, un cycle master.db par lot au lieu d'un par ligne.** Mécanisme confirmé ligne à
  ligne. Les deux inconnues sont levées : le `master.db` réel de cette machine pèse
  **21 094 400 octets (20,1 Mo)**, et `derive_keys` coûte **228 ms en moyenne, 253 ms au pire**
  (5 tours, `--release`, après un tour à blanc — bench versionné
  `bench_derive_keys_cout_fixe_par_ouverture`, `src-tauri/src/rekordbox_masterdb.rs`).
  **La mesure JUSTIFIE la tranche** : ce coût est fixe par ouverture et indépendant de la taille
  du fichier, donc un lot de 200 pistes paie aujourd'hui ~45 secondes de pur PBKDF2 que le
  batching ramènerait à 228 ms.
  **Elle reste néanmoins non livrable en l'état**, pour une raison qui n'est plus la mesure : c'est
  le seul chantier qui change la sémantique transactionnelle d'écriture dans le fichier d'une
  application tierce, et son moyen de preuve — les quatre tests `*_round_trips_on_real_masterdb_copy`
  — exige `SIFT_M8_REAL_COPY_DIR` et Rekordbox fermé. Le garde-fou du `CLAUDE.md` (« jamais une
  écriture sur un système live sur la seule foi d'un rapport d'agent ») interdit de la livrer sans
  ce round-trip. Déclencheur de reprise : une session avec la copie réelle disponible.
- **Export XML, suppression des quadratiques de chirurgie texte.** Mesure prise, elle **retire** la
  tranche du périmètre : il n'existe aucun XML lié réel. Le réglage `rekordbox_xml_path` de la base
  de production pointe
  `C:\Users\LEETJ\Desktop\dj-assistant-m6a\scratchpad\m8-tier3-test\library.xml`, un artefact de
  test d'un worktree supprimé, absent du disque — donc pas de `stat` possible. Et la base compte
  **1 seule piste `filed` sur 3907** (3906 `pending`) : le terme quadratique n'a aucune amplitude
  observable. Déplacée en §5.4 ci-dessous.
  Note annexe, hors périmètre de ce PRD : qu'un réglage de production pointe un chemin de dev
  supprimé est un défaut en soi, distinct de la performance.

### 5.4 Différé avec déclencheur nommé

- **Export XML, suppression des quadratiques de chirurgie texte** — descendu de §5.3 le
  2026-07-31 sur mesure : aucun XML lié réel (le réglage pointe un fichier de test disparu) et
  1 seule piste `filed` sur 3907 en base. Le mécanisme quadratique reste réel dans le code ; c'est
  son amplitude qui est nulle aujourd'hui. Déclencheur : un XML Rekordbox réellement lié ET plus
  de quelques centaines de pistes `filed` — auquel cas reprendre par le `stat` et le décompte
  décrits en §5.3, qui redeviennent alors mesurables.
- **Pagination de `list_filed` / `list_pending`** — mesurée fluide à 15 000 (18,6 ms), 165 à
  250 ms à 100 000, hors cible D1. Déclencheur : un utilisateur réel avec plus de 30 000 pistes
  rangées et une lenteur perçue.
- **Index composite de tri sur `metadata(artist,title)`** — déjà essayé et mesuré sans effet le
  2026-07-14, migration retirée avant d'être committée. Ne pas retenter sans un plan de requête
  qui montre autre chose.

### 5.5 Hors périmètre

- Les huit micro-interactions écartées en revue, listées avec leur motif dans le catalogue. Trois
  filtres a priori en sont ressortis et valent pour toute proposition future : tout mouvement
  rejoué à chaque action de la boucle de rangement est disqualifié ; toute animation attachée à une
  ligne de liste virtualisée est disqualifiée ; une animation ne répare pas un défaut de
  conception statique.
- Le tri automatique, la refonte d'écran, tout changement de parcours : relèvent du chantier UX
  qui suivra, pas de celui-ci.

## 6. Ce que ce PRD ne garantit pas

Trois limites, énoncées pour qu'aucune relecture future ne les prenne pour des acquis.

1. **Le balayage n'est pas exhaustif.** La recherche s'est arrêtée sur un plafond de trois tours
   sans jamais atteindre un tour à vide, et le troisième produisait encore des findings solides.
2. **Trois chemins chauds n'ont été audités par personne** : le pool d'encodage phase 2, qui est le
   coût dominant du geste central et qui a servi d'argument pour écarter d'autres findings sans
   jamais être examiné lui-même ; le refetch complet de la file à chaque tick ; et le coût de
   `JSON.parse` côté WebView2 sur les gros payloads.
3. **Il n'existe aucune suite de tests frontend.** `tsc --noEmit` prouve la compilation et rien
   d'autre. Toute vérification de rendu, de timing ou d'interaction reste manuelle, dans la vraie
   fenêtre `tauri dev`.

## 7. Dette adjacente relevée, à arbitrer séparément

- `ensureStyles()` dans `frontend/report-view.ts` est devenu du code entièrement mort : la seule
  règle restante cible une classe qui n'est posée nulle part.
- Une troisième implémentation de `toast` subsiste dans `frontend/library-detail.ts:33-51`, avec
  son propre minuteur non annulable.
- L'annulation par Ctrl+Z (`frontend/filing.ts:573-577`) reste muette en cas d'échec, alors que le
  même échec s'affiche depuis le bouton du toast.
- `folders_added` sur-compte : redéposer un dossier déjà surveillé annonce un ajout qui n'a pas eu
  lieu, `sources::add` et `create_bin` renvoyant `Ok` sans rien créer.
- La borne périmée de 204 800 octets survit dans deux documents, `2026-07-02-rapport-final-audit-sift.md:33`
  et `2026-07-02-plan-fix-post-audit.md:85`. Elle a déjà induit un audit en erreur cette semaine.
