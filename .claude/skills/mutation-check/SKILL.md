---
name: mutation-check
description: >-
  Mesurer qu'un test de non-régression TIENT, en mutant le code qu'il garde (CLAUDE.md § Méthode,
  barème cran 1 : « vert ne veut pas dire tenant »). Se déclenche de lui-même à chaque création ou
  modification d'un test Vitest ou d'un test Rust qui fige une valeur, une forme ou une règle —
  avant de déclarer le test couvrant, jamais après. Aussi invocable par /mutation-check sur un test
  existant, et pendant une revue de code qui touche des tests.
---

# Mutation-check — un test se mesure en le cassant

Un test vert prouve sa propre exécution, pas ce qu'il garde. La preuve est l'échec **provoqué** :
muter la ligne gardée, voir le test tomber, restaurer, le voir repasser. Mesuré sur ce dépôt :
`frontend/b85.ts` (24 vecteurs verts qui toléraient quatre constantes interdites), et un vecteur de
`test/lint-commit-msg.test.ts` qui passait au vert **sans rien mesurer** (sa ligne portait un accent,
le linter l'écartait pour la mauvaise raison) — attrapé uniquement par la mutation.

## Protocole

1. **Identifier la ou les lignes gardées** — celles dont le test prétend empêcher la dérive. Une
   mutation par claim du test, pas une seule pour tout le fichier.
2. **Sauvegarder** le fichier muté dans le scratchpad de session (`cp F "$SP/F.bak"`) — JAMAIS
   `git checkout` pour restaurer (règle mémoire : une restauration git peut embarquer d'autres
   éditions).
3. **Muter** la valeur/le motif exact que le test fige. Mutations types :
   - valeur figée → valeur voisine plausible (13 → 12, `>` → `>=`) ;
   - branche → priorité inversée (échanger deux `if` en cascade) ;
   - garde → suppression (`replace(/…/, '')` → regex qui ne matche rien `(?!)`) ;
   - population → l'autre population plausible (le bug exact que le test doit interdire).
4. **Vérifier que la mutation a PRIS** : re-grep de la ligne mutée avant de lancer le test
   (mémoire : `mutation-test-must-confirm-the-mutation-landed` — une mutation non appliquée rend
   un faux « le test tient »).
5. **Lancer le test ciblé** (`npx vitest run test/X.test.ts` ou
   `cargo test --manifest-path src-tauri/Cargo.toml <nom>`) — attendu : **au moins un échec**, et
   noter COMBIEN de cas tombent.
6. **Restaurer depuis la sauvegarde**, prouver l'identité (`diff`), relancer : **tout vert**.
7. **Rapporter** la table mutation → cas tombés. Une mutation qui ne fait rien tomber = le test ne
   garde pas cette ligne : corriger le test (ou reconnaître le trou), pas la mutation.

## Pièges de ce dépôt

- Fichiers CRLF : muter par script Python avec `newline=""`, jamais `sed -i` qui normalise.
- Ne jamais muter pendant qu'un `tauri dev` compile (lock du `target/` côté Rust ; HMR côté front
  rejouerait la mutation dans la fenêtre).
- Un test qui lit les VRAIS fichiers (`font-weights`, `queue-row-height`) se mute dans le fichier
  SOURCE (styles.css, queue-panel.ts), pas dans le test.
- Restauration : `diff` de preuve obligatoire — « identique » affiché, pas supposé.
