# Sift — Design System

Palette, typo et composants de la direction visuelle de Sift. Réutilisable tel quel pour tout
nouvel écran ou nouveau projet Sift.

## Palette — mode clair (défaut)
- Canvas : `#E7E2DB`
- Nav : `#EDE9E2`
- Queue : `#EAE5DE`
- Cartes / contrôles : `#F1EDE7`
- Surface (boutons/popovers élevés) : `#FBF9F4`
- Track (fond des toggles/segmented controls) : `#E0DAD1`
- Ligne active (row hover/focus) : `#F3EFE9`
- Nav item actif : `#E1DBD2`
- Texte primaire : `#34302B`
- Texte secondaire : `#5C554E`
- Texte tertiaire : `#8A857D`
- Texte quaternaire (micro-labels) : `#B3AEA5`
- Bordure fine : `rgba(40,34,28,0.09)`
- Bordure forte : `rgba(40,34,28,0.16)`
- CTA primaire : fond `#3A352F`, texte `#F7F4EF`
- Désactivé : `#CFC9BF`

## Palette — mode sombre
Même famille grise chaude, inversée. Suit `prefers-color-scheme`, override manuel Auto/Clair/
Sombre, persiste en localStorage (`sift-theme`).
- Canvas : `#282825`
- Nav : `#323230`
- Queue : `#2E2E2B`
- Cartes / contrôles : `#3B3A35`
- Surface : `#46453F`
- Ligne active : `#413F38`
- Nav item actif : `#3D3B35`
- Texte primaire : `#F5F1E9`
- Texte secondaire : `#C9C2B7`
- Texte tertiaire : `#9C968D`
- Texte quaternaire : `#847E75`
- Bordure fine : `rgba(255,255,255,0.12)`
- Bordure forte : `rgba(255,255,255,0.22)`
- CTA primaire : fond `#F5F1E9`, texte `#26251F` (inversion — neutre clair sur fond sombre)
- Désactivé : `#57554D`

## Couleur sémantique (identique dans les deux modes)
- Vert (OK / lossless) : `#4C7B57` (fond/dot), texte `#3f6d4c` clair / `#9fe0af` sombre
- Ambre (doute / erreur / attention) : `#B07A28` (fond/dot), texte `#8f6318` clair / `#f2c274` sombre
- **Règle stricte : 2 couleurs sémantiques seulement.** Pas de bleu, pas de rouge, pas de 3e
  teinte — le gris neutre sert pour "en cours" (ce n'est pas un jugement). Ajouter une couleur
  dilue le principe "couleur = sens uniquement".

## Typographie
- UI : **Outfit** (300–700)
- Chiffres, chemins de fichiers, hints clavier : **JetBrains Mono** (400–500)
- Tailles courantes : titres de carte 16px, corps 13–14px, micro-labels uppercase 9–10px
  (letter-spacing ~0.06–0.11em, couleur texte quaternaire)

## Grammaire de layout
3 colonnes fixes, partagées par tous les écrans (Accueil, Revue, Écartés, Journal, Bibliothèque) :
- **Nav** — 152px, fixe
- **Queue** — 272px, fixe (liste de la colonne du milieu : pistes / journal / sources / TOC réglages)
- **Inspecteur** — flexible, contenu de l'écran actif

Pas d'ombres portées, sauf : pochette (élévation ponctuelle) et popovers/palette de commandes
(nécessaire pour la lisibilité d'un calque flottant).

## Composants
- **Carte** : `background:var(--card); border:1px solid var(--border); border-radius:11px;
  padding:14-18px`
- **Bouton bordé (secondaire / "ouvre un menu")** : `padding:4-6px 10-13px; border-radius:6-7px;
  border:1px solid var(--borderStrong); background:var(--surface)`. Utilisé pour tout ce qui ouvre
  un popover/éditeur (Modifier, Changer…, Destination) — jamais un simple lien texte souligné pour
  ces cas-là.
- **CTA primaire (pill pleine largeur du rail)** : `background:var(--ctaBg); color:var(--ctaText);
  border-radius:8px; font-weight:600`
- **Pastille clavier** (raccourcis) : petit rectangle mono `border:1px solid var(--borderStrong);
  background:var(--surface); border-radius:5px`, suivi du mot en toutes lettres (Espace, Entrée,
  Suppr) — jamais de glyphes bruts (␣ ↵ ⌫) seuls.
- **Chip de statut** (LOSSLESS/FAKE/DUPLICATE) : pilule pleine `border-radius:999px`, fond teinté
  ~30-45% d'opacité de la couleur sémantique, bordure ~0.6-0.85 d'opacité, texte theme-aware
  (`semGreen()`/`semAmber()`) — jamais un fond gris "washed out".
- **Tag genre** : petite pilule discrète `background:var(--track); color:var(--text2)` — chaque
  genre est un tag séparé, jamais une string concaténée (prépare le tri auto par genre).
- **Slider custom** (Volume/Tempo) : jamais de `<input type=range>` natif (ne peut pas être stylé
  au-delà de `accent-color`). Piste fine 3px + poignée ronde 13px en divs, draggable au clic +
  mousedown/mousemove. Volume : fill depuis la gauche, pas de %. Tempo : fill depuis le centre
  (0 = neutre), double-clic pour revenir à 0%.
- **État vide** : aligné en haut (jamais centré verticalement), titre + note, lien "Aller à X →"
  seulement si l'écran est un vrai cul-de-sac (pas sur l'écran d'entrée).
- **Feedback hover** : quasi tout élément cliquable a un `style-hover` (fond `var(--rowActive)`
  pour les lignes/listes, `filter:brightness(0.93-0.95)` pour les boutons pleins/chips).

## Ce qui est explicitement hors système
- Pas d'emoji (sauf 📁 utilitaire dans le picker de dossier — décoratif minimal, pas un ton
  "brand").
- Pas de dégradés décoratifs, pas de glassmorphism.
- Jargon technique gardé en anglais partout : LOSSLESS, DUPLICATE, MATCH/CHECK MATCH, FAKE,
  kbps, kHz. Tout le reste (labels, boutons, messages, toasts) en français.
