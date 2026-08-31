import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import type { Plugin } from "vite";

interface PluginManifest { name: string; }

const manifest = JSON.parse(readFileSync(resolve(import.meta.dirname, "plugin.json"), "utf-8")) as PluginManifest;

function deckyManifestPlugin(): Plugin {
  const moduleId = "@decky/manifest";
  const resolvedModuleId = `\0${moduleId}`;
  return {
    name: "decky-manifest",
    resolveId: (id) => id === moduleId ? resolvedModuleId : undefined,
    load: (id) => id === resolvedModuleId ? `export default ${JSON.stringify({ name: manifest.name })};` : undefined
  };
}

export default defineConfig({
  plugins: [deckyManifestPlugin(), react()],
  build: {
    lib: { entry: resolve(process.cwd(), "src/index.tsx"), name: "DeckyMyRig", formats: ["iife"], fileName: () => "index.js" },
    target: "es2020",
    minify: false,
    rollupOptions: {
      external: ["react", "react-dom", "react/jsx-runtime", "@decky/ui"],
      output: { footer: "export default DeckyMyRig;", globals: { react: "SP_REACT", "react-dom": "SP_REACTDOM", "react/jsx-runtime": "SP_JSX", "@decky/ui": "DFL" } }
    }
  },
  test: { environment: "jsdom", setupFiles: ["src/test/setup.ts"], include: ["src/**/*.test.ts", "src/**/*.test.tsx"] }
});
