import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
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
