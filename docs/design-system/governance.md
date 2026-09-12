# Sift Design System - Governance

## Règle Centrale

Le design system de Sift est un système opérationnel, pas une bibliothèque
parallèle. Toute décision durable doit être vérifiée contre l'app réelle.

Sources obligatoires :

1. `frontend/styles.css` pour les tokens ;
2. `docs/design-system-states.md` pour les composants et états ;
3. `docs/skills/sift-ui-design-governance.md` pour le routage skills/agents ;
4. `AGENTS.md` pour les contraintes projet ;
5. `PRODUCT.md` pour la direction produit.

## Référence Externe

Apple HIG pour les macro-décisions desktop, Apple Design Resources pour les guides
couleur amont ; micro-composants via les MCP `shadcn` et `ui-thing`. Ces sources
s'étudient — jamais installées dans `package.json`, jamais une valeur recopiée.

Trois règles de manipulation, chacune née d'une erreur réelle :

- **accès** : les pages HIG sont des SPA. `WebFetch` renvoie une non-réponse qui
  ressemble à un refus du modèle ; passer par le Browser pane.
  `developer.apple.com/design/` est un hall d'entrée sans page "principles" ;
- **transposition** : appliquer le test organe-du-système vs fait-humain de
  `foundations.md` avant de replier une règle. Sift cible Windows *et* macOS ;
- **preuve** : une règle externe ne se replie dans ce dossier **qu'accompagnée de sa
  confrontation à l'app** — fichier:ligne, ou l'absence constatée. Une règle citée sans
  preuve décrit une intention, pas l'artefact.

Inventaire en cours : `docs/superpowers/changes/2026-08-05-hig/`.

## Routage Skills Et Agents

Avant toute tâche substantielle, consulter
`docs/skills/sift-ui-design-governance.md`. Le choix
du skill/agent fait partie du design system parce qu'il détermine la qualité de
la décision. ⚠️ `~/.claude/skills-view.md`, cité ici jusqu'au 2026-08-05 comme
inventaire des skills, **n'existe plus** (supprimé par le reset vanilla du
2026-07-31, récupérable au tag `pre-reset-vanilla`) : s'en tenir aux skills
réellement listées par le harnais.

Routage courant :

- toute tâche UI/UX/design/thème/parcours utilisateur sur Sift :
  `sift-ui-design-governance` en premier, comme garde-fou projet ;
- retouche/polish d'un écran existant : `impeccable` en priorité ;
- audit post-implémentation : `design-review` ;
- nouveau chantier UI significatif : `design-flow` ;
- a11y/WCAG ponctuel : `ui-ux-pro-max` ;
- refactor/legacy : `working-with-legacy-code`, `refactoring-patterns`,
  `clean-code` ;
- Rust/backend : `rust-best-practices`, `error-handling-patterns` ;
  pointu/review → session + `.claude/rules/rust.md` + agent `auditor` ;
- audit dette : `tech-debt-audit`.

Ne pas invoquer sur Sift :

- `design-taste-frontend` ;
- `redesign-existing-projects` ;
- `gpt-taste` ;
- `top-design`.

Si une skill est indisponible dans la session, noter l'indisponibilité et
continuer avec le fallback humain/documentaire, sans inventer une nouvelle
source de vérité.

## Quand Modifier Quel Fichier

| Changement | Fichier à modifier |
|---|---|
| Nouvelle valeur de couleur/spacing/typo | `frontend/styles.css` |
| Nouvel état visuel réel | `docs/design-system-states.md` |
| Nouveau rôle de composant | `docs/design-system/components.md` |
| Nouveau pattern de parcours | `docs/design-system/patterns.md` |
| Nouveau libellé canonique | `docs/design-system/content.md` |
| Nouvelle règle de process | `docs/design-system/governance.md` |
| Nouveau doc sous `docs/` | négation dans `.gitignore` (plus de catalogue) |

## Interdits

- créer un thème parallèle hors `frontend/styles.css` ;
- traiter une maquette HTML comme source de vérité ;
- ajouter une carte pour régler un problème de hiérarchie ;
- dupliquer un warning dans deux zones ;
- utiliser une couleur sémantique pour une catégorie neutre ;
- vérifier une UI Tauri avec une simple preview navigateur si le code vit dans
  le bloc `inTauri`.

## Vérification UI

Pour une modification UI réelle :

1. lire le code qui rend le composant ;
2. modifier les fichiers de production ;
3. lancer `npx tsc --noEmit` si TypeScript touché ;
4. vérifier dans `tauri dev` quand le changement dépend de Tauri/WebView2 ;
5. demander un screenshot utilisateur si la vérification automatique serait trop
   coûteuse ;
6. mettre à jour la doc canonique concernée.

Pour une modification docs-only :

1. vérifier que les docs ne contredisent pas `AGENTS.md` ;
2. ne pas lancer de build inutile.

## Définition De Fini

Un changement de design system est fini quand :

- la source de vérité réelle est modifiée ou référencée ;
- la doc correspond au comportement réel ;
- le routage skill/agent reste explicite ;
- tout nouveau document sous `docs/` a sa négation dans `.gitignore` (⚠️ il n'y
  a plus de catalogue : `docs/INDEX.json` n'existe plus depuis le 2026-08-05) ;
- les limites de vérification sont dites clairement.
