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
  // elements + 54 derived L4 rows (four introspected components now that
  // model-service and sync-engine are mapped for drift detection, ADR-0019;
  // the last three arrived with 0.9.0 — ui/js/menu.js from the box menu, and
  // sync.rs's SourceInput and ViewFileTarget from the source-mapping and
  // view-flag operations)
  // + 20 deployment rows under their own root (ADR-0018)
  await expect(page.locator('.tree-row')).toHaveCount(99);
  await expect(page.locator('.tree-row.is-derived')).toHaveCount(54);
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

  // CI has no view of its own, so it dives one altitude at a time — the
  // product's default everywhere.
  await node('GitHub Actions').dblclick();
  await expect(node('ubuntu-latest Runner')).toBeVisible();
  await expect(page.locator('.node.is-nested')).toHaveCount(0);
  await node('windows-latest Runner').dblclick();
  await expect(node('MSIX Packaging')).toBeVisible();
  await expect(page.locator('#breadcrumb')).toContainText('windows-latest Runner');
  await page.screenshot({ path: 'test-results/webkit-LD.png', fullPage: true });

  // Escape climbs back out of the tree.
  await page.locator('#canvas').click({ position: { x: 5, y: 5 } });
  await page.keyboard.press('Escape');
  await expect(node('ubuntu-latest Runner')).toBeVisible();
  expect(page.errors).toEqual([]);
});

// Containment is opt-in per view (ADR-0018): the developer machine declares
// `nested: true`, so diving into it draws boxes inside boxes instead of one
// altitude at a time. It is the one place the product does this, which is why
// the CI environment above still dives.
test('LD: a view with nested:true draws containment in one frame', async ({ page }) => {
  const node = (title) =>
    page.locator('#nodes .node', { has: page.locator('.node-title', { hasText: title }) });
  await page.locator('#level-seg .seg-opt', { hasText: 'D' }).click();
  await node('Developer Machine').dblclick();

  const workstation = node('Windows 11 Workstation');
  await expect(workstation).toHaveClass(/is-nested/);
  // Two altitudes below the workstation, in the same frame: the containers
  // actually running, named after what they instantiate.
  await expect(node('Blastradius (dev build)')).toHaveClass(/is-nested/);
  await expect(page.locator('.node.is-container-instance')).toHaveCount(4);
  await expect(node('Canvas UI')).toBeVisible();
  await expect(node('Canvas UI').locator('.node-kicker')).toContainText('Container instance');

  // Every child is drawn inside its container's box, and behind nothing.
  const boxes = await page.locator('#nodes .node').evaluateAll((els) =>
    Object.fromEntries(els.map((e) => {
      const r = e.getBoundingClientRect();
      return [e.dataset.id, { l: r.left, t: r.top, r: r.right, b: r.bottom }];
    }))
  );
  const host = boxes['dev-machine.workstation'];
  for (const id of Object.keys(boxes)) {
    if (!id.startsWith('dev-machine.workstation.')) continue;
    const c = boxes[id];
    expect(
      c.l >= host.l - 1 && c.t >= host.t - 1 && c.r <= host.r + 1 && c.b <= host.b + 1,
      `${id} escaped its container`
    ).toBe(true);
  }
  await page.screenshot({ path: 'test-results/webkit-LD-nested.png', fullPage: true });
  expect(page.errors).toEqual([]);
});

// The other half of containment: a container also draws *itself* — a kicker,
// a name, a meta line — and ELK sizes it from its children alone, so anything
// it draws has to be reserved as padding. It was a constant tall enough for a
// one-line kicker, and `[DEPLOYMENT NODE: POWERSHELL]` wraps to two: the
// Terminal container's own name rendered underneath the CLI box inside it.
test('LD: a container never has its own chrome sat on by what it holds', async ({ page }) => {
  const node = (title) =>
    page.locator('#nodes .node', { has: page.locator('.node-title', { hasText: title }) });
  await page.locator('#level-seg .seg-opt', { hasText: 'D' }).click();
  await node('Developer Machine').dblclick();
  await expect(node('Terminal')).toHaveClass(/is-nested/);

  const overlaps = await page.locator('#nodes .node.is-nested').evaluateAll((hosts) =>
    hosts.flatMap((host) => {
      const own = [...host.children].filter((c) => c.matches('.node-kicker, .node-title, .node-meta, .node-desc'));
      const kids = [...host.parentElement.querySelectorAll('.node')].filter(
        (n) => n !== host && n.dataset.id?.startsWith(host.dataset.id + '.')
      );
      const hits = (a, b) =>
        a.left < b.right - 1 && a.right > b.left + 1 && a.top < b.bottom - 1 && a.bottom > b.top + 1;
      return own.flatMap((part) =>
        kids
          .filter((kid) => hits(part.getBoundingClientRect(), kid.getBoundingClientRect()))
          .map((kid) => `${host.dataset.id} ${part.className} under ${kid.dataset.id}`)
      );
    })
  );
  expect(overlaps).toEqual([]);
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

// The dot sheet is endless (0.7.1). It used to be painted on .canvas-camera,
// which is viewport-sized and *translated*, so the dotted area slid away with
// the drawing and the model appeared to sit in a corner of a finite rectangle
// of dots. It now lives on .canvas — which fills the pane and never moves —
// with its size and offset driven from the camera, so it still belongs to the
// drawing rather than to the screen.
test('the dot grid fills the canvas and travels with the drawing', async ({ page }) => {
  const bg = (sel, prop) =>
    page.locator(sel).evaluate((el, p) => getComputedStyle(el)[p], prop);

  // The grid is on the canvas, not on the thing that moves.
  expect(await bg('#canvas', 'backgroundImage')).toContain('radial-gradient');
  expect(await bg('#camera', 'backgroundImage')).toBe('none');

  // It covers the whole pane, at every scroll position, by construction: the
  // element it is painted on is the pane.
  const covers = await page.locator('#canvas').evaluate((el) => {
    const r = el.getBoundingClientRect();
    return r.width > 0 && r.height > 0 && getComputedStyle(el).overflow === 'hidden';
  });
  expect(covers).toBe(true);

  // Panning moves the dots with the model rather than leaving them behind.
  const before = await bg('#canvas', 'backgroundPosition');
  await page.mouse.move(400, 300);
  await page.mouse.down();
  await page.mouse.move(480, 360, { steps: 8 });
  await page.mouse.up();
  const after = await bg('#canvas', 'backgroundPosition');
  expect(after).not.toBe(before);

  // Zooming changes the pitch, so the grid is part of the drawing's scale.
  const pitchBefore = await bg('#canvas', 'backgroundSize');
  await page.locator('#zoom-in').click();
  await page.locator('#zoom-in').click();
  expect(await bg('#canvas', 'backgroundSize')).not.toBe(pitchBefore);
  expect(page.errors).toEqual([]);
});

// Dragging one node used to move every other node (0.7.1). A pinned node
// leaves the ELK graph, so what remains is a *different* graph and gets
// re-laid out: on this repo's own L3 view, one drag moved all eight other
// components by 325-425px each. The first drag in a view now settles the
// whole view — the dragged node where you put it, everything else exactly
// where it already was — so nothing but the dragged node appears to move.
test('dragging one node leaves every other node where it was', async ({ page }) => {
  // `?nogit`, and not by taste: the mock's git fixture carries a merge
  // conflict, and a conflicted workspace is read-only — `canPin()` is false,
  // `beginNodeDrag` returns immediately, and what looks like a drag is a
  // canvas pan. A pan moves every node equally, so every assertion below
  // passes without a pin ever being written. Found 2026-08-29, while adding
  // the unpin tests below.
  await page.goto('/index.html?nogit');
  await expect(page.locator('#nodes .node')).not.toHaveCount(0);
  // Land on a view with enough nodes for "everything moved" to be visible,
  // by diving. This used to go through the command palette, which reaches the
  // same place when it is quick enough and stays at L1 when Enter beats the
  // result list — and L1 has no view file in the mock, whose `pin` cannot
  // author one, so the pins vanished and the drag became a pan. The guard
  // below now catches that; the dive stops it happening.
  const node = (title) =>
    page.locator('#nodes .node', { has: page.locator('.node-title', { hasText: title }) });
  await node('Blastradius').first().dblclick();
  await expect(page.locator('#breadcrumb')).toContainText('Containers');
  await node('Core').first().dblclick();
  await expect(page.locator('#breadcrumb')).toContainText('Components');
  await expect(page.locator('#app')).toHaveClass(/can-edit/);
  await expect(page.locator('#nodes .node')).not.toHaveCount(0);

  // Screen positions divided by the camera scale, i.e. model space. A drop
  // that extends the drawing re-fits the camera, which both slides and
  // *shrinks* everything on screen without rearranging anything.
  const boxes = async () =>
    page.locator('#nodes .node').evaluateAll((els) => {
      // The live matrix, not --camera-scale: the variable holds the camera's
      // *destination* while the transition is still flying, so dividing by it
      // mid-flight scales every reading wrongly and fakes a rearrangement.
      const m = new DOMMatrix(getComputedStyle(document.getElementById('camera')).transform);
      const scale = m.a || 1;
      return Object.fromEntries(els.map((e) => {
        const r = e.getBoundingClientRect();
        return [e.dataset.id, [r.left / scale, r.top / scale]];
      }));
    });
  const before = await boxes();
  const ids = Object.keys(before);
  expect(ids.length).toBeGreaterThan(3);

  const target = page.locator(`#nodes .node[data-id="${ids[0]}"]`);
  const box = await target.boundingBox();
  await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
  await page.mouse.down();
  await page.mouse.move(box.x + box.width / 2 + 90, box.y + box.height / 2 + 60, { steps: 10 });
  await page.mouse.up();
  await expect
    .poll(async () => Math.round((await boxes())[ids[0]][0]))
    .not.toBe(Math.round(before[ids[0]][0]));
  // A pin actually reached the engine: without this the test passes on a
  // canvas *pan*, which moves the dragged node and every other node equally.
  await expect(page.locator('#undo-btn')).toBeEnabled();

  const after = await boxes();

  // Compare the *arrangement*, not absolute screen positions: a drop that
  // extends the drawing re-fits the camera, which slides everything equally
  // and is not the complaint. The distance between any two nodes that were
  // not dragged is what must not change — that is what "everything jumps
  // around" actually means.
  //
  // Tolerance: settling rounds each node to the 26px grid, so an endpoint can
  // shift by at most half a unit on each axis — hypot(13, 13) = 18.4px — and a
  // *pair* by twice that, 37px. 40 covers it and still catches the old
  // behaviour by a factor of ten, which moved nodes 325-425px each.
  const others = ids.slice(1).filter((id) => after[id] && before[id]);
  expect(others.length).toBeGreaterThan(3);
  const gap = (m, a, b) => Math.hypot(m[a][0] - m[b][0], m[a][1] - m[b][1]);
  for (let i = 0; i < others.length; i++) {
    for (let j = i + 1; j < others.length; j++) {
      const [a, b] = [others[i], others[j]];
      expect(
        Math.abs(gap(after, a, b) - gap(before, a, b)),
        `${a} and ${b} moved relative to each other`
      ).toBeLessThan(40);
    }
  }
  expect(page.errors).toEqual([]);
});

// 0.8.0 made the first drag in a view pin every node, so one drag converts a
// view to fully manual — and until 0.9.0 there was no unpin operation at all,
// in the engine or anywhere else, so the only way back was an undo that was
// still on the stack. The way out is where the pinning happened: on the
// diagram.
test('a view pinned by one drag returns to auto-layout in one action', async ({ page }) => {
  await page.goto('/index.html?nogit'); // editing is off in a conflicted workspace
  const node = (title) =>
    page.locator('#nodes .node', { has: page.locator('.node-title', { hasText: title }) });
  // Dived rather than searched: the palette can land on a derived L4 element,
  // where nothing is pinnable and a drag is a pan — which looks like a drag to
  // any assertion about the node having moved.
  await node('Blastradius').first().dblclick();
  await expect(page.locator('#breadcrumb')).toContainText('Containers');
  await node('Core').first().dblclick();
  await expect(page.locator('#breadcrumb')).toContainText('Components');
  await expect(page.locator('#app')).toHaveClass(/can-edit/);
  await expect(page.locator('#nodes .node')).not.toHaveCount(0);

  // Model space, for the reason the settle test gives: a drop that extends the
  // drawing re-fits the camera, which slides and shrinks everything on screen
  // without rearranging anything.
  const boxes = async () =>
    page.locator('#nodes .node').evaluateAll((els) => {
      const m = new DOMMatrix(getComputedStyle(document.getElementById('camera')).transform);
      const scale = m.a || 1;
      return Object.fromEntries(els.map((e) => {
        const r = e.getBoundingClientRect();
        return [e.dataset.id, [r.left / scale, r.top / scale]];
      }));
    });
  const before = await boxes();
  const ids = Object.keys(before);
  expect(ids.length).toBeGreaterThan(3);

  const target = page.locator(`#nodes .node[data-id="${ids[0]}"]`);
  const box = await target.boundingBox();
  await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
  await page.mouse.down();
  await page.mouse.move(box.x + box.width / 2 + 90, box.y + box.height / 2 + 60, { steps: 10 });
  await page.mouse.up();
  await expect
    .poll(async () => Math.round((await boxes())[ids[0]][0]))
    .not.toBe(Math.round(before[ids[0]][0]));
  // The node moves on pointerdown, for feedback, well before the pins reach
  // the engine — so wait for the transaction rather than for the picture, or
  // the right-click below can arrive at a view with nothing pinned in it yet.
  await expect(page.locator('#undo-btn')).toBeEnabled();

  // One drag pinned the whole view, and the canvas says so where it happened.
  await page.locator('#canvas').click({ button: 'right', position: { x: 6, y: 6 } });
  const reset = page.locator('.ctx-menu .ctx-item', { hasText: 'Back to auto-layout' });
  await expect(reset).toContainText(`${ids.length} pinned`);
  await reset.click();

  // And the arrangement is the one the layout engine produced before anything
  // was pinned. Pairwise distances rather than positions, for the reason the
  // settle test gives: releasing the pins shrinks the drawing back and re-fits
  // the camera, which slides everything without rearranging it. Auto-layout is
  // deterministic (ADR-0006), so the tolerance here is a pixel, not a grid
  // cell — this is the same arrangement, not a similar one.
  const gap = (m, a, b) => Math.hypot(m[a][0] - m[b][0], m[a][1] - m[b][1]);
  await expect.poll(async () => {
    const after = await boxes();
    if (!ids.every((id) => after[id])) return false;
    return ids.every((a) => ids.every((b) => Math.abs(gap(after, a, b) - gap(before, a, b)) < 2));
  }, { message: 'the released view returns to its auto-layout arrangement' }).toBe(true);

  // Nothing is pinned now, so the canvas has nothing to offer.
  await page.locator('#canvas').click({ button: 'right', position: { x: 6, y: 6 } });
  await expect(page.locator('.ctx-menu')).toHaveCount(0);

  // It was one operation, so one undo brings the whole arrangement back.
  await page.keyboard.press('Control+z');
  await expect
    .poll(async () => Math.round((await boxes())[ids[0]][0]))
    .not.toBe(Math.round(before[ids[0]][0]));
  expect(page.errors).toEqual([]);
});

// The box carries the single-element half of the same idea.
test('a pinned box offers to release just itself', async ({ page }) => {
  await page.goto('/index.html?nogit'); // editing is off in a conflicted workspace
  const node = (title) =>
    page.locator('#nodes .node', { has: page.locator('.node-title', { hasText: title }) });
  await node('Blastradius').dblclick();
  await expect(page.locator('#breadcrumb')).toContainText('Containers');
  // The L2 view is pinned in the fixture, so the boxes here are pinned ones.
  const core = node('Core').first();
  await core.click({ button: 'right' });
  const count = await page.locator('.ctx-menu .ctx-item', { hasText: 'Back to auto-layout' }).textContent();
  const pinnedBefore = Number(count.match(/([0-9]+) pinned/)[1]);
  expect(pinnedBefore).toBeGreaterThan(1);
  await page.locator('.ctx-menu .ctx-item', { hasText: 'Unpin this element' }).click();

  // That box alone is released: the others keep their pins, so the menu still
  // offers the view-wide reset, counting one fewer.
  await core.click({ button: 'right' });
  await expect(page.locator('.ctx-menu .ctx-item', { hasText: 'Back to auto-layout' }))
    .toContainText(`${pinnedBefore - 1} pinned`);
  // And with its pin gone, the box is not offered a release it no longer needs.
  await expect(page.locator('.ctx-menu .ctx-item', { hasText: 'Unpin this element' })).toHaveCount(0);
  expect(page.errors).toEqual([]);
});
