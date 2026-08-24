// Bundled in-app help (docs/roadmap.md 0.4.0 theme 3): reachable, offline,
// and navigable without leaving the app.
import { test, expect } from '@playwright/test';

test.beforeEach(async ({ page }) => {
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  page.on('console', (m) => m.type() === 'error' && errors.push(m.text()));
  page.errors = errors;
  await page.goto('/index.html');
  await expect(page.locator('#nodes .node')).not.toHaveCount(0);
});

test('Help opens an index covering every feature area', async ({ page }) => {
  await page.locator('#help-btn').click();
  await expect(page.locator('#side-title')).toHaveText('Help');

  // The exit criterion: every shipped feature is reachable from here.
  for (const title of [
    'Getting started',
    'Navigating the canvas',
    'Editing the model',
    'Deployment views',
    'Code-level detail (L4)',
    'Git: diff, history, conflicts',
    'Sharing and export',
    'Coding agents (MCP)',
    'Model format reference',
    'Keyboard shortcuts',
    'Privacy',
  ]) {
    await expect(page.locator('#side-body [data-help]', { hasText: title })).toBeVisible();
  }
  expect(page.errors).toEqual([]);
});

test('a page renders its markdown, and cross-links stay in the panel', async ({ page }) => {
  await page.locator('#help-btn').click();
  await page.locator('#side-body [data-help="getting-started"]').click();
  await expect(page.locator('#side-title')).toHaveText('Getting started');
  await expect(page.locator('.doc-body h1')).toHaveText('Getting started');
  await expect(page.locator('.doc-body table')).toBeVisible(); // the altitude table
  await expect(page.locator('.doc-body pre')).not.toHaveCount(0); // the YAML samples

  // A link to another help page navigates in place rather than unloading the
  // app — the panel has no router to come back through.
  const url = page.url();
  await page.locator('.doc-body a[data-help="canvas"]').first().click();
  await expect(page.locator('#side-title')).toHaveText('Navigating the canvas');
  expect(page.url()).toBe(url);

  // Back returns to the index, not to the inspector.
  await page.locator('#side-back').click();
  await expect(page.locator('#side-title')).toHaveText('Help');
  expect(page.errors).toEqual([]);
});

test('help is keyboard-reachable and toggles', async ({ page }) => {
  await page.locator('#canvas').click({ position: { x: 5, y: 5 } });
  await page.keyboard.press('?');
  await expect(page.locator('#side-title')).toHaveText('Help');
  await page.keyboard.press('?');
  await expect(page.locator('#side-title')).not.toHaveText('Help');
  expect(page.errors).toEqual([]);
});

test('help never reaches the network', async ({ page }) => {
  const external = [];
  await page.route('**', (route) => {
    const url = route.request().url();
    if (!url.startsWith('http://127.0.0.1') && !url.startsWith('http://localhost')) external.push(url);
    return route.continue();
  });
  await page.locator('#help-btn').click();
  for (const id of ['getting-started', 'deployment', 'privacy']) {
    await page.locator(`#side-body [data-help="${id}"]`).click();
    await expect(page.locator('.doc-body')).toBeVisible();
    await page.locator('#side-back').click();
  }
  expect(external).toEqual([]);
});

test('the welcome screen offers help before any workspace is open', async ({ page }) => {
  await page.goto('/index.html?nogit&noworkspace');
  await expect(page.locator('.welcome')).toBeVisible();
  await page.locator('#welcome-help').click();
  await expect(page.locator('#side-title')).toHaveText('Help');
  expect(page.errors).toEqual([]);
});
