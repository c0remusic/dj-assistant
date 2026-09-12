import js from "@eslint/js";
import tseslint from "typescript-eslint";
import globals from "globals";

export default tseslint.config(
  {
    ignores: [
      "dist/**",
      "src-tauri/**",
      "node_modules/**",
      "storybook-static/**",
      ".claude/**",
      "docs/**",
      // Scripts de build/outillage : Node, hors du périmètre `tsconfig.json`
      // (`include: ["frontend", "shared"]`). Ils ne partagent ni les globals
      // navigateur ni les conventions du front.
      "scripts/**",
      // La maquette d'origine, chargée INCONDITIONNELLEMENT par `frontend/main.ts`
      // (voir CLAUDE.md § « Le frontend a deux vies »). Elle tourne en prod, mais
      // `tsconfig.json` la laisse déjà hors type-check (`checkJs: false`) : la
      // linter serait le seul gardien d'un fichier que personne ne réécrit.
      // La retirer de cette liste est une décision à prendre pour elle-même.
      "frontend/app.js",
    ],
  },
  {
    files: ["frontend/**/*.ts", "shared/**/*.ts"],
    extends: [
      js.configs.recommended,
      // Version NON type-checked : aucun programme TypeScript n'est chargé, donc
      // le lint reste rapide et ne double pas `npx tsc --noEmit`, qui est déjà la
      // gate de types du projet (CI + `.claude/verify.sh`).
      ...tseslint.configs.recommended,
    ],
    languageOptions: {
      ecmaVersion: 2022,
      sourceType: "module",
      globals: globals.browser,
    },
    rules: {
      // `tsconfig.json` a déjà `noUnusedLocals` + `noUnusedParameters`, et le
      // compilateur TypeScript exempte nativement les identifiants préfixés `_`.
      // Sans cet alignement, les deux gates se contredisent sur le même fichier :
      // `tsc --noEmit` passe et `eslint` échoue. Cas réel au premier lancement —
      // `verdictCardHtml(_r)` (`frontend/report-view.ts`), un no-op délibéré dont
      // le paramètre est conservé pour ne pas toucher ses appelants.
      "@typescript-eslint/no-unused-vars": [
        "error",
        {
          argsIgnorePattern: "^_",
          varsIgnorePattern: "^_",
          caughtErrorsIgnorePattern: "^_",
        },
      ],
    },
  },
  {
    // La suite Vitest tourne en environnement Node (voir `vitest.config.ts`) : lui
    // donner les globals navigateur ferait passer un `document` qui n'existe pas.
    files: ["test/**/*.ts"],
    extends: [js.configs.recommended, ...tseslint.configs.recommended],
    languageOptions: {
      ecmaVersion: 2022,
      sourceType: "module",
      globals: globals.node,
    },
  },
);
