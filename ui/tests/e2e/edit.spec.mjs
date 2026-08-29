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

  // Right-click puts it on the box, and the box grows to hold it. Measured by
  // polling, not by one read: an edit empties and rebuilds `#nodes`, so a
  // handle can be detached at the moment it is asked for its size.
  const height = async () => (await core.boundingBox())?.height ?? 0;
  await expect.poll(height).toBeGreaterThan(0);
  const before = await height();
  await core.click({ button: 'right' });
  await page.locator('.ctx-menu .ctx-item', { hasText: 'Show description' }).click();
  await expect(core.locator('.node-desc')).toHaveText('The domain, and nothing that draws pixels.');
  await expect.poll(height, { message: 'the box grows to hold the description' })
    .toBeGreaterThan(before);


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

// 0.9.0 C: the inspector edited two fields of an element and the format has
// several more. `tech` was whitelisted by the engine and offered by nothing —
// while every box renders it in brackets — and `group`, `replicas`, `external`
// and `source:` had no operation at all, so they were YAML-only.
test('technology and group are written from the inspector', async ({ page }) => {
  await node(page, 'Blastradius').dblclick(); // L2
  await expect(page.locator('#breadcrumb')).toContainText('Containers');
  await node(page, 'CLI').first().click();

  const tech = page.locator('#insp-tech');
  await expect(tech).toHaveValue('Rust');
  await tech.fill('Rust + clap');
  await tech.press('Enter');
  // The kicker is where technology reads, in C4 brackets.
  await expect(page.locator('.insp-kicker')).toHaveText('[Container: Rust + clap]');

  const group = page.locator('#insp-group');
  await expect(group).toHaveValue('');
  await group.fill('Tooling');
  await group.press('Enter');
  await node(page, 'CLI').first().click();
  await expect(page.locator('#insp-group')).toHaveValue('Tooling');
  expect(page.errors).toEqual([]);
});

test('an emptied field is removed, not blanked', async ({ page }) => {
  await node(page, 'Blastradius').dblclick();
  await node(page, 'CLI').first().click();
  await page.locator('#insp-tech').fill('');
  await page.locator('#insp-tech').press('Enter');
  // No technology left to render, so the kicker is the kind alone.
  await expect(page.locator('.insp-kicker')).toHaveText('[Container]');
  expect(page.errors).toEqual([]);
});

test('replicas count the things that run, and only those', async ({ page }) => {
  // A container is not a thing that runs — the node that hosts it is — so the
  // field is not offered here at all.
  await node(page, 'Blastradius').dblclick();
  await expect(page.locator('#breadcrumb')).toContainText('Containers');
  await node(page, 'CLI').first().click();
  await expect(page.locator('#insp-replicas')).toHaveCount(0);

  await page.locator('#level-seg .seg-opt', { hasText: 'D' }).click();
  await node(page, 'Developer Machine').first().dblclick();
  await expect(page.locator('#breadcrumb')).toContainText('Developer Machine');
  const box = node(page, 'Windows 11 Workstation').first();
  await box.click({ position: { x: 24, y: 12 } });
  await page.locator('#insp-replicas').fill('3');
  await page.locator('#insp-replicas').press('Enter');
  await expect(box).toContainText('×3');
  expect(page.errors).toEqual([]);
});

test('external marks a system as outside your control', async ({ page }) => {
  await node(page, 'Blastradius').first().click();
  await page.locator('#insp-external').check();
  await expect(node(page, 'Blastradius').first()).toHaveClass(/is-external/);
  await page.locator('#insp-external').uncheck();
  await expect(node(page, 'Blastradius').first()).not.toHaveClass(/is-external/);
  expect(page.errors).toEqual([]);
});

test('a component is pointed at its code from the inspector', async ({ page }) => {
  await node(page, 'Blastradius').dblclick();
  await node(page, 'Core').first().dblclick(); // L3
  await expect(page.locator('#breadcrumb')).toContainText('Components');

  // A component with no mapping is offered one — this is the step that used to
  // mean reading spec/l4-introspection.md and hand-writing YAML.
  // The box offers to start one; the inspector is where it is then edited.
  await node(page, 'Exporter').first().click({ button: 'right' });
  await page.locator('.ctx-menu .ctx-item', { hasText: 'Point at its code…' }).click();
  await page.locator('#dlg-language').selectOption('rust');
  await page.locator('#dlg-root').fill('crates/blastradius-core/src');
  await page.locator('#dlg-ok').click();
  await expect(page.locator('#map-root')).toHaveValue('crates/blastradius-core/src');
  await expect(page.locator('#map-language')).toHaveValue('rust');

  // Editing the mapping is a save, not a keystroke: several fields make one
  // mapping, and half of one cannot be introspected.
  await page.locator('#map-include').fill('export.rs, snapshot.rs');
  await page.locator('#map-save').click();
  await node(page, 'Exporter').first().click();
  await expect(page.locator('#map-include')).toHaveValue('export.rs, snapshot.rs');

  // And the mapping can be taken off again, which stops introspecting it.
  await page.locator('#map-remove').click();
  await expect(page.locator('#map-add')).toBeVisible();
  expect(page.errors).toEqual([]);
});

test('a mapped component offers to run the extractor', async ({ page }) => {
  await node(page, 'Blastradius').dblclick();
  await node(page, 'Core').first().dblclick();
  await node(page, 'Git Service').first().click();
  // The dogfood model maps this one, so the editor shows the mapping it has.
  await expect(page.locator('#map-root')).toHaveValue('crates/blastradius-core/src');
  await page.locator('#map-run').click();
  // The mock has no compilers; what matters here is that the button reaches
  // the command rather than what a real extractor would say.
  await expect(page.locator('.travel-banner.is-toast')).toContainText('0 code elements derived · mock harness: introspection needs the real app');
  expect(page.errors).toEqual([]);
});

// 0.9.0 D: `show-groups`, `include-context` and `nested` are view-file keys
// (spec §4) that nothing in the app could reach — so a `group:` label written
// in the inspector stayed invisible, and no screen said a view file existed at
// all. The View panel is where a diagram's own settings live.
const viewPanel = (page) => page.locator('#side-mode .seg-opt', { hasText: 'View' });

test('groups written on elements become visible from the view panel', async ({ page }) => {
  // L1 has no view file in this workspace — which is the point: the first
  // setting changed writes one.
  await node(page, 'Platform Architect').first().click();
  await page.locator('#insp-group').fill('People');
  await page.locator('#insp-group').press('Enter');
  await node(page, 'Reviewer').first().click();
  await page.locator('#insp-group').fill('People');
  await page.locator('#insp-group').press('Enter');

  // Written, and drawn nowhere: grouping is presentation, opt-in per view.
  await expect(page.locator('.group-box')).toHaveCount(0);

  await viewPanel(page).click();
  await expect(page.locator('#side-body')).toContainText('No view file yet');
  await page.locator('[data-flag="show-groups"]').check();
  await expect(page.locator('.group-box')).toHaveCount(1);
  await expect(page.locator('.group-box')).toContainText('People');

  // And the panel now knows which file it wrote into.
  await expect(page.locator('#side-body')).not.toContainText('No view file yet');
  expect(page.errors).toEqual([]);
});

test('the view panel names the file it writes, and offers to open it', async ({ page }) => {
  // It said "Written to containers" and handed `open_in_editor` an empty
  // string: the snapshot's views carried no file, so 0.9.0 shipped a button
  // that opened nothing. Found while building the mock/engine contract.
  await node(page, 'Blastradius').dblclick();
  await viewPanel(page).click();
  const link = page.locator('#side-body [data-editfile]');
  await expect(link).toHaveText('views/containers.yaml');
  await expect(link).toHaveAttribute('data-editfile', 'views/containers.yaml');
  expect(page.errors).toEqual([]);
});

test('the view panel turns context off and on', async ({ page }) => {
  await node(page, 'Blastradius').dblclick(); // L2, which has pins and context
  await expect(page.locator('#breadcrumb')).toContainText('Containers');
  const people = page.locator('.node.is-person');
  await expect(people).not.toHaveCount(0);

  await viewPanel(page).click();
  await page.locator('[data-flag="include-context"]').uncheck();
  await expect(people).toHaveCount(0);
  await page.locator('[data-flag="include-context"]').check();
  await expect(people).not.toHaveCount(0);
  expect(page.errors).toEqual([]);
});

test('the view panel lists the pins and releases them', async ({ page }) => {
  await node(page, 'Blastradius').dblclick(); // the L2 view is pinned in the fixture
  await viewPanel(page).click();
  const pins = page.locator('#side-body [data-unpin]');
  await expect(pins).not.toHaveCount(0);
  const before = await pins.count();

  await pins.first().click();
  await expect(page.locator('#side-body [data-unpin]')).toHaveCount(before - 1);

  await page.locator('#view-reset').click();
  await expect(page.locator('#side-body')).toContainText('Nothing pinned');
  expect(page.errors).toEqual([]);
});

test('nested boxes are offered where they mean something', async ({ page }) => {
  await viewPanel(page).click();
  // L1 dives; only a deployment view draws containment (ADR-0018).
  await expect(page.locator('[data-flag="nested"]')).toHaveCount(0);
  await page.locator('#level-seg .seg-opt', { hasText: 'D' }).click();
  await expect(page.locator('[data-flag="nested"]')).toHaveCount(1);

  // And code level has no view file to have settings in at all.
  await page.locator('#level-seg .seg-opt', { hasText: 'L4' }).click();
  await expect(page.locator('#side-body')).toContainText('no view file');
  await expect(page.locator('[data-flag]')).toHaveCount(0);
  expect(page.errors).toEqual([]);
});

// 0.9.0 F: drift detection (ADR-0019) found three real problems in this repo's
// own model on its first run — and then reported them as warning strings in a
// chip. `drift::detect` returns structure (from, to, kind, and the file that
// evidences it) and `drift::diagnose` threw all of it away. The remedy for an
// undeclared dependency is one operation the app already had.
//
// `?drift` seeds two findings, one of each kind: the dogfood model is
// drift-free by policy — conformance.rs fails the build otherwise — so there
// is nothing real to draw.
const edgeHit = (page, from, to) =>
  page.locator(`#edges path.edge-hit[data-from="${from}"][data-to="${to}"]`);

test('an undeclared dependency is drawn as a ghost, and declaring it clears it', async ({ page }) => {
  await page.goto('/index.html?nogit&drift');
  await expect(page.locator('#nodes .node')).not.toHaveCount(0);
  await node(page, 'Blastradius').dblclick();
  await node(page, 'Core').first().dblclick(); // L3, where the finding lives
  await expect(page.locator('#breadcrumb')).toContainText('Components');

  // Drawn unlike a relation, because it is not one: nothing in the model says
  // this yet.
  await expect(page.locator('#edges path.edge.is-drift')).toHaveCount(1);

  await edgeHit(page, 'blastradius.core.exporter', 'blastradius.core.git-service')
    .dispatchEvent('click');
  const side = page.locator('#side-body');
  await expect(side).toContainText('Undeclared dependency');
  await expect(side).toContainText('crates/blastradius-core/src/export.rs');

  await page.locator('#drift-declare').click();
  await page.locator('#dlg-label').fill('writes through');
  await page.locator('#dlg-ok').click();

  // Declared: the ghost is gone and a real relation stands in its place.
  await expect(page.locator('#edges path.edge.is-drift')).toHaveCount(0);
  await node(page, 'Exporter').first().click();
  await expect(page.locator('.insp-rel', { hasText: 'Git Service' })).toContainText('writes through');
  expect(page.errors).toEqual([]);
});

test('an unbacked relation is marked, and reverses in one action', async ({ page }) => {
  await page.goto('/index.html?nogit&drift');
  await expect(page.locator('#nodes .node')).not.toHaveCount(0);
  await node(page, 'Blastradius').dblclick();
  await node(page, 'Core').first().dblclick();

  await expect(page.locator('#edges path.edge.is-unbacked')).toHaveCount(1);
  await edgeHit(page, 'blastradius.core.exporter', 'blastradius.core.model-service')
    .dispatchEvent('click');
  await expect(page.locator('#side-body')).toContainText('no code reference supports it');

  // The most common cause, and the one this repo's own model hit: the
  // dependency runs the other way.
  await page.locator('#rel-reverse').click();
  await expect(page.locator('#edges path.edge.is-unbacked')).toHaveCount(0);
  await node(page, 'Model Service').first().click();
  await expect(page.locator('.insp-rel', { hasText: 'Exporter' })).toHaveText(/^→ Exporter/);
  // One transaction: a single undo puts the original relation back.
  await page.locator('#undo-btn').click();
  await expect(page.locator('#edges path.edge.is-unbacked')).toHaveCount(1);
  expect(page.errors).toEqual([]);
});

test('drift is not drawn over a diff, where it would be about another tree', async ({ page }) => {
  await page.goto('/index.html?drift'); // git fixture present, so diff is available
  await expect(page.locator('#nodes .node')).not.toHaveCount(0);
  await node(page, 'Blastradius').dblclick();
  await node(page, 'Core').first().dblclick();
  await expect(page.locator('#edges path.edge.is-drift')).toHaveCount(1);
  await page.locator('#diff-btn').click();
  await expect(page.locator('#edges path.edge.is-drift')).toHaveCount(0);
  expect(page.errors).toEqual([]);
});
