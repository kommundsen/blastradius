---
doc: spec-git-diff
type: spec
status: draft
elements: [blastradius.core.git-service]
---

# Spec: git integration and semantic diff

Implements ADR-0007. Read-only in v1: the app observes the repository; the
user's own tooling writes to it.

## Detection & status

On workspace open, walk up from the manifest to find `.git`. Absent → all
git UI is hidden (not disabled). Present → the chrome shows branch, dirty
count and ahead/behind (design system: `⎇ main` tag + semantic tags), updated
on watcher events and window focus.

## Semantic diff

Diff = compare two **parsed models**, not two texts.

- **Base selection**: default merge-base with the default branch; switchable
  to any ref or commit via the History control.
- The git service materialises the model files at base (in-memory via git2
  blob reads — no checkout), parses both revisions with the same model
  service, and diffs the element graphs.

Classification per element and relation:

| State | Meaning | Canvas rendering (design system) |
| --- | --- | --- |
| added | id exists only in working model | `.is-added` + badge `+` |
| removed | id exists only at base | `.is-removed` ghost + badge `−` |
| changed | same id, differing fields (name/tech/description/relations) | `.is-changed` + badge `~` |

Removed elements render as ghosts *in the diff view only* — they must be
visible to review a deletion. The sidebar tree mirrors the same states
(`.tree-row.is-added/-removed`).

**Layout changes are not architecture changes**: views-file diffs are excluded
from the default diff and from the status chip counts, behind a "show layout
changes" toggle. Doc changes (frontmatter links) count as `changed` on the
linked elements.

Renames: same id = same element (ADR-0003), so a rename is a `changed` name
field — never an add+remove pair. Id reuse across deletions is the documented
hazard, not detected in v1.

## Conflicts

When the repository has merge conflicts touching workspace files:

- Conflicted model files are inherently STALE (conflict markers do not parse).
  The engine additionally reads stage 2 ("ours") and stage 3 ("theirs") via
  git2 and parses each side.
- The canvas renders the **ours** side (stage-2 overlay) — the on-disk files
  carry conflict markers and do not parse, but the model must stay viewable.
- Elements that differ between the sides render `.is-conflict` (hatched) with
  badge `!`; the inspector shows ours/theirs field values read-only,
  side by side.
- **In-app resolution** (shipped 2026-08-23, ADR-0015): the inspector offers
  keep-ours/keep-theirs per conflicted element and one apply action
  (undecided elements keep ours). The core rebuilds each conflicted file
  from the chosen side's stage text with CST splices — comments and
  formatting of the kept side survive — validates the complete outcome
  before writing, then stages via the user's own `git add` (libgit2 stays
  read-only). Whole-system divergence resolves per file. The
  "resolve in editor" affordance remains for markdown and exotic cases.
- **MCP resolution** (shipped 2026-08-23, 0.3.0 theme 2): the same
  pipeline is agent-callable — `git_status` and `git_conflicts` read
  repository state (conflicts element-shaped, ours/theirs field values
  inline), `resolve_conflicts` applies a per-element resolution and
  stages it; the commit stays the user's. Integration-tested end to
  end against a manufactured merge conflict in tests/mcp.rs.

## History

The History control lists commits touching workspace files (git2 revwalk with
path filter). Selecting a commit enters read-only time-travel: the canvas
renders that revision, diff-against-base recomputed. Leaving time-travel
returns to the working tree. Editing is disabled while time-travelling.
