# Échecs silencieux — design

Date d'ouverture : 2026-08-11. Ouvert sur décision d'Antoine après l'inventaire du premier
lancement ([issue #15](https://github.com/c0remusic/sift/issues/15)), qui a localisé 21 impasses
dont 15 silencieuses.

**Ce chantier ne fait pas partie de la map wayfinder**
([#6](https://github.com/c0remusic/sift/issues/6)). Cette map planifie — chaque ticket y résout
une décision. Ici il n'y a rien à décider : ce sont des défauts localisés, à corriger. Le lien
entre les deux est l'inventaire, qui les a produits en cherchant autre chose.

## Le défaut de fond, en une phrase

**Un écran vide et un écran cassé se ressemblent.** Quinze fois dans le code, un `catch` décide
localement d'afficher un état vide, sans que rien en amont ne détienne la distinction entre
« il n'y a rien » et « ça a échoué ». Le message qui en résulte n'est pas neutre : il *affirme*
quelque chose de faux — « Aucun dossier surveillé », « Rien dans cette session », « à jour ».

C'est la même faute que celle décrite dans l'autre moitié du travail du jour
([#7](https://github.com/c0remusic/sift/issues/7)) : la cohérence se perd quand la décision
descend trop bas. Là c'était l'apparence d'un composant, ici c'est le sens d'un état.

## Le modèle à suivre, déjà dans le dépôt

`usb-view.ts` est le seul écran qui sépare correctement les deux cas : `usb-view.ts:125-133`
pour l'absence de clé (avec l'explication du lecteur de cartes vide), `usb-view.ts:114-124`
pour l'énumération en échec, chaîne brute affichée. Il tient parce que le backend le lui
permet — `usb_format/windows.rs:736-747` rend une `UsbFormatError::Enumeration` au lieu d'un
`Vec` vide.

Son commentaire (`usb-view.ts:116-118`) dit pourquoi ça compte : masquer l'erreur « a fait
passer une requête WMI cassée pour "aucune clé branchée" pendant des mois ».

**Corollaire de méthode** : plusieurs correctifs ci-dessous ne sont pas dans le `catch` du
frontend mais dans la signature de la fonction Rust qui l'alimente. Un front ne peut pas
distinguer deux cas qu'on lui livre déjà confondus.

## Provenance — à lire avant de corriger

Deux niveaux, délibérément distingués :

- **Vérifiées à la main** (8) : A2, A3, A7, A9, A13, A17, A19, A21. Lues ligne à ligne le
  2026-08-11.
- **Établies par lecture de code d'un agent** (13), avec leur `fichier:ligne`. Elles n'ont pas
  été relues indépendamment. **Re-vérifier chacune contre le code courant avant de la
  corriger** — un constat daté n'est pas un fait présent.

Aucune n'a été observée dans la vraie fenêtre : tout est établi par lecture. Aucune capture,
aucune session CDP.

## Les impasses

Ordre chronologique du parcours d'un utilisateur, **pas** un ordre de gravité — aucune
hiérarchie n'a été mesurée.

### Frappent l'utilisateur actuel, pas seulement un nouveau venu

Les trois plus graves ne concernent pas l'accueil. Elles se déclenchent sur une installation
en service.

- **A17 — `list_journal` avale ses erreurs par construction.** ✅ vérifiée.
  `actions.rs:1130` a pour signature `-> Vec<JournalEntry>`, pas `Result` :
  `Err(_) => return Vec::new()` sur `prepare` (`:1146`), `Err(_) => Vec::new()` sur `query_map`
  (`:1162`), et `.filter_map(|r| r.ok())` (`:1161`) qui jette les lignes illisibles une par une.
  Le wrapper IPC ne peut rien remonter (`ipc_filing.rs:1191`). L'écran peint « Rien dans cette
  session » (`journal.ts:341-347`). **Le correctif est la signature, pas le `catch`.**
  ⚠️ Le commentaire `journal.ts:331` — « Fail-fast: both calls throw on IPC error — no silent
  fallback » — est vrai pour la couche TS et faux deux couches plus bas. Le corriger fait
  partie du correctif.
- **A21 — Le thème confirme une préférence qu'il n'a pas su enregistrer.** ✅ vérifiée.
  `theme.ts:31-36` : `apply(choice)` d'abord, puis `setSetting` dans un `try/catch`
  console-only. Le segmented control bascule aussi son état `.on` inconditionnellement
  (`reglages-view.ts:315-318`). Le choix est perdu au prochain lancement.
- **A13 / A14 — Rekordbox affiche « à jour » précisément quand rien ne peut fonctionner.**
  ✅ vérifiée (A13). `rekordbox-view.ts:174-176` : `idleLabel` vaut « indisponible » seulement
  si le XML lié est illisible — **jamais si `master.db` est introuvable**. Les détecteurs
  transforment leurs erreurs en `None` muet (`actions.rs:233-246` écrase tous les
  `MasterDbError` ; `rekordbox_repairs.rs:155-169` écrase trois causes distinctes sans log).
  Zéro ligne détectée → corps vide → « à jour ». A14 : les quatre `catch` de section remettent
  leur tableau à `[]` (`rekordbox-view.ts:603, 613, 623, 633`) **avant** le calcul de
  `totalPending` (`:640`) qui pilote l'en-tête — quatre cartes en erreur sous un titre qui
  annonce que tout va bien.

### Au premier lancement

- **A1 — Aucun parcours de premier lancement n'existe.** Une recherche sur
  `onboard|bienvenue|welcome|premier lancement|first.run` dans `frontend/`, `src-tauri/src/` et
  `shared/` ne rend qu'un commentaire sans rapport (`progress-zone.ts:59`). Les trois prérequis
  réels — un dossier surveillé, une racine de bibliothèque, un jeton Discogs — ne sont énoncés
  nulle part ensemble. Ce point n'est PAS à corriger ici : c'est la décision
  [#16](https://github.com/c0remusic/sift/issues/16) de la map.
- **A2 — FFmpeg manquant n'est signalé qu'à la console.** ✅ vérifiée.
  `main.ts:34-37` fait `console.error` + `report_smoke(ok:false)` → `log::error!` (`ipc.rs:48`),
  rien à l'écran. L'analyse passant par Symphonia in-process, l'utilisateur peut ajouter,
  scanner, analyser et écouter ; il ne découvre l'absence qu'au premier « Ranger ».
  ⚠️ **Le message est un conseil faux** : `filing-actions.ts:126` dit « Une erreur est survenue
  pendant la conversion. **Réessaie.** » alors que réessayer ne peut jamais aboutir. La cause
  réelle est `ffmpeg: spawn failed` (`encode.rs:154`, `Display` en `encode.rs:83`).
  Note : la branche `/no such file|not found|introuvable/i` (`filing-actions.ts:125`) précède le
  générique, donc le message affiché dépend de la chaîne exacte de l'erreur d'E/S. Cela se
  détermine **en lisant** `encode.rs` et le `Display` de l'erreur, pas en lançant l'app.
- **A3 — « + Ajouter un dossier » peut échouer sans un mot.** ✅ vérifiée.
  `home-sources.ts:339-341` : `catch (e) { console.error("addSource failed", e); }`. Le
  sélecteur se ferme, la liste continue d'afficher « Aucun dossier surveillé — ajoute-en un
  ci-dessous. » (`home-sources.ts:107`). C'est le mensonge type. Trois handlers voisins ont le
  même défaut : `setSourceWatched` (`:302-304`), `rescanSource` (`:311-313`), `removeSource`
  (`:324-326`) — ce dernier **après une confirmation destructive acceptée**.
- **A4 — Un dossier fraîchement ajouté affiche un badge vert « À jour ».**
  `statusMeta` (`home-sources.ts:69-74`) n'a pas d'état « scan en cours » ; `add_source` rend la
  source avec `pending_count = 0` (`ipc.rs:55-64`), le scan partant en tâche de fond. Le cas
  transitoire se corrige seul, mais **quatre chemins rendent le mensonge permanent** :
  `ipc.rs:502` (échec `app_data_dir()`), `:507-509` (échec d'ouverture DB), `:520` (ligne source
  disparue), `:534` (échec de réconciliation, `log::error!` seul). Même effet si le dossier ne
  contient aucune extension reconnue (`scanner.rs:9`).
- **A5 — Déposer un dossier sans racine de bibliothèque → « Rien d'importable dans ce dépôt ».**
  `ipc.rs:196-204` : sans `library_root`, `dest_root` vaut `None`, la création de bin
  (`ipc.rs:210-215`) est sautée, `folders_added` reste 0, et le front toaste (`chrome.ts:86-88`).
  Le message accuse le contenu déposé au lieu de nommer le réglage absent. **Le commentaire du
  code nomme lui-même le trou** (`chrome.ts:81`). Second aplatissement au même endroit :
  `create_bin(...).is_ok()` (`ipc.rs:212`) jette l'échec réel.

### Écrans

- **A6 — Revue vide est le seul cul-de-sac sans action de l'app.** `filing.ts:252-256` ne passe
  ni `backToRevue` ni `actionHtml`, alors que le composant supporte les deux
  (`empty-state.ts:15-19`). Choix assumé en commentaire (`filing.ts:237`) — mais Bibliothèque,
  Écartés et Journal renvoient tous *vers* Revue, si bien qu'un nouvel utilisateur fait deux
  clics pour atterrir sur l'écran qui lui dit d'aller ailleurs à la main.
- **A7 — Spinner perpétuel du rail de Revue si `list_queue` échoue.** ✅ vérifiée.
  `queue-panel.ts:422-425` peint « Chargement… », puis `:430-432` fait `console.error` + `return`
  sec. Le spinner ne part jamais. ⚠️ `home-sources.ts:234-236` nomme et corrige explicitement ce
  défaut sur le rail jumeau (« un spinner permanent est un échec silencieux ») — **la correction
  n'a pas été portée ici.**
- **A8 — L'arbre de destination annonce « racine non choisie » quand c'est la lecture qui a
  échoué.** `filing-bins.ts:101-105` : le `catch` pose `rootSet = false`, ce qui rend la porte
  « Choisis ta racine de bibliothèque » (`:267`) à quelqu'un qui l'a déjà choisie.
- **A15 — « Impossible de charger — réessaie plus tard. » pour une condition permanente.**
  `rekordbox-view.ts:164`, posé en `:614`. Rekordbox n'étant pas installé, réessayer ne changera
  jamais rien.
- **A16 — Liaison XML : annulation muette, trois causes aplaties, une branche morte.**
  Annulation du sélecteur → `return` sans rien dire (`sift-live.ts:357`). Trois erreurs backend
  distinctes (`rekordbox_xml.rs:233`, `:159`, `ipc_library.rs:340`) deviennent un seul toast
  (`sift-live.ts:367`). Et `sift-live.ts:359-363` teste `status.error` alors que
  `ipc_library.rs:353` construit toujours `error: None` : **branche inatteignable**.
- **A18 — Journal : page blanche si l'IPC rejette pour de bon.** `app.js:318` vide `#content`
  **avant** l'appel, `sift-live.ts:178` `void`e la promesse sans `.catch`, et `journal.ts` n'a
  aucun `try/catch`. Pas même un état vide : un écran nu.
- **A19 — Le graphique d'occupation de Bibliothèque disparaît sans un mot.** ✅ vérifiée.
  `bibliotheque-view.ts:228-231` : `console.error` puis `slot.remove()`. La section n'a, du point
  de vue de l'utilisateur, jamais existé. À comparer au cas jumeau traité honnêtement sur Clé USB
  (`usb-view.ts:71-75`).
- **A20 — Carte d'erreur de Bibliothèque sans porte de sortie.** `bibliotheque-view.ts:264-272`,
  pas de bouton Réessayer — là où Écartés (`ecartes-view.ts:120-124`) et le bloc doublons
  (`bibliotheque-view.ts:353`) en ont un.

### Discogs — le groupe le plus trompeur

- **A9 — Réglages décrit une désactivation totale comme une dégradation.** ✅ vérifiée.
  `reglages-view.ts:79`, verbatim : « Sans jeton, les recherches sont limitées et plus lentes. »
  La réalité : `ipc_identify.rs:29-31` rend `Err("NO_TOKEN")` **avant tout appel réseau**, et
  `settings.rs:11` le dit — « Empty/unset = identification disabled ». Aucune recherche n'est ni
  limitée ni ralentie : il n'y en a aucune. Le message d'exécution (`filing-identify.ts:388`)
  parle de « recherches anonymes », qui n'existent pas davantage — mais il porte au moins un
  bouton « Ouvrir Réglages ».
- **A10 — Un jeton invalide, expiré ou révoqué s'affiche « Discogs injoignable ».**
  `discogs.rs:41-46` ne classe que le 429 ; **401 et 403 compris**, tout autre statut devient
  `ProviderError::Network`. Le front ne teste que `NO_TOKEN` puis `RATE_LIMITED:`
  (`filing-identify.ts:403`, `library-detail.ts:236`). L'utilisateur est envoyé vérifier sa
  connexion alors que son jeton est en cause.
- **A11 — Le jeton n'est jamais validé, et « Jeton enregistré. » s'affiche pour n'importe quelle
  saisie.** `reglages-view.ts:369-386` : débounce 600 ms, `setSetting`, puis
  `status.textContent = "Jeton enregistré."`. Aucun appel de test, aucun bouton « Vérifier ».
  Combiné à A10 : un jeton mal collé est confirmé valide, puis se manifeste plus tard comme une
  panne réseau. Détail annexe : le débounce n'est vidé ni à la navigation ni à la fermeture — une
  saisie suivie d'une sortie sous 600 ms est perdue sans trace.
- **A12 — Il n'existe aucune détection de Rekordbox.** `actions.rs:181-189` rend
  `Some(config_dir()/Pioneer/rekordbox)` — **chemin codé en dur, aucun `exists()`**, aucun
  réglage `pioneer_dir` hors `#[cfg(test)]`. Sur une machine vierge, la fonction rend
  `Some(<chemin inexistant>)`, et le seul message qui parle du dossier Pioneer
  (`rekordbox_repairs.rs:66`) est de fait inatteignable.

## Ce qui est sain, et qu'il ne faut pas casser en corrigeant

Clé USB (voir plus haut) · la détection de doublons de Bibliothèque, qui distingue explicitement
échec et zéro depuis la régression CC-1 (`bibliotheque-view.ts:53-67`) · le `setup()` de
`lib.rs:216-256`, fail-fast avec messages explicites · le portail `NoLibraryRoot` au rangement
(`filing-actions.ts:117`, `batch-panel.ts:719-721`) · la relance d'analyse pour les pistes non
analysées (`queue-panel.ts:530-626`) · les gardes d'écriture `master.db`
(`rekordbox_masterdb.rs:928-996` — refus si Rekordbox tourne, backup vérifié relisible, rename
atomique, restauration automatique).

## L'exécution sur profil vierge — ÉCARTÉE le 2026-08-11

L'inventaire proposait 7 vérifications à mener sur un profil vide, ce qui demande de déplacer la
vraie base de l'utilisateur (`app_data_dir()/sift.db` + ses `-wal`/`-shm`, `lib.rs:222`).
Antoine a demandé ce qu'elles achetaient. Réponse : **pas grand-chose, et aucun correctif de ce
document ne les attend.**

- Savoir si Discogs répond bien 401 ne change pas le correctif d'A10 : il faut classer 401/403
  quel que soit ce que l'API renvoie.
- La durée du badge vert est cosmétique — les quatre chemins qui rendent A4 *permanent* sont
  déjà établis par lecture.
- Le message d'échec FFmpeg, le comportement de `dirs::config_dir()` et la zone grise WMI de
  Clé USB se déterminent **en lisant**.

**Un seul trou réel subsistait, et il se lisait aussi** : `frontend/app.js` n'avait jamais été lu,
alors qu'il est importé inconditionnellement par `main.ts` et tourne donc en production. Fait le
2026-08-11 — voir la section suivante. **Le trou est fermé, et la réponse est rassurante.**

## `app.js` en production — angle mort fermé le 2026-08-11

439 lignes, lues intégralement. `main.ts:12` l'importe **avant** `installLiveWiring()` (`:24`),
et son IIFE se termine par un `render()` (`app.js:438`) : la maquette peint donc bien, au boot,
avant le wiring live.

**Elle ne peint pas ses données de démo pour autant.** Huit des dix fonctions de rendu portent la
même garde `if(!('__TAURI_INTERNALS__' in window))` et ne rendent, dans Tauri, qu'une coquille :
`renderHome` (`:78`), `renderRevue` (`:134`), `renderRkb` (`:253`), `renderBiblio` (`:265`),
`renderEcarts` (`:324`), `renderCle` (`:369`), `renderReglages` (`:382`), plus le handler clavier
(`:432`) qui sort immédiatement. `renderJournal` (`:318`) vide `#content` et délègue à
`window.__siftJournal()`.

Deux fonctions n'ont pas de garde, et il fallait vérifier leur atteignabilité :

- **`renderMid` (`:156`) est inatteignable** : son unique appel depuis `renderRevue` est ligne
  150, **à l'intérieur** de la garde.
- **`renderBatch` (`:240`) n'a pas de garde et serait atteignable** par `render()` (`:70`) dès que
  `revMode === "batch"`. Sa seule porte d'entrée est le bascule `[data-act="revmode"]`
  (`app.js:405`), et **`injectLeanStyle()` le masque** (`chrome.ts:139`). Cette injection est
  appelée depuis `sift-live.ts:181`, donc dans Tauri uniquement. Le vrai mode Lot vit ailleurs,
  sous `data-sift="reviewmode"` (`queue-panel.ts:496`).

**Mesuré dans la vraie fenêtre**, pas déduit : les 2 bascules de la maquette sont présentes dans
le DOM, toutes deux en `display: none` et à 0×0 ; la feuille `#sift-lean-style` est bien injectée ;
les 2 boutons du vrai segmented control sont là.

⚠️ **Risque résiduel, pour un agent et non pour un utilisateur.** `display:none` retire l'élément
de la souris, pas du DOM : un clic synthétique (`.click()`) atteint toujours le handler, met
`revMode` à `"batch"` et repeint la maquette **par-dessus** la vue live, avec Mr. Fingers et Chez
Damier. C'est le piège que documente déjà `.claude/skills/run-sift/SKILL.md`. Il ne touche pas
l'utilisateur — aucun clic humain ne peut viser un élément de taille nulle.

Sur la présence du drapeau au moment du boot : `main.ts:20-21` calcule `inTauri` avec le **même**
test, et c'est lui qui conditionne `installLiveWiring()`. Le wiring live tourne, donc le drapeau
est présent à l'évaluation du module — et `app.js`, importé dans la même passe synchrone, le voit
aussi.

**Conséquence pour ce chantier** : aucune impasse ne vient de la maquette. Elle est correctement
neutralisée. Le point B4 est clos sans correctif.

## Ce que ce chantier ne fait pas

- Il ne conçoit **pas** le parcours de premier lancement (A1) : c'est la décision
  [#16](https://github.com/c0remusic/sift/issues/16) de la map, et la concevoir ici la
  court-circuiterait.
- Il ne touche pas à l'identité visuelle.
- Il n'ajoute pas de table code→message dans `errors.ts` : l'absence d'une telle table est un
  choix délibéré du projet (`CLAUDE.md`), et le correctif visé est de **ne plus confondre deux
  états**, pas d'enjoliver des libellés.
