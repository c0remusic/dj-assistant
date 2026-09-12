# Vérification du chantier HIG — protocole et résultats

## Résultats mesurés le 2026-08-05

Fenêtre `tauri dev` réelle, CDP sur le port **9333** (9222 *et* 9223 étaient occupés par
un projet voisin, `shaderlab` — l'identité a été vérifiée avant chaque mesure,
`document.title` = « Sift — prépa sons DJ »).

| Point | Résultat |
|---|---|
| **Plancher typographique** | **Tenu.** Volet détail Revue avec piste ouverte : 177 nœuds de texte mesurés, `minRendu = 10px`, aucun élément sous le plancher. Les badges `LOSSLESS` et `CDJ compatible` — qui étaient à 9 px — mesurent 10 px. Idem sur les 8 écrans de navigation. |
| **E7 placeholder Accueil** | **Fonctionne.** « Chargement… » apparaît immédiatement au retour sur Accueil, puis cède la place à la liste réelle. |
| **Clipping à fort zoom** | **Non reproduit** jusqu'à un zoom ×4 : `scrollWidth == clientWidth` sur `.pa`, `.mid`, `#content`, `.sb`, et aucun débordement du `body`. ⚠️ Test **partiel** — mené sur un écran 3440 px, où même 225 % laisse 1529 px CSS, au-dessus du `minWidth: 920`. Le cas dangereux reste une petite fenêtre. |

**Piège rencontré, à retenir.** Une première série de mesures est revenue « conforme » sur
des écrans où **rien n'était peint** : `#mid` avait zéro enfant, aucun badge n'existait. Un
tableau vide signifiait « rien à mesurer », pas « rien sous le plancher ». Toute mesure de
ce protocole doit être accompagnée d'un **compte positif** — `nbTextes` et `minRendu` — sans
quoi elle ne distingue pas la conformité de l'absence.

Autre artefact de pilotage : cliquer la bascule de mode de `app.js` fait repeindre la
maquette **par-dessus** le rendu live, avec ses pistes de démonstration. Passer par un
rechargement puis le chemin utilisateur réel.

## Reste à voir

- les badges du **mode Lot live** (`DUPLICATE`/`FAKE`) n'ont pas été peints ;
- **contraste** : « changer » sur pochettes claire et sombre, ligne « En analyse » ;
- **mouvement réduit** : le réglage système ne se force pas depuis la page ;
- **clipping en petite fenêtre**, seul cas où le risque existe.

## Protocole

Tout ce qui a été livré le 2026-08-05 a passé `tsc --noEmit`, `lint:tokens` et
`check:security`. **Ces trois gardiens sont statiques.** Ils ne disent rien du rendu, du
focus, ni des chemins d'erreur — et l'essentiel du chantier vit dans `installLiveWiring()`,
que ni un navigateur ni le Browser pane n'exécutent (`CLAUDE.md` § Architecture).

Ce document liste ce qui reste à voir dans la vraie fenêtre, et comment.

## Comment mesurer

Deux voies, selon la question.

**À l'œil**, dans `npm run tauri dev` — c'est le défaut du projet, et il suffit pour tout
ce qui est visuel.

**Au CDP**, pour ce qu'un regard ne tranche pas (tailles calculées, débordements,
`devicePixelRatio`). Lancer avec le port en variable d'environnement — **jamais** dans
`tauri.conf.json`, il s'appliquerait aux builds distribués :

```bash
WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9222 npm run tauri dev
```

Puis `.claude/scripts/cdp.cjs`. ⚠️ Vérifier que `document.title` correspond bien à Sift
avant de faire confiance à la session : le port est squattable par un projet Tauri voisin.

## 1. Plancher typographique — la seule mesure qui tranche

Après la migration, **aucun texte ne doit être rendu sous 10 px**. Une seule expression le
dit, sur n'importe quel écran :

```js
[...document.querySelectorAll('*')]
  .filter(e => e.textContent?.trim() && !e.children.length)
  .map(e => [e.className || e.tagName, parseFloat(getComputedStyle(e).fontSize)])
  .filter(([, px]) => px < 10)
```

Attendu : tableau vide. À rejouer sur Revue (mode Détail **et** mode Lot), Bibliothèque,
Écartés, Journal, Rekordbox, Clé USB — chaque écran peint ses propres classes.

**Deux résidus connus sous 10 px, hors `styles.css`. C'est cette requête qui tranche s'ils
comptent.**

- `frontend/chrome.ts:165` — `.sift-tb-mac .sift-win:hover{…font-size:8px}`. **Exemption
  légitime, à ne pas corriger** : ce sont les glyphes des boutons de fenêtre macOS, dans
  des pastilles de 12 px, invisibles au repos (`font-size:0`) et révélés au survol. macOS
  fait exactement cela. Le plancher HIG vise le **texte qu'on lit**, pas un glyphe dans un
  contrôle qui imite le système — et 10 px déborderait d'un cercle de 12. À constater sur
  la cible macOS, pas à changer.
- `frontend/app.js` — quatre sites à 9 px (le « bpm » du lecteur, la légende du
  spectrogramme, des pilules d'écartés). C'est la maquette, chargée inconditionnellement
  et donc **réellement exécutée en production**, mais dont `#mid` est réécrit par le
  wiring live sous Tauri. **Statut indécidable par lecture de source** : soit son rendu
  est remplacé avant d'être vu, soit il persiste. La requête ci-dessus le dit
  empiriquement — si un de ces textes remonte dans le tableau, il est réellement peint et
  doit migrer ; s'il n'y est pas, la maquette n'a jamais eu le dernier mot et il n'y a
  rien à faire.

Points à regarder à l'œil dans la foulée, ce sont les coûts assumés de la décision :

- **rail de navigation, 152 px** — `.sift-pz-count` y était déjà tronqué à 9 px. Vérifier
  sur une grosse bibliothèque que « Analyse » reste compréhensible malgré l'ellipse ;
- **mode Lot** — le badge `DUPLICATE` prend ~7-8 px de plus au nom de fichier, déjà
  tronqué. Vérifier que le nom reste identifiable ;
- **bannière de rangement** — elle s'enroule ; elle peut passer de 2 à 3 lignes ;
- **`.jrnl-cat-label`** porte `flex:none` : sur fenêtre étroite, son badge est poussé à
  droite. Chercher un tassement, pas une troncature.

## 2. Mouvement réduit

Activer « Animations réduites » (macOS) / « Afficher les animations » off (Windows), puis
relancer Sift.

- **les spinners doivent CONTINUER à tourner** — `.sift-spin`, `.sift-bt-spin`. C'est une
  exemption écrite dans le sélecteur : un indicateur figé se lit comme une app plantée ;
- tout le reste ne doit plus bouger : ouverture des collapses, thumbs de segmented,
  spectrogramme, flashs d'identification ;
- **le point qui vaut le détour** : identifier une piste, puis appliquer les tags. Les
  classes `.sift-identified-flash` et `.sift-applytags-flash` sont retirées par un
  `animationend`. La garde neutralise la *durée* et non la propriété précisément pour que
  l'événement continue de se déclencher. Si une de ces classes restait collée, la garde
  serait fausse.

## 3. Zoom

- Ctrl+`+`, Ctrl+`-`, Ctrl+molette doivent maintenant zoomer. Avant ce chantier ils ne
  faisaient rien — Tauri désactive par défaut un zoom que WebView2 offre ;
- **puis chercher le clipping**, c'est le vrai risque et il n'est pas réglé. À 225 % la
  fenêtre tombe sous le `minWidth: 920` qu'elle déclare, or il n'y a aucune media query de
  largeur et `html,body` porte `overflow:hidden` :

```js
['.pa', '.mid', '.home-body'].map(s => {
  const e = document.querySelector(s);
  return e ? [s, e.scrollWidth, e.clientWidth, e.scrollWidth > e.clientWidth] : [s, null];
})
```

Un `true` en quatrième position = contenu coupé sans moyen d'y accéder, ce qui échoue
WCAG 2.1 SC 1.4.4. C'est un problème de layout, à traiter à part.

- **Windows uniquement** : Paramètres > Accessibilité > Taille du texte à 225 %, relancer
  Sift, lire `window.devicePixelRatio`. Attendu ≈ 2,25 si le `TextScaleFactor` s'applique.
  C'est la mesure qui dit si Windows avait déjà une voie de secours.

## 4. Accueil

- au tout premier lancement (ou base vide), le rail doit afficher « Chargement… » au lieu
  de deux colonnes vides ;
- **puis faire des allers-retours rapides entre onglets** : le placeholder ne doit pas
  clignoter. C'est le risque déclaré par l'agent qui l'a posé — il n'y a pas de cache de
  sources à repeindre, contrairement à la file de Revue ;
- couper le réseau ou provoquer un échec de `listSources()` : une carte d'erreur avec
  « Réessayer » doit remplacer le spinner. Un spinner qui tourne indéfiniment serait
  l'échec silencieux que le projet interdit.

## 5. Contraste

- **« changer » sur une pochette** — le cas qui mesurait 1,42:1. À voir sur une pochette
  très sombre **et** une très claire, dans les deux thèmes ;
- **ligne « En analyse » du mode Lot** — l'opacité globale a été retirée. Le badge, le mot
  d'état et le bouton sont à pleine opacité ; seul le nom est atténué, par un token.
  Question à trancher à l'œil : le groupe paraît-il encore suffisamment inerte ? Si non,
  le levier est le token du nom, **jamais** une opacité de ligne — ça referait échouer AA ;
- bannière de rangement et badge de qualité neutre, en thème sombre.

## 6. Annulation

Ctrl+Z sur Windows, ⌘Z sur Mac — la garde accepte les deux modificateurs. Vérifier aussi
qu'un revert hors ordre affiche bien « Action plus récente à annuler d'abord. »

## Ce que cette vérification ne couvre pas

- **E6** — le gel d'IPC pendant une analyse froide. Se mesure au chronomètre, pas à l'œil :
  ouvrir une piste jamais analysée et voir si l'interface répond pendant le calcul. Le
  dépôt a l'outil (`bench_sqlite.rs`, `SIFT_BENCH_TRACKS_DIR`, `--ignored`) ;
- **E1** — le registre de contraste augmenté n'existe pas, il n'y a rien à vérifier ;
- la file de Revue reste non focalisable au clavier, volontairement (voir `plan.md`
  étape 7) : le conflit avec la couche clavier globale n'est pas levé.
