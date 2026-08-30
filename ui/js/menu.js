// What a box offers when you right-click it — decided here, bound to handlers
// in app.js.
//
// The editing operations existed long before a surface admitted to them:
// drawing a relation was bound to `R` and advertised nowhere, delete was the
// `Delete` key, and the menu itself carried a single item. So this module
// answers one question — for each `sync::Operation`, does the box offer it,
// and under what conditions — and `ui/tests/menu.test.mjs` reads the enum out
// of `crates/blastradius-core/src/sync.rs` and fails if a variant is neither
// offered nor listed in NOT_ON_THE_BOX. A new operation then cannot be added
// without someone deciding whether the diagram offers it.
//
// Pure: no DOM, no state — app.js passes what it knows and binds what comes
// back, which is what makes the rules testable in node.

/** Which kinds an element may contain, by its own kind (spec §3, §3b). A kind
 *  absent from here is a leaf: a component's insides are derived from source
 *  (spec/l4-introspection.md), and a person or external system has none. */
export const CHILD_KINDS = {
  system: ['container'],
  container: ['component'],
  environment: ['deployment-node'],
  'deployment-node': ['deployment-node', 'container-instance'],
};

/** Operations the box deliberately does not offer, and why. Exhaustive against
 *  `sync::Operation` together with the items below — the test asserts it. */
export const NOT_ON_THE_BOX = {
  pin: 'dragging the box is the pin; a menu item meaning "now drag me" is not one',
  'set-field': "an element's own fields are edited in the inspector, beside the text",
  'set-source': "a mapping is several fields at once: the box offers to start one, the inspector edits it",
  'set-view-flag': 'a flag belongs to the whole diagram, not to the box you happen to be over — the View panel has them',
  'delete-relation': 'a relation is chosen by clicking the edge, and has its own inspector',
  'set-relation-field': 'same: a relation is not a box',
  'reverse-relation': 'same again, and it is offered where the reason to reverse is visible: beside the drift finding that says the dependency runs the other way',
};

const article = (word) => (/^[aeiou]/i.test(word) ? 'an' : 'a');

/**
 * The menu for one box, in order. Items are `{ id, op, label }`; `{ sep: true }`
 * separates groups — what the element *is*, where it *sits*, and removing it —
 * and is only ever emitted between two groups that both have items.
 *
 * ctx: { canEdit, canPin, kind, pinned, pinnedCount, hasDescription, described,
 *        hasSource }
 */
export function boxMenuItems(ctx) {
  // Editing is off entirely while the model is stale, conflicted, or being
  // time-travelled; the caller checks too, but a menu builder that answers
  // "these are your options" must not answer it wrongly on its own.
  if (!ctx.canEdit) return [];

  const model = [
    { id: 'connect', op: 'add-relation', label: 'Connect to…' },
    { id: 'rename', op: 'rename', label: 'Rename…' },
    ctx.hasDescription
      ? {
          id: 'describe',
          op: 'show-description',
          label: ctx.described ? 'Hide description' : 'Show description',
        }
      // Nothing to draw until there is something to say, so this hands over to
      // the field that writes one rather than offering an empty box. It runs no
      // operation of its own — `op: null` — which is why 'set-field' stays
      // listed above as something the box does not do.
      : { id: 'add-description', op: null, label: 'Add a description…' },
  ];
  // A component with no code behind it yet: below it is where the code would
  // be, so the offer belongs beside "add a child" that other kinds get.
  if (ctx.kind === 'component' && !ctx.hasSource) {
    model.push({ id: 'map-source', op: null, label: 'Point at its code…' });
  }
  const kinds = CHILD_KINDS[ctx.kind] ?? [];
  if (kinds.length) {
    // Named when there is one answer, generic when the create dialog will ask:
    // a deployment node holds either more nodes or the containers that run on
    // it, and guessing which would be worse than a dialog that offers both.
    model.push({
      id: 'child',
      op: 'create',
      label: kinds.length === 1
        ? `Add ${article(kinds[0])} ${kinds[0]} inside…`
        : 'Add an element inside…',
    });
  }

  const layout = [];
  if (ctx.canPin && ctx.pinned) {
    layout.push({ id: 'unpin', op: 'unpin', label: 'Unpin this element' });
  }
  if (ctx.canPin && ctx.pinnedCount) {
    layout.push({
      id: 'reset-layout',
      op: 'unpin',
      label: `Back to auto-layout (${ctx.pinnedCount} pinned)`,
    });
  }

  const remove = [{ id: 'delete', op: 'delete', label: 'Delete…' }];

  return [model, layout, remove]
    .filter((group) => group.length)
    .reduce((out, group) => (out.length ? [...out, { sep: true }, ...group] : group), []);
}

/** The view's own menu, for a right-click on the canvas rather than on a box.
 *  There is exactly one thing to say about a diagram with no box in hand. */
export function canvasMenuItems(ctx) {
  if (!ctx.canPin || !ctx.pinnedCount) return [];
  return [{
    id: 'reset-layout',
    op: 'unpin',
    label: `Back to auto-layout (${ctx.pinnedCount} pinned)`,
  }];
}
