# Design Review: Reskin écran Revue (refonte 2026-07-01)

Reviewed against: `docs/brief-refonte-ui-2026-07-01.md`, `Sift.dc.html` (maquette hi-fi Claude Design)
Philosophy: outil pro sobre — gris chaud clair, couleur = sens uniquement (vert/ambre), pas de décoratif
Date: 2026-07-01

## Screenshots capturées

Captures via `computer-use` sur la fenêtre native Tauri (pas de navigateur — voir note méthode
ci-dessous), non sauvegardées sur disque (inspection directe uniquement).

| Vue | Ce qu'elle montre |
|---|---|
| Revue · Detail | Queue (272px) + inspecteur (hero, audition, verdict card, éditeur métadonnées) |
| Revue · Detail, zoom nav | Rail de navigation traduit, badge compteur, sélection active |
| Revue · Detail, zoom audition | Bouton play, waveform, hints clavier |
| Revue · Detail, popover Destination ouvert | Arborescence flottante par-dessus le rail d'action |
| Revue · Batch | Vue groupée par format + popover Destination (sous-arborescence) |
| Écartés | Confirmation palette héritée, écran hors scope non traduit |
| Accueil | Confirmation palette héritée, écran hors scope non traduit |

### Note méthode
Pas d'accès Playwright/cursor-ide-browser (app native Tauri, pas un site web). `npm run dev`
seul (sans Tauri) affiche l'ancien mockup `app.js`, pas le rendu réel — non pertinent pour cet
audit. Capturé directement la fenêtre native via `computer-use` (accès demandé et accordé sur
`sift.exe`), qui reflète le vrai comportement live.

## Suivi (2026-07-01, passe 2)

- Must Fix #1 (point rouge) : **corrigé**.
- Must Fix #2 (icônes Tabler) : **non confirmé** à la re-vérification — le CDN jsdelivr répond
  200 depuis cette machine ; sans accès direct au rendu au moment de la relecture, ce qui avait
  été lu comme un glyphe cassé était probablement un artefact de compression du screenshot zoomé
  (triangle play minuscule). Rétrogradé, à re-confirmer visuellement si le souci réapparaît.
- Should Fix #3 (hint "↑↓" → "++") : **corrigé** — remplacé par le texte "HAUT/BAS" (contourne
  le problème de police plutôt que d'en chercher la cause).
- Should Fix #4 (popover ne se ferme pas au clic queue) : **corrigé** (listener en phase capture).
- Should Fix #5 (case à cocher absente en Batch groupé) : **infirmé** — les cases existent bien
  en code (`.bx-ck` dans `readyRow`/`fakeRow`, sift-live.ts). Le vrai problème trouvé en creusant :
  5 fonds de sélection codés en dur en blanc (`rgba(255,255,255,...)`) dans `sift-live.ts` et
  `report-view.ts` (toggle Detail/Batch, lignes sélectionnées Ready/Fake, pill UNANALYZED, chip
  neutre) — quasi invisibles sur fond clair, ce qui explique l'impression de sélection peu
  lisible. Migrés vers `--overlay-hover`/`--overlay-selected`/`--color-background-primary`.

## Résumé

Le reskin tient globalement bien la route : palette cohérente sur tous les écrans testés,
traduction FR lisible et naturelle, popover Destination fonctionnel (le changement le plus
risqué de la passe). Un vrai bug de couleur casse la règle centrale du brief (rouge visible
en file d'attente, alors que la direction interdit explicitement une 3e teinte) — à corriger
avant de considérer cette passe close.

## Must Fix

1. **Point rouge dans la liste de la file (queue)** : `frontend/sift-live.ts:110-113`
   (`VERDICT_DOT`) a trois couleurs codées en dur héritées de l'ANCIENNE palette dark
   (`#5bc08c` / `#e2685e` / `#dda63f`), jamais migrées vers les tokens CSS du reskin. Le
   verdict `fake` s'affiche donc en **rouge** (`#e2685e`) — contredit directement le principe
   du brief ("Aucune autre couleur d'accent" / pas de 3e teinte, confirmé aussi par la note du
   wireframe "pas de 3e couleur"). Visible sur la piste `08-russell_taylor-fool_for_love` dans
   la capture Detail. _Fix : remplacer par `var(--color-text-warning)` (ambre) pour `fake`,
   `var(--color-text-success)` pour `ok`, idem pour `grey` — ou mieux, faire lire `verdictDot`
   directement les tokens CSS au lieu d'une map hex locale, pour que ce genre de dérive soit
   impossible à l'avenir._

2. **Icônes Tabler potentiellement cassées** : le bouton play de la bande d'audition affiche un
   glyphe qui ressemble à un caractère de repli («&nbsp;D&nbsp;») plutôt qu'un triangle play propre.
   `index.html` charge `@tabler/icons-webfont` depuis un CDN (`cdn.jsdelivr.net`), sans repli
   local — contrairement à Outfit/JetBrains Mono déjà self-hosted via `@fontsource`. Pas introduit
   par cette passe, mais bien plus visible sur fond clair que sur l'ancien fond sombre.
   _Fix : vérifier l'accès réseau du contexte Tauri au runtime, ou bundler les icônes Tabler en
   local comme les deux polices._

## Should Fix

3. **Hints clavier affichent "++" au lieu de "↑↓"** : `frontend/report-view.ts` (`keyboardHintsHtml`)
   passe le glyphe `↑↓` littéral, qui ne semble pas couvert par la police Outfit chargée — rendu en
   repli qui ressemble à "++". Pré-existant (je n'ai touché que le texte "navigate"→"naviguer"),
   mais mérite un fix simple (texte "haut/bas" à la place, ou police avec couverture Unicode des
   flèches).

4. **Le popover Destination ne se ferme pas au clic sur une ligne de la queue** : seul Échap le
   ferme de façon fiable. Cause : les handlers de clic délégués sur `#pa` (queue rows, etc.)
   appellent `e.stopPropagation()` (`frontend/sift-live.ts`, plusieurs occurrences autour de
   1048-1216) avant que le clic ne remonte jusqu'au listener `document` que j'ai câblé dans
   `ensureDestPopoverAutoClose` (`filing.ts`). _Fix : soit fermer le popover explicitement dans
   le handler de clic de ligne de queue, soit attacher le listener de fermeture en phase de
   capture (`{ capture: true }`) pour qu'il voie le clic avant le `stopPropagation`._

5. **Mode Batch : pas de case à cocher visible par ligne dans un groupe déplié** — seul le
   header de groupe a une case tri-état. À confirmer avec toi si c'est voulu (sélection uniquement
   par groupe, cohérent avec "1i groupé par format") ou un oubli visuel (impossible de voir d'un
   coup d'œil quelles pistes précises sont incluses dans la sélection du groupe).

## Could Improve

6. **Toggle "Sur place"** : inséré comme bloc plat entre l'inspecteur et le rail d'action
   (`ensureInPlaceToggle`, `insertBefore` sur `#filfoot`), au lieu d'être une ligne DANS le
   popover Destination comme dans la maquette. Fonctionnel, mais visuellement détaché du
   contrôle Destination auquel il se rapporte.

7. **Position du popover approximative** : ancré à `left:15px;bottom:60px` en dur plutôt que
   calculé depuis la position réelle du bouton Destination (`getBoundingClientRect`). Fonctionne
   bien à la taille actuelle du rail, mais pourrait se désaligner si sa hauteur change (ex. texte
   du bouton secondaire qui s'allonge).

## Ce qui marche bien

- **Palette cohérente sans effort supplémentaire** : Écartés et Accueil (hors scope, non
  retouchés) héritent déjà proprement du nouveau fond gris chaud rien qu'en changeant les
  valeurs des tokens CSS partagés — valide l'approche "mêmes noms de custom properties".
- **Popover Destination** : le changement structurel le plus risqué de la passe (colonne
  persistante → popover flottant) fonctionne correctement à l'usage — ouverture/fermeture au
  clic sur le bouton, état conservé indépendamment entre Detail et Batch (single source of
  truth sur `#fldz`), sélection d'un dossier referme bien le popover.
- **Carte verdict + chips** (LOSSLESS/UNIQUE, "Prêt à ranger") : bonne lisibilité sur fond clair,
  ton posé, cohérent avec le brief "outil pro, pas objet lifestyle".
  Traduction FR naturelle sur tout l'écran Revue (labels, boutons, messages d'erreur, hints).
- **Nav traduite** : libellés + badge de compteur lisibles à 152px, pas de troncature gênante
  observée sur les libellés actuels (Accueil/Revue/Écartés/Journal/Bibliothèque/Réglages).
