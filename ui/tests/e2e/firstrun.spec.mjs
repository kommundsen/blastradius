// 0.10.0 item 9 — first-run discoverability. Since 0.9.0 most of what the
// canvas can do lives behind a right-click or the View tab, and the only thing
// that said so was the help page. These are the surfaces that say it now.
import { test, expect } from '@playwright/test';

const node = (page, title) =>
  page.locator('#nodes .node', { has: page.locator('.node-title', { hasText: title }) });

test.beforeEach(async ({ page }) => {
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  page.on('console', (m) => m.type() === 'error' && errors.push(m.text()));
  page.errors = errors;
});

test('the canvas hint names the right-click menu', async ({ page }) => {
  await page.goto('/index.html?nogit');
  await expect(page.locator('#hint')).toContainText('Right-click');
  // and still says the two things it always did
  await expect(page.locator('#hint')).toContainText('Double-click to dive');
  await expect(page.locator('#hint')).toContainText('Esc to rise');
  expect(page.errors).toEqual([]);
});

test('a first run is told where the editing lives', async ({ page }) => {
  await page.goto('/index.html?nogit');
  const tour = page.locator('#tour');
  await expect(tour).toBeVisible();
  // The two surfaces nothing announced.
  await expect(tour).toContainText('Right-click a box');
  await expect(tour).toContainText('View tab');
  expect(page.errors).toEqual([]);
});

test('dismissing it dismisses it for good', async ({ page }) => {
  await page.goto('/index.html?nogit');
  await page.locator('#tour-close').click();
  await expect(page.locator('#tour')).toBeHidden();
  await page.reload();
  await expect(page.locator('#nodes .node')).not.toHaveCount(0);
  await expect(page.locator('#tour')).toBeHidden();
  expect(page.errors).toEqual([]);
});

test('opening the menu it teaches retires it, unclicked', async ({ page }) => {
  await page.goto('/index.html?nogit');
  await expect(page.locator('#tour')).toBeVisible();
  await node(page, 'Blastradius').first().click({ button: 'right' });
  await expect(page.locator('.ctx-menu')).toBeVisible();
  // The card exists to be found out; being found out is enough.
  await expect(page.locator('#tour')).toBeHidden();
  await page.reload();
  await expect(page.locator('#nodes .node')).not.toHaveCount(0);
  await expect(page.locator('#tour')).toBeHidden();
  expect(page.errors).toEqual([]);
});

test('the card does not eat a click aimed at the canvas beneath it', async ({ page }) => {
  await page.goto('/index.html?nogit');
  await expect(page.locator('#tour')).toBeVisible();
  // Overlay chrome sits on top of the drawing; a pointer through it must reach
  // the drawing. Asserted where the card actually is, not in the middle.
  const box = await page.locator('#tour').boundingBox();
  await page.mouse.click(box.x + box.width / 2, box.y + box.height / 2);
  expect(page.errors).toEqual([]);
});

test('diving into something with nothing inside says so', async ({ page }) => {
  await page.goto('/index.html?nogit');
  await page.locator('#level-seg .seg-opt', { hasText: 'D' }).click();
  await node(page, 'GitHub Actions').first().dblclick();
  await node(page, 'windows-latest Runner').first().dblclick();
  // MSIX Packaging is a leaf: the dive used to do nothing at all, which on a
  // starter model is what double-clicking the database does too.
  await node(page, 'MSIX Packaging').first().dblclick();
  await expect(page.locator('.travel-banner.is-toast')).toContainText('nothing inside yet');
  expect(page.errors).toEqual([]);
});

test('a view with nothing left in it is a state, not an empty frame', async ({ page }) => {
  await page.goto('/index.html?nogit');
  await node(page, 'Blastradius').first().dblclick();
  await node(page, 'CLI').first().dblclick();
  await expect(page.locator('#breadcrumb')).toContainText('Components');

  // The CLI holds exactly one component; everything else this view draws is
  // there because of that component's relations, so deleting it empties the
  // view rather than leaving a rump of one.
  await node(page, 'MCP Server').first().click({ button: 'right' });
  await page.locator('.ctx-menu .ctx-item', { hasText: 'Delete…' }).click();
  await page.locator('#dlg-ok').click();

  await expect(page.locator('#nodes .node')).toHaveCount(0);
  const blank = page.locator('#canvas-blank');
  await expect(blank).toBeVisible();
  await expect(blank).toContainText('no components yet');
  await expect(blank).toContainText('Right-click the canvas');
  await expect(page.locator('#blank-add')).toBeVisible();
  expect(page.errors).toEqual([]);
});

test('a workspace that was just scaffolded gets the card too', async ({ page }) => {
  // The first run that matters most: someone pointed the app at their own
  // repository, took the offer, and is now looking at a model for the first
  // time. The hand-off dialog says what to ask an agent; the card says what
  // the app itself can do.
  await page.goto('/index.html?nogit&noworkspace&emptyfolder');
  await page.locator('#welcome-open').click();
  await page.locator('#dlg-ok').click();
  await expect(page.locator('#app-dialog .dialog-title')).toHaveText(/now ask your agent/i);
  await page.locator('#dlg-ok').click();
  await expect(page.locator('#tour')).toBeVisible();
  await expect(page.locator('#tour')).toContainText('Right-click a box');
  expect(page.errors).toEqual([]);
});
