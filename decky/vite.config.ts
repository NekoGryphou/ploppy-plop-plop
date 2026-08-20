import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";
import { resolve } from "node:path";

export default defineConfig({
  plugins: [react()],
  build: {
    lib: { entry: resolve(process.cwd(), "src/index.tsx"), name: "RemotePCPower", formats: ["iife"], fileName: () => "index.js" },
    target: "es2020",
    minify: false,
    rollupOptions: {
      external: ["react", "react-dom", "react/jsx-runtime", "@decky/ui", "@decky/api"],
      output: { footer: "export default RemotePCPower;", globals: { react: "SP_REACT", "react-dom": "SP_REACTDOM", "react/jsx-runtime": "SP_JSX", "@decky/ui": "DFL", "@decky/api": "DECKY_API" } }
    }
  },
  test: { environment: "jsdom", setupFiles: ["src/test/setup.ts"], include: ["src/**/*.test.ts", "src/**/*.test.tsx"] }
});
