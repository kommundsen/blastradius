// Phase 5 onboarding: the welcome screen (no workspace open) and the runtime
// workspace-switch flow, exercised through the mock bridge (?noworkspace).
import { test, expect } from '@playwright/test';

test('welcome screen offers one way in', async ({ page }) => {
  await page.goto('/index.html?nogit&noworkspace');
  const welcome = page.locator('.welcome');
  await expect(welcome).toBeVisible();
  // One primary action. "New workspace in a folder…" is gone: opening a
  // folder with nothing in it now offers to make one, so a second button
  // asking the same question up front was a choice with no information behind
  // it (docs/roadmap.md, first-user findings).
  await expect(welcome.getByRole('button', { name: /open a folder or repository/i })).toBeVisible();
  await expect(welcome.getByRole('button', { name: /new workspace/i })).toHaveCount(0);
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

test('a folder with no workspace is an offer, not an error', async ({ page }) => {
  // The whole first-run experience for the first outside user was an error
  // naming the folder they had just picked.
  await page.goto('/index.html?nogit&noworkspace&emptyfolder');
  await page.locator('#welcome-open').click();

  const dialog = page.locator('#app-dialog');
  await expect(dialog.locator('.dialog-title')).toHaveText(/start a model here/i);
  await expect(dialog.getByText('/home/dev/my-repo')).toBeVisible();
  // The agent wiring is offered along with it, and on by default — it is what
  // makes the next step possible.
  await expect(page.locator('#dlg-agents')).toBeChecked();
  await page.locator('#dlg-ok').click();

  // Scaffolded, opened, and then handed the thing to do next.
  await expect(page.locator('.welcome')).toHaveCount(0);
  await expect(page.locator('#app-dialog .dialog-title')).toHaveText(/now ask your agent/i);
  await expect(page.locator('#dlg-prompt')).toContainText('model its architecture');
  await expect(page.locator('#app-dialog')).toContainText('.mcp.json');
});

test('declining the agent setup still creates the workspace', async ({ page }) => {
  await page.goto('/index.html?nogit&noworkspace&emptyfolder');
  await page.locator('#welcome-open').click();
  await page.locator('#dlg-agents').uncheck();
  await page.locator('#dlg-ok').click();
  // No agents wired, so no prompt to hand over — just the model.
  await expect(page.locator('#app-dialog')).toHaveCount(0);
  await expect(page.locator('#nodes .node').first()).toBeVisible();
});

test('a normal launch never shows the welcome screen', async ({ page }) => {
  await page.goto('/index.html?nogit');
  await expect(page.locator('#nodes .node').first()).toBeVisible();
  await expect(page.locator('.welcome')).toHaveCount(0);
  // the Open button is present for runtime switching
  await expect(page.locator('#open-btn')).toBeVisible();
});
