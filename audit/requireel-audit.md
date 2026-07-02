# Recensement requireEl — la classe « renderer qui throw hors de sa vue »

Lecture seule. AUCUNE correction. On cartographie TOUS les `requireEl` du front pour corriger la
CLASSE entière (renderHomeSources `.home-left` puis updateHeaderName `.sift-report-sub` étaient deux
instances d'un même bug), pas un cas isolé.

## Critère de tri (par preuve)

`requireEl(sel, ctx, root?)` **throw** si `sel` est absent ([dom.ts:14](frontend/dom.ts:14)). Donc :

- **Élément TOUJOURS monté** = le shell statique d'`index.html`, HORS `#content` : `#pa`, `#nav`
  (+ `.nav-foot`, `.nav-badge[data-badge=…]`, les `[data-view=…]`), et `#content` lui-même. Un
  `requireEl` dessus ne peut pas throw légitimement → **SÛR**.
- **Élément PROPRE À UNE VUE** = tout ce qu'`app.js`/les renderers écrivent DANS `#content` (Review :
  `#qcol`/`#ql`/`#mid`/`#fldz`/`#filfoot` + le pane détail `.sift-fil-*`/`.sift-report-*` ; Home :
  `.home-left` ; Library : `#bibplayer`/`.lib-*`). Pour ceux-là, trois sous-cas :
  - **SÛR (gardé en amont)** : la fonction n'est atteinte qu'APRÈS une garde de vue (p.ex. tout ce
    qui passe par `renderQueue`, gardé par `if (!ql) return` [sift-live.ts:91-92](frontend/sift-live.ts:91))
    ou par un clic sur un élément qui n'existe que dans la vue → la vue est forcément montée.
  - **SÛR (tmpl)** : l'élément est écrit par la fonction elle-même juste avant, scopé à ce `root` →
    présent par construction (querySelector le retrouve même si `root` est détaché).
  - **FRAGILE** : la fonction est atteignable alors que la vue/le pane n'est PAS (ou plus) monté —
    appel **cross-vue** (refresh/événement), reprise **après un `await`** (navigation pendant
    l'attente), ou **callback async** (identify/file/undo résolu après navigation). → throw → casse
    le cycle. **À auto-garder** (check de montage non-throw + early return / `if (el)`), comme
    `renderQueue`/`renderHomeSources`.

## Tableau complet

| Fichier:ligne | sélecteur | fonction | monté | chemins d'appel | verdict |
|---|---|---|---|---|---|
| ecartes-view.ts:51 | `#content` | renderEcartes | toujours | vue Écartés | **SÛR** |
| filing.ts:343 | `.sift-report-name` | updateHeaderName | par-vue (pane report) | openFilingInto:851 (post-await), onIdentityApplied:394 (async), renderFoot:598 (edit) | **FRAGILE** |
| filing.ts:345 | `.sift-report-sub` | updateHeaderName | par-vue (pane report) | idem (PROUVÉ live) | **FRAGILE** |
| filing.ts:410 | `.sift-report-cover` | onIdentityApplied | par-vue (pane report) | filing.ts:471 (callback identify async) | **FRAGILE** |
| filing.ts:513 | `[data-view=reglages]` | goto-reglages | toujours (nav) | clic best-effort | **SÛR** |
| filing.ts:718 | `#filfoot` | showFiledConfirm | par-vue (rail Review) | doRanger:688 (après `await fileTrack`) | **FRAGILE** |
| filing.ts:776 | `#filfoot` | clearPane | par-vue (rail Review) | doRevert:731 / doSecondary:757 / undo:647 (tous async), syncDetail:952 | **FRAGILE** |
| filing.ts:811 | `.sift-fil-report` | openFilingInto | par-vue, **tmpl** | écrit en 806 juste avant, scopé `mid` | **SÛR (tmpl)** |
| filing.ts:814 | `#filfoot` | openFilingInto | par-vue | capturé à l'ENTRÉE (avant tout `await`, Review montée) | **SÛR (entrée)** |
| library-detail.ts:181 | `[data-view=reglages]` | goto-reglages | toujours (nav) | dispatch best-effort | **SÛR** |
| library-detail.ts:328 | `.lib-report` | openLibraryDetailInto | par-vue, **tmpl** | `host.innerHTML` écrit juste avant (322), scopé `host` | **SÛR (tmpl)** |
| library-detail.ts:329 | `.lib-edit` | openLibraryDetailInto | par-vue, **tmpl** | idem | **SÛR (tmpl)** |
| report-view.ts:363 | `.sift-wave` | mountPlayer | par-vue, **tmpl** | template player écrit dans `root` avant, scopé `root` | **SÛR (tmpl)** |
| sift-live.ts:123 | `#fldz` | renderQueue | par-vue | après garde `#ql` (Review montée) | **SÛR (gardé)** |
| sift-live.ts:132 | `#mid` | renderQueue | par-vue | après garde `#ql`, dans `if(touchDetail)` | **SÛR (gardé)** |
| sift-live.ts:224 | `#qcol` | ensureReviewSeg | par-vue | via renderQueue (post-`#ql`) / setReviewMode (Review) | **SÛR (gardé)** |
| sift-live.ts:249 | `#mid` | renderBatch | par-vue | via renderQueue (post-`#ql`) / setReviewMode | **SÛR (gardé)** |
| sift-live.ts:396 | `#filfoot` | renderBatchRail | par-vue | via renderBatch (Review) | **SÛR (gardé)** |
| sift-live.ts:397 | `#fldz` | renderBatchRail | par-vue | idem | **SÛR (gardé)** |
| sift-live.ts:414 | `#fldz` | setReviewMode | par-vue | toggles Detail/Batch + qi-click (Review) | **SÛR (gardé)** |
| sift-live.ts:526 | `.nav-badge[data-badge=revue]` | updateRevueBadge | toujours (nav) | refresh() sur tout écran | **SÛR** |
| sift-live.ts:537 | `#content` | renderReglagesLive | toujours | vue Réglages | **SÛR** |
| sift-live.ts:623 | `#content` | renderBiblioLive | toujours | vue Library | **SÛR** |
| sift-live.ts:697 | `#bibplayer` | openBiblioDetail | par-vue | clic `.lr` (Library) → entrée synchrone, Library montée | **SÛR (entrée)** |
| sift-live.ts:728 | `#pa` | installLiveWiring | toujours | bind unique au boot | **SÛR** |
| sift-live.ts:737 | `#mid` | qi-click handler | par-vue | clic `.qi` (Review) → entrée synchrone | **SÛR (entrée)** |
| sift-live.ts:838 | `#mid` | batchopen handler | par-vue | clic batch (Review) → entrée synchrone | **SÛR (entrée)** |
| sift-live.ts:850 | `#pa` | installLiveWiring | toujours | bind unique au boot | **SÛR** |
| progress-zone.ts:57 | `.nav-foot` | ensureZone | toujours (nav) | depuis n'importe quelle vue | **SÛR** |
| home-sources.ts | `.home-left` | renderHomeSources | par-vue (Home) | refresh() cross-vue | **CORRIGÉ** (déjà auto-gardé) |

## Les FRAGILES à auto-garder (la classe à corriger en une fois)

Toutes dans `filing.ts`, toutes atteignables hors du pane Review (callback async / post-`await` /
cross-vue). Pour deux d'entre elles, le code a DÉJÀ un `if (el)` mort juste après le `requireEl`
(preuve que l'intention était non-throw) — il suffit de remplacer `requireEl` par le `querySelector`
correspondant.

| # | fichier:ligne | fonction | check de montage non-throw recommandé |
|---|---|---|---|
| 1 | filing.ts:343 + :345 | `updateHeaderName(mid)` | en tête : `const nameEl = mid.querySelector<HTMLElement>(".sift-report-name"); if (!nameEl) return;` puis `subEl` reste en `querySelector` + le `if (subEl)` déjà présent ([:346](frontend/filing.ts:346)). (Le pane report peut avoir disparu après l'`await` d'openFilingInto, ou après navigation pendant un identify.) |
| 2 | filing.ts:410 | `onIdentityApplied(… mid …)` | `const covEl = mid.querySelector<HTMLImageElement>(".sift-report-cover");` (le `if (covEl)` est DÉJÀ là [:411](frontend/filing.ts:411)) — simple bascule requireEl→querySelector. |
| 3 | filing.ts:718 | `showFiledConfirm(mid, …)` | `const foot = document.getElementById("filfoot"); if (foot) foot.innerHTML = "";` (rail Review absent si l'utilisateur a quitté pendant `await fileTrack`). |
| 4 | filing.ts:776 | `clearPane(mid)` | `const ff = document.getElementById("filfoot"); if (ff) ff.innerHTML = "";` (appelée par doRevert/doSecondary/undo, tous async). |

Note : `openFilingInto:814` (`#filfoot`) et `:811` (`.sift-fil-report`) sont **SÛRS** (capturés à
l'entrée avant tout `await`, ou tmpl). La fragilité d'openFilingInto vit dans son code **post-`await`**
(`updateHeaderName` ligne 851) — couverte par le fix #1.

### Pattern de fix unique (Phase B, pas ici)
Aligner ces 4 sur `renderQueue`/`renderHomeSources` : **probe non-throw de la racine de vue/pane +
early-return / `if (el)`**, jamais de `requireEl` sur un élément par-vue atteignable hors de sa vue.
`requireEl` reste légitime pour le shell toujours-monté et pour les éléments tmpl scopés au `root`
qu'on vient d'écrire.

═══ STOP ═══ Recensement seulement. 4 sites FRAGILES (tous filing.ts) à auto-garder pour clore la
classe ; le reste est SÛR (shell toujours-monté, gardé-en-amont, ou tmpl). Aucune correction ici.
NE PAS toucher : le code, le moteur annulation/filing/revert, TRASH_PURGE_DAYS, P-6.
