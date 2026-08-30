// What is wrong with the workspace, as a list you can act on (0.11.0 item 6).
//
// Two kinds of finding share one panel because they answer the same question —
// *is this model still true?* — and differ in what "true" means:
//
//   - **validation** — the model contradicts itself. A dangling reference, a
//     bad field. `blastradius validate` says so and names a file and a line.
//   - **drift** (ADR-0019) — the model contradicts the *code*. Either the code
//     depends on something the model never declared, or the model declares
//     something no code reference supports.
//
// Pure: no DOM, no app state. app.js passes a snapshot and binds what comes
// back, which is what lets the rules be tested in node — the same shape as
// menu.js and mockops.js, and for the same reason.

/** Severity order: an error outranks a warning, and both outrank drift, which
 *  is a disagreement rather than a fault. Within a kind, insertion order. */
const RANK = { error: 0, warning: 1, drift: 2 };

/**
 * Every finding in the snapshot, ranked, as rows a panel can render.
 *
 * Each row carries what it takes to *act* on it, which is the whole difference
 * from the list of strings this replaces: `fix` names the operation the row
 * offers, and `focus` names what the canvas should fly to when the row is
 * clicked. A row with neither is a row that can only be read.
 *
 * `nameOf` turns an element id into what a person calls it; ids stay on the row
 * as the subtitle, because two components can share a name and never an id.
 */
export function problemRows(snapshot, { canEdit = true, nameOf = (id) => id } = {}) {
  const rows = [];

  for (const d of snapshot.diagnostics ?? []) {
    // `info` is not a problem — the parser saying it ignored a file without
    // frontmatter is a fact about the workspace, not a fault in it.
    if (d.severity !== 'error' && d.severity !== 'warning') continue;
    rows.push({
      kind: d.severity,
      title: d.message,
      subtitle: d.line ? `${d.file}:${d.line}` : d.file,
      file: d.file,
      // Opening the file is the only fix: what a dangling reference should
      // become is a modelling decision, not one a button can take.
      fix: { op: 'open', label: 'Open' },
      focus: null,
    });
  }

  for (const d of snapshot.drift ?? []) {
    const pair = `${nameOf(d.from)} → ${nameOf(d.to)}`;
    if (d.kind === 'undeclared') {
      rows.push({
        kind: 'drift',
        driftKind: 'undeclared',
        title: pair,
        subtitle: d.via
          ? `the code depends on this — ${d.via}`
          : 'the code depends on this, and the model does not say so',
        from: d.from, to: d.to, via: d.via ?? null,
        fix: canEdit ? { op: 'declare', label: 'Declare' } : null,
        focus: d.from,
      });
    } else if (d.kind === 'unbacked') {
      rows.push({
        kind: 'drift',
        driftKind: 'unbacked',
        title: pair,
        subtitle: 'declared, and no code reference supports it',
        from: d.from, to: d.to, via: null,
        // Reversing is the repair drift can actually prove — our own model got
        // this wrong once, with the dependency running the other way. It is
        // offered, not applied: the other answer is that the relation should
        // not exist, and only a person knows which.
        fix: canEdit ? { op: 'reverse', label: 'Reverse' } : null,
        focus: d.from,
      });
    }
  }

  return rows.sort((a, b) => RANK[a.kind] - RANK[b.kind]);
}

/** What the chip says. Errors win the colour; drift alone is not a failure. */
export function problemSummary(rows) {
  const errors = rows.filter((r) => r.kind === 'error').length;
  const warnings = rows.filter((r) => r.kind === 'warning').length;
  const drift = rows.filter((r) => r.kind === 'drift').length;
  if (!rows.length) return null;
  const parts = [];
  if (errors) parts.push(`${errors} error${errors > 1 ? 's' : ''}`);
  if (warnings) parts.push(`${warnings} warning${warnings > 1 ? 's' : ''}`);
  if (drift) parts.push(`${drift} drift`);
  return {
    label: parts.join(' · '),
    tone: errors ? 'danger' : warnings ? 'warning' : 'accent',
    errors, warnings, drift, total: rows.length,
  };
}
