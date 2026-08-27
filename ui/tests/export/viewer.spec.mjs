// The exported HTML, opened as a file (ADR-0009, spec/export.md).
//
// Everything else in the suite runs the *app* against the mock bridge. The
// export is a different artifact — the same modules concatenated into one
// classic script with no imports and no IPC — and nothing exercised it, which
// is how it shipped with code level (L4) missing altogether while the app had
// had it since 0.3.0.
//
// Needs architecture.html built first:
//   cargo run -p blastradius-cli -- export docs -o architecture.html
import { test, expect } from '@playwright/test';
import { pathToFileURL } from 'node:url';
import { resolve } from 'node:path';

const FILE = pathToFileURL(resolve(process.cwd(), 'architecture.html')).href;

/** The radio inputs are visually hidden; the label is the control. */
const pick = (page, text) => page.locator('#level-seg .seg-opt', { hasText: text }).click();

test.beforeEach(async ({ page }) => {
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  page.on('console', (m) => m.type() === 'error' && errors.push(m.text()));
  page.errors = errors;
  await page.goto(FILE);
  await expect(page.locator('#nodes .node')).not.toHaveCount(0);
});

test('opens from file:// with no network and no errors', async ({ page }) => {
  await expect(page.locator('#breadcrumb')).toContainText('Context');
  expect(page.errors).toEqual([]);
});

test('the L4 segment is live because the model carries derived facts', async ({ page }) => {
  await expect(page.locator('#level-seg input[value="L4"]')).toBeEnabled();
  await pick(page, 'L4');
  await expect(page.locator('.node.is-derived')).not.toHaveCount(0);
  await expect(page.locator('#breadcrumb')).toContainText('Code');
  expect(page.errors).toEqual([]);
});

test('diving from a component reaches its code, and Esc comes back', async ({ page }) => {
  await pick(page, 'L4');
  await expect(page.locator('.node.is-derived')).not.toHaveCount(0);

  // Down one more step: a module opens its types.
  const before = await page.locator('#nodes .node').count();
  const withChildren = page.locator('#nodes .node.is-derived', { has: page.locator('.node-meta') }).first();
  if (await withChildren.count()) {
    await withChildren.dblclick();
    await expect.poll(() => page.locator('#nodes .node').count()).not.toBe(before);
  }

  await page.locator('#canvas').click({ position: { x: 5, y: 5 } });
  await page.keyboard.press('Escape');
  await expect(page.locator('#breadcrumb')).not.toContainText('Context');
  expect(page.errors).toEqual([]);
});

test('the inspector reports where derived code lives, and offers nothing to click', async ({ page }) => {
  await pick(page, 'L4');
  await page.locator('.node.is-derived').first().click();
  const side = page.locator('#side-body');
  await expect(side.locator('.insp-title')).not.toBeEmpty();
  await expect(side).toContainText(/Source|External/);
  // An export has no machine to open a file on — the app's open-in-editor
  // button must not survive into it.
  await expect(side.locator('[data-opensrc]')).toHaveCount(0);
  expect(page.errors).toEqual([]);
});

test('the tree lists code under its component', async ({ page }) => {
  await expect(page.locator('.tree-row.is-derived')).not.toHaveCount(0);
  await page.locator('.tree-row.is-derived').first().click();
  await expect(page.locator('#breadcrumb')).toContainText('Code');
  await expect(page.locator('#nodes .node.is-active')).toHaveCount(1);
  expect(page.errors).toEqual([]);
});

test('the deployment altitude still works alongside it', async ({ page }) => {
  await expect(page.locator('#level-seg input[value="LD"]')).toBeEnabled();
  await pick(page, 'D');
  await expect(page.locator('.node.is-environment')).not.toHaveCount(0);
  expect(page.errors).toEqual([]);
});

test('a nested deployment view exports as containment, not as a dive', async ({ page }) => {
  // `nested: true` is a view option (ADR-0018), so the export has to honour
  // it or the shared file disagrees with the app about what the view is.
  await pick(page, 'D');
  await page.locator('#nodes .node', {
    has: page.locator('.node-title', { hasText: 'Developer Machine' }),
  }).dblclick();
  await expect(page.locator('.node.is-nested')).not.toHaveCount(0);
  await expect(page.locator('.node.is-container-instance')).not.toHaveCount(0);
  expect(page.errors).toEqual([]);
});
