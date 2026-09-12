# Sift Design System - Content

## Voix

Sift parle comme un outil de travail : sobre, précis, utile. Les textes doivent
être courts, mais pas cryptiques.

Ton :

- direct ;
- technique quand c'est nécessaire ;
- jamais marketing ;
- jamais paternaliste ;
- jamais ludique pour une erreur ou une action destructive.

## Vocabulaire Canonique

| Concept | Libellé recommandé |
|---|---|
| Écran de décision | Revue |
| Liste de traitement | File |
| Analyse audio | Diagnostic audio |
| Identification/tags | Métadonnées |
| Choix de dossier | Destination |
| Format de sortie | Format |
| Nom calculé | Nom final |
| Action principale | Convertir |
| Rejet | Écarter |
| État prêt | Prêt à ranger |
| État incomplet | À finaliser |
| Revérification | Rechercher à nouveau |
| Tags écrits | Appliquer les tags |
| Résultat Discogs | Match |

## États

Libellés courts recommandés :

- Prêt à ranger
- À finaliser
- À vérifier
- Métadonnées fiables
- Diagnostic OK
- Sur-encodé
- Faux lossless probable
- CDJ compatible
- CDJ incompatible
- Destination manquante

Règle : un état doit dire ce que l'utilisateur peut faire maintenant, pas
seulement nommer un résultat technique.

## Actions

Verbes préférés :

- Convertir
- Écarter
- Rechercher
- Appliquer
- Choisir
- Ouvrir
- Annuler

Note : "Convertir" est le libellé du bouton d'action principale (remplace
"Ranger" le 2026-07-10, retour utilisateur — rendu à `frontend/filing.ts:95`). Le concept
produit reste "déplacer = encoder + ranger" (CLAUDE.md) ; ce n'est plus le
libellé affiché. "Écarter" remplace "Jeter" (même date — commentaire d'origine
`filing.ts:169`, markup `filing.ts:175`).

⚠️ Les deux citations précédentes (`filing.ts:220` et `filing.ts:717`) étaient fausses et
ont été corrigées le 2026-08-05 : `filing.ts` ne compte que 650 lignes, et la ligne 220
est la fermeture d'un handler sans rapport.

Éviter :

- Valider, trop vague ;
- Confirmer, sauf dans une confirmation explicite ;
- Sauvegarder, si l'action encode et déplace ;
- Exporter, sauf pour un vrai fichier de sortie externe.

## Microcopy

La microcopy doit enlever une ambiguïté, pas expliquer l'interface.

Bon usage :

- "Choisis une destination pour convertir"
- "Nom final"
- "Destination manquante"

À éviter :

- longues explications de fonctionnement ;
- répétition d'un warning déjà visible ;
- texte pédagogique permanent dans une zone expert ;
- instructions clavier trop présentes.

## Données Techniques

Pour les valeurs audio et fichier :

- garder les unités visibles ;
- utiliser la police mono pour valeurs tabulaires ;
- aligner les paires label/valeur ;
- éviter les phrases quand une valeur suffit.

Exemples :

- `44 100 Hz`
- `22.1 kHz`
- `0.00 dBTP`
- `AIFF`
- `lossless`

## Tutoiement

L'app **tutoie** partout : « Choisis une destination », « Sélectionne un dossier »,
« Les pistes que **tu** écartes ». Ce n'est écrit nulle part jusqu'ici, donc rien ne
l'empêchait de dériver — et ça a dérivé : `frontend/empty-state.stories.ts:37` vouvoie
(« **Ajoutez** des sources dans Réglages pour commencer. »), et se trompe en plus
d'écran, les sources s'ajoutant depuis Accueil. Storybook étant le miroir vivant des
états, une story fausse est une source de vérité fausse.

Règle : tutoiement, sans exception, y compris dans les stories.

## Langue

L'interface est en français. Les formats, noms de fichiers, genres et sources
externes gardent leur casse et leur langue d'origine.

Éviter le franglais de contrôle quand un terme français clair existe. Garder
"Discogs", "CDJ", "ID3", "AIFF", "WAV", "MP3" tels quels.

Liste complète du jargon conservé, alignée sur `CLAUDE.md` : LOSSLESS, DUPLICATE, MATCH,
CHECK MATCH, FAKE, kbps, kHz, MP3, AIFF, WAV. Ne pas le "corriger".

**Divergence assumée vis-à-vis des HIG.** HIG Writing demande une langue simple et
prescrit d'éviter le jargon. Sift le garde parce que ce n'en est pas au sens visé : ce
n'est pas du vocabulaire d'implémentation qui fuit vers l'utilisateur, c'est le
vocabulaire professionnel de sa cible. Le traduire dégraderait la reconnaissance au lieu
de l'améliorer. La règle HIG suppose un public général ; Sift a un public spécialiste.

## Apports HIG Writing

Les HIG rejoignent ce qui précède sur trois points, qui valent d'être nommés :

- **la voix se décide, puis se tient.** Établir un vocabulaire commun et y revenir — le
  tableau § Vocabulaire Canonique est exactement cet instrument ;
- **le ton suit le contexte.** Même voix, registre différent selon que l'utilisateur
  réussit une action ou en rate une. Correspond à l'interdit projet du ton ludique sur
  une erreur ou une action destructive ;
- **un libellé de bouton est un verbe.** Déjà appliqué (§ Actions). Corollaire HIG :
  privilégier la clarté sur l'esprit — "Convertir" bat toute formule trouvée.

Un quatrième point n'est pas encore acquis : les HIG rappellent d'écrire aussi pour les
lecteurs d'écran, et donc de ne pas laisser un libellé ne prendre son sens que de sa
position ou de sa couleur.
