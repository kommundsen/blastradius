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
  // It says up front that nothing of yours gets overwritten — the 0.6.0 build
  // treated an existing README as fatal instead.
  await expect(dialog).toContainText(/already exist are left alone/i);

  // Where it goes is asked, with docs/ recommended: a repository root is for
  // source, and the model is documentation.
  await expect(page.locator('#dlg-location')).toHaveValue('docs');

  // The pieces are chosen, not all-or-nothing: which parts, which agents,
  // the same choice `blastradius init` offers. All on by default.
  await expect(page.locator('#dlg-mcp')).toBeChecked();
  await expect(page.locator('#dlg-skills')).toBeChecked();
  await expect(page.locator('.dlg-agent')).toHaveCount(4);
  for (const id of ['claude', 'copilot', 'cursor', 'codex']) {
    await expect(page.locator(`.dlg-agent[value="${id}"]`)).toBeChecked();
  }
  await page.locator('#dlg-ok').click();

  // Scaffolded, opened, and then handed the thing to do next.
  await expect(page.locator('.welcome')).toHaveCount(0);
  await expect(page.locator('#app-dialog .dialog-title')).toHaveText(/now ask your agent/i);
  await expect(page.locator('#dlg-prompt')).toContainText('model its architecture');
  await expect(page.locator('#app-dialog')).toContainText('mcp config (claude)');
});

test('the agent selection is honoured, not ignored', async ({ page }) => {
  await page.goto('/index.html?nogit&noworkspace&emptyfolder');
  await page.locator('#welcome-open').click();
  // Skills only, and only for Cursor.
  await page.locator('#dlg-mcp').uncheck();
  for (const id of ['claude', 'copilot', 'codex']) {
    await page.locator(`.dlg-agent[value="${id}"]`).uncheck();
  }
  await page.locator('#dlg-ok').click();

  const dialog = page.locator('#app-dialog');
  await expect(dialog.locator('.dialog-title')).toHaveText(/now ask your agent/i);
  await expect(dialog).toContainText('skill (cursor)');
  await expect(dialog).not.toContainText('mcp config');
  await expect(dialog).not.toContainText('(claude)');
});

test('declining everything still creates the workspace and closes', async ({ page }) => {
  await page.goto('/index.html?nogit&noworkspace&emptyfolder');
  await page.locator('#welcome-open').click();
  await page.locator('#dlg-mcp').uncheck();
  await page.locator('#dlg-skills').uncheck();
  await page.locator('#dlg-ok').click();
  // Nothing wired, so no prompt to hand over — just the model, and the dialog
  // goes away rather than sitting there.
  await expect(page.locator('#app-dialog')).toHaveCount(0);
  await expect(page.locator('#nodes .node').first()).toBeVisible();
});

test('a project that already has a doc folder is offered that one', async ({ page }) => {
  // Recommending `docs` next to an existing `doc/` would create a
  // near-duplicate of a folder the project already keeps its docs in.
  await page.goto('/index.html?nogit&noworkspace&emptyfolder&hasdoc');
  await page.locator('#welcome-open').click();
  await expect(page.locator('#dlg-location')).toHaveValue('doc');
});

test('the location is a recommendation, not a rule', async ({ page }) => {
  await page.goto('/index.html?nogit&noworkspace&emptyfolder');
  await page.locator('#welcome-open').click();
  await page.locator('#dlg-location').fill('.');
  await page.locator('#dlg-ok').click();
  // Chose the project root: still scaffolds and opens.
  await expect(page.locator('#app-dialog .dialog-title')).toHaveText(/now ask your agent/i);
});

test('files that were already there are reported as kept', async ({ page }) => {
  await page.goto('/index.html?nogit&noworkspace&emptyfolder&hasreadme');
  await page.locator('#welcome-open').click();
  await page.locator('#dlg-ok').click();
  // The markup wraps, so match the two halves rather than one exact phrase.
  await expect(page.locator('#app-dialog')).toContainText(/Kept your existing/i);
  await expect(page.locator('#app-dialog')).toContainText(/README\.md — untouched/i);
});

test('a normal launch never shows the welcome screen', async ({ page }) => {
  await page.goto('/index.html?nogit');
  await expect(page.locator('#nodes .node').first()).toBeVisible();
  await expect(page.locator('.welcome')).toHaveCount(0);
  // the Open button is present for runtime switching
  await expect(page.locator('#open-btn')).toBeVisible();
});
