# P-4 — Recensement des accès DOM (Phase A, lecture seule)

But : transformer les casses **silencieuses** (un `querySelector`/`getElementById` qui
renvoie `null` et que le code avale via `if(!x) return` ou `?.`) en erreurs **visibles**
là où c'est justifié. Cette passe ne fait que **recenser et trier**. Aucun fix.

## Méthode de tri (par preuve, pas par intuition)

Chaque accès est classé :

- **OBLIGATOIRE** — l'élément ne peut PAS légitimement manquer quand ce code tourne ;
  son absence est forcément un bug. Deux sous-types (même classe, valeur de fix différente) :
  - `(shell)` = l'élément appartient à **un autre fichier** (le shell mockup `app.js` :
    `#content`, `#mid`, `#filfoot`, `#fldz`, `#ql`, `#qcol`, `#pa`, la nav `[data-view]`,
    `.nav-badge`, `.home-left`) **ou à un autre module** (hooks de `report-view` :
    `.sift-report-name/-sub/-cover`, `.sift-vchips`). **C'est le vrai contrat implicite** :
    si `app.js` renomme un id, le live no-op en silence. → cible prioritaire Phase B.
  - `(tmpl)` = l'élément fait partie du `innerHTML` que **la fonction elle-même vient
    d'écrire** juste avant, de façon **inconditionnelle**. Absence ⇒ bug de template/refactor
    dans la même fonction. Risque réel faible, assertion peu coûteuse.
- **OPTIONNEL** — l'absence est un cas normal géré exprès. Indices : `?.`/`if(x)` sur un
  élément **conditionnel** (rendu seulement parfois), **sonde de présence** volontaire
  (`if(getElementById(x)) return;`, `?.remove()` idempotent), **détection de vue**
  (l'élément n'existe que sur tel écran et le code tourne sur tous), ou **délégation
  d'événement** (`e.target.closest(...)` = null quand le clic est ailleurs).
- **INDÉTERMINÉ** — je ne peux pas trancher (timing async cross-module) → à décider ensemble.
- **(liste)** — `querySelectorAll` : ne renvoie jamais `null` (NodeList vide). Hors sujet
  pour une garde d'existence ; listé pour exhaustivité, **non compté** dans les 3 classes.

Périmètre : les 7 fichiers `.ts` du live layer. `app.js` (le mockup, qui tourne aussi dans
la démo navigateur) est traité **à part** en fin de document — c'est un point de décision.

---

## Compte

| Classe | Live TS |
|---|---|
| **OBLIGATOIRE** | **81** (dont **26 `shell`** = cross-fichier/cross-module, **55 `tmpl`** = template propre) |
| **OPTIONNEL** | 35 |
| **INDÉTERMINÉ** | 2 |
| *(liste `querySelectorAll`)* | *13 (N/A)* |
| **Total accès TS** | 131 (+1 match en commentaire) |
| *app.js (mockup, décision à part)* | *14* |

Lecture courte : **26 accès `OBLIGATOIRE (shell)`** sont le cœur de P-4 (contrat implicite
entre le live et `app.js`/les autres modules). Les **55 `OBLIGATOIRE (tmpl)`** sont
internes à leur fonction ; les garder ou non (assertion sur le conteneur plutôt que sur
chaque feuille) est un choix de granularité à valider.

---

## sift-live.ts

| Accès | ligne | rôle | classe | preuve |
|---|---|---|---|---|
| `#ql` | 87 | liste de file (Revue) | OPTIONNEL | détection de vue : `renderQueue` tourne via `refresh()` sur **tout** écran ; `#ql` n'existe que sur Revue → `if(!ql) return` légitime |
| `#fldz` | 132 | colonne destinations | OBLIGATOIRE (shell) | atteint après la garde `#ql` (donc sur Revue) ; sibling du shell Revue |
| `#mid` | 141 | pane détail | OBLIGATOIRE (shell) | idem, dans `if(touchDetail)` après garde `#ql` |
| `.qi.cur` | 145 | lignes actives | (liste) | `querySelectorAll().forEach` |
| `.qi[data-id=…]` | 147 | ligne courante | OPTIONNEL | `?.` ; la ligne ciblée peut ne plus être dans le DOM |
| `#qcol` | 158 | colonne file | OBLIGATOIRE (shell) | `ensureReviewSeg` appelé par `renderQueue` après garde `#ql` ; `#ql` est dans `#qcol` |
| `#sift-revseg` | 160 | segmented Detail/Batch | OPTIONNEL | sonde idempotente : `if(!seg){create}` |
| `#mid` | 184 | pane batch | OBLIGATOIRE (shell) | `renderBatch` atteignable seulement sur Revue (via `renderQueue` post-garde) |
| `#filfoot` | 337 | rail droit | OBLIGATOIRE (shell) | `renderBatchRail` (batch/Revue) ; rail Revue |
| `#fldz` | 338 | arbre dossiers | OBLIGATOIRE (shell) | idem |
| `#fldz` | 356 | arbre dossiers | OBLIGATOIRE (shell) | `setReviewMode` déclenché sur Revue |
| `#filfoot` | 382 | rail droit | OBLIGATOIRE (shell) | `runBatchFile` (batch/Revue) |
| `#filfoot` | 402 | rail droit | OBLIGATOIRE (shell) | `runBatchIdentify` (batch/Revue) |
| `[data-batch-note]` | 410, 414, 423 | note transitoire | OPTIONNEL | `foot?.querySelector(...)?.remove()` idempotent |
| `.nav-badge[data-badge=revue]` | 458 | badge nav | OBLIGATOIRE (shell) | nav persistante sur tous les écrans ; badge statique (`.nav-badge:empty` le collapse) |
| `#content` | 470 | conteneur Réglages | OBLIGATOIRE (shell) | `renderReglagesLive` ne tourne que pour la vue Réglages |
| `#sift-reglages-live` | 474 | bloc live précédent | OPTIONNEL | `?.remove()` idempotent (anti-doublon) |
| `#sift-discogs-token` | 505 | input token | OBLIGATOIRE (tmpl) | dans `block.innerHTML` écrit juste avant (inconditionnel) |
| `#sift-discogs-status` | 506 | statut | OBLIGATOIRE (tmpl) | idem |
| `#sift-discogs-link` | 507 | lien get-token | OBLIGATOIRE (tmpl) | idem |
| `#content` | 557 | conteneur Biblio | OBLIGATOIRE (shell) | `renderBiblioLive` ne tourne que pour la vue Bibliothèque |
| `#bibq` | 615 | champ recherche | OBLIGATOIRE (tmpl) | dans `content.innerHTML` écrit juste avant (inconditionnel) |
| `#bibplayer` | 632 | hôte détail track | OBLIGATOIRE (tmpl) | écrit dans `content.innerHTML` (L613) inconditionnel ; `openBiblioDetail` sur Biblio |
| `.lr.cur` | 634 | lignes actives | (liste) | `querySelectorAll().forEach` |
| `.lr[data-id=…]` | 635 | ligne cliquée | OPTIONNEL | `?.classList` ; ligne peut être absente |
| `.lr[data-id] .bib-name` | 643 | label ligne | OPTIONNEL | callback post-async (`update_metadata`) ; liste a pu re-render → `if(span)` |
| `#pa` | 663 | racine app (délégation) | OBLIGATOIRE (shell) | `#pa` = conteneur racine d'`app.js` (toujours présent) ; bind une fois au boot |
| `#mid` | 672 | pane détail | OBLIGATOIRE (shell) | dans le handler clic `.qi` → on est sur Revue |
| `.qi.cur` | 674 | lignes actives | (liste) | `querySelectorAll().forEach` |
| `#mid` | 773 | pane détail | OBLIGATOIRE (shell) | handler `batchopen` → Revue |
| `#pa` | 788 | racine app (change) | OBLIGATOIRE (shell) | idem L663 |

## report-view.ts

| Accès | ligne | rôle | classe | preuve |
|---|---|---|---|---|
| `#sift-report-style` | 32 | style one-time | OPTIONNEL | sonde idempotente `if(...) return` |
| `.sift-wave` | 362 | conteneur waveform | OBLIGATOIRE (tmpl) | `playerRowHtml` (inconditionnel) rendu dans `root` avant `mountPlayer` ; `if(!container) return` |
| `.sift-play` | 363 | bouton play | OBLIGATOIRE (tmpl) | idem template player |
| `.sift-time` | 364 | affichage temps | OBLIGATOIRE (tmpl) | idem |
| `.sift-tempo` | 365 | fader tempo | OBLIGATOIRE (tmpl) | idem |
| `.sift-tempo-out` | 366 | valeur tempo | OBLIGATOIRE (tmpl) | idem |
| `playBtn > i` | 385 | icône play | OBLIGATOIRE (tmpl) | `<i>` toujours dans le bouton play |
| `.sift-key` | 388 | bouton key-lock | OBLIGATOIRE (tmpl) | template player inconditionnel |
| `.sift-time-val` | 403 | texte temps | OBLIGATOIRE (tmpl) | idem |
| `.sift-sg` | 450 | canvas spectro | OBLIGATOIRE (tmpl) | `spectroAndTagsHtml` (inconditionnel) ; `if(!sg||!toggle||…) return` |
| `.sift-sg-toggle` | 451 | toggle proof | OBLIGATOIRE (tmpl) | idem |
| `.sift-sg-body` | 452 | corps spectro | OBLIGATOIRE (tmpl) | idem |
| `.sift-sg-caret` | 453 | caret | OBLIGATOIRE (tmpl) | idem |
| `.sift-sg-hint` | 454 | hint show/hide | OBLIGATOIRE (tmpl) | idem |
| `.sift-verdict-stub` | 568, 586, 597 | stub verdict | OBLIGATOIRE (tmpl) | shell écrit en L547 (inconditionnel) ; `if(verdictEl)` |
| `.sift-analysis-body` | 569, 587 | corps analyse | OBLIGATOIRE (tmpl) | shell écrit en L549 (inconditionnel) ; `if(bodyEl)` |
| `#sift-report-overlay` | 611 | overlay modal | OPTIONNEL | `?.remove()` idempotent |
| `.sift-close` | 634 | bouton fermer | OBLIGATOIRE (tmpl) | `openReportModal` appelle `reportHtml(r, true)` → bouton émis ; `?.` |

## ecartes-view.ts

| Accès | ligne | rôle | classe | preuve |
|---|---|---|---|---|
| `#content` | 50 | conteneur Écartés | OBLIGATOIRE (shell) | `renderEcartes` ne tourne que pour la vue Écartés ; `if(!content) return` silencieux |

## home-sources.ts

| Accès | ligne | rôle | classe | preuve |
|---|---|---|---|---|
| `#content` | 16 | conteneur Accueil | OBLIGATOIRE (shell) | `renderHomeSources` ne tourne que pour Accueil ; `if(!content) return` |
| `#sift-sources` | 26 | bloc précédent | OPTIONNEL | `?.remove()` idempotent (anti-doublon) |
| `.home-left` | 59 | colonne gauche Accueil | OBLIGATOIRE (shell) | markup `app.js` (Accueil) ; **contrat implicite** : un renommage casse en silence ; `if(!left) return` |

## chrome.ts

| Accès | ligne | rôle | classe | preuve |
|---|---|---|---|---|
| `#sift-dz-style` | 12 | style drop one-time | OPTIONNEL | sonde idempotente |
| `.sift-dz-on` | 34 | zones actives | (liste) | `querySelectorAll().forEach` |
| `sel` (DROP_ZONES) | 39 | test de présence | OPTIONNEL | sonde de présence volontaire (`.filter(... document.querySelector(sel))`) |
| `sel` (DROP_ZONES) | 44 | zone présente | OPTIONNEL | `if(el)` ; cibles varient selon l'écran |
| `elementFromPoint` | 56 | élément sous le curseur | OPTIONNEL | peut être `null` (drop hors fenêtre) ; géré `el && el.closest` |
| `el.closest('.dest')` | 57 | colonne destination | OPTIONNEL | délégation/position ; `null` = drop côté source |
| `#sift-lean-style` | 87 | style lean one-time | OPTIONNEL | sonde idempotente |
| `#sift-titlebar` | 115 | titlebar one-time | OPTIONNEL | sonde idempotente |
| `.sift-win` | 129 | boutons fenêtre | (liste) | `querySelectorAll().forEach` sur la barre fraîchement créée |

*(L53 : « elementFromPoint » apparaît dans un commentaire — pas un accès.)*
**chrome.ts n'a aucun accès OBLIGATOIRE** : il crée ses propres éléments, avec des sondes
idempotentes. Rien à durcir ici.

## library-detail.ts

| Accès | ligne | rôle | classe | preuve |
|---|---|---|---|---|
| `#sift-toast` | 38 | toast précédent | OPTIONNEL | `?.remove()` idempotent |
| `[data-lib=<champ>]` | 109 | inputs éditeur | OBLIGATOIRE (tmpl) | champs inconditionnels de `renderEdit` ; `?.value ?? ""` |
| `[data-lib=cover]` | 130 | bouton cover | OBLIGATOIRE (tmpl) | template `renderEdit` inconditionnel |
| `[data-lib=release]` | 131 | bouton release | OPTIONNEL | **conditionnel** : émis seulement si `discogs_release_id` (`releaseRowHtml`) |
| `[data-lib=save]` | 135 | bouton save | OBLIGATOIRE (tmpl) | inconditionnel |
| `[data-lib=trash]` | 136 | bouton delete | OBLIGATOIRE (tmpl) | inconditionnel |
| `[data-lib=identifier]` | 138 | bouton identify | OBLIGATOIRE (tmpl) | `releaseRowHtml` émet « identifier » dans **les deux** branches |
| `.sift-cands` | 139 | hôte candidats | OBLIGATOIRE (tmpl) | inconditionnel (L97) |
| `[data-lib=goto-reglages]` | 179 | lien Réglages | OBLIGATOIRE (tmpl) | écrit dans la branche d'erreur NO_TOKEN juste avant ; `?.` |
| `[data-view=reglages]` | 181 | onglet nav Réglages | OBLIGATOIRE (shell) | nav `app.js` persistante ; navigation best-effort par dispatch |
| `[data-cand]` | 203 | lignes candidats | (liste) | `querySelectorAll().forEach` |
| `[data-lib=save]` | 256 | bouton save | OBLIGATOIRE (tmpl) | inconditionnel ; `btn?` |
| `.lib-report` | 327 | hôte rapport | OBLIGATOIRE (tmpl) | `host.innerHTML` écrit juste avant (L322) ; `if(!reportEl||!editEl) return` |
| `.lib-edit` | 328 | hôte éditeur | OBLIGATOIRE (tmpl) | idem |

## filing.ts

| Accès | ligne | rôle | classe | preuve |
|---|---|---|---|---|
| `[data-fil=pickroot]` | 202 | bouton choisir-root | OBLIGATOIRE (tmpl) | branche root-non-défini de `renderBins` (écrite juste avant) ; `?.` |
| `[data-fil=binfilter]` | 251, 257 | filtre dossiers | OPTIONNEL | **conditionnel** : `filterRow` émis seulement si `state.bins.length` |
| `[data-fil=caret]` | 262 | carets arbre | (liste) | `querySelectorAll().forEach` |
| `[data-fil=bin]` | 271 | nœuds dossiers | (liste) | `querySelectorAll().forEach` |
| `[data-fil=newbin]` | 278 | bouton « new » | OPTIONNEL | **conditionnel** : caché en filtrage / création |
| `[data-fil=newin]` | 282 | input nouveau dossier | OPTIONNEL | **conditionnel** : présent seulement si `state.creating` |
| `.sift-report-name` | 337 | titre rapport | OBLIGATOIRE (shell) | hook **cross-module** (`report-view` hero) ; rapport rendu avant `updateHeaderName` |
| `.sift-report-sub` | 339 | sous-titre rapport | OBLIGATOIRE (shell) | hook cross-module (`heroHtml`, L162) |
| `[data-fil=ranger] .sift-fil-bin` | 348 | label bin du CTA | OPTIONNEL | `renderFoot` met `foot.innerHTML=""` si pas de track ouvert → `.sift-fil-bin` absent légitimement ; `if(btn)` |
| `[data-fil=artist/title/version]` | 378, 379, 380 | inputs éditeur | OBLIGATOIRE (tmpl) | `renderFoot` rendu (track ouvert ⇒ identité applicable) |
| `.sift-fil-prev` | 386, 590 | aperçu nom | OBLIGATOIRE (tmpl) | template `renderFoot` inconditionnel |
| `.sift-vchips` | 392 | rangée chips verdict | INDÉTERMINÉ | hook **cross-module** + timing : le panneau verdict est rempli en async (seq-guard) ; peut manquer si on identifie avant la fin de l'analyse |
| `[data-chip=match]` | 394 | chip MATCH précédent | OPTIONNEL | `?.remove()` idempotent |
| `.sift-report-cover` | 404 | cover rapport | OBLIGATOIRE (shell) | hook cross-module (`heroHtml`) |
| `.sift-genres` | 413 | chips genres | OBLIGATOIRE (tmpl) | template `renderFoot` (L576) |
| `[data-fil=cand-changer]` | 435 | bouton « change » | OBLIGATOIRE (tmpl) | écrit dans `host.innerHTML` juste avant (L426) |
| `[data-cand]` | 456 | lignes candidats | (liste) | `querySelectorAll().forEach` |
| `[data-fil=goto-reglages]` | 504 | lien Réglages | OBLIGATOIRE (tmpl) | branche NO_TOKEN écrite juste avant ; `?.` |
| `[data-view=reglages]` | 507 | onglet nav Réglages | OBLIGATOIRE (shell) | nav `app.js` persistante ; dispatch best-effort |
| `[data-fil=artist/title/version]` | 583, 584, 585 | inputs (handler `upd`) | OBLIGATOIRE (tmpl) | footer rendu (canonical présent) |
| `[data-fil=artist/title/version]` | 595 | inputs (bind input) | (liste) | `querySelectorAll().forEach` |
| `[data-fil=fmt]` | 598 | chips format | (liste) | `querySelectorAll().forEach` |
| `[data-fil=ranger]` | 606 | bouton File | OBLIGATOIRE (tmpl) | toujours présent dans `renderFoot` ; `?.` |
| `[data-fil=resource]` | 609 | bouton Re-source | OPTIONNEL | **conditionnel** : `secondary` = resource **seulement si** `verdict==='fake'` |
| `[data-fil=trash]` | 612 | bouton Discard | OPTIONNEL | **conditionnel** : trash seulement si non-fake |
| `[data-fil=identifier]` | 615 | bouton Identify | OBLIGATOIRE (tmpl) | inconditionnel |
| `.sift-cands` | 616 | hôte candidats | OBLIGATOIRE (tmpl) | inconditionnel (L572) |
| `#sift-toast` | 624 | toast précédent | OPTIONNEL | `?.remove()` idempotent |
| `[data-fil=undo]` | 635 | bouton Annuler | OPTIONNEL | **conditionnel** : émis seulement si `undo` vrai |
| `#mid` | 640 | pane détail (callback undo) | OPTIONNEL | détection de vue : undo peut partir hors Revue ; `if(mid)` |
| `[data-fil=ranger/resource/trash]` | 655 | boutons action | (liste) | `querySelectorAll().forEach` |
| `[data-fil=ranger]` | 670 | bouton File (doRanger) | OBLIGATOIRE (tmpl) | footer rendu (track + canonical présents, garde en tête) ; `?.innerHTML` |
| `#filfoot` | 725 | rail droit (clearPane) | OBLIGATOIRE (shell) | rail Revue ; `clearPane` sur Revue ; `if(ff)` |
| `.sift-fil-report` | 759 | hôte rapport | OBLIGATOIRE (tmpl) | `mid.innerHTML` écrit juste avant (L754) ; `if(!reportEl||!footEl) return` |
| `#filfoot` | 762 | rail droit | OBLIGATOIRE (shell) | rail Revue ; `openFilingInto` sur Revue ; garde silencieuse |
| `.sift-fil-dup` | 773 | slot doublon | OBLIGATOIRE (tmpl) | template L756 inconditionnel ; `if(slot)` |
| `.sift-vchips` | 806 | rangée chips verdict | INDÉTERMINÉ | idem L392 : cross-module + timing (callback `dupP.then` après `openReportInto`) |
| `[data-chip=dup]` | 807 | chip dup existant | OPTIONNEL | sonde idempotente (anti-doublon) |
| `#ql .qi` | 831 | lignes file (clavier ↑↓) | (liste) | `querySelectorAll` ; `if(!rows.length) return` |
| `#ql .qi.cur` | 833 | ligne courante | OPTIONNEL | peut être `null` (aucune sélection) → `cur ? … : -1` |
| `[data-fil=ranger]` | 839 | File (touche ⏎) | OBLIGATOIRE (tmpl) | handler actif seulement si `state.track` (L823) ⇒ footer rendu ; `?.click()` |
| `[data-fil=resource/trash]` | 843 | Discard (touche ⌫) | OBLIGATOIRE (tmpl) | une des deux toujours présente ; `?.click()` |
| `[data-fil=identifier]` | 846 | Identify (touche I) | OBLIGATOIRE (tmpl) | inconditionnel ; `?.click()` |
| `.sift-fil` | 880 | « notre pane est-il là ? » | OPTIONNEL | **sonde de présence volontaire** : détecte si `app.js` a réécrit `#mid` |

---

## app.js (mockup) — point de décision, NON compté ci-dessus

`app.js` est le **shell de démo** (JS vanilla, tourne aussi dans la démo navigateur Vercel).
Le live l'« augmente » ; ses propres accès visent des éléments **qu'il vient lui-même de
rendre**. Y ajouter des `throw` (a) durcirait du code mockup et (b) risquerait de casser la
démo plein navigateur. Recommandation : **hors périmètre Phase B** (ou à trancher).

| Accès | ligne | classe (mockup) |
|---|---|---|
| `#content`, `#nav` | 23 | OBLIGATOIRE (tmpl mockup) — bootstrap |
| `nav.querySelectorAll('.nv')` | 34 | (liste) |
| `#pf` | 76 | OBLIGATOIRE (tmpl) |
| `#ql` | 88 | OBLIGATOIRE (tmpl) |
| `#fldz` | 91 | OBLIGATOIRE (tmpl) |
| `#newin` | 92 | OPTIONNEL (conditionnel `creating`) |
| `#qdrag` | 93 | OBLIGATOIRE (tmpl) |
| `#qcol` | 95 | OBLIGATOIRE (tmpl) |
| `#mid` | 101 | OPTIONNEL (`if(!mid)return` — vue) |
| `#spc` | 150 | OPTIONNEL (`if(!c||!c.getContext)`) |
| `#pa` | 277 | OBLIGATOIRE (tmpl) |
| `e.target.closest([data-view])` | 279 | OPTIONNEL (délégation) |
| `e.target.closest([data-act])` | 280 | OPTIONNEL (délégation) |
| `#tout` | 310 | OPTIONNEL (`if(o)`) |

---

## Pour la validation du tri (avant Phase B)

3 questions ouvertes à trancher ensemble :

1. **Granularité des `OBLIGATOIRE (tmpl)` (55).** Faut-il assister chaque feuille de
   template, ou seulement le **conteneur** de chaque rendu (si le conteneur est bon, ses
   feuilles inconditionnelles le sont) ? Recommandation : assertion au conteneur, pas à
   chaque feuille → ramène l'effort réel sur ~une douzaine de points.
2. **Les 2 INDÉTERMINÉ (`.sift-vchips` filing L392 & L806).** Hook cross-module rempli en
   async : OBLIGATOIRE seulement si l'analyse est finie. Faut-il garantir l'ordre (rendre le
   panneau verdict avant d'autoriser l'identify) ou laisser optionnel ? À décider.
3. **app.js.** Dans le périmètre P-4 ou non ? (Mon avis : non — c'est le mockup ; le contrat
   à protéger est le **live → app.js** côté live, pas l'intérieur d'app.js.)

**Cible Phase B recommandée** = les **26 `OBLIGATOIRE (shell)`** (le contrat implicite réel),
+ éventuellement les conteneurs de rendu. Pas les 55 feuilles `tmpl` ni app.js, sauf accord.
