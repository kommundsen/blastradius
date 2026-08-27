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
  await expect(page.locator('#nodes .node')).toHaveCount(7);
  await expect(page.locator('#edges path.edge')).toHaveCount(6);
  await expect(page.locator('.node.is-person')).toHaveCount(2);
  await expect(page.locator('.node.is-external')).toHaveCount(4);
  await expect(page.locator('#breadcrumb')).toContainText('Context');
  // the tree lists the whole model regardless of altitude: 25 authored
  // elements + 50 derived L4 rows (four introspected components now that
  // model-service and sync-engine are mapped for drift detection, ADR-0019;
  // the 49th is ui/js/labels.js from the C4 bracket rendering, the 50th
  // ui/js/search.js from the find palette)
  // + 20 deployment rows under their own root (ADR-0018)
  await expect(page.locator('.tree-row')).toHaveCount(95);
  await expect(page.locator('.tree-row.is-derived')).toHaveCount(50);
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
  await expect(side).toContainText('Repository access');

  await side.locator('[data-doc="adr-0007"]').click();
  await expect(page.locator('#side-title')).toHaveText('adr-0007');
  await expect(page.locator('.doc-body h1')).toContainText('Embedded libgit2');
  // doc -> element navigation exists
  await expect(side.locator('[data-el="blastradius.core.git-service"]')).toBeVisible();

  expect(page.errors).toEqual([]);
  await page.screenshot({ path: 'test-results/webkit-L3-adr.png', fullPage: true });
});

test('L4: dive into introspected code, inspect a derived type', async ({ page }) => {
  const node = (title) =>
    page.locator('#nodes .node', { has: page.locator('.node-title', { hasText: title }) });

  await node('Blastradius').dblclick();
  await node('Core').dblclick();
  await expect(page.locator('#breadcrumb')).toContainText('Components');

  // Below L3 lies the code: the opted-in component opens its module graph.
  await node('Git Service').first().dblclick();
  await expect(page.locator('#breadcrumb')).toContainText('Code');
  await expect(node('git.rs')).toBeVisible();
  await expect(node('resolve.rs')).toBeVisible();
  // Two modules plus the external crates they pull in, rolled up per package.
  await expect(page.locator('.node.is-derived')).toHaveCount(4);
  await expect(page.locator('.node.is-dependency')).toHaveCount(2);
  await expect(node('git.rs').locator('.node-kicker')).toContainText('derived');
  await expect(node('serde').locator('.node-kicker')).toContainText('external');
  // resolve.rs imports git.rs — a real edge from the committed facts
  await expect(page.locator('#edges path.edge')).not.toHaveCount(0);

  // One more step: a module opens its types.
  await node('resolve.rs').dblclick();
  await expect(node('Resolution')).toBeVisible();
  await expect(node('Side')).toBeVisible();

  // Derived inspector: read-only, points at the source file.
  await node('Resolution').click();
  const side = page.locator('#side-body');
  await expect(side).toContainText('Derived from source');
  await expect(side).toContainText('crates/blastradius-core/src/resolve.rs');
  await expect(side.locator('#insp-name')).toHaveCount(0); // no rename input

  // Escape climbs back out: types → modules → L3.
  await page.keyboard.press('Escape');
  await expect(node('git.rs')).toBeVisible();
  await page.keyboard.press('Escape');
  await expect(page.locator('#breadcrumb')).toContainText('Components');

  // The model explorer lists derived rows; clicking one jumps to its code.
  const treeRow = page.locator('.tree-row.is-derived', { hasText: 'GitContext' });
  await treeRow.click();
  await expect(page.locator('#breadcrumb')).toContainText('Code');
  await expect(node('GitContext')).toBeVisible();
  await expect(page.locator('#side-body')).toContainText('Derived from source');

  expect(page.errors).toEqual([]);
  await page.screenshot({ path: 'test-results/webkit-L4.png', fullPage: true });
});

test('L4 segment is live and jumps to an introspected component', async ({ page }) => {
  const l4 = page.locator('#level-seg input[value="L4"]');
  await expect(l4).toBeEnabled(); // the mock model has derived graphs
  await page.locator('#level-seg .seg-opt', { hasText: 'L4' }).click();
  await expect(page.locator('#breadcrumb')).toContainText('Code');
  await expect(page.locator('.node.is-derived')).not.toHaveCount(0);
  expect(page.errors).toEqual([]);
});

test('LD: dive the deployment tree down to a container instance', async ({ page }) => {
  const node = (title) =>
    page.locator('#nodes .node', { has: page.locator('.node-title', { hasText: title }) });
  const ld = page.locator('#level-seg input[value="LD"]');
  await expect(ld).toBeEnabled(); // the mock model declares environments
  await page.locator('#level-seg .seg-opt', { hasText: 'D' }).click();
  await expect(page.locator('#breadcrumb')).toContainText('Deployment');

  // The overview lists every environment, not a containment diagram
  // (ADR-0018): deployment dives like the logical model.
  await expect(page.locator('.node.is-environment')).toHaveCount(3);
  // And it shows the delivery chain between them. A deployment diagram with
  // no connectors says nothing about how software actually gets anywhere —
  // this asserts the relations survive lifting onto the environments.
  await expect(page.locator('#edges path.edge')).not.toHaveCount(0);
  await expect(page.locator('#edges text.edge-label')).toContainText(['triggers on push']);
  await page.screenshot({ path: 'test-results/webkit-LD-overview.png', fullPage: true });

  await node('Developer Machine').dblclick();
  await expect(node('Windows 11 Workstation')).toBeVisible();
  await node('Windows 11 Workstation').dblclick();
  await expect(node('Terminal')).toBeVisible();

  // One more step reaches the containers actually running there, named
  // after the containers they instantiate.
  await node('Blastradius (dev build)').dblclick();
  await expect(page.locator('.node.is-container-instance')).toHaveCount(3);
  await expect(node('Canvas UI')).toBeVisible();
  await expect(node('Canvas UI').locator('.node-kicker')).toContainText('Container instance');
  await expect(page.locator('#breadcrumb')).toContainText('Blastradius (dev build)');
  await page.screenshot({ path: 'test-results/webkit-LD.png', fullPage: true });

  // Escape climbs back out of the tree.
  await page.locator('#canvas').click({ position: { x: 5, y: 5 } });
  await page.keyboard.press('Escape');
  await expect(node('Terminal')).toBeVisible();
  expect(page.errors).toEqual([]);
});

test('groups draw boundaries behind their members, and only where asked', async ({ page }) => {
  const node = (title) =>
    page.locator('#nodes .node', { has: page.locator('.node-title', { hasText: title }) });

  // L1 has no grouped elements and no show-groups — nothing is drawn.
  await expect(page.locator('.group-box')).toHaveCount(0);

  await node('Blastradius').dblclick();
  // L2 has grouped elements below it but the view does not opt in (spec §3c):
  // labelling an element must never reshape a diagram on its own.
  await expect(page.locator('.group-box')).toHaveCount(0);

  await node('Core').dblclick();
  await expect(page.locator('#breadcrumb')).toContainText('Components');
  const boxes = page.locator('.group-box');
  await expect(boxes).toHaveCount(2);
  await expect(page.locator('.group-label', { hasText: 'Model' })).toBeVisible();
  await expect(page.locator('.group-label', { hasText: 'Interop' })).toBeVisible();

  // A boundary is not a node: every count the rest of the suite pins would
  // break if it were, and it must not swallow clicks meant for its members.
  await expect(page.locator('#nodes .node.group-box')).toHaveCount(0);
  await expect(boxes.first()).toHaveCSS('pointer-events', 'none');

  // It encloses its members and nothing else.
  const geom = await page.evaluate(() => {
    const r = (el) => { const b = el.getBoundingClientRect(); return { l: b.left, t: b.top, r: b.right, b: b.bottom }; };
    const boxes = [...document.querySelectorAll('.group-box')].map((b) => ({
      label: b.querySelector('.group-label').textContent, ...r(b),
    }));
    const nodes = [...document.querySelectorAll('#nodes .node')].map((n) => ({
      title: n.querySelector('.node-title').textContent, ...r(n),
    }));
    return { boxes, nodes };
  });
  const members = { Model: ['Model Service', 'Sync Engine'], Interop: ['Exporter', 'Structurizr Importer'] };
  for (const box of geom.boxes) {
    for (const n of geom.nodes) {
      const inside = n.l >= box.l - 1 && n.r <= box.r + 1 && n.t >= box.t - 1 && n.b <= box.b + 1;
      expect(inside, `${box.label} vs ${n.title}`).toBe(members[box.label].includes(n.title));
    }
  }

  expect(page.errors).toEqual([]);
  await page.screenshot({ path: 'test-results/webkit-groups.png', fullPage: true });
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

test('mouse wheel zooms the canvas about the cursor', async ({ page }) => {
  await page.goto('/index.html?nogit');
  await expect(page.locator('#nodes .node').first()).toBeVisible();
  const before = await page.locator('#zoom-reset').textContent();
  const box = await page.locator('#canvas').boundingBox();
  await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
  await page.mouse.wheel(0, -400); // wheel up = zoom in
  const zoomedIn = await page.locator('#zoom-reset').textContent();
  expect(parseInt(zoomedIn)).toBeGreaterThan(parseInt(before));
  await page.mouse.wheel(0, 800); // and back out further
  const zoomedOut = await page.locator('#zoom-reset').textContent();
  expect(parseInt(zoomedOut)).toBeLessThan(parseInt(zoomedIn));
});

test('side panels resize by dragging their grips, and persist', async ({ page }) => {
  await page.goto('/index.html?nogit');
  await expect(page.locator('#nodes .node').first()).toBeVisible();
  const nav = page.locator('.panel-nav');
  const w0 = (await nav.boundingBox()).width;
  const grip = await page.locator('#nav-grip').boundingBox();
  await page.mouse.move(grip.x + grip.width / 2, grip.y + 200);
  await page.mouse.down();
  await page.mouse.move(grip.x + grip.width / 2 + 80, grip.y + 200);
  await page.mouse.up();
  const w1 = (await nav.boundingBox()).width;
  expect(w1).toBeGreaterThan(w0 + 40);
  // clamped at the design-system maximum
  expect(w1).toBeLessThanOrEqual(320);
  // persisted across reloads
  await page.reload();
  await expect(page.locator('#nodes .node').first()).toBeVisible();
  expect((await nav.boundingBox()).width).toBeCloseTo(w1, 0);
  // keyboard resize on the inspector grip
  const side = page.locator('.panel-side');
  const s0 = (await side.boundingBox()).width;
  await page.locator('#side-grip').focus();
  await page.keyboard.press('ArrowLeft');
  await page.keyboard.press('ArrowLeft');
  expect((await side.boundingBox()).width).toBeCloseTo(s0 + 32, 0);
});

test('edge labels never sit on node boxes (label de-collision)', async ({ page }) => {
  for (const path of ['/index.html?nogit', '/index.html?nogit#l2']) {
    await page.goto(path);
    await expect(page.locator('#nodes .node').first()).toBeVisible();
    if (path.endsWith('#l2')) {
      await page.locator('#nodes .node', {
        has: page.locator('.node-title', { hasText: 'Blastradius' }),
      }).dblclick();
      await expect(page.locator('#breadcrumb')).toContainText('Containers');
    }
    const overlaps = await page.evaluate(() => {
      const boxes = [...document.querySelectorAll('#nodes .node')].map((n) => n.getBoundingClientRect());
      const bad = [];
      for (const label of document.querySelectorAll('.edge-label')) {
        const r = label.getBoundingClientRect();
        for (const b of boxes) {
          const w = Math.min(r.right, b.right) - Math.max(r.left, b.left);
          const h = Math.min(r.bottom, b.bottom) - Math.max(r.top, b.top);
          if (w > 2 && h > 2) bad.push(label.textContent);
        }
      }
      return bad;
    });
    expect(overlaps, `labels overlapping nodes at ${path}`).toEqual([]);
  }
});

test('edges never pass under node boxes (obstacle routing)', async ({ page }) => {
  for (const path of ['/index.html?nogit', '/index.html?nogit#l2']) {
    await page.goto(path);
    await expect(page.locator('#nodes .node').first()).toBeVisible();
    if (path.endsWith('#l2')) {
      await page.locator('#nodes .node', {
        has: page.locator('.node-title', { hasText: 'Blastradius' }),
      }).dblclick();
      await expect(page.locator('#breadcrumb')).toContainText('Containers');
    }
    // layout-space geometry: node style positions and raw path points (the
    // camera transform applies to both equally, so skip it entirely)
    const bad = await page.evaluate(() => {
      const boxes = [...document.querySelectorAll('#nodes .node')].map((n) => ({
        id: n.dataset.id,
        x: parseFloat(n.style.left), y: parseFloat(n.style.top),
        w: n.offsetWidth, h: n.offsetHeight,
      }));
      const offenders = [];
      for (const p of document.querySelectorAll('#edges path.edge')) {
        const pts = p.getAttribute('d').split(/[ML] ?/).filter(Boolean)
          .map((s) => s.split(',').map(Number)).map(([x, y]) => ({ x, y }));
        let total = 0;
        const seg = [];
        for (let i = 1; i < pts.length; i++) {
          const l = Math.hypot(pts[i].x - pts[i - 1].x, pts[i].y - pts[i - 1].y);
          seg.push(l); total += l;
        }
        for (const b of boxes) {
          if (b.id === p.dataset.from || b.id === p.dataset.to) continue;
          let inside = 0;
          const samples = 64;
          for (let s = 0; s <= samples; s++) {
            let want = (total * s) / samples;
            let pt = pts[0];
            for (let i = 0; i < seg.length; i++) {
              if (want <= seg[i]) {
                const t = seg[i] ? want / seg[i] : 0;
                pt = { x: pts[i].x + (pts[i + 1].x - pts[i].x) * t,
                       y: pts[i].y + (pts[i + 1].y - pts[i].y) * t };
                break;
              }
              want -= seg[i]; pt = pts[i + 1];
            }
            if (pt.x > b.x + 1 && pt.x < b.x + b.w - 1 && pt.y > b.y + 1 && pt.y < b.y + b.h - 1) inside++;
          }
          if (inside > 1) offenders.push(`${p.dataset.from}->${p.dataset.to} under ${b.id}`);
        }
      }
      return offenders;
    });
    expect(bad, `edges under nodes at ${path}`).toEqual([]);
  }
});

test('dropping a node onto another nudges it to clear space (min distance)', async ({ page }) => {
  await page.goto('/index.html?nogit');
  const node = (t) => page.locator('#nodes .node', { has: page.locator('.node-title', { hasText: t }) });
  await node('Blastradius').dblclick();
  await expect(page.locator('#breadcrumb')).toContainText('Containers');
  const cli = await node('CLI').first().boundingBox();
  const core = await node('Core').first().boundingBox();
  await page.mouse.move(cli.x + cli.width / 2, cli.y + 10);
  await page.mouse.down();
  await page.mouse.move(core.x + core.width / 2, core.y + 10, { steps: 5 });
  await page.mouse.up();
  await page.waitForTimeout(300); // pin op + re-render
  const overlapping = await page.evaluate(() => {
    const rects = [...document.querySelectorAll('#nodes .node')].map((n) => {
      const r = n.getBoundingClientRect();
      return { id: n.dataset.id, l: r.left, t: r.top, r: r.right, b: r.bottom };
    });
    const bad = [];
    for (let i = 0; i < rects.length; i++)
      for (let j = i + 1; j < rects.length; j++) {
        const a = rects[i], b = rects[j];
        if (Math.min(a.r, b.r) - Math.max(a.l, b.l) > 1 &&
            Math.min(a.b, b.b) - Math.max(a.t, b.t) > 1) bad.push([a.id, b.id]);
      }
    return bad;
  });
  expect(overlapping).toEqual([]);
});
