import { defineConfig } from "vite";
import base from "./vite.config";
export default defineConfig({ ...base, optimizeDeps: { include: ["@graphiql/react > react-compiler-runtime", "react-compiler-runtime"], exclude: ["graphiql", "@graphiql/react", "graphiql/setup-workers/vite", "@graphiql/react/setup-workers/vite"] }, server: { port: 1423, host: "127.0.0.1" }, build: { ...base.build, outDir: "/tmp/moleapi-localization-built", rollupOptions: { ...base.build?.rollupOptions, input: "localization-check.html" } } });
