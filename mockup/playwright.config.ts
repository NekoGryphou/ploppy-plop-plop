import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: ".",
  testMatch: "visual.spec.ts",
  outputDir: "../artifacts/ui",
  use: { baseURL: "http://127.0.0.1:4173", viewport: { width: 1050, height: 760 }, colorScheme: "dark" },
  webServer: { command: "npm run visual:serve", cwd: ".", port: 4173, reuseExistingServer: true, timeout: 30_000 }
});
