import { defineConfig } from "@playwright/test";
export default defineConfig({
  testDir: "tests/e2e",
  timeout: 30000,
  workers: 1,
  reporter: "list",
  use: {
    baseURL: "http://127.0.0.1:8787",
    headless: true,
    viewport: { width: 1440, height: 1050 },
    launchOptions: {
      executablePath: process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE,
    },
  },
  webServer: {
    command: "target/release/sentinelflow-ids serve 8787",
    cwd: "..",
    url: "http://127.0.0.1:8787/api/health",
    reuseExistingServer: false,
  },
});
