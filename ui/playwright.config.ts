import { defineConfig, devices } from "@playwright/test";

// Runs the real UI in a browser against the development server (ADR-0010).
export default defineConfig({
  testDir: "e2e",
  globalSetup: "./e2e/global-setup.ts",
  // The tests share one dev server and catalog.
  workers: 1,
  fullyParallel: false,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 1 : 0,
  reporter: process.env.CI ? [["list"], ["html", { open: "never" }]] : "list",
  use: {
    baseURL: "http://127.0.0.1:5173",
    trace: "retain-on-failure",
  },
  projects: [{ name: "chromium", use: { ...devices["Desktop Chrome"] } }],
  webServer: [
    {
      // A fresh catalog for every run.
      command:
        "node -e \"require('fs').rmSync('target/e2e-data',{recursive:true,force:true})\" && cargo run --quiet -p picturesque-devserver",
      cwd: "..",
      env: {
        PICTURESQUE_DATA_DIR: "target/e2e-data",
        // Models served by the tests themselves (e2e/models.spec.ts).
        PICTURESQUE_MODEL_MANIFEST: "ui/e2e/fixtures/models.json",
      },
      url: "http://127.0.0.1:5174/api/health",
      reuseExistingServer: !process.env.CI,
      timeout: 300_000,
    },
    {
      command: "npm run dev",
      url: "http://127.0.0.1:5173",
      reuseExistingServer: !process.env.CI,
    },
  ],
});
