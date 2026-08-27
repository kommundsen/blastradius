// Find anything (docs/roadmap.md 0.7.0): the palette in the constraining
// engine (ADR-0011). The ranking is unit-tested in ui/tests/search.test.mjs;
// what needs a browser is that the keyboard opens it, that Enter lands the
// camera on the right thing, and that it reaches the two kinds the sidebar
// tree cannot show — relations and code-level detail.
import { test, expect } from '@playwright/test';

test.beforeEach(async ({ page }) => {
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  page.on('console', (m) => m.type() === 'error' && errors.push(m.text()));
  page.errors = errors;
  await page.goto('/index.html');
  await expect(page.locator('#nodes .node')).not.toHaveCount(0);
});

const open = async (page) => {
  await page.keyboard.press('Control+k');
  await expect(page.locator('#app-palette')).toBeVisible();
  return page.locator('#palette-q');
};

test('Ctrl+K opens the palette onto the context altitude', async ({ page }) => {
  await open(page);
  // An empty query is not a blank list: it offers the top of the model.
  await expect(page.locator('.palette-row')).not.toHaveCount(0);
  expect(page.errors).toEqual([]);
});

test('Escape closes it, and Ctrl+K toggles', async ({ page }) => {
  await open(page);
  await page.keyboard.press('Escape');
  await expect(page.locator('#app-palette')).toHaveCount(0);
  await page.keyboard.press('Control+k');
  await expect(page.locator('#app-palette')).toBeVisible();
  await page.keyboard.press('Control+k');
  await expect(page.locator('#app-palette')).toHaveCount(0);
});

test('the Find button opens the same thing', async ({ page }) => {
  await page.locator('#find-btn').click();
  await expect(page.locator('#app-palette')).toBeVisible();
});

test('Enter flies to a component at its own altitude', async ({ page }) => {
  const input = await open(page);
  await input.fill('git-service');
  await expect(page.locator('.palette-row').first()).toContainText('Git Service');
  await page.keyboard.press('Enter');

  await expect(page.locator('#app-palette')).toHaveCount(0);
  // A component lives at L3, and we started at L1 — the palette had to move
  // the camera, not just change the selection.
  await expect(page.locator('#breadcrumb')).toContainText('Components');
  await expect(page.locator('#nodes .node.is-active')).toHaveCount(1);
  expect(page.errors).toEqual([]);
});

test('a document opens in the side panel', async ({ page }) => {
  const input = await open(page);
  await input.fill('adr-0006');
  await expect(page.locator('.palette-row').first()).toContainText('adr-0006');
  await page.keyboard.press('Enter');
  await expect(page.locator('#side-title')).toHaveText('adr-0006');
});

test('arrow keys move the highlight', async ({ page }) => {
  const input = await open(page);
  await input.fill('a');
  const rows = page.locator('.palette-row');
  await expect(rows.nth(0)).toHaveClass(/is-active/);
  await page.keyboard.press('ArrowDown');
  await expect(rows.nth(1)).toHaveClass(/is-active/);
  await expect(rows.nth(0)).not.toHaveClass(/is-active/);
});

test('a query that matches nothing says so', async ({ page }) => {
  const input = await open(page);
  await input.fill('zzzzznotathing');
  await expect(page.locator('.palette-row')).toHaveCount(0);
  await expect(page.locator('.palette-empty')).toBeVisible();
});
