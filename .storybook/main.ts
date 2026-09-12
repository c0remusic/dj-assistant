import type { StorybookConfig } from "@storybook/html-vite";
import type { InlineConfig } from "vite";

// Sift has no component framework (vanilla TS + DOM manipulation, see CLAUDE.md) —
// @storybook/html-vite is the framework-agnostic builder that works directly with
// plain HTML/DOM and reuses the project's existing Vite setup (see ../vite.config.ts).
const config: StorybookConfig = {
  stories: ["../frontend/**/*.stories.@(ts|tsx)"],
  addons: ["@storybook/addon-a11y"],
  framework: {
    name: "@storybook/html-vite",
    options: {},
  },
  // Reuse the app's real stylesheet so tokens/classes documented in
  // docs/design-system-states.md render with their actual values.
  staticDirs: [],
  // Vite's fs watcher otherwise crawls src-tauri/target (Rust build output) and can
  // crash with EBUSY when cargo/tauri is writing to a locked .dll at the same time.
  viteFinal: async (viteConfig) => {
    (viteConfig as InlineConfig).server = {
      ...(viteConfig as InlineConfig).server,
      watch: {
        ...(viteConfig as InlineConfig).server?.watch,
        ignored: ["**/src-tauri/target/**"],
      },
    };
    return viteConfig;
  },
};

export default config;
