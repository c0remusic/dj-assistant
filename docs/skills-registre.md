# Registre des skills / agents / plugins disponibles

> Consulté avant d'invoquer un outil, pour éviter d'en référencer un qui n'existe
> pas ou de manquer celui qui était pertinent. Mis à jour à la main quand un
> outil est ajouté/retiré. Recensé le 2026-06-30. **Mis à jour 2026-06-30 soir** :
> ajout skills Google Stitch (7 skills) + designer-skills Oczkowski (8 skills).

Trois portées : **projet** (ce repo uniquement, `.claude/`), **global**
(tous les projets, `~/.claude/`), **plugin** (global, packagé, peut contenir
plusieurs sous-skills).

Deux modes d'invocation : **auto** (Claude Code la charge seul si la description
matche la tâche) vs **manuel** (commande explicite `/nom`, ne se déclenche jamais
seul — noté `disable-model-invocation: true` dans le SKILL.md).

---

## Méthode / développement (à utiliser sur Sift)

| Nom | Portée | Invocation | Usage |
|---|---|---|---|
| `sift` | global | auto (mots-clés "sift", "dj-assistant") | Charge le contexte projet en début de session. Point d'entrée. |
| `rust-best-practices` | projet | auto | Tout code Rust écrit/revu (ownership, erreurs, clippy, tests). |
| `error-handling-patterns` | projet | auto | Erreurs Rust/Tauri — Result + serde IPC, fail-fast, retry réservé Discogs/AcoustID. |
| `release-skills` | projet | auto | Release / bump version — 3 fichiers en synchro, depuis `main`. |
| `rust-engineer` (agent) | projet | via outil Agent | Rust pointu (async/perf/unsafe). Relativiser checklist MIRI/criterion. |
| `architect` (agent) | global | via outil Agent | Design d'archi avant gros refactor / nouvelle feature. |
| `rust-analyzer-lsp` (plugin) | global | auto (édition fichiers .rs) | Diagnostics rust-analyzer + hooks rustfmt/clippy/cargo-check. |
| `superpowers` (plugin, 12 sous-skills) | global | mixte — `using-superpowers` route les autres | Méta-cadre méthode : `writing-plans`, `executing-plans`, `systematic-debugging`, `test-driven-development`, `verification-before-completion`, `requesting-code-review`/`receiving-code-review`, `using-git-worktrees`, `subagent-driven-development`, `dispatching-parallel-agents`, `finishing-a-development-branch`, `brainstorming`, `writing-skills`. C'est ce qu'on invoque par "utilise superpowers pour planifier". |
| `tech-debt-audit` | global | **manuel** `/tech-debt-audit` | Audit de dette complet (Rust + TS), produit `TECH_DEBT_AUDIT.md` cité file:line. Repeat-run mode (marque RESOLVED/NEW). |
| `code-review` (plugin) | global | ? — non lu en détail | Probablement revue de code automatisée. À vérifier avant 1ère invocation. |
| `code-modernization` (plugin) | global | ? — non lu en détail | À vérifier avant 1ère invocation. |
| `skill-creator` (plugin) | global | manuel probable | Pour créer de nouvelles skills custom (ex: une skill Sift dédiée). |
| `working-with-legacy-code` | global | auto (mots-clés "legacy code", "no tests", "afraid to change this") | Lu (80/214 lignes). Méthode Feathers complète : change algorithm, seams, characterization tests, dependency-breaking. **Pertinent pour D3** (split de sift-live.ts ~942 lignes, probablement sans tests dédiés) — couvrir avant de découper plutôt que refactorer à l'aveugle. |
| `refactoring-patterns` | global | auto ("refactor this", "extract method", "code smells") | Lu (30/216 lignes). Smell catalog → refactorings nommés, transformations comportement-préservantes une à la fois. **Pertinent pour D3** (god file sift-live.ts = smell direct). |
| `refactoring-ui` | global | ? — non lu | Probablement UI (Refactoring UI le livre), pas code — à vérifier, distinct de refactoring-patterns. |
| `clean-code` | global | auto ("code review", "naming", "function too long") | Lu (25/231 lignes). SRP, naming, error handling, formatage. Fondation générale applicable à tout code écrit. |
| `clean-architecture` | global | auto ("architecture layers", "ports and adapters", "hexagonal") | Lu (12/205 lignes). Couches use-case/entities, règle de dépendance — pour systèmes multi-équipes. HORS SCOPE confirmé : Sift est un mono-binaire d'un seul dev. |
| `domain-driven-design` | global | auto ("domain modeling", "bounded context") | Lu (12/203 lignes). "Splitting a monolith into services" — HORS SCOPE confirmé, pas de microservices envisagés. |
| `pragmatic-programmer` | global | auto ("best practices", "broken windows", "tracer bullet") | Lu (12/225 lignes). VERDICT CORRIGÉ : principes intemporels (DRY, réversibilité, broken windows) agnostiques de la taille du projet — PERTINENT pour Sift, pas disproportionné comme supposé initialement. Proche de clean-code. |
| `software-design-philosophy` | global | auto ("module design", "deep module", "shallow class") | Lu (12/203 lignes). VERDICT CORRIGÉ : "deep module" (interface simple, implémentation puissante) recoupe directement "simple beats complex"/separation of concerns d'Antoine. PERTINENT pour juger un futur découpage de sift-live.ts (D3). |
| `system-design` | global | auto ("scale this", "millions of concurrent users") | Lu (12/216 lignes). HORS SCOPE confirmé sans ambiguïté — app desktop locale, pas de système distribué. |
| `ddia-systems` | global | auto ("replication", "partitioning", "consistency") | Lu (12/231 lignes). HORS SCOPE confirmé — SQLite local mono-fichier, pas de réplication. |
| `team-topologies` | global | auto ("Conway's law", "team boundaries") | Lu (12/212 lignes). HORS SCOPE confirmé sans ambiguïté — organisation d'équipes, Antoine travaille seul. |
| `release-it` | global | auto ("circuit breaker", "production outage", "zero-downtime") | Lu (12/219 lignes). Circuit breakers, bulkheads, microservices résilients — conçu pour systèmes serveur en production continue. HORS SCOPE confirmé — Sift est une app desktop locale mono-utilisateur. |
| `high-perf-browser` | global | auto ("page load speed", "Core Web Vitals", "HTTP/2") | Lu (12/215 lignes). Perf réseau (TCP/TLS, latence, Core Web Vitals) — HORS SCOPE confirmé, Sift n'a pas de problématique réseau (app locale). |
| `remotion-best-practices` | global | auto | Lu (12/364 lignes). Doublon de `remotion` (vidéos React) déjà recensé — même verdict hors-scope dev actuel. |
| `design-taste-frontend-v1` | global | auto | Lu (12/226 lignes). Version antérieure conservée pour compat arrière de `design-taste-frontend` (dials variance/motion/density). Même famille hors-scope, confirme la lignée. |
| `feature-dev` (plugin) | global | manuel `/feature-dev` | Lu (25/125 lignes). Commande structurée : comprendre le code → poser questions concrètes sur ambiguïtés → architecture → implémentation, avec TodoWrite. Recoupe superpowers/writing-plans mais pour une feature précise. Pertinent pour D3 ou toute feature substantielle. |
| `find-skills` | global | auto ("find a skill for X", "is there a skill that...") | Lu en entier (142 lignes). **CLARIFICATION IMPORTANTE** : ne cherche PAS dans les skills déjà installées (ce que fait CE registre) — cherche dans l'écosystème EXTERNE (`npx skills find`, skills.sh) pour installer du NOUVEAU. Complémentaire au registre, ne le remplace pas. |
| `remember` (plugin) | global | hooks auto (session_start, before_save, after_consolidate) + skill manuelle `remember` | Lu le skill (35 lignes) + l'architecture (hooks Python). Écrit un handoff de fin de session (`.remember/{projet}/remember.md`, <20 lignes, format State/Next/Context). **C'est ce qui explique pourquoi Claude Code reprend le contexte Sift entre sessions sans tout réexpliquer** — le `.remember/` vu dans le worktree en est la preuve. |
| `claude-md-management` (plugin) | global | manuel `/revise-claude-md` | Lu (40/54 lignes de la commande). Automatise exactement ce que j'ai fait à la main ce soir (éditer CLAUDE.md avec les apprentissages de session). Distingue CLAUDE.md (partagé git) vs .claude.local.md (perso gitignored). À utiliser en fin de session plutôt que d'éditer CLAUDE.md manuellement. |
| `full-output-enforcement` | global | pas de mots-clés explicites — probablement garantie de fond | Lu en entier (49 lignes). Anti-troncature : bannit "// rest of code", "for brevity", les skeletons à la place d'implémentations complètes. Pas un domaine technique, une discipline transverse. |

## Skills Google Stitch (génération UI par IA — installées 30/06, fusion des deux passes)

Pont complet pour explorer des directions visuelles via Google Stitch, installé en
deux temps ce soir : serveur MCP `stitch` (`~/.claude.json`, `npx @_davideast/stitch-mcp
proxy`, authentifié via gcloud) + 14 skills de `google-labs-code/stitch-skills`
installées globalement. **Noms de dossier réels vérifiés** (`~/.claude/skills/` ET
`~/.agents/skills/` — symlink confirmé fonctionnel par double timestamp identique,
PAS le bug connu vercel-labs/skills#851) : tirets, pas `::` (le `::` n'apparaît que
dans le `name` interne du frontmatter SKILL.md, pas le nom de dossier).

**Pertinence Sift** : Sift = Tauri v2 + Vite vanilla TS, PAS React. Les skills React/RN
sont hors scope direct, utiles seulement si le résultat est ensuite porté à la main.

| Nom (dossier réel) | Invocation | Pertinence Sift |
|---|---|---|
| `stitch-generate-design` | auto ("generate screen", "new design", "design variant") | **PRIORITAIRE pour l'exploration page-blanche.** Génère des écrans depuis texte/image, pipeline d'enrichissement de prompt automatique, édite avec tokens de design system. |
| `enhance-prompt` | auto ("enhance this prompt", "improve prompt for Stitch") | **Utile en amont** — transforme une description vague en prompt Stitch optimisé. À chaîner avant `stitch-generate-design`. |
| `stitch-extract-design-md` | auto ("extract design system", "DESIGN.md") | Génère un DESIGN.md depuis un projet Stitch existant — utile une fois une direction choisie, pour la documenter au format `.interface-design/system.md` équivalent. |
| `design-md` | auto | Variante générique d'extract-design-md (pas spécifique au namespace stitch). Fonction proche, garder les deux pour l'instant, clarifier si redondance gênante à l'usage. |
| `stitch-extract-static-html` | auto | Récupère le HTML/CSS produit par Stitch — le pont retour vers le code, à adapter ensuite en vanilla TS (Stitch ne génère PAS vanilla TS nativement). |
| `stitch-manage-design-system` | auto ("tokens", "design system") | Gestion de design system Stitch (tokens, cohérence inter-écrans). |
| `stitch-upload-to-stitch` | auto | Upload de design existant vers Stitch (itérer depuis une capture). |
| `stitch-code-to-design` | auto | Inverse d'extract — code → design Stitch. Moins pertinent pour notre sens d'usage habituel. |
| `stitch-loop` | auto ("stitch loop", "iterative build") | Génère un site/app multi-pages complet en boucle autonome depuis un prompt — PAS pour un panneau précis (le rail détail), plutôt pour explorer une app entière d'un coup. |
| `taste-design` | manuel/explicite | Non lue en détail — probablement proche de design-taste-frontend (même prudence que les skills "goût visuel" : vérifier le scope avant d'invoquer sur Sift). |
| `stitch-react-components` | auto ("convert to React") | **HORS SCOPE Sift** — Sift est vanilla TS, pas React. |
| `stitch-react-native` | auto | **HORS SCOPE Sift et Tuple** — mobile natif. |
| `shadcn-ui` | auto ("shadcn") | **HORS SCOPE Sift** — Sift n'utilise pas shadcn/ui. |
| `remotion` | auto ("walkthrough video") | **HORS SCOPE dev actuel** — génère des vidéos. Security scan à l'install : Snyk "Med Risk", à garder en tête si utilisée. Pertinent si promo/marketing un jour. |

⚠️ Skills tierces (pas officielles Anthropic), tournent avec les pleines permissions
de l'agent — rappelé explicitement par le CLI d'installation ("Review skills before
use"). Pas de raison de méfiance particulière, mais à garder en tête pour celles non
encore essayées en pratique.

**WORKFLOW RECOMMANDÉ pour explorer une direction Sift** : `enhance-prompt` (affiner
la demande) → `stitch-generate-design` (générer l'écran, ex. le rail de validation
détail) → regarder le résultat → si satisfaisant, `stitch-extract-static-html`
(récupérer le code) → adapter manuellement en vanilla TS/tokens CSS existants.

## Design process — workflow structuré (julianoczkowski/designer-skills, installé 30/06)

Chaîne séquentielle complète pour un chantier UI/UX significatif (nouveau screen,
refonte). Toutes installées globalement, vérifiées présentes dans `~/.claude/skills/`.
Pour des retouches ponctuelles, `impeccable` ou `interface-design` restent la voie
courte (pas besoin de la chaîne complète).

| Nom | Invocation | Usage / ordre dans le workflow |
|---|---|---|
| `grill-me` | auto ("stress-test", "grill me", "challenge this plan") | **Étape 0** — interrogatoire des requirements avant de toucher au code. Décision trees jusqu'à résolution de toutes les ambiguïtés. |
| `design-brief` | auto ("design brief", "plan this feature", "UI direction") | **Étape 1** — génère un document de brief après audit du codebase. Pose des questions sur le ton émotionnel et les références visuelles. Sauvegarde en markdown. |
| `information-architecture` | auto ("IA", "site structure", "navigation", "user flows") | **Étape 2** — structure pages/nav/hiérarchie de contenu avant tout design visuel. |
| `design-tokens` | auto ("tokens", "design system", "CSS variables") | **Étape 3** — génère les CSS custom properties. **À SAUTER sur Sift** : `.interface-design/system.md` + `styles.css` tokens existent déjà — ne pas écraser. |
| `brief-to-tasks` | auto ("break down", "tasks", "breakdown") | **Étape 4** — décompose le brief en tâches ordonnées par dépendances (vertical slices). |
| `frontend-design` | auto ("build this", "generate component", "build page") | **Étape 5** — phase de build. **Sur Sift** : subordonner aux tokens de `.interface-design/system.md`, ne pas partir de zéro. **CONFLIT DE NOM RÉSOLU (30/06)** : c'est un fichier DIFFÉRENT du plugin officiel Anthropic du même nom (déjà recensé dans la section "Frontend / design visuel" plus bas) — celui-ci (Oczkowski, `~/.claude/skills/frontend-design/`) parle aussi de "named aesthetic philosophies", même famille probablement hors-scope desktop-dense que les autres skills "goût visuel". Les deux skills portent le MÊME nom de dossier `frontend-design` — collision potentielle si les deux sont installées (laquelle Claude Code charge en cas de description ambiguë ? non vérifié). À surveiller : si un comportement bizarre apparaît sur une invocation "frontend-design", vérifier laquelle des deux a été chargée. |
| `design-review` | auto ("design review", "critique", "QA pass", "polish") | **Étape 6** — audit post-build : hiérarchie visuelle, cohérence, responsive, a11y, fidélité au brief. Playwright optionnel. |
| `design-flow` | auto ("full design flow", "design process", "start from scratch") | **Orchestrateur** — lance les 7 steps en séquence guidée. |

**Priorité de routage UI/design sur Sift** :
1. Retouche ponctuelle / polish → `impeccable` ou `interface-design`.
2. Chantier nouveau screen / refonte significative → `design-flow` (ou steps manuels).
3. Exploration rapide d'une direction (avant de coder) → `enhance-prompt` →
   `stitch-generate-design` → porter en vanilla TS.
4. Review post-implémentation → `design-review`.

**NE PAS** invoquer `design-tokens` sur Sift sans vérifier d'abord `styles.css` et
`.interface-design/system.md` — les tokens existent, écraser les casserait.

---

## Frontend / design visuel (re-skin, UI)

⚠️ **CORRECTION (30/06, plus tard dans la session)** : Sift n'a PAS de direction
visuelle figée. "Carbone usiné" est la direction de **Tuple**, pas de Sift — confusion
corrigée. Antoine n'aime pas le design actuel de Sift et la direction est **à explorer
librement**. Donc les verdicts ci-dessous qui disaient "pas la direction Sift" sont
ANNULÉS — seul le critère de SCOPE (desktop dense vs marketing/mobile) reste valide
pour écarter une skill, pas un style qui n'existe pas encore.

⚠️ **DÉCOUVERTE IMPORTANTE (30/06)** : les deux outils que CLAUDE.md recommandait
en premier ("à utiliser pour toute évolution d'interface") sont **mal adaptés à
Sift**, vérifié par lecture du contenu réel, pas du nom :

| Nom | Portée | Invocation | Usage réel (vérifié) |
|---|---|---|---|
| `design-taste-frontend` | global | auto | **HORS SCOPE SIFT.** Lu en entier (1206 lignes). C'est un manuel pour landing pages / portfolios / redesigns marketing : React/Next.js, Tailwind v4, Motion/GSAP, système de dials VARIANCE/MOTION/DENSITY, ~50 items de Pre-Flight Check anti-"AI tells" (em-dash banni, eyebrows rationnés, etc.). Section 13 "OUT OF SCOPE" exclut explicitement les dashboards / dense product UI — exactement ce qu'est Sift. Ne pas invoquer sur le batch/détail/journal. |
| `ui-ux-pro-max` (plugin, 7 sous-skills) | global | auto probable | **PARTIELLEMENT PERTINENT.** Lu le sous-skill `ui-ux-pro-max` (661 lignes). La "Quick Reference" (priorités 1-10 : accessibilité, touch targets, perf, contraste) est générique et valide pour Sift. Mais le bas du fichier porte la mention explicite *"Scope notice: rules below are for App UI (iOS/Android/React Native/Flutter), not desktop-web"* — et le CLI de recherche (`search.py --design-system`) cible React Native par défaut, conçu pour générer un design system depuis zéro, pas auditer un re-skin existant vanilla TS. Utile en lecture ponctuelle (Quick Reference seulement), pas à invoquer en pilote automatique. |
| **`impeccable`** (plugin) | global | manuel `/impeccable [cmd] [target]` | **LE PLUS PERTINENT DES TROIS, mais nécessitait une précondition.** Lu en entier (168 lignes + init.md). 23 commandes (`craft`, `shape`, `critique`, `audit`, `polish`, `bolder`, `quieter`, `distill`, `harden`, `onboard`, `animate`, `colorize`, `typeset`...). Distinction explicite **register `brand`** (design EST le produit, marketing/landing) vs **register `product`** (design SERT le produit, app UI/dashboard/tool) — Sift = `product`. Étape 3 du protocole force la lecture des conventions existantes avant d'agir ("don't reinvent the wheel"). **Bloquait sur `NO_PRODUCT_MD`** : `PRODUCT.md` n'existait pas à la racine de Sift → **créé le 30/06** (register `product`, anti-références "pas d'app grand public colorée/ludique", design principles "réemployer l'existant", pas de contrainte WCAG formelle). Utilisable maintenant : `/impeccable audit batch`, `/impeccable critique journal`, etc. `DESIGN.md` et le live config ne sont PAS encore créés (étapes optionnelles suivantes, à faire à la 1ère invocation réelle via `/impeccable document`). |
| `frontend-design` (plugin officiel Anthropic) | global | ? — non lu | Existe en plus de design-taste-frontend — pas encore lu, vérifier doublon avant usage. |
| **`interface-design`** | global | auto ("dashboard", "admin panel", "tool", "data interface" — explicitement "Not for landing pages") | **DEUXIÈME MEILLEUR CANDIDAT après impeccable, déjà UTILISÉE sur Sift dans le passé.** Lu en entier (406 lignes). Scope exact : "dashboards, admin panels, SaaS apps, tools... Not for landing pages, marketing." Insiste sur l'intent-first (qui est l'utilisateur, quelle tâche, quel ressenti) avant tout choix visuel, et sur le craft invisible (layering subtil, hiérarchie de texte à 4 niveaux primary/secondary/tertiary/muted — exactement nos tokens `--color-text-tertiary` déjà en place). Mécanisme `.interface-design/system.md` : **CE FICHIER EXISTE DÉJÀ** dans Sift (`dj-assistant-m6a/.interface-design/system.md`, 156 lignes) — cette skill a donc déjà tourné sur le projet. Contient tokens couleur exacts (#2c2c2a bg, #3b7df0 accent unique, etc.), échelle d'espacement 4/8/12/16/24/32, radius, typo (Outfit + JetBrains Mono), lexique (File/LOSSLESS/DUPLICATE), interdits ("jamais ranger un FAUX en masse"). **MAIS sa section "Mode Batch" est confirmée PÉRIMÉE par Antoine (30/06)** — décrit un tableau ☑/titre/chip/format avec barre d'action en bas, remplacé par le re-skin de cette session (groupes repliables, sélection tri-état, bouton adaptatif). Les fondations (couleurs/espacement/typo/règles de profondeur) restent valides. **DETTE IDENTIFIÉE, PAS RÉSOLUE** : actualiser la section Mode Batch de ce system.md pour refléter le re-skin réel — à faire dans une session dédiée avec lecture du code actuel comme preuve, pas de mémoire. |
| `minimalist-ui` | global | manuel/explicite | Direction esthétique nommée (palette monochrome chaud, bento grid, éditorial). VERDICT CORRIGÉ (30/06) : à reconsidérer comme option réelle pour Sift puisque sa direction visuelle est ouverte — ce n'est PLUS écarté pour "incompatibilité avec une direction existante" (qui était une confusion avec Tuple). Reste un choix de style parmi d'autres, pas le scope qui pose problème. |
| `industrial-brutalist-ui` | global | manuel/explicite | Esthétique "tactical/industrielle" (Swiss typographic + terminal militaire, scanlines, dithering), cible explicitement "data-heavy dashboards" — donc DANS LE SCOPE desktop-dense. VERDICT CORRIGÉ (30/06) : à reconsidérer comme option réelle, écarté précédemment sur la base d'une confusion avec la direction Tuple. Pertinent si Antoine veut une direction "instrument professionnel" plutôt que purement sobre. |
| `microinteractions` | global | auto ("button feedback", "loading state", "animation detail") | Lu (frontmatter). Triggers/rules/feedback/loops — exactement le langage du PRODUCT.md Sift ("petits détails avec intention"). Pertinent pour la prochaine passe de polish UI (barres, transitions Journal). |
| `hooked-ux` | global | auto ("engagement loops", "habit formation") | HORS SCOPE — gamification/notifications, à l'opposé de l'anti-référence Sift ("pas un jouet"). |
| `ios-hig-design` | global | auto | Hors scope Sift (iOS natif/SwiftUI, pas desktop Tauri). |
| `web-typography` | global | auto ("font pairing", "type hierarchy") | Lu (frontmatter). Pertinent en théorie (échelle typo) mais focus web fonts loading — moins central que interface-design qui couvre déjà la typo desktop (Outfit + JetBrains Mono déjà fixés dans system.md). |
| `high-end-visual-design` / `top-design` / `gpt-taste` | global | auto | HORS SCOPE — même famille que design-taste-frontend (Awwwards/agency/GSAP), confirmé par lecture frontmatter des trois. |
| `steve-jobs-design-review` | global | auto ("design review", "too many features", "saying no") | Lu (frontmatter). PAS une direction visuelle — une grille de critique (simplicité ruthless, "the no list"). Pertinence indirecte : recoupe la philosophie "simple beats complex" déjà dans les préférences d'Antoine. Utilisable pour trancher des questions de scope/feature, pas de pixels. |
| `stitch-design-taste` | global | manuel | Génère des DESIGN.md pour l'outil externe Google Stitch — pas pour Claude Code lui-même. Hors scope direct. |
| `redesign-existing-projects` | global | manuel/explicite | Lu (50/178 lignes). Même biais que design-taste-frontend malgré la promesse "works with vanilla CSS" — l'audit cible cards à 3 colonnes, hero 100dvh, "feature row" : vocabulaire marketing/landing. Pas adapté à une liste dense de morceaux. |
| `ux-heuristics` | global | auto ("usability audit", "Nielsen heuristics") | Lu (frontmatter). Heuristiques de Nielsen, agnostique du stack — applicable à Sift en théorie pour un audit ponctuel, pas encore essayé. |
| `refactoring-ui` | global | auto ("my UI looks off", "visual hierarchy", "spacing scale") | Lu (60/267 lignes). Malgré "Tailwind styling" dans les déclencheurs, le CONTENU est agnostique du stack — hiérarchie via taille/poids/couleur, échelle d'espacement contrainte, grayscale-first. Réellement utile pour Sift (ce qu'on a fait empiriquement ce soir avec --color-text-tertiary etc., sans le formaliser). |
| `design-everyday-things` | global | auto ("why is this confusing", "affordance", "mental model") | Lu (frontmatter). Don Norman — affordances/signifiants/modèles mentaux, fondation UX universelle, pas spécifique web/marketing. Utile pour tout audit d'utilisabilité Sift. |
| `agent-browser` (skill) | global | via outil Agent | Tester/QA l'UI servie par Tauri — navigation, clics, screenshots. Préférer aux outils navigateur intégrés pour bug hunt. **Pertinent pour valider visuellement nos changements UI sans capture manuelle** — pas encore invoquée, à essayer. |
| `ui-refactor` | global | auto ("make this look better", "fix cluttered interface") | Lu (12/33 lignes — courte mais ciblée). "Feature first, not shell first" : commencer par la fonctionnalité concrète, pas la nav/sidebar. Règles tactiques plutôt qu'artistiques. PERTINENT pour Sift, même esprit que refactoring-ui. |
| `37signals-way` | global | auto ("Getting Real", "Shape Up", "build less", "say no") | Lu (12/201 lignes). Philosophie "build less"/"say no by default" recoupe directement les préférences d'Antoine (simple beats complex). Mécanisme de paris/cycles 6 semaines = conçu pour petites équipes, disproportionné pour 1 dev solo — utiliser la PHILOSOPHIE, pas le rituel d'équipe. |
| `image-to-code` | global | auto | Lu (12/1228 lignes). Conçu pour Codex/sites web, hero sections "premium artistic" — HORS SCOPE confirmé, même famille marketing-web. |
| `imagegen-frontend-web` | global | auto | Lu (10/987 lignes). "Optimized for landing pages, marketing sites" explicite — HORS SCOPE confirmé. |
| `imagegen-frontend-mobile` | global | auto | Lu (10/1465 lignes). iOS/Android phone mockups — HORS SCOPE confirmé (Sift = desktop Tauri). |

**Conséquence pour CLAUDE.md** : la recommandation "ui-ux-pro-max + impeccable → à
utiliser pour toute évolution d'interface" doit être affinée. Préférer `impeccable`
(register product, audit-first) pour Sift ; ui-ux-pro-max seulement en lecture
ponctuelle de sa Quick Reference (accessibilité/perf) ; ne jamais invoquer
design-taste-frontend sur Sift.

## Produit / business (probablement hors scope Sift technique, utiles pour Tuple marketing ou roadmap)

`jobs-to-be-done`, `lean-startup`, `lean-ux`, `lean-analytics`, `continuous-discovery`,
`inspired-product`, `obviously-awesome`, `crossing-the-chasm`, `blue-ocean-strategy`,
`good-strategy-bad-strategy`, `traction-eos`, `high-output-management`,
`cro-methodology`, `scorecard-marketing`, `one-page-marketing`, `storybrand-messaging`,
`made-to-stick`, `contagious`, `influence-psychology`, `negotiation`,
`cold-start-problem`, `drive-motivation`, `improve-retention`, `mom-test`,
`brandkit` — non lues en détail, probablement pour decisions produit/marketing
(pertinentes pour la campagne promo Tuple notée dans les mémoires, pas pour Sift backend).

⚠️ **MONÉTISATION SIFT EN SUSPENS (30/06)** : Antoine envisage potentiellement de
rendre Sift payant/freemium, pas encore décidé ("à explorer"). PRODUCT.md NON modifié
sur ce point (toujours "gratuit" — ne pas figer une décision non prise). Si la
monétisation se précise, ces skills deviennent pertinentes pour Sift, pas juste Tuple :
`hundred-million-offers` (Value Equation, pricing irrésistible), `monetizing-innovation`
(willingness-to-pay, freemium vs trial, packaging good-better-best),
`predictable-revenue` (si vente outbound B2B — probablement pas pour un outil DJ indé).

⚠️ **BRANDKIT REVALORISÉ (30/06)** : `brandkit` (génération d'images logo/identité)
écartée plus haut comme "pas de chantier en cours" — reconsidérée : utile pour la
phase promo de Tuple (campagne notée en mémoire : seeding Reddit/Discord, outreach
YouTubeurs) et potentiellement pour Sift si un jour packaging/asset visuel de promo
est nécessaire.

## Infra / outils techniques transverses

| Nom | Portée | Invocation | Usage |
|---|---|---|---|
| `desktop-commander` (plugin) | global | outils directs | Accès filesystem + terminal réel sur la machine d'Antoine. C'est CE qui me permet de lire/écrire ce fichier. |
| `github-actions-docs` | global | auto | CI build cross-platform Win+Mac. |
| `context7` (plugin) | global | auto (docs libs) | Doc à jour de libs externes (déjà vu dans mes outils Claude.ai aussi). |
| `rust-analyzer-lsp` (plugin) | global | connecteur LSP (pas une skill invocable) | Lu le README (34 lignes) : c'est juste un connecteur du Language Server `.rs` (rustup/brew/apt), pas de mécanisme d'invocation. CORRECTION : `rust-lsp` mentionné dans CLAUDE.md Sift original n'existe PAS comme plugin séparé — `installed_plugins.json` ne liste que `rust-analyzer-lsp`. Pas un doublon réel, une référence imprécise à corriger dans CLAUDE.md. |
| `ecc` (Everything-Claude-Code) | global | mixte (60+ agents, 271 skills, commandes epic-*) | Méga-framework, trop large pour audit exhaustif. Contient `rust-reviewer` (agent générique : cargo check/clippy/fmt/test) — lu et comparé : REDONDANT avec `rust-best-practices`/`rust-engineer` déjà personnalisés pour Sift (MSRV 1.77.2, fail-fast). Ne pas invoquer ses agents Rust génériques sur Sift, préférer les versions projet. Contient aussi des skills hors-scope évidentes (`defi-amm-security`, `customs-trade-compliance`, `cisco-ios-patterns`) — confirme que c'est un fourre-tout multi-domaines, pas un outil ciblé. |
| `learned` | global | n/a — dossier vide | Vérifié : dossier vide, aucun contenu actuel. Probablement un emplacement réservé pour un mécanisme d'apprentissage futur, rien à invoquer aujourd'hui. |

## Spécifiques à d'autres projets d'Antoine (PAS Sift — ne pas invoquer ici)

`tuple`, `tupline`, `tupline-revoice` — skills Max for Live / Ableton, hors scope Sift.

## Non recensés en détail (à vérifier avant 1ère invocation)

`vercel`, `figma`, `qdrant-skills` (Sift a écarté Qdrant — probablement inutile),
`maxmcp` (Max for Live, hors Sift), `brightdata-plugin`, `outputai`, `coderabbit`,
`appwrite`, `adobe-for-creativity`, `discord`, `feature-dev`.

---

## Pont Stitch MCP (installé 30/06, pour exploration visuelle Sift)

Serveur MCP `stitch` configuré dans `~/.claude.json` (`npx @_davideast/stitch-mcp proxy`,
authentifié via gcloud). 14 skills de `google-labs-code/stitch-skills` installées
globalement (`~/.agents/skills/`, symlinkées vers Claude Code) :

| Skill | Pertinence Sift |
|---|---|
| `stitch::generate-design` | **PRIORITAIRE pour l'exploration page-blanche.** Génère des écrans depuis texte/image, pipeline d'enrichissement de prompt automatique, édite avec tokens de design system. Utilise le MCP `stitch` configuré. |
| `stitch::extract-design-md` | Génère un DESIGN.md depuis un projet Stitch existant — utile une fois une direction choisie, pour la documenter au format `.interface-design/system.md` équivalent. |
| `stitch::extract-static-html` | Récupère le HTML/CSS produit par Stitch — c'est le pont retour vers le code, à adapter ensuite en vanilla TS (Stitch ne génère PAS vanilla TS nativement). |
| `stitch::manage-design-system` | Gestion de design system Stitch (tokens, cohérence inter-écrans). |
| `stitch::upload-to-stitch` | Upload de design existant vers Stitch (probablement pour itérer sur une capture). |
| `stitch::code-to-design` | Inverse de extract — code → design Stitch. Moins pertinent pour notre sens d'usage. |
| `design-md` | Doublon de fonction avec extract-design-md mais générique (pas spécifique stitch::). |
| `enhance-prompt` | Transforme une idée vague en prompt Stitch optimisé — utile en amont de generate-design. |
| `stitch-loop` | Génère un site multi-pages complet depuis un prompt — PAS pour un panneau précis (le rail détail), plutôt pour explorer une app entière d'un coup. |
| `taste-design` | Non lue en détail — probablement proche de design-taste-frontend (vérifier avant d'invoquer sur Sift, même prudence que pour les skills "goût visuel" du début de session). |
| `stitch::react-components` | HORS SCOPE Sift — génère React, incompatible vanilla TS. |
| `stitch::react-native` | HORS SCOPE Sift et Tuple — mobile natif. |
| `shadcn-ui` | HORS SCOPE Sift — composants shadcn/React. |
| `remotion` | HORS SCOPE dev — génère des vidéos de walkthrough. Security scan : Snyk "Med Risk", à garder en tête si utilisée un jour. |

⚠️ Toutes ces skills tierces (pas officielles Anthropic) tournent avec les pleines
permissions de l'agent — le CLI d'installation l'a rappelé explicitement
("Review skills before use"). Pas de raison de méfiance particulière à ce stade,
mais à garder en tête, surtout pour celles non encore essayées en pratique.

WORKFLOW RECOMMANDÉ pour explorer une direction Sift : `enhance-prompt` (affiner
la demande) → `stitch::generate-design` (générer l'écran, ex. le rail de validation
détail) → regarder le résultat → si satisfaisant, `stitch::extract-static-html`
(récupérer le code) → adapter manuellement en vanilla TS/tokens CSS existants
(Stitch ne connaît pas le stack Sift, c'est à nous de traduire après coup).

Avant de planifier une tâche : identifier son domaine, chercher dans ce registre,
charger la skill correspondante si elle existe. Si une ligne dit "? — non lu",
lire le SKILL.md avant de l'invoquer pour de vrai — ne pas supposer son comportement
depuis le nom seul. Si rien ne correspond, continuer sans inventer.

Ce fichier est incomplet par construction (recensement initial, pas tout lu en
détail) — le compléter au fur et à mesure qu'un outil marqué "? — non lu" est
effectivement utilisé.
