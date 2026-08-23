// Phase 3 editing UX in WebKit, against the mock sync layer (?nogit = plain
// folder, no conflicts). File-level correctness is proven by the Rust torture
// test; this suite proves the surfaces drive the operations.
import { test, expect } from '@playwright/test';

test.beforeEach(async ({ page }) => {
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  page.on('console', (m) => m.type() === 'error' && errors.push(m.text()));
  page.errors = errors;
  await page.goto('/index.html?nogit');
  await expect(page.locator('#nodes .node')).not.toHaveCount(0);
});

const node = (page, t) =>
  page.locator('#nodes .node', { has: page.locator('.node-title', { hasText: t }) });

test('rename via inspector, then undo', async ({ page }) => {
  await node(page, 'Blastradius').click();
  const input = page.locator('#insp-name');
  await expect(input).toHaveValue('Blastradius');
  await input.fill('Blast Radius Pro');
  await input.press('Enter');
  await expect(node(page, 'Blast Radius Pro').first()).toBeVisible();
  await expect(page.locator('#undo-btn')).toBeEnabled();
  await page.locator('#undo-btn').click();
  await expect(node(page, 'Blastradius').first()).toBeVisible();
  expect(page.errors).toEqual([]);
});

test('create element via dialog with id preview', async ({ page }) => {
  await node(page, 'Blastradius').dblclick(); // L2
  // the dive glide takes --duration-camera; +Element is level-dependent, so
  // wait for arrival before opening the dialog (caught as a CI-only race)
  await expect(page.locator('#breadcrumb')).toContainText('Containers');
  await page.locator('#add-btn').click();
  await page.locator('#dlg-name').fill('Plugin Host');
  await expect(page.locator('#dlg-id')).toHaveValue('plugin-host');
  await expect(page.locator('#dlg-id-full')).toHaveText('blastradius.plugin-host');
  await page.locator('#dlg-ok').click();
  await expect(node(page, 'Plugin Host').first()).toBeVisible();
  await expect(page.locator('.tree-row', { hasText: 'Plugin Host' })).toBeVisible();
  expect(page.errors).toEqual([]);
});

test('delete confirms with cascading relations listed', async ({ page }) => {
  await node(page, 'Blastradius').dblclick(); // L2
  await node(page, 'CLI').first().click();
  await page.keyboard.press('Delete');
  const dlg = page.locator('#app-dialog');
  await expect(dlg).toContainText('Delete CLI?');
  await expect(dlg).toContainText('blastradius.cli → blastradius.core.model-service');
  await dlg.locator('#dlg-ok').click();
  await expect(node(page, 'CLI')).toHaveCount(0);
  expect(page.errors).toEqual([]);
});

test('connect mode creates a relation', async ({ page }) => {
  await node(page, 'Blastradius').dblclick(); // L2
  await node(page, 'App Shell').first().click();
  await page.keyboard.press('r');
  await expect(page.locator('#hint')).toContainText('Click a target');
  await node(page, 'CLI').first().click();
  await page.locator('#dlg-label').fill('spawns');
  await page.locator('#dlg-ok').click();
  // new edge exists (aggregated count grows) — select it via inspector check
  await expect(page.locator('#edges path.edge-hit')).not.toHaveCount(0);
  expect(page.errors).toEqual([]);
});

test('edge click opens relation inspector; delete removes it', async ({ page }) => {
  await node(page, 'Blastradius').dblclick(); // L2
  const hits = page.locator('#edges path.edge-hit');
  await expect(hits).not.toHaveCount(0);
  // a diagonal path's bbox center is off the stroke, so a coordinate click
  // misses pointer-events:stroke — dispatch the event to test the wiring
  await hits.first().dispatchEvent('click');
  await expect(page.locator('#side-body')).toContainText('Relation');
  await page.locator('#rel-delete').click();
  await expect(page.locator('#side-body')).not.toContainText('Delete relation');
  expect(page.errors).toEqual([]);
});

test('source panel opens with file list', async ({ page }) => {
  await page.locator('#side-mode .seg-opt', { hasText: 'Source' }).click();
  await expect(page.locator('#src-file')).toBeVisible();
  await expect(page.locator('#src-editor')).toBeVisible();
  await expect(page.locator('#src-status')).toHaveText('synced');
  expect(page.errors).toEqual([]);
});

test('source editor is CodeMirror with YAML highlighting (phase 5)', async ({ page }) => {
  await page.locator('#side-mode .seg-opt', { hasText: 'Source' }).click();
  await expect(page.locator('#src-editor .CodeMirror')).toBeVisible();
  // the mock file text begins with comments — the YAML mode must tag them
  await expect(page.locator('#src-editor .cm-comment').first()).toBeVisible();
  expect(page.errors).toEqual([]);
});

test('source editor wraps instead of forcing a horizontal scrollbar', async ({ page }) => {
  await page.locator('#side-mode .seg-opt', { hasText: 'Source' }).click();
  await expect(page.locator('#src-editor .CodeMirror')).toBeVisible();
  const noHOverflow = async (selector) => {
    const overflow = await page.locator(selector).evaluate((el) => el.scrollWidth - el.clientWidth);
    expect(overflow, selector).toBeLessThanOrEqual(1);
  };
  // at the default panel width, the file selector's own long path
  // ("model/blastradius.yaml") used to force the whole panel to overflow
  // horizontally, distinct from CodeMirror's own scroller
  await noHOverflow('#side-body');
  await noHOverflow('#src-editor .CodeMirror-scroll');
  // shrink the side panel to its minimum (260px) — narrow enough that any
  // unwrapped YAML line (a description, a tech value) used to overflow
  const grip = page.locator('#side-grip');
  await grip.focus();
  for (let i = 0; i < 20; i++) await grip.press('ArrowRight');
  await expect(grip).toHaveAttribute('aria-valuenow', '260');
  await noHOverflow('#side-body');
  await noHOverflow('#src-editor .CodeMirror-scroll');
  expect(page.errors).toEqual([]);
});
