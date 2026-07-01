# Brief — Refonte UI Sift (à travailler dans Claude Design)

> Créé 2026-07-01. Point de départ pour la refonte du layout + de la direction
> visuelle, à construire dans Claude Design puis itérer (Stitch en sortie HTML
> statique, pas React). Le comportement (machine d'états) est préservé — voir
> `.interface-design/interaction-model.md`.

## Intention
Créer un **système de layout à zones fixes** qui gouverne tous les écrans
(Review, Library, Discarded, Journal) pour garantir la cohérence inter-écrans —
c'est LE problème à résoudre (l'ancienne UI manquait de grammaire spatiale
unificatrice, chaque écran composé pour lui-même). La grammaire doit être si
claire qu'elle dicte d'elle-même où placer les fonctions futures.

## Direction visuelle (nouvelle — validée 2026-07-01)
**Outil pro, pas objet lifestyle.** Lisible sur des heures de tri. Remplace
l'ancienne direction « table du digger » sombre chaude (jugée trop objet-lifestyle
et trop sombre).
- **Base** : gris chaud, surfaces relevées (PAS de noir profond — ça doit
  respirer). Empilement surf-0 (canvas) -> surf-1 (cartes) -> surf-2 (contrôles),
  en gris chaud clair.
- **Principe couleur central** : la couleur ne sert QUE le sens, jamais la déco.
  - **Vert** = ok / lossless / verdict positif.
  - **Ambre** = doute / pending (fake?, DUPLICATE, fonctions non implémentées).
    Réservé, jamais décoratif. (Ce principe résout la collision ambre : comme
    aucune couleur n'est décorative, l'ambre ne veut dire qu'une chose.)
  - Aucune autre couleur d'accent. Play, progression, boutons neutres = gris
    relevé, pas colorés.
- **Typo** : sans-serif (Outfit) pour l'humain (titres, artistes) ; mono
  (JetBrains Mono) pour le technique (valeurs, codecs, compteurs, temps).
- **Profondeur** : surfaces + bordures 1px basse-opacité. Pas d'ombres portées
  (tolérance : ombre douce sous la pochette).
- **Motion** : retenue, décélération, jamais de spring/bounce.
- **Densité** : équilibrée, sobre. Outil de travail reposant.

## Fenêtre cible
**Large (~1400-1900px)**, Windows d'abord. Concevoir pour la vraie largeur — les
colonnes doivent respirer (les maquettes d'explo étaient bridées à 680px).

## Structure : 3 colonnes à rôles FIXES

**Colonne 1 — Navigation (~132-160px, fixe)** — ne change jamais selon l'écran.
- « Sift »
- Review (compteur queue) · Library · Discarded · Journal
- Groupe « Export » en bas : Rekordbox · USB drive — **pending** (opacité 0.5 +
  point ambre 5px, non-interactifs, « not implemented yet » au clic)

**Colonne 2 — Queue (~232-300px, fixe)** — toujours visible pour le contexte.
- En-tête : « Queue » + toggle **Detail / Batch**
- Lignes : titre (ellipsis) + méta (verdict coloré + artiste + marqueur `dup`)
- Ligne active : fond neutre relevé + barre de focus à gauche (2px)
- Verdicts : lossless (vert), fake? (ambre), pending (muted)

**Colonne 3 — Inspecteur (flexible)** — détail + action. Ordre vertical
**son d'abord** (non négociable) :
1. Fil d'Ariane — Review > [titre]
2. **Hero** — pochette + titre + artiste + version
3. **Audition** — play + waveform-transport + temps (mono) + pitch. *Le son
   avant tout.*
4. **CLAIMED vs ACTUAL** côte à côte :
   - CLAIMED = ce que le fichier prétend (FLAC · kbps · kHz · bit)
   - ACTUAL = verdict « Ready to file » + chips-preuve interactives
5. **Chips-preuve** (chacune ouvre sa preuve inline, divulgation progressive) :
   - `lossless` -> spectre (chute ~22 kHz) · `92% match` -> détail Discogs ·
     `no dup` -> dédup (-> `DUPLICATE` ambre + carte comparaison si doublon)
6. **IDENTIFICATION · Discogs** — Label · Year · Genre + lien « Change »
7. **Rail d'action (ancré bas, surface relevée)** :
   - DESTINATION (picker, accolée au CTA)
   - Hint clavier (mono) : entrée = file · backspace = discard
   - **Discard** (secondaire) · **Move and encode** (primaire, un seul CTA
     primaire par écran)

## Mode Batch (variante col 3)
Sélection multiple (Space toggle) ; rail devient « Move selection (n) » +
« Discard (n) » (ce dernier **pending**, ambre). Le reste de la grammaire ne
bouge pas.

## Cohérence inter-écrans (le cœur)
Library, Discarded, Journal **réutilisent les 3 mêmes zones** : nav (col 1) et
structure identiques, seul le contenu de col 2/3 change. C'est ce qui crée la
cohérence et « shape les fonctions futures ».

## Toutes les fonctions à intégrer (ne rien oublier)
Sources (add/remove/rescan/watch/import) · Queue (list/analyze/progress) · Play ·
Filing (reconcile/file_track/file_batch/reject/trash/undo/revert/requeue/restore/
purge) · Duplicate detection · Journal · Discarded · Bins (list/create
destination) · Discogs (identify/apply) · Library (list/folders/update_metadata) ·
Settings.

## Contraintes techniques (pour le pont vers le code après)
- Stack : **Vite vanilla TS, PAS de React** -> sortie transposable en HTML/CSS
  vanilla (si Stitch : viser HTML statique, pas React).
- **Préserver la machine d'états** (Focus/Selection/Mode/Expansion) et le clavier
  Linear-like (haut/bas · Space · entrée · backspace · I · Cmd+Z · Tab · Cmd+K).
  Le layout habille le comportement, ne le change pas. Voir
  `.interface-design/interaction-model.md`.
- Tokens existants : space 4/8/12/16/24/32 · radius 4/6/10/999 · height
  32/36/40/44.

## Priorités
Lisibilité/hiérarchie · ordre son-d'abord · place à la queue · rail
destination+actions ancré · **couleur = sens uniquement**.

## Méthode de travail
1. Construire **Review** d'abord (écran le plus dense — s'il tient, les autres
   suivent).
2. Décliner Library / Discarded / Journal sur la MÊME grille (test de cohérence).
3. Une fois la direction validée en grand : pont vers le code (styles.css +
   vues vanilla), en préservant la machine d'états.
