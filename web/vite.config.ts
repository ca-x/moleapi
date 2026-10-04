import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";
export default defineConfig({
  plugins: [react()],
  test: { setupFiles: ["./src/testSetup.ts"] },
  clearScreen: false,
  worker: { format: "es" },
  server: {
    port: 1420,
    strictPort: true,
    proxy: { "/api": "http://127.0.0.1:8787" },
  },
  build: {
    target: "es2022",
    rollupOptions: {
      output: {
        manualChunks(moduleId) {
          if (
            moduleId.includes("/node_modules/@codemirror/lang-javascript/") ||
            moduleId.includes("/node_modules/@lezer/javascript/")
          )
            return "javascript-language";
          if (
            moduleId.includes("/node_modules/@codemirror/") ||
            moduleId.includes("/node_modules/@uiw/") ||
            moduleId.includes("/node_modules/@lezer/")
          )
            return "editor";
          if (moduleId.includes("/node_modules/@radix-ui/")) return "radix";
        },
      },
    },
  },
});
