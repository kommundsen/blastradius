---
doc: adr-0020
type: adr
status: accepted
elements: [blastradius.ui, blastradius.ui.canvas]
---

# ADR-0020: Where a surface goes, and what the app bar is allowed to carry

## Status

Accepted — 2026-08-30

## Context

0.11.0 item 6 asked a question that had no answer to appeal to: *where does the
problems panel go?* The app had three plausible homes — a fourth side-panel
tab, a bottom drawer, or the floating overlay the diagnostics list already
uses — and no stated rule that made one of them right. Picking on cost alone
would have been picking on nothing, which is what prompted the review: the
owner's words were "at some point we need a design review. Things are starting
to become cluttered."

The design system already fixes the shell
(`.app > .app-bar + .app-body > .panel-nav | .canvas | .panel-side`), the
density steps, the spacing rhythm and the colour semantics. What it has never
said is **which region a given piece of UI belongs in**, or **what the app bar
is allowed to accumulate**. Eleven releases of features each answered that
question locally, and locally each answer was reasonable.

### The measurement

The review argued from numbers rather than from an impression.
`ui/tests/e2e/chrome-audit.spec.mjs` records them.

**The app bar overflows, and the overflow is unreachable.** The bar is
`flex-wrap: nowrap` with a documented drop mechanism at three breakpoints
(`bar-drop-1/2/3` at 980 / 820 / 680px). Exactly one of nine buttons is tagged
with it — Theme, at `bar-drop-2`. `bar-drop-1` and `bar-drop-3` are tagged on
nothing at all.

| window | bar overflow | dropped | pushed off-screen |
| --- | --- | --- | --- |
| 1280 | 0 | — | — |
| 980 | 0 | — | — |
| 820 | 0 | theme | — |
| 680 | 0 | theme | — |
| 560 | 91px | theme | help, share |
| 480 | 171px | theme | open, help, **share** |

The window's own minimum is 480×400 (`min_inner_size`), so **at a size the app
permits, three controls including the primary action cannot be reached**, and
there is no menu behind which they live. That is not a matter of taste; it is a
defect with a number on it, and it has been shipping since the bar reached its
current length.

**The canvas overlays are fine, and were the thing most feared.** At the
default 1280×800 window the bottom-left cluster covers 3.1% of the canvas and
the first-run tour card 10.7%, and the card is `pointer-events: none` with
`auto` on its two buttons — 0.10.0 item 9 did that work properly and there is
an e2e test that clicks straight through it. The one overlay not measured is
the diagnostics list, because the dogfood model is valid by policy
(`conformance.rs` fails the build otherwise) and it therefore never appears.

**The inspector overflows at the default window, and not where it was
assumed.** The panel body is roughly 747px tall at an 800px window:

| inspector | inputs | buttons | sections | scrollHeight |
| --- | --- | --- | --- | --- |
| system (L1) | 5 | 8 | 3 | 907px |
| component (L3) | 4 | 6 | 4 | 821px |
| relation, with drift | 3 | 4 | 6 | 810px |

All three scroll. The relation inspector — the one 0.11.0 item 5 had just made
denser, and the one the review set out expecting to indict — is the *lightest*
of the three on every count. The system inspector is the heaviest and nothing
recent touched it. Recording that because it is the sort of thing a review
finds only by measuring, and the sort of conclusion that gets quietly dropped
when it contradicts the reason the review was called.

## Decision

### 1. The app bar carries state and the primary action; everything else is behind one menu

The bar's contents are ranked, and the ranking is written down so the next
feature has somewhere to be told no:

- **Always** — brand, breadcrumb, level segment, git chips, problem chips,
  undo/redo, Share. Where you are, what is true, what you just did, and the one
  thing the product is for.
- **Behind an overflow menu** (`⋯`, which itself never drops) — Open, Theme,
  Help, and the git actions Diff and History. Each keeps its keyboard shortcut,
  and the menu names it.
- **Find** stays in the bar. It is the entry to every model bigger than a
  screen, and demoting the answer to "how do I get anywhere" into a menu would
  undo 0.7.0.

A new control goes in the menu unless someone argues it into the always list,
which is the direction the default should point.

**Gated, not asserted**: `chrome-audit.spec.mjs` fails the build if any bar
control is pushed off-screen at 480px — the minimum window the app itself
permits. A rule with no test is a comment.

#### What implementing it changed

Moving five buttons was not enough on its own: at 480px the bar was still 29px
over. The measurement said why, and it was not the buttons.

- **Empty slots were costing a gap each.** The breadcrumb, the git chips, the
  diagnostics chips and the spacer are all zero-width when there is nothing to
  say, and the bar was paying its full 14px gap around every one of them —
  roughly 70px of pure nothing. Empty chip spans are now `display: none`, with
  the spacer exempt because being empty is its whole job.
- **A narrow window tightens the rhythm** before it drops another control:
  `gap` steps from `--space-4` to `--space-2` under 680px, which is the
  language the density steps already speak.
- **Find gets a fourth drop step**, at 520px, and this is the one place the
  decision above bent. Find holds down to 560px, but at 480 the level segment
  alone is 169px and something has to yield. Find is the only candidate whose
  keyboard route survives — Ctrl+K still opens it — so it drops last rather
  than not at all. `bar-drop-4` is new; `bar-drop-1` and `bar-drop-3` remain
  tagged on nothing, which is now a deliberate reserve rather than an oversight.

### 2. Three regions, three jobs

Every surface answers one of three questions, and that answer names its home:

| the surface is about… | it lives in | examples |
| --- | --- | --- |
| **the drawing** — what is on screen and how to move around it | canvas overlay | zoom, +Element, the hint, the tour card, the empty state, the box and canvas menus |
| **the selected thing** | the side panel | element inspector, relation inspector, the View tab, Source |
| **the workspace as a whole**, consulted and acted on | a dismissible panel anchored to its chip in the app bar | validation diagnostics, drift findings — the problems panel |

The third row is the new one, and it is what item 6 was missing. It is *not*
"the cheapest option won": a problems panel belongs neither to the drawing (it
outlives any one level, and its findings name elements you are not looking at)
nor to the selection (it is what you consult *before* there is a selection).
Anchoring it to the chip that counts it makes the count and the list the same
object, and keeping it out of the side panel means the list survives while you
fix what it points at — which is the whole difference between a report and a
workflow.

Consequence taken deliberately: the panel covers part of the canvas.

**Amended 2026-08-30, during implementation.** This first said the panel would
follow the tour card's rules — "dismissible, and never eating a gesture aimed at
a node beneath it". The second half is not achievable and should not have been
written: the tour card can be `pointer-events: none` because it is a notice, and
a panel whose rows carry a *Declare* and a *Reverse* button has to take its own
clicks. An e2e test caught it doing exactly what the sentence forbade — after a
repair the layout moved a node under the panel, and `elementFromPoint` at that
node's centre returned the panel.

So the guarantee is the achievable one instead:

- closed by default, and it never opens itself — the chip is a count until
  someone asks for the list;
- three ways out (the chip toggles, its own ✕, Escape), so a panel in the way is
  one keystroke from gone;
- it does not reappear after an edit: a repaint keeps the panel open only if it
  was already open, and closes it when nothing is left to show.

What remains true is that it covers part of the drawing while it is open, and
that is now a measured cost rather than a denied one. If it proves to be the
wrong trade, the region table above does not change — only which of the three
homes the third row means, and a bottom drawer is the alternative that keeps the
list and the canvas both fully usable.

### 3. The inspector renders nothing empty, and long sections collapse

Sections with no content do not render a heading (the Documents section says
"None linked." today, which is a heading and a sentence to say nothing).
Sections that are long and secondary — Documents, Conflicts, and the
properties block — collapse, with the state remembered per element kind rather
than per element. The system inspector at 907px is the target; the relation
inspector at 810px is not the problem and is not to be cut to make a point.

## Consequences

- One bar control's home is now a decision someone has to make rather than a
  place to append to, and the gate makes appending fail.
- Item 6 has a home and a reason, and is unblocked.
- The `bar-drop-1/2/3` mechanism either gets used or gets deleted; this ADR
  uses it for the always-list under pressure, so it stays.
- The design system gains a page it did not have: which region a surface
  belongs in. `design-system/guidelines/` documents tokens, density and
  elevation, and has never documented information architecture.
- `.app-bar`'s rules changed in `design-system/components/components.css`, and
  therefore in `ui/ds/components/components.css` — **which was a hand-maintained
  copy with nothing checking it**. A product whose thesis is that documentation
  should not be able to rot quietly had a copy of its own design system that
  could. Found by this change having to edit both by hand, and **fixed in the
  same release**: `tools/sync-ds.mjs` copies the source to the shipped subset,
  and `--check` fails the build when they differ — the same shape as
  `blastradius introspect --check`, which is the product's own answer to
  exactly this problem pointed at source code.

  What ships is derived rather than listed: whatever `styles.css` imports,
  transitively, plus `assets/`. Adding a file to the design system does not need
  the script edited — importing it does. A file in the copy that nothing imports
  any more is reported as orphaned and left alone, because a script that quietly
  deletes is a script nobody runs.

## What this deliberately does not decide

- **The level segment's five options** (L1 L2 L3 L4 D), two of which are
  usually disabled. It is the app's most concentrated piece of chrome and the
  most load-bearing; changing it is a navigation decision, not a layout one,
  and it wants its own look at how people actually move between altitudes.
- **Whether the side panel should carry four tabs.** This ADR removes the
  reason to ask by giving the problems panel a different home; if a fifth
  surface wants the panel, the question returns and should be answered then.
- **A command palette for actions** as an alternative to the overflow menu.
  `search.js` ranks elements, docs and relations, not commands. Worth revisiting
  if the menu grows past the handful of items it starts with.
