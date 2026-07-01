# Product

## Register

product

## Users

DJs professionnels et amateurs sérieux qui préparent leur set avant un gig.
Contexte d'usage : à la maison, en amont d'un événement, souvent avec un gros
volume de morceaux à trier (centaines à plusieurs milliers). Pas d'usage en
direct sur scène — c'est un outil de préparation épisodique, pas un outil
temps réel. L'utilisateur connaît son métier (formats audio, structure de
bibliothèque, matériel CDJ) et n'a pas besoin d'être materné par l'interface.

## Product Purpose

Sift prépare une bibliothèque musicale pour DJ : analyse (détection de faux
lossless, BPM, empreinte), identification (métadonnées Discogs), et rangement
(renommage + classement dans une hiérarchie de dossiers). Principe : « déplacer
= encoder + ranger », en un seul geste par morceau. Le succès se mesure à un
seul moment de vérité : que ça marche quand on branche sa clé au club (le
fichier est dans le bon format, bien nommé, lisible par le matériel CDJ).

## Brand Personality

Pro, précis, sobre — avec de petits détails soignés (micro-interactions,
finitions visuelles) sans en faire trop. L'eye candy est un supplément
discret, jamais la couche principale. La densité d'information prime sur la
décoration : l'app traite des centaines de morceaux à la fois, l'interface
doit rester lisible et rapide à scanner, pas spectaculaire.

## Anti-references

Apps grand public colorées et ludiques (gamification, mascottes, couleurs
vives, animations décoratives permanentes) — Sift n'est pas un jouet. Éviter
aussi l'écueil inverse : un look « logiciel pro complexe et austère » façon
suite d'édition professionnelle surchargée de menus. L'objectif est entre les
deux : dense mais clair, pro mais pas froid.

## Design Principles

- Réemployer l'existant avant d'inventer : tokens, classes (`col-h`,
  `inputCss`, `chip`), grammaire visuelle déjà posée dans le détail avant de
  styliser une nouvelle zone (batch, journal, etc.).
- Densité avant décoration : une ligne par morceau, pas de cartes, jamais au
  prix de la clarté pour des centaines/milliers d'items à la fois.
- Ne jamais mentir à l'écran : si une destination ou un nom final n'est pas
  une vérité unique (ex. mode "sur place"), montrer la règle plutôt
  qu'inventer une valeur agrégée fausse.
- Détective avant retouche : comprendre pourquoi un élément visuel existe
  avant de le changer (cf. méthode globale du projet, CLAUDE.md).
- Petits détails avec intention, jamais en rafale : un micro-détail soigné
  par endroit (transition, état hover) vaut mieux que de la décoration
  généralisée.

## Accessibility & Inclusion

Pas d'exigence WCAG formelle pour l'instant (app personnelle / niche DJ, pas
de contrainte réglementaire). Discipline de contraste de fait via les tokens
de palette existants (`--color-text-tertiary`, `--color-border-tertiary`,
etc.) — à maintenir par convention plutôt que par règle d'audit formelle.
