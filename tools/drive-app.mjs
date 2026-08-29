// Drive the *packaged* app over CDP (docs/roadmap.md 0.10.0 item 3).
//
// `tools/smoke-install.ps1` takes a finished CLI and puts it through a new
// user's flow, which is why 0.7.0 stopped the run of install-only bugs. The
// app had no such gate: the e2e suite runs against the mock bridge (ADR-0011)
// and is structurally blind to everything on the IPC boundary, so every
// app-side feature since — three sync operations and `introspect_component`
// among them — shipped with its only exercise being a mock that answers
// "introspection needs the real app".
//
// The technique is the one that found the 0.6.1 scaffold bug by hand:
// WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port lets Playwright
// attach to the WebView2 inside the real window. Nothing here is mocked. The
// window is the shipped binary, the workspace is a real repository on disk,
// and the extractor that runs is the one staged beside the executable.
//
//   node tools/drive-app.mjs --port 9222 --repo C:\path\to\throwaway
//
// Exits non-zero with a named step on the first failure. `smoke-app.ps1` owns
// the staging, the launch and the on-disk assertions; this owns the window.
import { chromium } from '@playwright/test';

const arg = (name, fallback) => {
  const i = process.argv.indexOf(`--${name}`);
  if (i >= 0 && process.argv[i + 1]) return process.argv[i + 1];
  if (fallback !== undefined) return fallback;
  throw new Error(`missing --${name}`);
};

const port = arg('port', '9222');
const repo = arg('repo');
const timeout = Number(arg('timeout', '30000'));

let step = 'connect';
const at = (name) => { step = name; console.log(`  · ${name}`); };
const fail = (msg) => { throw new Error(`[${step}] ${msg}`); };

/** Poll a page expression until it is truthy, or give up with the step name. */
async function until(page, what, expr) {
  const deadline = Date.now() + timeout;
  let last;
  for (;;) {
    try {
      last = await page.evaluate(expr);
      if (last) return last;
    } catch (e) {
      last = String(e);
    }
    if (Date.now() > deadline) fail(`${what} — last saw ${JSON.stringify(last)}`);
    await new Promise((r) => setTimeout(r, 250));
  }
}

const click = (page, selector) =>
  page.evaluate((s) => {
    const el = document.querySelector(s);
    if (!el) throw new Error(`no ${s}`);
    el.click();
    return true;
  }, selector);

const fill = (page, selector, value) =>
  page.evaluate(([s, v]) => {
    const el = document.querySelector(s);
    if (!el) throw new Error(`no ${s}`);
    el.value = v;
    el.dispatchEvent(new Event('input', { bubbles: true }));
    el.dispatchEvent(new Event('change', { bubbles: true }));
    return true;
  }, [selector, value]);

const text = (page, selector) =>
  page.evaluate((s) => document.querySelector(s)?.textContent?.trim() ?? null, selector);

const browser = await chromium.connectOverCDP(`http://127.0.0.1:${port}`);
const context = browser.contexts()[0];
const page = context.pages()[0] ?? (await context.waitForEvent('page'));
await page.waitForLoadState('domcontentloaded').catch(() => {});

const errors = [];
page.on('pageerror', (e) => errors.push(String(e)));
page.on('console', (m) => m.type() === 'error' && errors.push(m.text()));

try {
  // 1. The window was launched with a folder that holds no workspace, so it
  //    opens on the offer rather than a welcome screen with no memory of it.
  at('the offer is on screen');
  await until(page, 'the "Start a model here?" dialog', () =>
    /start a model here/i.test(document.querySelector('#app-dialog .dialog-title')?.textContent ?? ''));
  const named = await page.evaluate(() => document.querySelector('#app-dialog').textContent);
  const leaf = repo.replace(/[\\/]+$/, '').split(/[\\/]/).pop();
  if (!named.includes(leaf)) fail(`the dialog does not name ${leaf}`);

  // 2. Take it. This is the first time scaffold + agent setup has ever run
  //    from an installed layout with a window attached.
  at('scaffold the workspace');
  await click(page, '#dlg-ok');
  await until(page, 'the hand-off dialog', () =>
    /now ask your agent/i.test(document.querySelector('#app-dialog .dialog-title')?.textContent ?? ''));
  const handoff = await page.evaluate(() => document.querySelector('#app-dialog').textContent);
  if (!/blastradius:model/.test(handoff)) fail('the hand-off names no workflow to run');
  await click(page, '#dlg-ok');

  // 3. A rendered model, out of an install. The whole point of the release.
  at('the model renders');
  const nodes = await until(page, 'nodes on the canvas', () => {
    const n = document.querySelectorAll('#nodes .node').length;
    return n > 0 ? n : false;
  });
  console.log(`    ${nodes} nodes`);
  await until(page, 'the tree', () => document.querySelectorAll('.tree-row').length > 0);

  // 4. Dive into the system: the starter model's containers live at L2, and
  //    the canvas opens on the context.
  at('dive into the system');
  await page.evaluate(() => {
    const box = document.querySelector('#nodes .node:not(.is-person):not(.is-external)');
    if (!box) throw new Error('no system box at the context level');
    box.dispatchEvent(new MouseEvent('dblclick', { bubbles: true }));
  });
  await until(page, 'the container level', () =>
    /containers/i.test(document.querySelector('#breadcrumb')?.textContent ?? '')
    && [...document.querySelectorAll('#nodes .node .node-title')].some((t) => /application/i.test(t.textContent)));

  // 5. Add a component to the starter model — an edit through the real sync
  //    engine, writing real YAML on disk.
  at('add a component through the box menu');
  await page.evaluate(() => {
    const box = [...document.querySelectorAll('#nodes .node')]
      .find((n) => /application/i.test(n.querySelector('.node-title')?.textContent ?? ''));
    if (!box) throw new Error('no Application box');
    const r = box.getBoundingClientRect();
    box.dispatchEvent(new MouseEvent('contextmenu', {
      bubbles: true, clientX: r.left + r.width / 2, clientY: r.top + r.height / 2,
    }));
  });
  await until(page, 'the box menu', () => document.querySelectorAll('.ctx-menu .ctx-item').length > 0);
  await page.evaluate(() => {
    const item = [...document.querySelectorAll('.ctx-menu .ctx-item')]
      .find((i) => /inside/i.test(i.textContent));
    if (!item) throw new Error('no "add inside" item');
    item.click();
  });
  await until(page, 'the create dialog', () => !!document.querySelector('#dlg-name'));
  await fill(page, '#dlg-name', 'Extractor Probe');
  await fill(page, '#dlg-id', 'probe');
  await click(page, '#dlg-ok');
  await until(page, 'the component to exist', () =>
    [...document.querySelectorAll('.tree-row')].some((r) => /extractor probe/i.test(r.textContent)));

  // 6. Point it at the repository's own source and run the extractor. This is
  //    `introspect_component`'s first exercise anywhere outside a mock that
  //    answers "needs the real app" — and the extractor it runs is the one
  //    staged beside the executable, which is where 0.6.0 and 0.6.2 went wrong.
  at('point the component at code');
  await page.evaluate(() => {
    const row = [...document.querySelectorAll('.tree-row')]
      .find((r) => /extractor probe/i.test(r.textContent));
    row.click();
  });
  await until(page, 'the source section', () => !!document.querySelector('#map-add'));
  await click(page, '#map-add');
  await until(page, 'the mapping dialog', () => !!document.querySelector('#dlg-root'));
  await fill(page, '#dlg-language', 'typescript');
  await fill(page, '#dlg-root', 'src');
  await click(page, '#dlg-ok');
  await until(page, 'the mapping to be written', () => !!document.querySelector('#map-run'));

  at('run the extractor from the install');
  await click(page, '#map-run');
  // The toast itself is reported on failure, not just "false": when the
  // extractor is missing from the install — the 0.6.0 bug — it is the toast
  // that says so, and a gate that only says "no" is a gate you have to debug.
  const toast = await until(page, 'the introspection result', () => {
    const t = document.querySelector('.travel-banner.is-toast')?.textContent ?? '';
    if (/code elements derived/.test(t)) return t;
    return /introspecting/.test(t) || !t ? false : `NOT DERIVED: ${t}`;
  });
  if (toast.startsWith('NOT DERIVED:')) fail(toast);
  console.log(`    ${toast.trim()}`);
  const derived = Number(/(\d+) code elements derived/.exec(toast)?.[1] ?? 0);
  if (derived < 1) fail(`the extractor derived nothing from the install: ${toast.trim()}`);

  // 7. And the derived elements are actually on the canvas: dive into the
  //    component, which is only possible when facts were written and reloaded.
  at('the derived code is reachable');
  await page.evaluate(() => {
    const row = [...document.querySelectorAll('.tree-row')]
      .find((r) => /extractor probe/i.test(r.textContent));
    row.dispatchEvent(new MouseEvent('dblclick', { bubbles: true }));
  });
  await until(page, 'the code level', () =>
    /code/i.test(document.querySelector('#breadcrumb')?.textContent ?? '')
    && document.querySelectorAll('#nodes .node').length > 0);

  if (errors.length) fail(`the window logged errors: ${errors.join(' | ')}`);
  console.log('\nAPP SMOKE PASSED');
} catch (e) {
  console.error(`\nAPP SMOKE FAILED ${e.message}`);
  if (errors.length) console.error(`window errors: ${errors.join(' | ')}`);
  try {
    console.error('title: ' + (await text(page, '#app-dialog .dialog-title')));
    console.error('breadcrumb: ' + (await text(page, '#breadcrumb')));
    await page.screenshot({ path: 'app-smoke-failure.png' }).catch(() => {});
  } catch (_) { /* the window may be gone */ }
  process.exitCode = 1;
} finally {
  await browser.close().catch(() => {});
}
