import { defineConfig, configDefaults } from "vitest/config";

// Un SEUL projet, en environnement Node — pas de jsdom.
//
// Ce n'est pas un raccourci : le frontend de Sift n'installe son wiring live que
// si Tauri est présent (`"__TAURI_INTERNALS__" in window`, voir CLAUDE.md
// § « Le frontend a deux vies »). Un jsdom donnerait un `window` sans Tauri —
// donc un faux navigateur qui n'exécute justement PAS le code qu'on voudrait
// tester, tout en donnant l'illusion de le couvrir. La vérification du DOM réel
// passe par la vraie fenêtre WebView2 (skill `run-sift`, CDP), et les états
// visuels par Storybook.
//
// Le périmètre de cette suite est donc explicite : la LOGIQUE PURE, celle dont
// une erreur est silencieuse à l'exécution — codecs, échappement, mappings,
// calculs. Un module qui a besoin de `document` n'a pas sa place ici.
export default defineConfig({
  test: {
    name: "unit",
    environment: "node",
    include: ["test/**/*.test.ts"],
    exclude: [...configDefaults.exclude],
  },
});
