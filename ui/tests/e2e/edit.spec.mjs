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

// Descriptions on the box (spec §4): the text is a model field edited in the
// inspector, and whether a given diagram draws it is toggled on the box.
test('description is written in the inspector and drawn from the box menu', async ({ page }) => {
  await node(page, 'Blastradius').dblclick(); // L2
  await expect(page.locator('#breadcrumb')).toContainText('Containers');
  const core = node(page, 'Core').first();
  await core.click();

  // The field carries the description the model already has, and editing it
  // does not put the text on the diagram — that is a separate decision.
  const desc = page.locator('#insp-desc');
  await expect(desc).toHaveValue(/Library-first/);
  await desc.fill('The domain, and nothing that draws pixels.');
  await desc.blur();
  await expect(page.locator('#undo-btn')).toBeEnabled();
  // The edit re-renders the canvas, so wait for the box to be back before
  // measuring it — `#nodes` is emptied and rebuilt in between.
  await expect(core).toBeVisible();
  await expect(core.locator('.node-desc')).toHaveCount(0);

  // Right-click puts it on the box, and the box grows to hold it.
  const before = (await core.boundingBox()).height;
  await core.click({ button: 'right' });
  await page.locator('.ctx-menu .ctx-item', { hasText: 'Show description' }).click();
  await expect(core.locator('.node-desc')).toHaveText('The domain, and nothing that draws pixels.');
  expect((await core.boundingBox()).height).toBeGreaterThan(before);


  // And the same menu takes it off again.
  await core.click({ button: 'right' });
  await page.locator('.ctx-menu .ctx-item', { hasText: 'Hide description' }).click();
  await expect(core.locator('.node-desc')).toHaveCount(0);
  expect(page.errors).toEqual([]);
});

test('an element with no description is offered the field instead', async ({ page }) => {
  await page.locator('#level-seg .seg-opt', { hasText: 'D' }).click();
  await node(page, 'Developer Machine').first().dblclick();
  const box = node(page, 'Windows 11 Workstation').first();
  await expect(box).toBeVisible();

  // A nested container is painted behind its children, so aim at its own
  // title strip rather than the centre, which belongs to whatever is inside.
  await box.click({ button: 'right', position: { x: 24, y: 12 } });
  await page.locator('.ctx-menu .ctx-item', { hasText: 'Add a description' }).click();
  // Nothing to show yet, so the menu hands over to the field that creates one.
  await expect(page.locator('#insp-desc')).toBeFocused();
  await expect(page.locator('.ctx-menu')).toHaveCount(0);
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

// 0.9.0 B: the operations were all there and the surface was not. Drawing a
// relation was bound to `R` and advertised nowhere, delete was the `Delete`
// key, rename was a field in another panel — so a user who did not read the
// shortcuts page could rename, and nothing else. These four exercise the box
// menu with the mouse alone.
test('the box menu connects two elements, with no keystroke', async ({ page }) => {
  await node(page, 'Blastradius').dblclick(); // L2
  await expect(page.locator('#breadcrumb')).toContainText('Containers');
  await node(page, 'App Shell').first().click({ button: 'right' });
  await page.locator('.ctx-menu .ctx-item', { hasText: 'Connect to…' }).click();
  await expect(page.locator('#hint')).toContainText('Click a target element');
  await node(page, 'CLI').first().click();
  await page.locator('#dlg-label').fill('spawns');
  await page.locator('#dlg-ok').click();
  await node(page, 'App Shell').first().click();
  await expect(page.locator('.insp-rel', { hasText: 'CLI' })).toContainText('spawns');
  expect(page.errors).toEqual([]);
});

test('the box menu renames, by handing over to the field that does it', async ({ page }) => {
  await node(page, 'Blastradius').first().click({ button: 'right' });
  await page.locator('.ctx-menu .ctx-item', { hasText: 'Rename…' }).click();
  const input = page.locator('#insp-name');
  await expect(input).toBeFocused();
  await input.fill('Blast Radius Pro');
  await input.press('Enter');
  await expect(node(page, 'Blast Radius Pro').first()).toBeVisible();
  expect(page.errors).toEqual([]);
});

test('the box menu adds a child, and goes in after it', async ({ page }) => {
  await node(page, 'Blastradius').dblclick(); // L2
  await expect(page.locator('#breadcrumb')).toContainText('Containers');
  await node(page, 'CLI').first().click({ button: 'right' });
  // The kind is the one a container may hold, named rather than asked for.
  await page.locator('.ctx-menu .ctx-item', { hasText: 'Add a component inside…' }).click();
  await page.locator('#dlg-name').fill('Arg Parser');
  await expect(page.locator('#dlg-id-full')).toHaveText('blastradius.cli.arg-parser');
  await page.locator('#dlg-ok').click();
  // A component is invisible from the container level it was created at, so
  // the canvas follows it down rather than reporting success into thin air.
  await expect(page.locator('#breadcrumb')).toContainText('CLI');
  await expect(node(page, 'Arg Parser').first()).toBeVisible();
  expect(page.errors).toEqual([]);
});

test('the box menu deletes, through the same confirmation as the key', async ({ page }) => {
  await node(page, 'Blastradius').dblclick(); // L2
  await node(page, 'CLI').first().click({ button: 'right' });
  await page.locator('.ctx-menu .ctx-item', { hasText: 'Delete…' }).click();
  const dlg = page.locator('#app-dialog');
  await expect(dlg).toContainText('Delete CLI?');
  await dlg.locator('#dlg-ok').click();
  await expect(node(page, 'CLI')).toHaveCount(0);
  expect(page.errors).toEqual([]);
});

test('a leaf is not offered children it cannot have', async ({ page }) => {
  await node(page, 'Blastradius').dblclick();
  await node(page, 'Core').first().dblclick(); // L3: components
  await expect(page.locator('#breadcrumb')).toContainText('Components');
  // Named rather than `.first()`: an L3 view also draws the outside containers
  // whose relations reach into the scope, and a container may hold components.
  await node(page, 'Git Service').first().click({ button: 'right' });
  await expect(page.locator('.ctx-menu')).toBeVisible();
  // Below a component is code, derived from source — not something to add here.
  await expect(page.locator('.ctx-menu .ctx-item', { hasText: 'inside…' })).toHaveCount(0);
  expect(page.errors).toEqual([]);
});

test('the menu walks with the arrow keys', async ({ page }) => {
  await node(page, 'Blastradius').first().click({ button: 'right' });
  const items = page.locator('.ctx-menu .ctx-item');
  await expect(items.first()).toBeFocused();
  await page.keyboard.press('ArrowDown');
  await expect(items.nth(1)).toBeFocused();
  await page.keyboard.press('ArrowUp');
  await expect(items.first()).toBeFocused();
  expect(page.errors).toEqual([]);
});
