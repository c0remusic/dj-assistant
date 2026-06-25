# Sift — Interface Design System

> Source de vérité de l'UI Sift (Tauri v2 + front web vanilla, Windows d'abord).
> Direction travaillée dans Penpot. Boards canoniques (page « Sift — shell ») :
> **Détail = « Revue · plat »** `49975f37-649c-80c0-8008-39eb475e8b73` (x5500),
> **Batch = « Revue · batch »** `284acdb7-967e-8038-8008-3a1f415c4596` (x5500/y820).
> UI **en anglais**. Convergence partielle faite côté front réel (2026-06-25) :
> tokens **couleur + radius** appliqués au CSS, libellés **migrés en anglais**, clavier
> **↑/↓ + ⌫** câblé (cf. interaction-model.md §7). Restent non alignés : espacement
> (px hors grille), typographie (Outfit/JetBrains pas chargées), layout Détail/Batch.

## Direction & ressenti
« La table du digger », mais **outil de DJ : le son passe avant l'image**. Interface sombre,
chaude, retenue et premium pour préparer sa musique (analyse, dédoublonnage, identification,
rangement). La coque chuchote ; le **verdict** et l'**action** mènent ; la **pré-écoute précède**
les autres étapes. Température : **gris neutre chaud**. Densité équilibrée (4/10). Motion retenue
(décélération, pas de spring/bounce).

## Ordre de l'écran Détail (son d'abord)
chemin (fil d'Ariane) → **hero** (pochette + titre + artiste + version) → **audition** (play +
waveform-transport + temps + pitch) → **CLAIMED** (ce que le fichier prétend : FLAC/kbps/kHz) →
**ACTUAL** (bandeau verdict « Ready to file » + chips) → **IDENTIFICATION Discogs**.
Le **spectre (« ▸ Proof »)** est en **divulgation progressive** sous le bandeau verdict.

### CLAIMED vs ACTUAL
On montre **ce que le fichier prétend être** (métadonnées déclarées) PUIS **ce qu'il est vraiment**
(verdict de l'analyse). C'est le cœur de Sift (détection faux-lossless).

### Chips verdict = interactives (à câbler)
Les chips `LOSSLESS` / `92% MATCH` / `NO DUP` ne sont pas des étiquettes mortes : **chacune ouvre
sa preuve** dans un tiroir inline sous le bandeau. LOSSLESS→spectre (chute à ~22 kHz) ;
92% MATCH→détail du score Discogs ; NO DUP→résultat dédup (devient **DUPLICATE** ambre + carte de
comparaison quand il y a doublon). Résout la zone vide + respecte la divulgation progressive.

## Profondeur : SURFACES + BORDURES (committé)
Pas d'ombre portée pour la mise en page. Élévation = paliers de clarté de surface + bordures
1px basse-opacité.
- Surfaces empilées : bg → surf-1 (cartes) → surf-2 (chips/pochette).
- Sidebar = **même fond que le canvas**, séparée par un filet (pas une autre couleur).
- Inputs/wells : **plus sombres** que l'entourage (inset).
- Seule tolérance d'ombre : une ombre douce sur la pochette. Tout le reste = bordures.

## Alignement & centrage — RÈGLE CRAFT #1
Cause racine des désalignements : un texte posé en `x,y` se cale en **coin haut-gauche**, jamais
centré dans sa bande. **Tout texte dans une ligne / bande / chip / bouton se construit dans une
boîte de hauteur fixe** (`growType="fixed"` + `resize(w, bandH)`) avec **`verticalAlign:"center"`**.
- **Colonnes sur x fixes** : titres `align:"left"`, valeurs numériques `align:"right"`.
- Cases/chips/icônes **centrées verticalement** dans la bande (y = bandTop + (bandH−elem)/2).
- Penpot supporte `verticalAlign` (top/center/bottom) ET `align` (left/center/right) — vérifié.
- Rythme de ligne constant (bande 34px), gaps de section homogènes.

## Tokens couleur (dark)
- `bg` **#2c2c2a** — canvas + rail + zone haute (gris chaud)
- `surf-1` **#313130** — cartes (idcard, lignes candidats), panneau surélevé
- `surf-2` **#3b3b39** — chips, tuile pochette, bouton play, pill segmented actif
- `inset` **#242422** — well du segmented, fonds d'input
- `text-hi` **#f3f3f1** · `text` **#c9c8c1** · `text-faint` **#aeada5** · `eyebrow` **#a5a49c** (remonté a11y)
- `border` **rgba(255,255,255,.06)** · `border-strong` **rgba(255,255,255,.10)** (pochette/emphase)
- `accent` **#3b7df0** — LA couleur d'action, réservée au **SEUL CTA** (variante AA approfondie #2f6fe0)
- `ok` **#5bc08c** · `faux` **#e2685e** · `doute` **#dda63f** — sémantique verdict
- `logo` **#F0EDE6** — crème (mark + wordmark), blanc cassé chaud

Règle : **un seul accent** (bleu), réservé à l'action. Marque en crème. Couleur sémantique pour le
verdict/statut uniquement. Aucune couleur décorative.

## Espacement — tokens Sift (set Penpot « System »)
Échelle UNIQUE : `xs 4` · `sm 8` · `md 12` · `lg 16` · `xl 24` · `xxl 32`. Toute autre valeur interdite.
**Hiérarchie de profondeur** (la nidification se lit par l'espace) :
section↔section **24 (xl)** > padding carte **16 (lg)** > groupe eyebrow→valeur / chip↔chip **12 (md)** >
interne control icône↔label **8 (sm)**. Sift = **densité d'abord** (la liste Queue reste dense, scrollable).

## Radius — tokens Sift
`sharp` **4** · `default` **6** (boutons, inputs, controls) · `soft` **10** (cartes, panneaux) · `pill` **999** (badges, segmented, tags).
(Ancien 8/12 reconverti : boutons/controls → 6, cartes → 10.)

## Hauteurs (inputs / boutons / dropdowns)
`compact` **32** (lignes/tables) · `default` **36** · `comfortable` **40** (action principale, dropdowns) · `large` **44**.

## Dropdown / select
État fermé = control **h40** (comfortable), radius **6**, fond inset `#242422` + filet 1px, padding horizontal **16**,
chevron à droite (16 du bord). Overlay à l'ouverture (ne pousse pas le layout). Détail comportemental : `interaction-model.md` §8.

## Lexique (termes canoniques, anglais, persistants)
Acte central = **File** (CTA « File & encode » / « File selection » ; « Ready to file », « Undo filing »).
Verdicts : **LOSSLESS** · **LOSSY** (faux-lossless, jamais « FAUX ») · **DUPLICATE** · **UNIQUE** (pas « NO DUP ») · **NO MATCH**.
Destination = **DESTINATION** partout (jamais « GOING TO »). Source de vérité comportementale : `interaction-model.md`.

## Typographie
- Familles : **Outfit** (UI, 600 titres/labels, 400 corps) + **JetBrains Mono** (chiffres).
- **Mono FAIT** : tous les chiffres techniques (kbps, kHz, %, pitch, temps, match%) en JetBrains Mono.
- Échelle : track title 30/600 · artiste 17 · headline verdict 16/600 vert · corps 13 · eyebrow 11
  capitales · badge/chip 10-11.
- **Règle build** : tout texte créé reçoit `Outfit.applyToText` OU `JetBrainsMono.applyToText`
  (sinon Penpot retombe sur « sourcesanspro »).

## Composants
- **Segmented** (Queue/Discarded/Trashed ET toggle Detail|Batch) : well inset `#242422` + **pill
  actif** `#3b3b39`, label actif #f3f3f1 / inactif #aeada5, icône actif #f3f3f1 / inactif #a8a79f.
  Texte centré (verticalAlign center). Le **toggle Detail|Batch** va en **haut de la col2** (Queue).
- **Badge/chip statut** : pill 11px ; sémantique = teinte@14-15% + texte sémantique ; neutre = blanc@.06
  + #c9c8c1. Texte centré (boîte vAlign center).
- **Ligne morceau / ligne batch** : pastille ou case à gauche, titre 13/500, colonnes alignées
  (chip verdict, format mono, match% à droite). Sélection = blanc@.045 + (file) barre blanche, jamais bleu.
- **Carte verdict (ACTUAL)** : bandeau vert #5bc08c@14% + bordure 1px, headline « Ready to file » + rangée
  de chips. Padding 16.
- **Hero** : tuile pochette (radius **10** soft, bordure @.10) + titre 30 + artiste 17 + chip version + tags.
- **Bande d'audition** : play + waveform (transport/seek) + temps (mono) + pill pitch (neutre, bleu si ≠0).
- **CTA primaire** : aplat bleu #3b7df0, texte blanc, radius **6** (default), hauteur **40** (comfortable). Le SEUL bleu.
- **Action négative (Discard)** : rouge **ghost** (texte #ef8b81 + bordure #e2685e@.45 + fond #e2685e@.09).
  Jamais un aplat rouge plein (action récupérable).
- **Panneau droit (Détail)** = pile de validation : DESTINATION / FINAL NAME / GENRES / FORMAT puis CTA.

## Mode Batch
Deuxième mode de l'écran Review (toggle Detail|Batch). **Groupé par confiance** (sécurité) :
- **Ready to file (n)** — lossless propre + identifié, multi-sélection, « Select all ».
- **Needs review (n)** — FAUX / DUPLICATE / NO MATCH, **cases désactivées**, en quarantaine →
  « open each in Detail ». **Jamais ranger un FAUX en masse.**
- Tableau colonnes ☑ / titre / chip verdict / format / match%.
- **Barre d'action** (à coller au bas de la fenêtre) : DESTINATION dossier + Discard (n) ghost +
  Move selection (n) CTA bleu.
- **Panneau droit = récap sélection** : SELECTION n / GOING TO / WILL ENCODE / EXCLUDED.

## Iconographie
- Un seul jeu : **Tabler** line, trait **1.5px**, 16px (13-14 dans le segmented).
- Couleur : **#a8a79f** (inactif), **#f3f3f1** (actif). Jamais #6e6e68.
- Pas d'icône décorative.

## Chrome fenêtre
- Barre native OFF (`decorations:false`). Custom titlebar (logo haut-gauche, contrôles min/max/close
  flottants haut-droite câblés `getCurrentWindow()`, bande de drag via `data-tauri-drag-region`).
- **Tout contrôle interactif dans le topbar (ex. toggle) doit opter-out du drag** pour rester cliquable.

## Séparateurs / dégradés
- Dividers verticaux : dégradé blanc en fondu (opacité 0→.10→.10→0 aux offsets 0/.14/.86/1) — seamless.
- Haut du board : fondu vertical #242422→#2c2c2a (seamless avec la barre Windows).

## Notes de travail Penpot
- **NE PAS grouper/déplacer en masse dans l'éditeur** : un groupe couvrant 2 boards se rattache à un
  seul board → l'autre se vide, la moitié est rognée. Faire les déplacements **via le plugin (code)**.
  Récup d'un tel accident : re-trier les enfants par **Y absolu** vers le bon board (`appendChild`
  préserve la position absolue), puis supprimer le groupe vide.
- `export_shape` peut renvoyer un **rendu périmé** juste après une édition → ré-exporter + croiser
  avec les coordonnées lues.
- Construire **formes puis icônes** (createShapeFromSvg lent). Texte Penpot ne peut pas être vide
  (`hidden=true`). `text.width`=0 juste après `createText` → relayouter dans un 2e appel.

## Interdits
- Pas de bleu hors CTA. Pas de 2e accent. Pas de couleur décorative.
- Pas d'ombre portée pour la mise en page (surfaces + bordures).
- Pas de texte posé en coin (toujours boîte vAlign center).
- Pas d'icône plus sombre que les labels. Pas de noir pur, pas de glow néon, pas d'emoji.
- Pas d'espacement hors grille 4px. Pas de bouton destructif rouge plein (ghost seulement).
- **Jamais ranger un FAUX en masse** (batch).
