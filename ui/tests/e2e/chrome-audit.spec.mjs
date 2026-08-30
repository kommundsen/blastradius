// A measurement, not a gate (0.11.0 design review). Records what the app bar,
// the canvas overlays and the inspector actually do, so the review argues from
// numbers rather than from an impression of clutter.
//
// Run: npx playwright test chrome-audit --reporter=line
import { test, expect } from '@playwright/test';

const WIDTHS = [1280, 980, 820, 680, 560, 480];

const node = (page, t) =>
  page.locator('#nodes .node', { has: page.locator('.node-title', { hasText: t }) });

test('measure the app bar as the window narrows', async ({ page }) => {
  await page.goto('/index.html?nogit');
  await expect(page.locator('#nodes .node')).not.toHaveCount(0);

  const rows = [];
  for (const w of WIDTHS) {
    await page.setViewportSize({ width: w, height: 800 });
    await page.waitForTimeout(120);
    const m = await page.evaluate(() => {
      const bar = document.querySelector('.app-bar');
      const vis = (el) => {
        const r = el.getBoundingClientRect();
        return r.width > 0 && r.height > 0 && getComputedStyle(el).display !== 'none';
      };
      const buttons = [...bar.querySelectorAll('button')]
        .filter((b) => !b.hidden)
        .map((b) => ({ id: b.id, shown: vis(b), right: Math.round(b.getBoundingClientRect().right) }));
      const canvas = document.querySelector('.canvas').getBoundingClientRect();
      return {
        overflow: bar.scrollWidth - bar.clientWidth,
        offscreen: buttons.filter((b) => b.shown && b.right > bar.clientWidth + 1).map((b) => b.id),
        hidden: buttons.filter((b) => !b.shown).map((b) => b.id),
        shown: buttons.filter((b) => b.shown).length,
        canvasW: Math.round(canvas.width),
      };
    });
    rows.push({ w, ...m });
  }

  console.log('\nwidth | overflow | shown | canvas | dropped | pushed off-screen');
  for (const r of rows) {
    console.log(
      `${String(r.w).padStart(5)} | ${String(r.overflow).padStart(8)} | ${String(r.shown).padStart(5)}` +
      ` | ${String(r.canvasW).padStart(6)} | ${(r.hidden.join(',') || '—').padEnd(12)} | ${r.offscreen.join(',') || '—'}`
    );
  }
});

test('measure what the canvas overlays cover', async ({ page }) => {
  await page.goto('/index.html?nogit&drift');
  await expect(page.locator('#nodes .node')).not.toHaveCount(0);
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.waitForTimeout(120);

  const m = await page.evaluate(() => {
    const canvas = document.querySelector('.canvas').getBoundingClientRect();
    const area = canvas.width * canvas.height;
    const of = (sel) => {
      const el = document.querySelector(sel);
      if (!el || el.hidden) return null;
      const r = el.getBoundingClientRect();
      if (!r.width || !r.height) return null;
      return { sel, w: Math.round(r.width), h: Math.round(r.height),
               pct: +((r.width * r.height) / area * 100).toFixed(1),
               events: getComputedStyle(el).pointerEvents };
    };
    return { canvas: { w: Math.round(canvas.width), h: Math.round(canvas.height) },
             overlays: ['.overlay-bl', '#tour', '#canvas-blank', '.diag-list'].map(of).filter(Boolean) };
  });
  console.log(`\ncanvas ${m.canvas.w}x${m.canvas.h}`);
  console.log('overlay          |   size    | % canvas | pointer-events');
  for (const o of m.overlays) {
    console.log(`${o.sel.padEnd(16)} | ${String(o.w).padStart(4)}x${String(o.h).padEnd(4)} | ${String(o.pct).padStart(8)} | ${o.events}`);
  }
});

test('count the controls each inspector puts on screen', async ({ page }) => {
  await page.goto('/index.html?nogit&drift');
  await expect(page.locator('#nodes .node')).not.toHaveCount(0);

  const count = async (label) => {
    const n = await page.evaluate(() => {
      const body = document.querySelector('#side-body');
      const q = (s) => body.querySelectorAll(s).length;
      return { inputs: q('input,textarea,select'), buttons: q('button'),
               sections: q('.insp-section'), height: body.scrollHeight };
    });
    console.log(`${label.padEnd(22)} | inputs ${String(n.inputs).padStart(2)} | buttons ${String(n.buttons).padStart(2)}` +
      ` | sections ${String(n.sections).padStart(2)} | scrollHeight ${n.height}`);
  };

  console.log('');
  await node(page, 'Blastradius').click();
  await count('system (L1)');
  await node(page, 'Blastradius').dblclick();
  await node(page, 'Core').first().dblclick();
  await node(page, 'Exporter').first().click();
  await count('component (L3)');
  await page.locator('#edges path.edge-hit[data-from="blastradius.core.exporter"][data-to="blastradius.core.model-service"]')
    .dispatchEvent('click');
  await count('relation (with drift)');
});
