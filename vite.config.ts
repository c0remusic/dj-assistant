import { defineConfig } from "vite";

export default defineConfig({
  clearScreen: false,
  server: {
    port: 5173,
    strictPort: true,
    // host:true (listen on all addresses, IPv4 + IPv6) instead of the default "localhost" —
    // that default let Vite bind IPv6-only ([::1]:5173) on this machine while WebView2 resolved
    // "localhost" to IPv4 (127.0.0.1), causing ERR_CONNECTION_REFUSED even though `netstat` showed
    // the port listening.
    host: true,
    // Never watch the Rust side — target/ and binaries/ churn during builds
    // and locking transient files (ffmpeg extraction) crashes the watcher.
    watch: { ignored: ["**/src-tauri/**"] },
  },
  build: { outDir: "dist", emptyOutDir: true },
});
