// The browser tests (REQ-E2E-010): Playwright in headless Chromium against
// `vite preview` of the release build of site/. `mise run test:site` builds
// the release wasm and the site first; `--grep @smoke` is the smoke set that
// `mise run check` runs (REQ-E2E-070).

import { defineConfig, devices } from "@playwright/test";

const PORT = 4310;

export default defineConfig({
  testDir: "e2e",
  fullyParallel: true,
  forbidOnly: true,
  retries: 0,
  timeout: 30_000,
  expect: { timeout: 5_000 },
  reporter: [["list"]],
  outputDir: "test-results",
  use: {
    baseURL: `http://localhost:${PORT}/`,
    trace: "retain-on-failure",
  },
  projects: [{ name: "chromium", use: { ...devices["Desktop Chrome"], viewport: { width: 1000, height: 800 } } }],
  webServer: {
    command: `npm run preview -- --port ${PORT}`,
    url: `http://localhost:${PORT}/blank.html`,
    reuseExistingServer: false,
    stdout: "ignore",
    stderr: "pipe",
    timeout: 30_000,
  },
});
