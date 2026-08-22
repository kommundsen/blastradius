// Phase 5 onboarding: the welcome screen (no workspace open) and the runtime
// workspace-switch flow, exercised through the mock bridge (?noworkspace).
import { test, expect } from '@playwright/test';

test('welcome screen renders when no workspace is open', async ({ page }) => {
  await page.goto('/index.html?nogit&noworkspace');
  const welcome = page.locator('.welcome');
  await expect(welcome).toBeVisible();
  await expect(welcome.getByRole('button', { name: /open a workspace folder/i })).toBeVisible();
  await expect(welcome.getByRole('button', { name: /new workspace/i })).toBeVisible();
  await expect(welcome.getByRole('button', { name: /demo workspace/i })).toBeVisible();
  // no model chrome pretends to work
  await expect(page.locator('#nodes .node')).toHaveCount(0);
});

test('demo flow leaves the welcome screen and renders a model', async ({ page }) => {
  await page.goto('/index.html?nogit&noworkspace');
  await page.locator('#welcome-demo').click();
  await expect(page.locator('.welcome')).toHaveCount(0);
  await expect(page.locator('#nodes .node').first()).toBeVisible();
});

test('a normal launch never shows the welcome screen', async ({ page }) => {
  await page.goto('/index.html?nogit');
  await expect(page.locator('#nodes .node').first()).toBeVisible();
  await expect(page.locator('.welcome')).toHaveCount(0);
  // the Open button is present for runtime switching
  await expect(page.locator('#open-btn')).toBeVisible();
});
