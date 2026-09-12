# Chantier identité visuelle — design

Date d'ouverture : 2026-08-05. **État : ébauche, avant accord L2.** Aucune valeur proposée,
délibérément.

## Pourquoi ce chantier existe, et pourquoi il est SÉPARÉ du précédent

Deux chantiers l'ont précédé le même jour, et leur échec est l'argument :

1. **`2026-08-05-hig/`** — conformité aux Apple HIG. 7 écarts mesurés, 5 corrigés. Antoine
   n'a **rien vu changer à l'écran**, et il avait raison : tous les correctifs étaient
   sous-pixel, conditionnels ou transitoires.
2. **`2026-08-05-design-apple/`** — mesure et composition. Deux propositions de mise en page,
   **deux rejets** : la « colonne bornée » écartée par un fait d'usage (la largeur de la
   waveform sert à lire la structure d'un morceau, `MAX_PEAKS = 4000` le chiffre), et la
   waveform haute rejetée à l'œil malgré le fait qu'elle servait l'usage déclaré.

Ce second rejet est le plus instructif du lot : **une surface plus utile n'est pas une
surface plus belle.** La démo faisait exactement ce qu'Antoine avait décrit vouloir lire, et
il a répondu « révoque, c'est pire ». Ce qui manque n'est donc pas de l'utilité ni de la
mesure.

`docs/skills/sift-ui-design-governance.md` § Lexical Granularity nomme ce moment : deux
correctifs visuels ratés d'affilée sur la même surface veulent dire que l'accord n'a jamais
eu lieu — **remonter d'un cran au lieu de préciser**. Le cran au-dessus de la mesure, c'est
l'identité.

Ce que dit Antoine, et qui est L1 : « on n'a aucune homogénéité, c'est moche, on n'a pas une
impression de fluidité continue ».

## Ce qui est mesuré — l'app est PLATE dans toutes ses dimensions à la fois

Toutes les valeurs ci-dessous sont relevées dans la vraie fenêtre `tauri dev`, dans le thème
réellement actif chez Antoine : **sombre système**, aucun attribut `data-theme` posé.

### Couleur et élévation

| rôle | valeur résolue | constat |
|---|---|---|
| fond primaire | `oklch(27.57% 0.009 77.5)` | |
| fond tertiaire | `oklch(31.64% 0.009 77.5)` | |
| fond secondaire | `oklch(34.77% 0.011 77.5)` | **7 points de clarté** pour les trois niveaux d'élévation réunis |
| texte primaire | `oklch(95.9% 0.0115 77.5)` | |
| texte secondaire | `oklch(83.5% 0.0171 77.5)` | |
| texte tertiaire | `oklch(81.92% 0.0148 77.5)` | **1,6 point** d'écart avec le secondaire — deux rôles nommés, une seule valeur à l'œil |
| bordure tertiaire | `oklch(100% 0 89.88 / 0.09)` | blanc à 9 % |

Chroma de toute la palette neutre : **0,009 à 0,017**. C'est du gris, à un souffle de chaud
près (teinte 77,5). Choix assumé à l'origine (« outil pro », refonte de juillet) — mais c'est
un fait à remettre sur la table, pas un acquis.

### Taille et rythme (repris du chantier précédent, mesures d'écran)

- **80 %** des nœuds de texte peints sont sous 13 px — la taille par défaut macOS, que l'app
  possède et n'emploie que 19 fois sur 289 occurrences source ;
- `--text-2xl` (26 px) est peint **une seule fois dans toute l'application** ;
- **66 %** de l'espacement passe par deux paliers (4 et 8 px) ; les deux larges sont morts ;
- deux tailles **hors échelle** contournent les tokens : 18 px (mot-symbole) et 12,5 px
  (libellés du rail).

### Le constat qui unifie les trois

L'app est plate **en taille, en espacement, en clarté et en couleur, simultanément**. Aucun
de ces axes ne porte de contraste. « Tout ressemble à tout » n'est pas une impression : c'est
la somme de quatre distributions écrasées.

C'est aussi pourquoi corriger un seul axe n'a rien donné. Le chantier HIG a bougé des pixels,
le chantier mesure a bougé des largeurs : dans les deux cas les trois autres axes sont restés
plats et l'écran n'a pas changé de caractère.

## ACCORD L2 — tranché par Antoine le 2026-08-05

**Caractère : un UTILITAIRE.** Réponse littérale : « app dj mais ça n'a rien de ludique, c'est
un UTILITAIRE ». À lire avec l'autre chose qu'il dit — que c'est moche. Les deux ensemble
donnent la contrainte réelle : **utilitaire ne veut pas dire sans caractère**. Ce qui est
refusé est le ludique, le décoratif, le clin d'œil DJ. Ce qui est demandé reste une app dont
on a envie de se servir. Un utilitaire beau tire sa beauté de sa **structure**, pas de son
ornement — c'est la famille d'un Transmit, d'un TablePlus, d'un Fork, pas celle d'un lecteur
grand public.

**Axes de contraste : clarté, taille, espace — et PAS la couleur.** Réponse : « Clarté taille
et espace sont tous très importants ». Le silence sur la couleur est un choix, pas un oubli :
ce sont exactement les trois axes **achromatiques et structurels**. La couleur reste
**sémantique et rien d'autre** — vert lossless, ambre doute, rouge danger, bleu interactif —
au lieu de devenir un moyen d'expression.

Cet accord est cohérent avec le caractère : dans un utilitaire, la hiérarchie se porte par la
structure, et la couleur ne sert qu'à signifier. Il explique aussi l'état actuel — l'app n'a
**ni** structure (les trois axes mesurés plats) **ni** couleur (chroma 0,009). Elle n'a donc
aucun registre pour se faire comprendre du regard.

### Ce que l'accord autorise, concrètement

- **Clarté** — élargir les paliers d'élévation, aujourd'hui écrasés sur 7 points de L. Une
  carte doit se détacher de son fond sans bordure ni ombre pour compenser.
- **Taille** — cesser de faire porter 80 % du texte par trois crans sous la ligne de base
  macOS. L'échelle existe (10/11/12/13/14/16/26), elle n'est simplement pas employée : `13 px`
  est le défaut système et l'app l'utilise 19 fois sur 289.
- **Espace** — rendre vivants `--space-24` et `--space-32`, employés 12 et 4 fois, pour
  séparer les sections au lieu de tout empiler à 4 et 8 px.

### Ce que l'accord interdit

- Toucher aux teintes sémantiques pour « faire joli ».
- Ajouter de l'ornement, une texture, un dégradé décoratif, une icône expressive.
- Compenser un manque de hiérarchie par une bordure ou une ombre — la règle existe déjà dans
  `docs/design-system/patterns.md` et elle est confirmée par cet accord.

## Ce qu'il fallait trancher — questions L2, pas L3

Aucune valeur ne doit être proposée avant que celles-ci aient une réponse.

1. **Quel caractère ?** « Outil pro discret » est le choix actuel, jamais rediscuté depuis la
   refonte de juillet. Est-ce toujours ce qu'Antoine veut, ou veut-il quelque chose qui ait
   une signature ? Une app de DJ n'a pas les mêmes conventions qu'un utilitaire système.
2. **Quel axe porte le contraste ?** On ne peut pas les remonter tous les quatre sans faire
   du bruit. Lequel devient l'axe principal — la clarté (élévation franche), la taille
   (hiérarchie typographique), la couleur (accents sémantiques plus présents), ou l'espace
   (respiration) ?
3. **La tension matériaux**, écrite dans `docs/design-system/patterns.md:40` et jamais
   tranchée : HIG Materials veut une couche de contrôles visuellement distincte du contenu,
   Sift a choisi la surface continue. Ce chantier-ci la pose enfin pour de bon, contrairement
   au précédent.
4. **Une référence, laquelle ?** Antoine a fourni l'UI Kit macOS 27
   (`https://www.sketch.com/s/57153a31-3379-4737-8ac6-dbfd6525f052`). Il donne des cotes et
   des structures. Il ne donne PAS un caractère — Apple a le sien, Sift doit avoir le sien.

## Contraintes qui s'appliquent

- **Étudier, pas importer.** `CLAUDE.md` interdit de copier la palette du kit ou de déplacer
  les tokens hors de `frontend/styles.css`. Le bouton « Export Design Tokens » du kit fait
  exactement ce qui est proscrit.
- **`frontend/styles.css` reste le canonique unique.** Toute édition de token doit rester
  cohérente dans `:root`, le bloc `prefers-color-scheme` **et** `:root[data-theme="dark"]`,
  et se vérifier sur les valeurs **résolues** dans les deux thèmes, pas sur les noms.
- **Les pages HIG se lisent par le Browser pane**, jamais par `WebFetch`.
- Tout état nouveau demande **sa story Storybook**.
- ⚠️ **Licence des Apple Design Resources non vérifiée**, et Sift cible aussi Windows. À
  trancher avant qu'un élément dérivé parte en release.

## Ce que ce chantier ne fera pas

- Il ne refait pas la mise en page : c'est `2026-08-05-design-apple/`, et sa règle de mesure
  par nature de surface reste valable même si ses deux propositions ont été rejetées.
- Il ne touche pas au jargon anglais volontairement conservé (LOSSLESS, DUPLICATE, MATCH,
  FAKE, kbps…).
