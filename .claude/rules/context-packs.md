<!-- Pas de frontmatter paths: volontaire — ce fichier est un menu de contexte
     consulté à la demande par l'orchestrateur, pas une règle liée à un type de
     fichier. Voir A-fondations-memoire.md § Path-specific rules. -->

# Packs de contexte (sizing) — Sift

Menus de 3-8 fichiers par type de tâche (l'orchestrateur choisit, ne colle pas
tout). ⚠️ La définition du sizing vivait dans `~/.claude/CLAUDE.md` § Sizing (+
skill `sizing-templates`) : **supprimés** par le reset vanilla du 2026-07-31
(récupérables au tag `pre-reset-vanilla`). Les packs ci-dessous restent valables
tels quels ; il n'y a simplement plus de doctrine générale derrière.
Migré depuis l'ex-`docs/skills-registre.md`
(supprimé 2026-07-16). Chemins vérifiés sur disque à la migration.

## Pack UI live

- `CLAUDE.md` : sections Vision, Front — événements répétés, Front — CSS,
  Vérification UI.
- `docs/design-system-states.md` : composants concernés seulement.
- `frontend/styles.css`.
- Fichiers frontend touchés (`report-view.ts`, `filing.ts`,
  `batch-tracklist.ts`, etc.).
- Commandes de vérification par défaut : `npx tsc --noEmit`, puis `npm run test`
  (Vitest, env Node) et `npm run lint`. Le harnais Vitest n'existe que depuis le
  2026-08-05 ; son périmètre est la LOGIQUE PURE, pas le DOM — un module qui a
  besoin de `document` se vérifie par Storybook ou par la vraie fenêtre
  (skill `run-sift`), jamais ici.

## Pack Rust backend

- `CLAUDE.md` : sections Stack, Commandes, Documentation lookups, Méthode.
- `.claude/rules/rust.md` : override projet (forme réelle du code, avant de
  proposer un pattern Rust).
- Fichiers `src-tauri/src/*.rs` concernés seulement.
- `src-tauri/Cargo.toml` si dépendances/features.
- Commandes : `cargo test --manifest-path src-tauri/Cargo.toml` et/ou
  `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`.

## Pack Rekordbox / master.db

- `CLAUDE.md` : garde-fou « Jamais une écriture sur un système live ».
- `docs/ressources-externes.md` : évaluations Rekordbox pertinentes (voir
  sommaire en tête de fichier, cibler par ligne).
- Specs/plans M8 actifs uniquement — les repérer en listant
  `docs/superpowers/changes/`. ⚠️ `docs/INDEX.json`, qui servait d'index, **n'existe
  plus** (vérifié absent le 2026-08-05, voir `CLAUDE.md` § `docs/`).
- Fichiers Rust/TS concernés.
- Toute vérification dans le vrai Rekordbox reste manuelle par Antoine.

## Pack docs / planning

- Le dossier de chantier visé sous `docs/superpowers/changes/`. ⚠️ `docs/INDEX.json`
  **n'existe plus** (vérifié absent le 2026-08-05) : il n'y a plus de catalogue, on
  liste le dossier.
- Spec/plan cible.
- Ce fichier (`context-packs.md`) si routage de packs. ⚠️ Pour le routage de
  skills il n'y a plus d'inventaire : `~/.claude/skills-view.md` a été **supprimé**
  par le reset vanilla du 2026-07-31 (récupérable au tag `pre-reset-vanilla`),
  comme l'ex-`docs/skills-registre.md` qu'il remplaçait. S'en tenir aux skills
  réellement listées par le harnais.
- Pas de lecture large de `frontend/` ou `src-tauri/` sauf question précise.

## Pack review adverse

- La spec approuvée.
- Le diff (`git diff -- <fichiers>`).
- Les fichiers modifiés uniquement.
- Les règles `CLAUDE.md` strictement pertinentes.
- Sortie : findings file:line, sévérité, test manquant, pas de résumé long.
