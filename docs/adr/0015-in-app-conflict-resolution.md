---
doc: adr-0015
type: adr
status: accepted
elements: [blastradius.core.git-service, blastradius.ui.canvas]
---

# ADR-0015: In-app conflict resolution — splices on the chosen side, staged by the user's git

## Status
Accepted — 2026-08-23

## Context

v1 deliberately stopped at detect-and-display (ADR-0007): conflicted
elements render hatched with ours/theirs shown read-only, and resolution
meant leaving the app for a text editor full of conflict markers. That
exit was always the named v2 candidate; 0.2.0 theme 3 scheduled it.

Two constraints shaped the design. **Pins-of-the-git-world:** ADR-0007's
libgit2 stays read-only — the app performs no merges, commits, or pushes.
**CST preservation:** every write in this product goes through
format-preserving splices; a resolution that reserializes YAML would be
the only write that destroys comments, which is exactly backwards for the
one moment users are comparing two texts they care about.

## Decision

Per-element ours/theirs decisions, applied as **CST splices onto the
chosen base side's stage text** (core `resolve` module):

1. Parse stage-2 and stage-3 through the ordinary loader (as conflict
   display already does); the element-level diff between them is the
   decision surface.
2. Each conflicted file starts as its base side's full text (default
   ours — git's convention, and the side whose comments survive).
   Elements decided the other way are spliced in: changed fields via
   `set_field`/field removal, deletions via `remove_entry`, additions via
   `insert_entry`. Whole-system divergence resolves per file, not per
   element.
3. The complete resolution is **validated before anything touches
   disk** — an invalid outcome (e.g. taking a deletion while the other
   side added a relation to it) is refused with the diagnostic, working
   tree untouched.
4. Files are written, then **staged by shelling to the user's own `git
   add`** — the same division of labour `blastradius init` uses for
   `git init`. libgit2 never writes; if no git binary is on PATH, the
   files are written and the error says to stage manually.

The inspector grew keep-ours/keep-theirs per element plus one apply
action ("undecided keep ours" stated on the button); the external-editor
path remains as the escape hatch for markdown and exotic conflicts.

## Consequences

- The 0.2.0 exit holds byte-level: `tests/resolve.rs` manufactures real
  merge conflicts and asserts the resolved file equals the base text with
  exactly the chosen splices — comments intact — index conflict-free,
  resolution staged, workspace valid.
- Undecided-keeps-ours makes partial attention safe: applying with zero
  decisions is precisely `git checkout --ours` + `git add`, with
  validation on top.
- The MCP server does not expose resolution yet — agents hitting a
  conflicted workspace still see stale files; recorded as a follow-up,
  not a gap discovered later.
