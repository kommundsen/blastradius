// WebKit rendering gate (ADR-0011): the Phase 1 exit-criterion flow, executed
// in the constraining engine against the mock harness — identical modules and
// CSS to the Tauri window.
import { test, expect } from '@playwright/test';

test.beforeEach(async ({ page }) => {
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  page.on('console', (m) => m.type() === 'error' && errors.push(m.text()));
  page.errors = errors;
  await page.goto('/index.html');
  await expect(page.locator('#nodes .node')).not.toHaveCount(0);
});

test('L1 context renders the dogfood model', async ({ page }) => {
  await expect(page.locator('#nodes .node')).toHaveCount(6);
  await expect(page.locator('#edges path.edge')).toHaveCount(5);
  await expect(page.locator('.node.is-person')).toHaveCount(2);
  await expect(page.locator('.node.is-external')).toHaveCount(3);
  await expect(page.locator('#breadcrumb')).toContainText('Context');
  // the tree lists the whole model regardless of altitude
  await expect(page.locator('.tree-row')).toHaveCount(21);
  expect(page.errors).toEqual([]);
  await page.screenshot({ path: 'test-results/webkit-L1.png', fullPage: true });
});

test('exit-criterion flow: dive to git-service, open ADR-0007', async ({ page }) => {
  const node = (title) =>
    page.locator('#nodes .node', { has: page.locator('.node-title', { hasText: title }) });

  await node('Blastradius').dblclick();
  await expect(page.locator('#breadcrumb')).toContainText('Containers');

  await node('Core').dblclick();
  await expect(page.locator('#breadcrumb')).toContainText('Components');

  await node('Git Service').first().click();
  const side = page.locator('#side-body');
  await expect(side).toContainText('blastradius.core.git-service');
  await expect(side).toContainText('Read-only repository access');

  await side.locator('[data-doc="adr-0007"]').click();
  await expect(page.locator('#side-title')).toHaveText('adr-0007');
  await expect(page.locator('.doc-body h1')).toContainText('Embedded libgit2');
  // doc -> element navigation exists
  await expect(side.locator('[data-el="blastradius.core.git-service"]')).toBeVisible();

  expect(page.errors).toEqual([]);
  await page.screenshot({ path: 'test-results/webkit-L3-adr.png', fullPage: true });
});

test('keyboard: arrows select, Escape rises', async ({ page }) => {
  await page.locator('#canvas').click({ position: { x: 30, y: 200 } });
  await page.keyboard.press('ArrowRight');
  await expect(page.locator('#nodes .node.is-active')).toHaveCount(1);
  const beforeCrumb = await page.locator('#breadcrumb').textContent();

  // dive via Enter on a diveable node (select the system first)
  const sys = page.locator('#nodes .node', {
    has: page.locator('.node-title', { hasText: 'Blastradius' }),
  });
  await sys.click();
  await page.locator('#canvas').press('Enter');
  await expect(page.locator('#breadcrumb')).toContainText('Containers');

  await page.locator('#canvas').press('Escape');
  await expect(page.locator('#breadcrumb')).toHaveText(beforeCrumb);
  expect(page.errors).toEqual([]);
});

test('edge labels knock out the grid (paint-order support)', async ({ page }) => {
  // WebKit-specific risk: paint-order + stroke on SVG text is the mechanism
  // edge labels rely on (design system). Assert computed support, not just CSS.
  const paintOrder = await page
    .locator('#edges text')
    .first()
    .evaluate((el) => getComputedStyle(el).paintOrder);
  expect(paintOrder).toContain('stroke');
});

test('theme pin overrides and returns to OS', async ({ page }) => {
  const bg = () => page.evaluate(() => getComputedStyle(document.body).backgroundColor);
  const auto = await bg();
  await page.locator('#theme-btn').click(); // light
  const light = await bg();
  await page.locator('#theme-btn').click(); // dark
  const dark = await bg();
  expect(light).not.toEqual(dark);
  await page.locator('#theme-btn').click(); // back to auto
  expect(await bg()).toEqual(auto);
});

test('dive choreography: identical destination under reduced motion (phase 5)', async ({ page }) => {
  // the glide is a vestibular hazard when motion is reduced — it must cut,
  // and the destination must be exactly the same scene
  await page.emulateMedia({ reducedMotion: 'reduce' });
  await page.goto('/index.html?nogit');
  const node = (title) =>
    page.locator('#nodes .node', { has: page.locator('.node-title', { hasText: title }) });
  await node('Blastradius').dblclick();
  await expect(page.locator('#breadcrumb')).toContainText('Containers');
  await page.locator('#canvas').press('Escape');
  await expect(node('Blastradius')).toBeVisible();
  // camera lands fully opaque with no animation residue
  const opacity = await page.locator('#camera').evaluate((el) => getComputedStyle(el).opacity);
  expect(opacity).toBe('1');
});
