import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import { resolve } from "node:path";

export default defineConfig({
  root: resolve(process.cwd(), "mockup"),
  plugins: [react()],
  resolve: { alias: { "@decky/ui": resolve(process.cwd(), "mockup/ui.tsx"), "@decky/api": resolve(process.cwd(), "mockup/api.ts") } },
  server: { host: "127.0.0.1", port: 4173, strictPort: true }
});
