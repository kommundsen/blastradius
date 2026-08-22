// A11y audit (Phase 5) against the design system's WCAG AA contract.
// axe-core scans every major surface — app shell, welcome, dialogs, panels —
// in both themes. Zero violations is the bar, not a report.
import { test, expect } from '@playwright/test';
import AxeBuilder from '@axe-core/playwright';

const TAGS = ['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa'];

async function scan(page, label) {
  const results = await new AxeBuilder({ page }).withTags(TAGS).analyze();
  const summary = results.violations.map((v) => ({
    id: v.id,
    impact: v.impact,
    nodes: v.nodes.map((n) => n.target.join(' ')).slice(0, 5),
  }));
  expect(summary, `${label}: ${JSON.stringify(summary, null, 2)}`).toEqual([]);
}

for (const theme of ['light', 'dark']) {
  test(`app shell + inspector are AA clean (${theme})`, async ({ page }) => {
    await page.emulateMedia({ colorScheme: theme });
    await page.goto('/index.html?nogit');
    await expect(page.locator('#nodes .node').first()).toBeVisible();
    await scan(page, `shell ${theme}`);

    await page.locator('#nodes .node', {
      has: page.locator('.node-title', { hasText: 'Blastradius' }),
    }).click();
    await expect(page.locator('#side-body')).toContainText('blastradius');
    await scan(page, `inspector ${theme}`);
  });
}

test('welcome screen is AA clean', async ({ page }) => {
  await page.goto('/index.html?nogit&noworkspace');
  await expect(page.locator('.welcome')).toBeVisible();
  await scan(page, 'welcome');
});

test('dialogs are AA clean', async ({ page }) => {
  await page.goto('/index.html?nogit');
  await expect(page.locator('#nodes .node').first()).toBeVisible();

  await page.locator('#add-btn').click();
  await expect(page.locator('#app-dialog .dialog')).toBeVisible();
  await scan(page, 'create dialog');
  await page.locator('#dlg-cancel').click();

  await page.locator('#share-btn').click();
  await expect(page.locator('#app-dialog .dialog')).toBeVisible();
  await scan(page, 'share dialog');
});

test('source panel (CodeMirror) is AA clean', async ({ page }) => {
  await page.goto('/index.html?nogit');
  await expect(page.locator('#nodes .node').first()).toBeVisible();
  await page.locator('#side-mode .seg-opt', { hasText: 'Source' }).click();
  await expect(page.locator('#src-editor .CodeMirror')).toBeVisible();
  await scan(page, 'source panel');
});
