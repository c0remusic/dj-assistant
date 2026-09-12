# Sift — Interface Design System

> ⚠️ **Nettoyé le 2026-07-23** (Antoine + Claude). Ce fichier documentait la
> direction Penpot dark d'origine (accent bleu, palette charbon) — **périmée
> depuis le 2026-07-01** (refonte gris chaud clair, vert/ambre uniquement,
> `Sift.dc.html`). L'avertissement existait déjà en tête, mais toutes les
> valeurs hex concrètes de l'ancienne palette restaient écrites plus bas dans
> le corps du doc — un agent qui grep "couleur"/"accent" sans lire l'en-tête
> jusqu'au bout pouvait retomber dessus et régénérer du bleu ou du dark par
> erreur. Ce nettoyage retire toute valeur de couleur concrète du corps actif
> du document ; l'ancienne palette complète reste consultable dans l'historique
> Git si besoin d'archéologie, mais plus dans ce fichier.
>
> **Seule source de vérité pour les couleurs : `frontend/styles.css`** (tokens
> `--color-*`, `--overlay-*`). **Seule source de vérité état-par-état (couleurs
> ET comportements réels) : `docs/design-system-states.md`.** Ce fichier-ci ne
> couvre plus que structure/espacement/radius/hauteurs/typo/iconographie/process
> Penpot — et même ces valeurs numériques sont **à revérifier contre
> `styles.css` avant usage**, pas à prendre pour argent comptant : le nettoyage
> a trouvé une dérive sur le radius (voir section Radius ci-dessous) entre ce
> qui était documenté ici et la valeur réelle en code. Si vous retrouvez
> d'autres écarts numériques en l'utilisant, corrigez-les ici dans le même
> geste plutôt que de laisser la dérive continuer.

> Direction travaillée dans Penpot. Boards canoniques (page « Sift — shell ») :
> **Détail = « Revue · plat »** `49975f37-649c-80c0-8008-39eb475e8b73` (x5500),
> **Batch = « Revue · batch »** `284acdb7-967e-8038-8008-3a1f415c4596` (x5500/y820).
> UI **en anglais**. Convergence faite côté front réel (2026-06-25) : tokens
> **couleur + radius** au CSS, libellés **anglais**, clavier **↑/↓ + ⌫** + **focus
> ring** (`:focus-visible`), **typographie chargée** (Outfit + JetBrains Mono
> self-hosted via @fontsource), **layout Détail refondu** — son-d'abord (hero →
> audition → carte verdict « Ready to file » → preuve) + **rail de validation** :
> la colonne destination porte la pile DESTINATION → FINAL NAME → GENRES →
> FORMAT → File/Discard. Restent non alignés : espacement (quelques px hors
> grille), layout **Batch** (bloqué sur `reject_batch` Rust, cf. §7).

## Direction & ressenti (structure, pas couleur)

Outil de DJ : le son passe avant l'image. La coque chuchote ; le **verdict** et
l'**action** mènent ; la **pré-écoute précède** les autres étapes. Densité
équilibrée (4/10). Motion retenue (décélération, pas de spring/bounce).
(Le ressenti "chaud/premium" tient aujourd'hui à la palette gris chaud clair de
`styles.css`, pas à la description dark ci-dessous — ne pas se fier à un
adjectif de couleur écrit ici sans vérifier le token réel.)

## Ordre de l'écran Détail (son d'abord)

chemin (fil d'Ariane) → **hero** (pochette + titre + artiste + version) →
**audition** (play + waveform-transport + temps + pitch) → **CLAIMED** (ce que
le fichier prétend : FLAC/kbps/kHz) → **ACTUAL** (bandeau verdict « Ready to
file » + chips) → **IDENTIFICATION Discogs**. Le **spectre (« ▸ Proof »)** est
en **divulgation progressive** sous le bandeau verdict.

### CLAIMED vs ACTUAL

On montre **ce que le fichier prétend être** (métadonnées déclarées) PUIS **ce
qu'il est vraiment** (verdict de l'analyse). C'est le cœur de Sift (détection
faux-lossless).

### Chips verdict = interactives (à câbler)

Les chips `LOSSLESS` / `92% MATCH` / `NO DUP` ne sont pas des étiquettes
mortes : **chacune ouvre sa preuve** dans un tiroir inline sous le bandeau.
LOSSLESS→spectre (chute à ~22 kHz) ; 92% MATCH→détail du score Discogs ;
NO DUP→résultat dédup (devient **DUPLICATE** ambre + carte de comparaison
quand il y a doublon). Résout la zone vide + respecte la divulgation
progressive.

## Profondeur : SURFACES + BORDURES (committé, structurel)

Pas d'ombre portée pour la mise en page. Élévation = paliers de clarté de
surface (tokens `--color-background-*`/`--color-surface-raised` de
`styles.css`, pas les valeurs hex ci-dessous qui sont périmées) + bordures 1px
basse-opacité (`--color-border-*`).
- Surfaces empilées : bg → surf-1 (cartes) → surf-2 (chips/pochette).
- Sidebar = **même fond que le canvas**, séparée par un filet (pas une autre
  couleur).
- Inputs/wells : **plus sombres/enfoncés** que l'entourage (inset) — voir
  `--color-track`.
- Seule tolérance d'ombre : une ombre douce sur la pochette
  (`--shadow-panel-subtle`). Tout le reste = bordures.

## Alignement & centrage — RÈGLE CRAFT #1

Cause racine des désalignements : un texte posé en `x,y` se cale en **coin
haut-gauche**, jamais centré dans sa bande. **Tout texte dans une ligne /
bande / chip / bouton se construit dans une boîte de hauteur fixe**
(`growType="fixed"` + `resize(w, bandH)`) avec **`verticalAlign:"center"`**.
- **Colonnes sur x fixes** : titres `align:"left"`, valeurs numériques
  `align:"right"`.
- Cases/chips/icônes **centrées verticalement** dans la bande
  (y = bandTop + (bandH−elem)/2).
- Penpot supporte `verticalAlign` (top/center/bottom) ET `align`
  (left/center/right) — vérifié.
- Rythme de ligne constant (bande 34px), gaps de section homogènes.

## Espacement — ⚠️ à revérifier contre `styles.css`

Ce doc listait autrefois : `xs 4` · `sm 8` · `md 12` · `lg 16` · `xl 24` ·
`xxl 32`. `styles.css` ne déclare aujourd'hui que `--space-4/8/12/16` (pas de
token `xl`/`xxl` distinct en CSS au moment du nettoyage) — soit ces deux
paliers n'ont jamais été portés en token CSS, soit ils sont gérés en littéral
quelque part. **Ne pas assumer que 24/32 existent comme tokens sans grep
`--space` dans `styles.css` au préalable.** Hiérarchie de profondeur voulue
(la nidification se lit par l'espace), à confirmer contre le code avant de
s'en servir telle quelle : section↔section 24 (xl) > padding carte 16 (lg) >
groupe eyebrow→valeur / chip↔chip 12 (md) > interne control icône↔label 8
(sm). Densité d'abord (la liste Queue reste dense, scrollable).

## Radius — ⚠️ dérive confirmée, ne pas utiliser ces chiffres sans vérifier

Ce doc listait : `sharp 4` · `default 6` (boutons, inputs, controls) ·
`soft 10` (cartes, panneaux) · `pill 999` (badges, segmented, tags).
`styles.css` déclare aujourd'hui `--border-radius-base:14px` avec
`sm = base-6 = 8px`, `md = base-4 = 10px`, `lg = base = 14px`, `pill = 999px`
— ni les libellés ni au moins une valeur (`sm` 8 vs "default" 6) ne
correspondent exactement à ce qui était documenté ici. Découvert pendant le
nettoyage du 2026-07-23, pas corrigé plus finement à ce stade (nécessite de
vérifier quel composant réel utilise quel token, hors scope de ce passage).
**Traiter les nombres ci-dessus comme un signal historique, pas une vérité
actuelle ; grep `--border-radius` dans `styles.css` avant de coder quoi que ce
soit dessus.**

## Hauteurs (inputs / boutons / dropdowns) — ⚠️ non revérifié

`compact` **32** (lignes/tables) · `default` **36** · `comfortable` **40**
(action principale, dropdowns) · `large` **44**. `styles.css` ne déclare
aujourd'hui qu'un seul token de hauteur (`--h-40`) — les autres paliers
listés ici ne sont peut-être plus que des littéraux ou ont disparu avec
l'audit du 2026-07-03 (qui a déjà supprimé `--h-32`/`--h-44` faute de
consommateur). Vérifier avant de s'y fier.

## Dropdown / select

État fermé = control **h40** (comfortable) — c'est le seul palier de hauteur
confirmé exister comme token (`--h-40`). Radius, fond inset et couleurs :
voir `styles.css` + `docs/design-system-states.md`, pas les valeurs qui
suivaient ici avant nettoyage. Overlay à l'ouverture (ne pousse pas le
layout). Détail comportemental : `interaction-model.md` §8.

## Lexique (termes canoniques, anglais, persistants)

Acte central = **File** (CTA « File & encode » / « File selection » ; « Ready
to file », « Undo filing »). Verdicts : **LOSSLESS** · **LOSSY** (faux-lossless,
jamais « FAUX ») · **DUPLICATE** · **UNIQUE** (pas « NO DUP ») · **NO MATCH**.
Destination = **DESTINATION** partout (jamais « GOING TO »). Source de vérité
comportementale : `interaction-model.md`.

## Typographie

- Familles : **Outfit** (UI, 600 titres/labels, 400 corps) + **JetBrains Mono**
  (chiffres) — confirmé toujours en usage dans `styles.css`
  (`--font-ui`/`--font-mono`).
- **Mono FAIT** : tous les chiffres techniques (kbps, kHz, %, pitch, temps,
  match%) en JetBrains Mono.
- Échelle : voir `--text-*` dans `styles.css` (source à jour) plutôt que les
  valeurs px qui étaient listées ici avant nettoyage — non revérifiées une
  par une.
- **Règle build** : tout texte créé reçoit `Outfit.applyToText` OU
  `JetBrainsMono.applyToText` (sinon Penpot retombe sur « sourcesanspro »).

## Composants — structure uniquement, couleurs dans styles.css

- **Segmented** (Queue/Discarded/Trashed ET toggle Detail|Batch) : well inset
  + pill actif, texte centré (verticalAlign center). Voir `.sift-seg`/
  `.sift-seg-opt` dans `docs/design-system-states.md` (L603) pour l'état réel
  à jour — c'est la version unifiée, pas celle décrite historiquement ici.
- **Badge/chip statut** : pill, sémantique = fond teinté + texte sémantique,
  neutre = overlay neutre + texte secondaire (voir `--overlay-badge`). Texte
  centré (boîte vAlign center).
- **Ligne morceau / ligne batch** : pastille ou case à gauche, titre, colonnes
  alignées (chip verdict, format mono, match% à droite). Sélection = overlay
  neutre + barre latérale (`--overlay-selected`), **jamais de couleur
  d'accent**.
- **Carte verdict (ACTUAL)** : bandeau `--color-background-success` + bordure
  1px, headline « Ready to file » + rangée de chips. Padding lg.
- **Hero** : tuile pochette (radius soft, bordure) + titre + artiste + chip
  version + tags.
- **Bande d'audition** : play + waveform (transport/seek) + temps (mono) +
  pill pitch.
- **CTA primaire** : aplat de la couleur d'accent définie dans `styles.css`
  (actuellement **neutre foncé**, plus le bleu de l'ancienne direction —
  voir l'en-tête de ce fichier), texte sur accent (`--color-text-on-accent`),
  radius `md`, hauteur `--h-40`. **Un seul CTA par écran.**
- **Action négative (Discard)** : ghost sémantique danger (texte + bordure +
  fond `--color-*-danger` à faible opacité). Jamais un aplat plein (action
  récupérable).
- **Panneau droit (Détail)** = pile de validation : DESTINATION / FINAL NAME /
  GENRES / FORMAT puis CTA.

## Mode Batch

Deuxième mode de l'écran Review (toggle Detail|Batch). **Groupé par
confiance** (sécurité) :
- **Ready to file (n)** — lossless propre + identifié, multi-sélection,
  « Select all ».
- **Needs review (n)** — FAUX / DUPLICATE / NO MATCH, **cases désactivées**,
  en quarantaine → « open each in Detail ». **Jamais ranger un FAUX en
  masse.**
- Tableau colonnes ☑ / titre / chip verdict / format / match%.
- **Barre d'action** (à coller au bas de la fenêtre) : DESTINATION dossier +
  Discard (n) ghost + Move selection (n) CTA accent.
- **Panneau droit = récap sélection** : SELECTION n / GOING TO / WILL ENCODE /
  EXCLUDED.

## Iconographie

- Un seul jeu : **Tabler** line, trait **1.5px**, 16px (13-14 dans le
  segmented).
- Couleur : suit les tokens texte actif/inactif du thème courant (voir
  `styles.css`), jamais une valeur hex fixe — l'ancien couple de gris hex
  documenté ici avant nettoyage ne survivait pas au thème clair actuel.
- Pas d'icône décorative.

## Chrome fenêtre

- Barre native OFF (`decorations:false`). Custom titlebar (logo haut-gauche,
  contrôles min/max/close flottants haut-droite câblés `getCurrentWindow()`,
  bande de drag via `data-tauri-drag-region`).
- **Tout contrôle interactif dans le topbar (ex. toggle) doit opter-out du
  drag** pour rester cliquable.

## Séparateurs / dégradés

- Dividers verticaux : dégradé en fondu (opacité 0→bordure→bordure→0 aux
  offsets 0/.14/.86/1) — seamless, sur la base du token de bordure courant,
  pas une couleur fixe.
- Haut du board : fondu vertical entre deux surfaces adjacentes du thème
  courant, seamless avec la barre Windows.

## Notes de travail Penpot

- **NE PAS grouper/déplacer en masse dans l'éditeur** : un groupe couvrant 2
  boards se rattache à un seul board → l'autre se vide, la moitié est
  rognée. Faire les déplacements **via le plugin (code)**. Récup d'un tel
  accident : re-trier les enfants par **Y absolu** vers le bon board
  (`appendChild` préserve la position absolue), puis supprimer le groupe
  vide.
- `export_shape` peut renvoyer un **rendu périmé** juste après une édition →
  ré-exporter + croiser avec les coordonnées lues.
- Construire **formes puis icônes** (createShapeFromSvg lent). Texte Penpot
  ne peut pas être vide (`hidden=true`). `text.width`=0 juste après
  `createText` → relayouter dans un 2e appel.

## Interdits (structurels — toujours valides)

- Pas de 2ᵉ accent en dehors du CTA. Pas de couleur décorative.
- Pas d'ombre portée pour la mise en page (surfaces + bordures).
- Pas de texte posé en coin (toujours boîte vAlign center).
- Pas d'icône plus sombre que les labels.
- Pas de noir pur, pas de glow néon, pas d'emoji.
- Pas d'espacement hors grille (voir avertissement Espacement ci-dessus pour
  la grille exacte à jour).
- Pas de bouton destructif rouge/danger plein (ghost seulement).
- **Jamais ranger un FAUX en masse** (batch).
