# Relevé — « il faut cliquer Stop ~3 fois » à l'annulation du filing

Enquête LECTURE SEULE (méthode détective, fail-fast). Aucune modification, aucun commit.
Symptôme : l'utilisateur clique Stop ~3 fois avant que l'annulation prenne effet, **surtout** quand
le batch contient des **conversions** (hi-res/FLAC lents à encoder). On départage A (feedback
manquant, bénin) vs B (clic perdu, vrai bug).

---

## VERDICT

**HYPOTHÈSE A PROUVÉE** (latence réelle + feedback faible). **HYPOTHÈSE B INFIRMÉE** (le 1er clic
EST pris : le flag passe à `true` immédiatement, n'est jamais perdu ni remis à `false` en cours de
batch, et le bouton disparaît dès le 1er clic).

Le délai = la **durée du fichier en cours dans `execute_file`** (l'encode ffmpeg), car le flag
d'annulation n'est testé qu'**ENTRE** les fichiers, jamais pendant l'encode. Conversions = encode
ffmpeg multi-secondes → délai long ; move conforme = simple `rename` → quasi instantané. D'où
« pire sur les conversions ». Les clics 2-3 sont **redondants** (bouton déjà masqué + garde de
ré-entrée) : ils ne sont pas « nécessaires », ils sont du spam pendant l'attente.

---

## 1. Chemin exact du clic Stop (bouton → flag) — tout est PROUVÉ, rien n'est perdu

1. **Bouton** rendu sur la ligne « Filing » de la zone de progression **uniquement** si
   `state==="running" && !stopping && cancelHandlers.has("file")`
   ([progress-zone.ts:79-82](frontend/progress-zone.ts:79)).
2. **Clic** → un listener délégué unique sur la zone route vers le handler enregistré :
   `cancelHandlers.get("file")?.()` ([progress-zone.ts:64-67](frontend/progress-zone.ts:64)).
3. Handler = `onFileStop`, enregistré via `setCancelHandler("file", onFileStop)`
   ([sift-live.ts:849](frontend/sift-live.ts:849)).
4. **`onFileStop`** ([sift-live.ts:197-209](frontend/sift-live.ts:197)) :
   - `if (fileStopping) return;` — garde de ré-entrée (les re-clics suivants sont des no-op).
   - `fileStopping = true;` — état transitoire posé **inconditionnellement**.
   - `setTask("file", { …, stopping: true })` — passe la ligne en « Stopping… » et **masque** le
     bouton. (Sous garde `if (lastFileProgress)` — voir §3, sans effet pratique.)
   - `void fileCancel();` — **appelé inconditionnellement**, hors de la garde ci-dessus.
5. **`fileCancel`** → `invoke("file_cancel")` ([ipc.ts:100](frontend/ipc.ts:100)).
6. **`file_cancel`** (backend) → `app.state::<FilingCancel>().0.store(true, SeqCst)`
   ([ipc_filing.rs:122-126](src-tauri/src/ipc_filing.rs:122)).

→ **Le flag passe à `true` au PREMIER clic**, sans condition fragile (étapes 4→6 hors de toute
garde sensible). **B (clic perdu) est INFIRMÉE.**

## 2. Où est testé le flag dans `run_file_batch` → la latence

- Le flag n'est lu **qu'en tête de boucle**, AVANT plan/execute/commit :
  `if cancel.0.load(SeqCst) { cancelled = true; break; }` ([ipc_filing.rs:161-164](src-tauri/src/ipc_filing.rs:161)).
- La phase 2 **`execute_file`** (l'encode ffmpeg lent + les déplacements) tourne **sans relire le
  flag** ([ipc_filing.rs:192-199](src-tauri/src/ipc_filing.rs:192)). Idem phases 1/3 sous lock.
- Donc après le clic (flag=true), la boucle **finit le fichier courant** (ses 3 phases), revient en
  tête, lit le flag, et **break**.

**Mesure conceptuelle** : latence d'arrêt = temps restant du `execute_file` en cours (+ son commit).
- **Conversion** (hi-res/FLAC → AIFF) : `execute_file` lance un encode ffmpeg complet → de quelques
  secondes à des dizaines de secondes pour un gros fichier. C'est CE délai que ressent l'utilisateur.
- **Move conforme** : `execute_file` = un `rename` → quasi nul. D'où la dépendance au type de fichier,
  exactement comme décrit (« surtout sur les conversions »).

C'est la **règle de cohérence établie** (sous-étape 3) qui impose ce plancher : on ne coupe jamais un
`execute_file` en plein milieu → la granularité minimale d'annulation = **un fichier**.

## 3. Le bouton reste-t-il cliquable après le 1er clic ?

**Non — il disparaît.** Le rendu ne produit le bouton que si `!p.stopping`
([progress-zone.ts:79-82](frontend/progress-zone.ts:79)) ; dès `stopping:true` la branche renvoie `""`
(bouton **retiré du DOM**, pas seulement désactivé) et le libellé devient « Stopping… »
([progress-zone.ts:77](frontend/progress-zone.ts:77)). L'état transitoire **existe et s'affiche**.

De plus, chaque `file:progress` suivant ré-émet `stopping: fileStopping` (donc `true`)
([sift-live.ts:186](frontend/sift-live.ts:186)) → le bouton **reste masqué** jusqu'au `file:done`.
Les re-clics ne peuvent donc PAS retomber sur le bouton ; et s'ils tombent sur le vide laissé, la
garde `if (fileStopping) return` ([sift-live.ts:198](frontend/sift-live.ts:198)) les neutralise.

Nuance mineure (pas un bug) : la mise à « Stopping… » est sous `if (lastFileProgress)`
([sift-live.ts:200](frontend/sift-live.ts:200)). En pratique le bouton n'est cliquable que si une
ligne « file » running existe, ce qui n'arrive qu'après le 1er `file:progress` (done:0) qui pose
`lastFileProgress` ([sift-live.ts:178-179](frontend/sift-live.ts:178)) — donc la garde passe
toujours quand le bouton est visible. Le flag, lui, est posé même si la garde échouait.

## 4. Le flag est-il remis à `false` par erreur entre les clics ?

**Non.** Le seul reset est `store(false)` **au démarrage d'un nouveau batch** dans `file_batch`
([ipc_filing.rs:107](src-tauri/src/ipc_filing.rs:107)), et côté front `fileStopping=false` au lancement
(`runBatchFile` [sift-live.ts:431](frontend/sift-live.ts:431)) et à la fin (`onFileBatchDone`
[sift-live.ts:467](frontend/sift-live.ts:467)). **Jamais entre deux clics du même batch.** Aucun
re-render ne réinitialise le flag (le store TS est au niveau module, pas un état recréé au rendu).
→ **B point 4 INFIRMÉ.**

## Pourquoi la perception « ~3 clics »

Le 1er clic a déjà tout fait (flag=true, bouton masqué, « Stopping… » affiché). Mais pendant l'encode
en cours **rien ne bouge visiblement** : aucun `file:progress` n'est émis pendant un `execute_file`
(le prochain est en tête de l'itération suivante, [ipc_filing.rs:168](src-tauri/src/ipc_filing.rs:168)),
donc le compteur `done` est figé et le seul retour est le petit libellé « Stopping… » **tout en bas
du rail de nav** (la zone est appendue au bas de `#nav`, [progress-zone.ts:54-62](frontend/progress-zone.ts:54)),
loin de l'endroit où l'utilisateur vient de cliquer « File » (#filfoot). Ne voyant pas d'effet, il
re-clique — sur un bouton déjà disparu / dans le vide. L'annulation tombe quand l'encode courant
finit, **indépendamment** des clics supplémentaires.

→ « 3 clics » = **latence réelle + feedback faible/mal placé**, PAS des clics perdus.

---

## Options de correction (coût / risque) — AUCUNE appliquée ici

### (a) Corriger le FEEDBACK — recommandé (cause de la perception « 3 clics »)
Confirmer immédiatement que le clic est pris et expliquer l'attente, **là où l'utilisateur regarde**.
- Au clic, poser un message près du rail de batch (#filfoot, via `fileNote`) du type « Annulation
  demandée — fin du fichier en cours… » en plus du « Stopping… » de la zone basse.
- Optionnel : enrichir le libellé de la ligne (« Stopping… (finishing current file) ») et/ou garder
  le bouton **visible mais désactivé/grisé** plutôt que retiré, pour un retour visuel explicite.
- Optionnel : durcir la nuance §3 (faire le `setTask` « Stopping… » même si `lastFileProgress` est
  null) — cosmétique.
- **Coût : petit** (front uniquement, aucun changement moteur). **Risque : faible.** Traite
  directement le symptôme rapporté.

### (b) Réduire la LATENCE réelle — borné par la règle de cohérence
- Le plancher est **un fichier** : on ne peut pas tester le flag « pendant » un encode sans couper
  `execute_file`, ce que la règle établie (sous-étape 3) **interdit**. Tester le flag plus souvent
  AUTOUR de l'encode n'aide pas : entre deux fichiers il est déjà testé ; pendant un fichier il n'y
  a pas de point de découpe sûr.
- Le SEUL moyen de descendre sous « un fichier » serait de **tuer le process ffmpeg** en cours
  (l'encode produirait une sortie partielle à nettoyer — `execute_file` nettoie déjà son orphelin
  en cas d'échec, mais ici il faudrait traiter un ffmpeg tué comme un abandon propre). Cela
  **viole la règle « ne jamais couper un `execute_file` »** et ajoute du risque (sortie partielle,
  états mi-faits). **À ne PAS faire** sans rouvrir explicitement cette règle.
- **Coût : moyen-élevé. Risque : élevé** (cohérence FS + journal). Non recommandé.

**Recommandation** : faire (a) — feedback immédiat et bien placé — qui élimine la perception « 3
clics » sans toucher au moteur. (b) est plafonné par la règle de cohérence : la latence d'une
conversion en cours est incompressible tant qu'on ne tue pas ffmpeg (déconseillé).

═══ STOP ═══ Hypothèse A prouvée (latence d'un fichier + feedback faible) ; B infirmée (1er clic
pris, flag posé immédiatement, jamais perdu/réinitialisé, bouton masqué). Aucune correction ici.
NE PAS toucher (rappel) : le code, le moteur filing/annulation, TRASH_PURGE_DAYS, P-6.
