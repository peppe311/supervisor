import { svelte } from "@sveltejs/vite-plugin-svelte";
import { defineConfig } from "vite";

export default defineConfig({
  plugins: [svelte()],
  build: {
    cssCodeSplit: false,
    emptyOutDir: true,
    lib: {
      entry: "src/entry.ts",
      fileName: () => "central-agent-ui.js",
      formats: ["iife"],
      name: "CentralAgentUiBundle"
    },
    minify: false,
    outDir: "dist",
    rollupOptions: {
      output: {
        banner: "/*! Supervisor: MPL-2.0. Source: https://github.com/peppe311/supervisor. Third-party components retain their licenses; see THIRD_PARTY_NOTICES.md. */",
        assetFileNames: "central-agent-ui[extname]"
      }
    },
    sourcemap: false,
    target: "es2022"
  }
});
