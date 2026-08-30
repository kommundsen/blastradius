// The problems panel (0.11.0 item 6, ADR-0020).
//
// The exit criterion this suite exists for: every finding in the panel is
// actionable without leaving it, against seeded drift of both kinds. Neither
// kind of finding occurs in the dogfood model — it is valid and drift-free by
// policy, `conformance.rs` fails the build otherwise — so both are seeded:
// `?drift` for the code/model disagreements, `?invalid` for validation.
import { test, expect } from '@playwright/test';

const node = (page, t) =>
  page.locator('#nodes .node', { has: page.locator('.node-title', { hasText: t }) });

test.beforeEach(async ({ page }) => {
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  page.on('console', (m) => m.type() === 'error' && errors.push(m.text()));
  page.errors = errors;
});

test('a clean workspace says nothing at all', async ({ page }) => {
  await page.goto('/index.html?nogit');
  await expect(page.locator('#nodes .node')).not.toHaveCount(0);
  // No chip, and therefore no panel: the dogfood model is valid and drift-free,
  // and chrome that reports "0 problems" is chrome that is always on screen.
  await expect(page.locator('#diag-btn')).toHaveCount(0);
  expect(page.errors).toEqual([]);
});

test('the chip counts both kinds and the panel groups them by what they mean', async ({ page }) => {
  await page.goto('/index.html?nogit&drift&invalid');
  await expect(page.locator('#nodes .node')).not.toHaveCount(0);

  // One error, one warning, two drift findings — and the seeded `info` is
  // absent, because a file without frontmatter is a fact, not a fault.
  await expect(page.locator('#diag-btn')).toHaveText('1 error · 1 warning · 2 drift');
  await page.locator('#diag-btn').click();

  const panel = page.locator('.problems');
  await expect(panel).toBeVisible();
  await expect(panel.locator('.problems-head')).toHaveText([
    /The model contradicts itself · 2/,
    /The model and the code disagree · 2/,
  ]);
  await expect(panel.locator('.problems-row')).toHaveCount(4);
  expect(page.errors).toEqual([]);
});

test('a drift row is element-shaped, not a string', async ({ page }) => {
  await page.goto('/index.html?nogit&drift&invalid');
  await expect(page.locator('#nodes .node')).not.toHaveCount(0);
  await page.locator('#diag-btn').click();

  // Names people use, ids nowhere in the title — the difference from the list
  // of strings this replaced.
  const row = page.locator('.problems-row', { hasText: 'Exporter → Git Service' });
  await expect(row).toBeVisible();
  await expect(row).toContainText('undeclared');
  await expect(row).toContainText('crates/blastradius-core/src/export.rs');
  expect(page.errors).toEqual([]);
});

test('clicking a finding lands on it', async ({ page }) => {
  await page.goto('/index.html?nogit&drift&invalid');
  await expect(page.locator('#nodes .node')).not.toHaveCount(0);
  await page.locator('#diag-btn').click();

  // The finding is about two components three levels down; the canvas knows how
  // to fly to either, and the row is what asks it to.
  await page.locator('.problems-row', { hasText: 'Exporter → Git Service' })
    .locator('.problems-open').click();
  await expect(page.locator('#breadcrumb')).toContainText('Core');
  // The inspector's name is an editable field, so it is a value rather than text.
  await expect(page.locator('#insp-name')).toHaveValue('Exporter');
  await expect(page.locator('#side-body')).toContainText('blastradius.core.exporter');
  expect(page.errors).toEqual([]);
});

test('an undeclared dependency is declared from the row, without leaving the panel', async ({ page }) => {
  await page.goto('/index.html?nogit&drift&invalid');
  await expect(page.locator('#nodes .node')).not.toHaveCount(0);
  await page.locator('#diag-btn').click();

  await page.locator('.problems-row', { hasText: 'Exporter → Git Service' })
    .locator('.problems-fix').click();
  await page.locator('#dlg-label').fill('writes through');
  await page.locator('#dlg-ok').click();

  // The finding is gone, the count follows it, and the panel is still open on
  // what is left — which is the whole difference from a report.
  await expect(page.locator('#diag-btn')).toHaveText('1 error · 1 warning · 1 drift');
  await expect(page.locator('.problems')).toBeVisible();
  await expect(page.locator('.problems-row', { hasText: 'Exporter → Git Service' })).toHaveCount(0);
  expect(page.errors).toEqual([]);
});

test('an unbacked relation is reversed from the row, in one click', async ({ page }) => {
  await page.goto('/index.html?nogit&drift&invalid');
  await expect(page.locator('#nodes .node')).not.toHaveCount(0);
  await page.locator('#diag-btn').click();

  const row = page.locator('.problems-row', { hasText: 'Exporter → Model Service' });
  await expect(row).toContainText('unbacked');
  await row.locator('.problems-fix').click();

  await expect(page.locator('#diag-btn')).toHaveText('1 error · 1 warning · 1 drift');
  // And it really reversed rather than being deleted: the relation is there,
  // pointing the other way. Escape first — the panel is open over the canvas,
  // which is the cost ADR-0020 accepts and the reason it has three ways out.
  await page.keyboard.press('Escape');
  await expect(page.locator('.problems')).toHaveCount(0);
  await node(page, 'Blastradius').dblclick();
  await node(page, 'Core').first().dblclick();
  await node(page, 'Model Service').first().click();
  await expect(page.locator('.insp-rel', { hasText: 'Exporter' })).toHaveText(/^→ Exporter/);
  expect(page.errors).toEqual([]);
});

test('a validation error opens its file, and offers nothing it cannot do', async ({ page }) => {
  await page.goto('/index.html?nogit&drift&invalid');
  await expect(page.locator('#nodes .node')).not.toHaveCount(0);
  await page.locator('#diag-btn').click();

  const row = page.locator('.problems-row', { hasText: 'dangling reference' });
  await expect(row).toContainText('model/blastradius.yaml:42');
  // What a dangling reference should become is a modelling decision: the row
  // offers to open the file and does not pretend a button can take it.
  await expect(row.locator('.problems-fix')).toHaveText('Open');
  expect(page.errors).toEqual([]);
});

test('the panel closes, and does not eat a click meant for the canvas', async ({ page }) => {
  await page.goto('/index.html?nogit&drift&invalid');
  await expect(page.locator('#nodes .node')).not.toHaveCount(0);
  await page.locator('#diag-btn').click();
  await expect(page.locator('.problems')).toBeVisible();

  // The chip toggles it, and so does its own close button.
  await page.locator('#diag-btn').click();
  await expect(page.locator('.problems')).toHaveCount(0);
  await page.locator('#diag-btn').click();
  await page.locator('#problems-close').click();
  await expect(page.locator('.problems')).toHaveCount(0);
  // And Escape, because a panel sitting over the node you want is the cost of
  // this region and one keystroke has to undo it.
  await page.locator('#diag-btn').click();
  await expect(page.locator('.problems')).toBeVisible();
  await page.keyboard.press('Escape');
  await expect(page.locator('.problems')).toHaveCount(0);

  // Unlike the tour card this one is a real panel and takes its own clicks —
  // so it must be closable rather than click-through, and the canvas beneath it
  // must still work once it is gone.
  await node(page, 'Blastradius').click();
  await expect(page.locator('#side-body')).toContainText('Blastradius');
  expect(page.errors).toEqual([]);
});

test('drift is not reported over a diff, where it would be about another tree', async ({ page }) => {
  await page.goto('/index.html?drift&invalid'); // git fixture present, so diff is available
  await expect(page.locator('#nodes .node')).not.toHaveCount(0);
  await expect(page.locator('#diag-btn')).toHaveText(/2 drift/);
  const { barAction } = await import('./_chrome.mjs');
  await barAction(page, 'Diff');
  // The same rule the canvas already applies to ghost edges: drift is a fact
  // about the code as it is now, and a diff is about another tree.
  await expect(page.locator('#diag-btn')).toHaveText('1 error · 1 warning');
  expect(page.errors).toEqual([]);
});
