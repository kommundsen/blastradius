// Render share of the "keystroke → canvas < 250ms" budget
// (spec/sync-engine.md): layout (ELK) + DOM for the current view must land in
// under 100ms — the core share (parse + validate) is enforced at 150ms by
// crates/blastradius-core/tests/budgets.rs. Measured as a dive round-trip
// with reduced motion (the glide would otherwise dominate the clock).
import { test, expect } from '@playwright/test';

test('view render (layout + DOM) stays under its 100ms budget share', async ({ page }) => {
  await page.emulateMedia({ reducedMotion: 'reduce' });
  await page.goto('/index.html?nogit');
  await expect(page.locator('#nodes .node').first()).toBeVisible();

  const best = await page.evaluate(async () => {
    const breadcrumb = document.getElementById('breadcrumb');
    const until = (pred) =>
      new Promise((resolve) => {
        const tick = () => (pred() ? resolve() : requestAnimationFrame(tick));
        tick();
      });
    const diveInto = async (title) => {
      const node = [...document.querySelectorAll('.node-title')]
        .find((e) => e.textContent.includes(title))
        .closest('.node');
      node.dispatchEvent(new MouseEvent('dblclick', { bubbles: true }));
      await until(() => breadcrumb.textContent.includes('Containers'));
    };
    const rise = async () => {
      document.getElementById('canvas').dispatchEvent(
        new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
      await until(() => !breadcrumb.textContent.includes('Containers'));
    };

    let best = Infinity;
    for (let i = 0; i < 3; i++) {
      const t0 = performance.now();
      await diveInto('Blastradius');
      best = Math.min(best, performance.now() - t0);
      await rise();
    }
    return best;
  });

  console.log(`view render best-of-3: ${best.toFixed(1)}ms (budget share 100ms)`);
  expect(best).toBeLessThan(100);
});
