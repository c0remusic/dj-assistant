import { defineConfig } from "vite";

export default defineConfig({
  clearScreen: false,
  server: {
    port: 5173,
    strictPort: true,
    // Never watch the Rust side — target/ and binaries/ churn during builds
    // and locking transient files (ffmpeg extraction) crashes the watcher.
    watch: { ignored: ["**/src-tauri/**"] },
  },
  build: { outDir: "dist", emptyOutDir: true },
});
