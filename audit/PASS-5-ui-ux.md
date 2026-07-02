# PASS 5 — Audit UI/UX — Sift

Date : 2026-07-02. Branche `m6a-discogs`, worktree `dj-assistant-m6a`.
Méthode : détective (fichier:ligne + extrait réel), sévère (pré-commercialisation).
Outillage invoqué : `interface-design` (source de vérité tokens : `.interface-design/system.md`,
section Mode Batch écartée par consigne), `ux-heuristics` (Nielsen), `design-everyday-things`
(affordances/signifiants/feedback), `refactoring-ui` (hiérarchie visuelle).

Fichiers lus intégralement : `frontend/filing.ts` (1653 l.), `frontend/report-view.ts` (826 l.),
`frontend/progress-zone.ts` (204 l.), `frontend/journal.ts` (358 l.), `frontend/library-detail.ts`
(337 l.), `frontend/ecartes-view.ts` (113 l.), `frontend/batch-tracklist.ts` (95 l.),
`frontend/chrome.ts` (158 l.), `frontend/empty-state.ts` (46 l.), `index.html`,
`.interface-design/system.md`. `frontend/sift-live.ts` lu par sections ciblées (rail batch,
nav a11y) via grep + lecture directe des lignes 540-720.

---

## CRITICAL

### C1 — Bouton "Ranger" reste dans le DOM sans feedback pendant l'appel réseau, mais un double-clic
peut passer avant le `disabled`

**Fichier** : `frontend/filing.ts:1217-1266` (`doRanger`)

```ts
async function doRanger(mid: HTMLElement): Promise<void> {
  if (!state.track || !state.canonical || acting) return;
  ...
  const ranger = document.querySelector<HTMLElement>('[data-fil="ranger"]');
  const orig = ranger?.innerHTML ?? null;
  acting = true;
  setActionsDisabled(true);
  if (ranger)
    ranger.innerHTML =
      '<i class="ti ti-loader-2 sift-spin sift-icon-inline-md"></i> Rangement en cours…';
  try {
    const res = await fileTrack(state.track.id, dest, state.target, state.canonical);
```

**Description** : `acting` est un flag global (l.1202), donc en pratique une seconde frappe Entrée
est bloquée dès la première ligne (`if (... || acting) return`). MAIS le raccourci clavier
(`installFilingKeys`, l.1553-1555) déclenche un `.click()` synthétique sur le même bouton — la
garde `acting` protège correctement contre le double-submit. Ce point est donc **couvert** côté
race condition logique. La partie réellement Critical est ailleurs : `setActionsDisabled` (l.1206-
1214) ne désactive QUE `[data-fil="ranger"],[data-fil="resource"],[data-fil="trash"]`. Le bouton
Destination (`[data-fil="destbtn"]`, l.885) et les chips de format (`[data-fil="fmt"]`, l.901-907)
restent cliquables pendant le rangement en cours (le spinner tourne sur Ranger, mais l'utilisateur
peut changer le format ou rouvrir le popover destination PENDANT l'encodage FFmpeg réel).

**Impact concret** : un DJ presse Entrée pour ranger, puis clique sur AIFF pour changer le format
avant que l'encodage soit fini — `state.target` change sous le pied de l'appel `fileTrack` déjà en
vol (qui a capturé `state.target` par valeur au moment de l'appel, donc pas de corruption de
données, mais l'UI affiche un format différent de celui réellement en cours d'encodage, ce qui
peut faire croire à l'utilisateur que sa modification s'applique alors qu'elle ne s'appliquera
qu'au PROCHAIN morceau).

**Cause probable** : `setActionsDisabled` a été pensé pour les 3 actions principales seulement,
sans englober la destination/format qui restent "en lecture" pendant l'action en cours.

**Correction proposée** : geler aussi `[data-fil="destbtn"]` et les chips `[data-fil="fmt"]`
pendant `acting`.

**Effort** : S (ajouter 2 sélecteurs à `setActionsDisabled`).
**Bénéfice** : élimine une confusion réelle bien que rare (fenêtre de quelques centaines de ms à
quelques secondes selon la taille du fichier encodé).

---

### C2 — Aucun `aria-label` sur AUCUN bouton icon-only des écrans Revue/Bibliothèque/Écartés/Journal

**Fichiers** (zéro occurrence d'`aria-label` confirmée par grep) :
`frontend/filing.ts`, `frontend/report-view.ts`, `frontend/journal.ts`, `frontend/ecartes-view.ts`,
`frontend/library-detail.ts`.

Exemples concrets :
- `frontend/report-view.ts:173` : `<button class="sift-play sift-play-btn" title="Lecture / pause (espace)"><i class="ti ti-player-play"></i></button>` — play/pause, aucun texte ni aria-label, seul un `title` (tooltip hover, invisible au clavier/lecteur d'écran tant que l'élément n'a pas le focus ET que l'AT lit les title, ce qui n'est pas garanti — NVDA/JAWS ne lisent PAS systématiquement `title` sur un `<button>`).
- `frontend/journal.ts:95` : `<button class="jrnl-revert" data-jact="revert" data-batch-id="${bid}" title="Annuler">&#x21A9;</button>` — le contenu textuel visible est le caractère Unicode `↩`, pas un vrai label ; un lecteur d'écran énoncera probablement "flèche courbée gauche" ou rien.
- `frontend/filing.ts:1290` : `<button data-fil="filed-close" title="Fermer" class="sift-filed-banner-close"><i class="ti ti-x"></i></button>` — fermeture de la bannière "Rangé", icon-only.
- `frontend/report-view.ts:163` : `<button class="sift-close sift-report-close">fermer</button>` — celui-ci a un vrai texte, donc OK.

**Contraste avec ce qui EST corrigé** : `frontend/sift-live.ts:1072-1073` a bien `aria-label`
(`aria-label="Discogs page"`, `aria-label="Identify"`) sur les liens Bibliothèque — donc le
correctif partiel mentionné dans `docs/ressources-externes.md` ("Veille UX", point 1, gap connu)
n'a été appliqué **qu'à sift-live.ts**, pas répercuté aux modules extraits (filing.ts, report-view.ts,
journal.ts, ecartes-view.ts, library-detail.ts) qui contiennent la majorité des boutons icon-only
de l'app.

**Impact concret** : un DJ utilisant un lecteur d'écran (ou juste naviguant au clavier avec un
`title` qui ne s'affiche qu'au survol souris) ne peut pas identifier la fonction du bouton Play,
Revert, Fermer, Trash, Restaurer sans deviner depuis l'icône seule.

**Cause probable** : le pattern `aria-label` a été introduit une fois (sift-live.ts, Bibliothèque)
mais jamais généralisé aux autres modules extraits du god-file — dette de cohérence, pas un choix
délibéré.

**Correction proposée** : ajouter `aria-label` (calqué sur le `title` déjà présent, quand il existe)
à tous les boutons icon-only recensés ci-dessus, dans les 5 fichiers listés.

**Effort** : M (une vingtaine de boutons à travers 5 fichiers, mécanique une fois le pattern choisi).
**Bénéfice** : conformité a11y de base (WCAG 4.1.2 Name, Role, Value), déjà notée comme gap connu
dans la veille produit — corrige un point que le projet s'était lui-même engagé à traiter.

---

## HIGH

### H1 — Aucun état "disabled" visuel sur les contrôles de format pendant `identify()` / `doIdentify`

**Fichier** : `frontend/filing.ts:803-849` (`doIdentify`)

```ts
async function doIdentify(
  btn: HTMLButtonElement,
  host: HTMLElement,
  editor: HTMLElement,
  mid: HTMLElement,
): Promise<void> {
  if (!state.track) return;
  const trackId = state.track.id;
  const origLabel = btn.innerHTML;
  btn.disabled = true;
  btn.innerHTML = '<i class="ti ti-loader-2 sift-spin sift-searching-icon"></i> Recherche…';
```

**Description** : seul le bouton Identifier lui-même (`btn`) est désactivé pendant la recherche
Discogs. Rien n'empêche l'utilisateur de cliquer "Ranger" (`[data-fil="ranger"]`) PENDANT que
`identify()` est en vol — `doRanger` (l.1217) ne vérifie que `acting`, une variable distincte de
l'état "identification en cours". Si l'utilisateur range le morceau avant que la réponse Discogs
arrive, `onIdentityApplied` (l.634) continuera à écrire dans `state.canonical` — mais `state.track`
peut avoir changé entre-temps (auto-avance après `doRanger`, l.1250), créant une désynchronisation :
l'identité Discogs de l'ANCIEN morceau s'applique au NOUVEAU morceau affiché à l'écran, sans garde
`openSeq` dans `wireCandidateClicks`/`onIdentityApplied` (contrairement à `openFilingInto`, qui
lui EST protégé par `myseq !== openSeq`, l.1463).

**Impact concret** : DJ presse "I" puis "Enter" rapidement (deux raccourcis clavier documentés,
`installFilingKeys`, l.1531-1565) sur un morceau — l'identification Discogs qui revient après le
Ranger peut potentiellement peindre du texte sur le mauvais morceau (l'éditeur du NOUVEAU morceau
ouvert, cf. `editor`/`mid` passés par closure dans `doIdentify` — ces références DOM restent celles
du morceau précédent après un `renderFoot`/`renderEditor` qui recrée `#filfoot`/.sift-fil-editor
par innerHTML, donc en pratique la mise à jour tombe dans le vide DOM détaché la plupart du temps,
mais reste un risque de correction silencieuse incorrecte plutôt qu'un crash visible).

**Cause probable** : deux flux asynchrones concurrents (identify vs file) sans jeton de session
partagé entre eux — seul `openFilingInto` a `openSeq`.

**Correction proposée** : soit désactiver Ranger/Discard pendant qu'une identification est en vol
pour CE morceau, soit faire porter `myseq`/`openSeq` par `doIdentify`/`onIdentityApplied` comme il
l'est déjà pour `openFilingInto`.

**Effort** : M.
**Bénéfice** : élimine une classe de bug de données silencieuses (métadonnées croisées entre
morceaux), pas juste un problème de style.

---

### H2 — Feedback "Appliquer les tags ID3" et "Rangement" utilisent des patterns visuels différents
pour la même idée (succès + annulation)

**Fichiers** :
- `frontend/filing.ts:1105-1111` (`setApplyApplied`) :
```ts
function setApplyApplied(btn: HTMLButtonElement, batchId: string): void {
  btn.disabled = false;
  btn.style.color = "var(--color-text-success)";
  btn.innerHTML =
    '<i class="ti ti-circle-check sift-icon-inline-md"></i> Appliqué ✓ — <span class="sift-underline">Annuler</span>';
  btn.onclick = () => void doUndoApply(btn, batchId);
}
```
- `frontend/filing.ts:1274-1301` (`showFiledConfirm`) : bannière séparée, DOM distinct, avec ✕ de
  fermeture ET bouton "Annuler" séparé.

**Description** : deux confirmations "action faite + annulable" pour deux actions très proches
(Appliquer les tags vs Ranger) sont implémentées avec deux patterns visuels totalement différents —
l'un transforme le bouton lui-même en toggle "Appliqué ✓ / Annuler" (état persistant tant qu'on ne
clique pas ailleurs), l'autre crée une bannière DOM à part avec ✕ + bouton Annuler séparés. Aucun
des deux ne correspond au `toast()` générique (l.1173-1199) utilisé pour Re-source/Jeter (`doSecondary`,
l.1335, 1338), qui lui est un troisième pattern (toast flottant bas-droite, disparaît après 6s).

**Impact concret** : le DJ voit 3 façons différentes de dire "c'est fait, tu peux annuler" selon
l'action (Apply tags = bouton qui change de libellé, File = bannière persistante dans le rail,
Discard/Trash = toast éphémère 6s) — aucune règle perceptible pour deviner laquelle s'applique où,
ce qui viole Nielsen #4 (Consistency and Standards).

**Cause probable** : ces trois mécanismes ont été ajoutés à des moments différents (les noms de
fonctions et les commit récents — `progress-zone`, `journal`, `player` — suggèrent un
développement incrémental sans consolidation de pattern).

**Correction proposée** : converger vers UN système de confirmation réversible (a minima même
timing de disparition, même position, même vocabulaire "Annuler" vs "Undo" vs "↩").

**Effort** : M (pas de refonte de logique, juste harmonisation du markup/CSS des 3 mécanismes).
**Bénéfice** : cohérence perçue → confiance accrue dans "est-ce que mon clic a marché".

---

### H3 — Écran Bibliothèque (`library-detail.ts`) réinvente son propre style de bouton icon+texte au
lieu de réutiliser les classes `sift-*` de filing.ts/report-view.ts

**Fichier** : `frontend/library-detail.ts:100-101`

```ts
`<button data-lib="save" style="flex:1;background:var(--color-background-info);color:var(--color-text-info);border:none;font-weight:500"><i class="ti ti-device-floppy" style="font-size:var(--text-md);vertical-align:-2px"></i> Save</button>` +
`<button data-lib="trash" style="color:var(--color-text-danger)" title="Send to trash"><i class="ti ti-trash" style="font-size:var(--text-md);vertical-align:-2px"></i> Delete</button>` +
```

**Description** : tout le module `library-detail.ts` (337 lignes) construit ses boutons/inputs avec
des styles inline ad hoc (`inputCss`, l.26-27 ; styles répétés `font-size:var(--text-md);vertical-
align:-2px` à quasiment chaque bouton) au lieu des classes déjà établies côté Revue (`.sift-ranger-
btn`, `.sift-secondary-trash`, `.sift-id-btn`). Le bouton "Delete" ici n'est pas un ghost rouge
comme l'exige `.interface-design/system.md` ("Action négative (Discard)" — l.121-122 : "Jamais un
aplat rouge plein (action récupérable)") — il n'a même pas de fond défini du tout (juste
`color:var(--color-text-danger)`), ce qui le rend visuellement plus proche d'un lien texte que d'un
bouton d'action destructive. De plus, l'UI Bibliothèque est **en anglais** ("Save", "Delete",
"Identify") alors que Revue/Écartés/Journal sont **en français** ("Ranger", "Jeter", "Identifier")
— incohérence de langue à l'intérieur de la même app, dans le même parcours de travail (identifier
→ ranger → retrouver en bibliothèque).

**Impact concret** : le DJ passe de l'écran Revue (français) à l'écran Bibliothèque (anglais) pour
éditer le même genre de métadonnées — rupture de continuité linguistique et visuelle qui donne
l'impression de deux applications différentes collées ensemble.

**Cause probable** : `library-detail.ts` documenté comme M6b (feature plus récente, livrée
séparément) — n'a pas reçu la même passe de polish français + tokens que Revue.

**Correction proposée** : traduire Bibliothèque en français (cohérent avec le reste), remplacer les
styles inline par les classes `.sift-*` déjà établies.

**Effort** : M (traduction de chaînes + remplacement de styles inline, pas de nouvelle logique).
**Bénéfice** : cohérence de langue et de style dans un même produit — actuellement l'un des signaux
les plus visibles de "pas fini" pour un auditeur externe.

---

### H4 — `renderBatchRail` et le bouton d'action adaptatif encodent la logique métier dans le texte du
bouton, sans état intermédiaire visible pendant le calcul de sélection

**Fichier** : `frontend/sift-live.ts:582-597` (`actionButtonHtml`)

```ts
function actionButtonHtml(running: boolean): string {
  if (running) {
    return '<button data-sift="batchstop" class="sift-baction" ...>Stop</button>';
  }
  const fileN = batchSel.size;
  const fakeN = batchFakeSel.size;
  if (fileN === 0 && fakeN === 0)
    return '<button class="sift-baction" disabled ...>Filer (0)</button>';
```

**Description** : c'est en réalité un BON pattern (bouton désactivé visuellement avec `opacity:.5`
quand la sélection est vide, l.591) — noté ici pour objectivité, pas comme un problème. Mais il
illustre le contraste avec le rail détail : en mode batch, le bouton EST bien désactivé à sélection
vide ; en mode détail (`doRanger`), rien n'empêche de cliquer "Ranger" avec un `state.canonical`
nul autre qu'un retour silencieux (`if (!state.track || !state.canonical || acting) return;`,
l.1218) — aucun état visuel "disabled" n'accompagne ce cas, contrairement au batch. Le bouton
Ranger (l.892) n'a jamais d'attribut `disabled` posé structurellement (seulement pendant `acting`) —
tant que `state.canonical` peut être `null` transitoirement (ex. pendant le chargement initial d'un
morceau, `openFilingInto` étant asynchrone, l.1400-1462) le bouton Ranger reste visuellement actif
et cliquable pour un no-op silencieux.

**Impact concret** : un DJ qui double-clique vite sur une ligne de la queue peut cliquer "Ranger"
dans la fenêtre où `state.canonical` est encore `null` (reconcile pas encore résolu) — le clic ne
fait rien, sans aucune indication visuelle de pourquoi.

**Cause probable** : le rail détail n'a pas de re-rendu du bouton corrélé à l'état de chargement
(contrairement à batch qui recalcule `actionButtonHtml` à chaque changement de sélection).

**Correction proposée** : griser Ranger tant que `state.canonical` est `null` (le temps du premier
chargement), symétrique au comportement batch.

**Effort** : S.
**Bénéfice** : élimine un clic mort silencieux, cohérent avec le pattern déjà en place côté batch.

---

## MEDIUM

### M1 — `progress-zone.ts` : progression réelle basée sur données, MAIS bloc "Rangement en arrière-plan"
(`fileNote`) est un texte statique sans pourcentage tant que le premier événement `file:progress`
n'est pas arrivé

**Fichier** : `frontend/sift-live.ts:705-707` (dans `runBatchFile`)

```ts
fileNote(
  '<i class="ti ti-loader sift-spin" style="font-size:var(--text-md);vertical-align:-1px"></i> Rangement en arrière-plan…',
);
```

**Description** — Nuance positive d'abord : `progress-zone.ts` lui-même (l.113-127, `rowInner`) est
un BON exemple de progression basée sur données réelles (`done`/`total`, barre `<div class="sift-
pz-fill" style="width:${pct}%">`). Le problème est localisé : entre le clic sur "Filer (n)" et la
première mise à jour `file:progress`, le SEUL signal est ce spinner générique + "Rangement en
arrière-plan…" — aucun pourcentage, aucun compte "0/12", pendant potentiellement plusieurs secondes
sur un gros batch (le premier fichier peut prendre du temps à encoder en FFmpeg avant que le
premier tick de progression n'arrive).

**Impact concret** : sur un batch de N fichiers avec un premier fichier volumineux, le DJ voit un
spinner "en arrière-plan" sans confirmation que le travail a bien démarré sur le bon lot, pendant
une fenêtre non négligeable.

**Cause probable** : le composant `progress-zone` (`setTask`) n'est monté/alimenté qu'à la
réception du premier événement backend, pas au moment du clic.

**Correction proposée** : appeler `setTask("file", {done:0, total:ids.length, state:"running"})`
immédiatement après le lancement (`runBatchFile`), avant même la réponse de `fileBatch`, pour que
la barre 0/N apparaisse tout de suite au lieu du texte générique.

**Effort** : S.
**Bénéfice** : feedback "system status" immédiat (Nielsen #1) dès le clic, pas seulement au premier
tick backend.

---

### M2 — Hiérarchie visuelle du rail filing (`renderFoot`) : Destination, Format, hints clavier et
actions sont posés au même niveau de poids visuel (tous dans le même conteneur flex, sans
distinction taille/poids/couleur claire entre primaire et secondaire)

**Fichier** : `frontend/filing.ts:884-892`

```ts
foot.innerHTML =
  `<button data-fil="destbtn" class="sift-dest-btn">...</button>` +
  `<div class="sift-rail-fmt-group"><span class="col-h">Format</span><div class="sift-fmt-chips">${chips}</div></div>` +
  `<div class="sift-rail-spacer"></div>` +
  keyboardHintsHtml() +
  secondary +
  `<button data-fil="ranger" class="sift-ranger-btn">...</button>`;
```

**Description** : le CTA primaire (`Ranger`) et l'action secondaire destructive (`Jeter`/`Re-source`)
sont deux `<button>` frères sans wrapper qui les distingue, séparés seulement par
`keyboardHintsHtml()` intercalé ENTRE eux dans le DOM — l'ordre visuel dans le flexbox dépend du CSS
(non lu ici en détail pour cette ligne précise), mais la structure HTML place les hints clavier
comme un élément de poids équivalent entre deux actions, ce qui, par la règle refactoring-ui
("les labels/méta sont secondaires, ne doivent pas rivaliser avec l'action"), est un signal faible
de hiérarchie plate. `.interface-design/system.md` documente pourtant explicitement une hiérarchie
voulue (CTA bleu unique, ghost rouge pour discard) — le respect des TOKENS de couleur est réel
(vérifié par grep : `sift-ranger-btn` est distinct de `sift-secondary-trash`), donc ceci est un
problème d'ORDRE DOM/groupement plus que de tokens.

**Impact concret** : mineur en usage (les classes CSS font sans doute le travail visuel), mais
un futur changement de layout (ex. rail plus étroit, responsive) risque de casser l'ordre logique
attendu (Format → Destination → Action) car rien dans le HTML n'exprime ce groupement autrement
que l'ordre linéaire.

**Cause probable** : ajouts successifs (hints clavier déplacés ici récemment selon le commentaire
l.888-890 : "moved here from report-view.ts").

**Correction proposée** : envelopper primaire/secondaire dans un groupe `.sift-rail-actions`
distinct des hints, pour que la hiérarchie soit lisible dans le DOM et pas seulement dans le CSS.

**Effort** : S.
**Bénéfice** : robustesse du layout à un futur refactor CSS, lisibilité du code.

---

### M3 — `ecartes-view.ts` construit ses lignes entièrement en styles inline, dupliquant des valeurs
d'espacement hors grille 4/8/12/16/24/32 documentée

**Fichier** : `frontend/ecartes-view.ts:73-83`

```ts
`<div style="padding:7px 4px;border-bottom:0.5px solid var(--color-border-tertiary)">` +
`<div style="display:flex;align-items:center;gap:7px">` +
```

**Description** : `.interface-design/system.md` (l.79) impose une échelle UNIQUE `xs 4 / sm 8 / md
12 / lg 16 / xl 24 / xxl 32`, "toute autre valeur interdite". `ecartes-view.ts` utilise `7px`,
`4px` (OK, xs), `7px` (hors grille — ni 4 ni 8), à plusieurs endroits (l.73, 78, 80, 88, 89, 103,
etc. — `gap:7px` apparaît au moins 3 fois). Ce n'est pas une section marquée périmée (contrairement
à "Mode Batch") donc la règle d'espacement s'applique bien ici.

**Impact concret** : micro-incohérence de densité entre Écartés et Revue (qui elle respecte
davantage la grille via ses classes `.sift-*` dédiées) — invisible isolément, mais visible en
changeant rapidement d'écran (rythme vertical légèrement différent).

**Cause probable** : `ecartes-view.ts` a été extrait tôt (commentaire l.1 : "Extracted from
sift-live.ts (audit P-3)") avant que la discipline de tokens ne soit formalisée pour ce module —
n'a jamais reçu de passe de conformité comme Revue.

**Correction proposée** : remplacer `7px` par `8px` (le `sm` le plus proche) partout dans ce fichier.

**Effort** : S (recherche/remplacement ciblé, un seul fichier).
**Bénéfice** : conformité tokens, cohérence de densité inter-écrans.

---

### M4 — Chip de raison "à re-sourcer" en `ecartes-view.ts` réutilise le token danger pour un état qui
n'est PAS une erreur mais une action à faire

**Fichier** : `frontend/ecartes-view.ts:23`

```ts
return '<span class="sift-vchip" style="background:var(--color-background-danger);color:var(--color-text-danger);flex:none"><i class="ti ti-alert-circle" style="font-size:var(--text-2xs)"></i> à re-sourcer</span>';
```

**Description** : les 3 raisons possibles (`tronqué`, `faux`, générique "à re-sourcer") utilisent
toutes trois soit `warning` soit `danger` — mais "à re-sourcer" n'est même pas une DÉTECTION
d'anomalie dans ce cas générique (fallback, l.19-24 : c'est le cas par défaut quand ni `truncated`
ni `verdict==="fake"` n'est vrai), pourtant elle porte la même couleur rouge "danger" que "faux"
(fraude confirmée). Selon `refactoring-ui`, la couleur sémantique doit correspondre à la sévérité
réelle — ici deux sévérités différentes (faux-lossless confirmé vs simplement écarté pour une autre
raison) sont visuellement identiques (rouge).

**Impact concret** : le DJ voit tout en rouge dans Écartés sans pouvoir distinguer d'un coup d'œil
"ce fichier est un vrai problème de qualité (transcodage)" de "ce fichier a juste été écarté pour
une raison neutre" — dilue la signification de la couleur danger ailleurs dans l'app (carte verdict
ACTUAL en Revue, où rouge = vraiment fake).

**Cause probable** : un seul style copié-collé pour les 3 branches (`sift-vchip` avec juste l'icône
qui change) sans dériver la couleur du contexte réel.

**Correction proposée** : "à re-sourcer" générique → ton neutre/tertiaire ; garder danger
uniquement pour `verdict==="fake"`.

**Effort** : S.
**Bénéfice** : la couleur redevient un signal fiable de sévérité, cohérent avec le reste de l'app.

---

## LOW

### L1 — `library-detail.ts` : le champ "Delete" n'a pas de confirmation ni d'annulation visible dans
l'UI immédiate (contrairement à `doSecondary`/`doTrash` de filing.ts qui montrent un toast avec
Annuler)

**Fichier** : `frontend/library-detail.ts:292-301` (`doTrash`)

```ts
async function doTrash(st: EditState): Promise<void> {
  try {
    await trashTrack(st.track.id);
    toast("Sent to trash");
    deletedCb?.();
```

**Description** : `toast()` dans `library-detail.ts` (l.38-47) n'a **aucune option d'action Undo**
(contrairement au `toast()` de `filing.ts`, l.1170-1199, qui accepte un paramètre `undo`/`onUndo`).
La suppression d'un morceau de la bibliothèque filée n'offre donc, à l'écran, aucun chemin de
retour direct — l'utilisateur doit deviner qu'il faut aller au Journal pour un revert (ce qui EST
possible côté backend via `revertBatch`, mais pas exposé ici).

**Impact concret** : mineur (le Journal permet un revert), mais rupture du principe "l'annulation
doit être visible au moment de l'action" (Nielsen #3, User Control and Freedom) déjà respecté
ailleurs dans l'app (Revue, Journal) mais pas ici.

**Cause probable** : `library-detail.ts`'s toast est une implémentation locale minimaliste, pas
partagée avec celle de `filing.ts`.

**Correction proposée** : donner à `doTrash` un toast avec bouton Annuler (calqué sur le pattern
`toast(message, true, onUndo)` de filing.ts) ou au minimum un lien direct vers le Journal.

**Effort** : S.
**Bénéfice** : cohérence de sécurité perçue pour une action destructive.

---

### L2 — Le nav rail (`index.html`) porte `aria-hidden="true"` sur les icônes ET un texte visible pour
chaque item — c'est le pattern CORRECT, noté positivement, aucune correction nécessaire

**Fichier** : `index.html:14-23`

Vérifié pour objectivité (item de la checklist a11y "icon-only buttons") : le nav principal n'est
PAS icon-only — chaque `.nv` a un `<span>` texte visible en plus de l'icône `aria-hidden`. Bon
pattern, à répliquer pour les boutons icon-only relevés en C2.

---

## HYPOTHÈSES NON VÉRIFIÉES

- **Navigation clavier au-delà de Report** : `installFilingKeys` (filing.ts:1531-1565) couvre
  Espace/↑↓/Entrée/⌫/I sur l'écran Revue uniquement. Je n'ai pas trouvé d'équivalent pour
  Bibliothèque, Écartés ou Journal (aucun `addEventListener("keydown"...)` trouvé dans
  `library-detail.ts`, `ecartes-view.ts`, `journal.ts` par lecture complète) — mais je n'ai pas
  vérifié si `sift-live.ts` (1412 lignes, lu seulement par sections ciblées) enregistre un handler
  clavier global qui couvrirait ces écrans par délégation. À vérifier avant d'affirmer une absence
  totale de nav clavier hors Revue.
- **Contraste texte réel (WCAG AA)** : je n'ai pas mesuré les valeurs de contraste calculées des
  tokens `--color-text-tertiary`/`--color-text-2xs` sur leurs fonds respectifs (nécessiterait de
  lire les valeurs hex réelles dans `styles.css`, non fait dans cette passe) — la section a11y de
  `.interface-design/system.md` mentionne "remonté a11y" pour `eyebrow`, suggérant qu'un travail a
  déjà eu lieu, mais je n'ai pas de preuve chiffrée de conformité AA sur les tokens actuels.
- **`.sift-fld-icon`/tree focus visible au clavier** : le rail de destination (`binNodeHtml`,
  filing.ts:255-280) est un ensemble de `<div data-fil="bin">` cliquables (pas des `<button>`) —
  probable absence de focus clavier nativement (divs non focusables sans `tabindex`), mais je n'ai
  pas vérifié `styles.css` pour un éventuel `tabindex`/`:focus-visible` géré autrement.

---

## Résumé pour reporting

- **Critical** : 2
- **High** : 4
- **Medium** : 4
- **Low** : 2
- **Hypothèses non vérifiées** : 3
