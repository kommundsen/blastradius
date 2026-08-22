// WebKit is the gate (ADR-0011): it is the constraining engine (macOS Tauri)
// and the one a native-window suite cannot reach in CI. Chromium/WebView2 is
// covered by the Windows dev machine; add it here only if it starts drifting.
import { defineConfig, devices } from '@playwright/test';

export default defineConfig({
  testDir: 'ui/tests/e2e',
  timeout: 30_000,
  retries: process.env.CI ? 1 : 0,
  reporter: process.env.CI ? [['list'], ['html', { open: 'never' }]] : 'list',
  use: {
    baseURL: 'http://127.0.0.1:4173',
    screenshot: 'only-on-failure',
  },
  projects: [{ name: 'webkit', use: { ...devices['Desktop Safari'] } }],
  webServer: {
    command: 'node ui/tests/serve.mjs 4173',
    url: 'http://127.0.0.1:4173/index.html',
    reuseExistingServer: !process.env.CI,
  },
});
