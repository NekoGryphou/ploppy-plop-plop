import { defineConfig } from "@playwright/test";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const deckyDirectory = resolve(dirname(fileURLToPath(import.meta.url)), "..");

export default defineConfig({
  testDir: ".",
  testMatch: "visual.spec.ts",
  outputDir: "../../out/tests/playwright",
  use: { baseURL: "http://127.0.0.1:4173", viewport: { width: 1050, height: 760 }, colorScheme: "dark" },
  webServer: {
    command: "npm run visual:serve",
    cwd: deckyDirectory,
    url: "http://127.0.0.1:4173",
    reuseExistingServer: !process.env.CI,
    timeout: 30_000
  }
});
