// Phase 2 rendering gate: diff mode, ghosts, conflict inspector, history,
// time-travel — against the fabricated mock/git.json fixture, in WebKit.
import { test, expect } from '@playwright/test';

test.beforeEach(async ({ page }) => {
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  page.on('console', (m) => m.type() === 'error' && errors.push(m.text()));
  page.errors = errors;
  await page.goto('/index.html');
  await expect(page.locator('#nodes .node')).not.toHaveCount(0);
});

test('git chrome renders branch and conflict chip', async ({ page }) => {
  await expect(page.locator('#git-chips')).toContainText('⎇ feature/sync-engine');
  await expect(page.locator('#git-chips .tag-danger')).toContainText('1 conflicted');
  await expect(page.locator('#diff-btn')).toBeVisible();
  await expect(page.locator('#history-btn')).toBeVisible();
  expect(page.errors).toEqual([]);
});

test('diff mode: states, ghost, counts, layout toggle', async ({ page }) => {
  // go to L2 where the diff fixture lives
  const node = (t) => page.locator('#nodes .node', { has: page.locator('.node-title', { hasText: t }) });
  await node('Blastradius').dblclick();
  await expect(page.locator('#breadcrumb')).toContainText('Containers');

  await page.locator('#diff-btn').click();
  await expect(page.locator('#git-chips .tag-success')).toContainText('1 added');
  await expect(page.locator('#git-chips .tag-warning')).toContainText('1 changed');
  await expect(page.locator('#git-chips .tag-danger').last()).toContainText('1 removed');

  // CLI marked added; ghost Legacy Sync appears with removed state
  await expect(node('CLI').first()).toHaveClass(/is-added/);
  const ghost = node('Legacy Sync').first();
  await expect(ghost).toBeVisible();
  await expect(ghost).toHaveClass(/is-removed/);
  // removed relation renders
  await expect(page.locator('#edges path.edge.is-removed')).toHaveCount(1);
  // added relation state
  await expect(page.locator('#edges path.edge.is-added')).not.toHaveCount(0);

  // conflict overrides diff state on the ui container
  await expect(node('Canvas UI').first()).toHaveClass(/is-conflict/);

  // layout toggle marks the moved pin
  await page.locator('#layout-toggle').click();
  const badges = page.locator('#nodes .node-badge[title*="Pin moved"]');
  await expect(badges).toHaveCount(1);
  expect(page.errors).toEqual([]);
  await page.screenshot({ path: 'test-results/webkit-diff.png', fullPage: true });
});

test('conflict inspector shows ours/theirs and resolve affordance', async ({ page }) => {
  const node = (t) => page.locator('#nodes .node', { has: page.locator('.node-title', { hasText: t }) });
  await node('Blastradius').dblclick();
  await node('Canvas UI').first().click();
  const side = page.locator('#side-body');
  await expect(side).toContainText('Merge conflict');
  await expect(side.locator('.conf-table')).toContainText('Canvas UI');
  await expect(side.locator('.conf-table')).toContainText('Web Frontend');
  await expect(side.locator('[data-editfile="model/blastradius.yaml"]')).toBeVisible();
  expect(page.errors).toEqual([]);
});

test('history: set base recomputes, view travels and returns', async ({ page }) => {
  await page.locator('#history-btn').click();
  await expect(page.locator('#side-title')).toHaveText('History');
  await expect(page.locator('.hist-row')).toHaveCount(3);

  // travel to the middle commit: ui renamed, cli absent
  await page.locator('.hist-row', { hasText: 'Add CLI container' }).locator('[data-view]').click();
  await expect(page.locator('.travel-banner')).toContainText('abc12345');
  await expect(page.locator('.tree-row', { hasText: 'Web UI (pre-rename)' })).toBeVisible();
  await expect(page.locator('.tree-row', { hasText: /^CLI$/ })).toHaveCount(0);

  await page.locator('#travel-return').click();
  await expect(page.locator('.travel-banner')).toHaveCount(0);
  await expect(page.locator('.tree-row', { hasText: 'Canvas UI' })).toBeVisible();
  expect(page.errors).toEqual([]);
});
