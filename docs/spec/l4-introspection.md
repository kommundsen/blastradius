---
doc: spec-l4-introspection
type: spec
status: draft
elements: [blastradius.core.model-service, blastradius.cli, blastradius.ui.canvas]
---

# Spec: L4 code introspection

Implements ADR-0016. Components opt in to a source mapping; per-language
extractors emit a common facts file; core derives read-only L4 elements
from it. Language priority: TypeScript/JavaScript and C#/.NET.

## The `source:` mapping

On a component (L3), in the model YAML:

```yaml
components:
  canvas:
    name: Canvas
    source:
      language: typescript        # typescript | csharp
      root: ui/js                 # repo-root-relative (ADR-0014 anchor)
      include: ["**/*.mjs"]       # optional, extractor defaults apply
      exclude: ["**/*.test.mjs"]  # optional
```

`root` is relative to the **repository root**, not the workspace
directory — code lives beside the workspace, and ADR-0014's discovery
already anchors every workspace inside a repo. Validation errors if the
root does not exist or `language` is unknown; `include`/`exclude` are
gitignore-style globs evaluated by the extractor.

## Facts files

One per opted-in component, committed:

```
docs/model/derived/<component-id>.l4.json     # e.g. blastradius.ui.canvas.l4.json
```

Schema v1:

```json
{
  "schema": 1,
  "language": "typescript",
  "extractor": "blastradius-extract-ts 0.1.0",
  "component": "blastradius.ui.canvas",
  "root": "ui/js",
  "sourceDigest": "sha256:…",
  "elements": [
    { "id": "layout",  "kind": "module", "name": "layout.js", "path": "ui/js/layout.js" },
    { "id": "svg",     "kind": "module", "name": "svg.js",    "path": "ui/js/svg.js" }
  ],
  "edges": [
    { "from": "app", "to": "layout", "kind": "imports" }
  ]
}
```

- `elements[].kind`: `module`, `namespace`, `class`, `interface`,
  `record`, `enum`. Type elements carry `parent` (the containing module
  or namespace id). `path` is repo-root-relative with forward slashes;
  type elements add `line` (1-based declaration line) for click-through.
- `edges[].kind`: `imports` (module→module), `references`
  (type→type, name-based), `extends`, `implements`.
- `sourceDigest`: sha256 over the sorted list of (relative path,
  content sha256) of every file the extractor consumed. Cheap to
  recompute without running the extractor — this is the staleness probe.
- **Determinism contract**: elements and edges sorted by id/(from,to),
  LF line endings, two-space indent, no timestamps, no absolute paths.
  Running an extractor twice on the same tree is byte-identical.
- Ids follow ADR-0003 (derived from stable names, not positions):
  module id = root-relative path minus extension, slashes → dots
  (`js/layout` under root `ui` → `js.layout`); C# type id =
  fully-qualified metadata name (`Acme.Billing.Invoice`).

## Derived elements in the model

At workspace open, core loads every facts file, validates it against
the schema and the component's current `source:` mapping, and grafts
read-only elements beneath the owning component:

- Full id: `<component-id>.src.<fact-id>` — the `src` segment keeps the
  derived namespace disjoint from hand-modeled children forever.
- `derived: true` on every grafted element; `apply_operation` and the
  sync engine refuse writes to them with an error naming the source
  file to edit instead.
- A facts file whose `component` no longer exists or no longer has a
  `source:` mapping is a validation **warning** (stale artifact, safe
  to delete); a mapping with no facts file is an **info** (run
  `blastradius introspect`).
- If the recomputed `sourceDigest` disagrees with the facts file, the
  elements still load but carry `stale: true` — surfaced as a canvas
  badge and a validation warning, never an error (committed facts may
  lag the working tree mid-edit; that is normal).

## CLI

```
blastradius introspect [<component-id>] [--check]
```

- No argument: extract every opted-in component. With an id: just that
  one. Workspace resolution follows ADR-0014 discovery like every other
  command.
- Core spawns the extractor for the mapping's language, passes the
  mapping as JSON on stdin, reads facts JSON on stdout, validates, and
  writes `docs/model/derived/…` atomically. Nonzero extractor exit or
  schema-invalid output fails the command with the extractor's stderr.
- `--check`: extract to memory and byte-compare against the committed
  file; nonzero exit on drift. This is the CI staleness gate, same
  pattern as the snapshot gate. CI runs `--check` for the dogfood
  TypeScript mapping (Node is already in the frontend job); the C#
  extractor is gated by its own fixture tests, not by dogfood.

Extractor commands (overridable per mapping with `extractor:`, for
monorepos with pinned toolchains):

| language   | default command                                   |
|------------|---------------------------------------------------|
| typescript | `node <repo>/extractors/typescript/extract.mjs`   |
| csharp     | `dotnet run --project <repo>/extractors/dotnet -c Release` |

The defaults resolve against the Blastradius install dir first, then
the repo, so users don't need the extractors vendored in their repo.

## TypeScript / JavaScript extractor

`extractors/typescript/` — Node ≥ 20, sole dependency the `typescript`
npm package (the compiler API; MIT; the same engine as tsserver).

- Program construction: if `root` (or an ancestor up to the repo root)
  has a `tsconfig.json`, use it via `ts.getParsedCommandLineOfConfigFile`
  filtered to files under `root`; otherwise a default program with
  `allowJs: true, checkJs: false` over the include set (default
  `**/*.{ts,tsx,mts,js,mjs,jsx}`, minus `node_modules`, `.d.ts`, and
  the mapping's excludes).
- **Modules**: every program source file under `root` becomes a
  `module` element. **Types**: exported `class`/`interface`/`enum`
  declarations become children of their module (modules + types
  granularity; functions/consts are not elements in v1).
- **Edges**: `imports` from import declarations, `export … from`
  re-exports, and dynamic `import()` with string literals — resolved
  through the compiler's own module resolution (tsconfig `paths`,
  index files, extension probing). Imports that resolve outside `root`
  or into `node_modules` are dropped in v1 (recorded follow-up:
  external-dependency rollup nodes). Type-only imports collapse into
  the same `imports` edge. `extends`/`implements` edges between
  extracted types via the checker's declared heritage.
- Dogfood: `blastradius.ui.canvas` maps `root: ui/js` — diving below
  the Canvas component shows the real `app/layout/svg/…` module graph.
  Determinism proven by double-run byte-compare in the node test suite.

## C# extractor

`extractors/dotnet/` — a small dotnet console project on
`Microsoft.CodeAnalysis.CSharp`, **syntax-level only** in v1
(ADR-0016): no MSBuild, no restore, works on any checkout regardless of
whether the solution builds.

- Input: `**/*.cs` under `root`, minus `obj/`, `bin/`, generated files
  (`*.g.cs`, `*.Designer.cs`), and the mapping's excludes.
- **Elements**: namespaces (block-scoped and file-scoped) become
  `namespace` elements; `class`/`interface`/`record`/`enum`
  declarations become children of their namespace. Partial types merge
  into one element keyed by fully-qualified name; nested types fold
  into their outermost type in v1.
- **Edges, name-based** (the honest limit of syntax-level): base-list
  identifiers matched against the extracted type corpus produce
  `extends`/`implements`; identifier + qualified-name occurrences in
  type bodies matched against the corpus (respecting that file's
  `using` set) produce `references`. Ambiguous or extern names are
  dropped, not guessed — under-reporting beats false edges.
- Tested against a fixture corpus (`extractors/dotnet/fixtures/`)
  covering file-scoped namespaces, partials, records, and a
  cross-namespace reference — asserting exact facts bytes.
- **Follow-up (recorded, not v1)**: `--semantic` mode via
  `MSBuildWorkspace` for true symbol resolution where a restorable
  solution exists, falling back to syntax-level. Deliberately excluded
  from v1 for its SDK-version fragility.

## Rust and other languages

Deferred (user priority). Recorded candidates for Rust: `syn`
(embeddable in core directly — the one language that wouldn't need an
external process), rustdoc JSON, rust-analyzer. The facts schema is the
extension point: any tool that emits valid facts is a Blastradius
extractor, which is also the future SCIP/LSIF import path (ADR-0016
option 3).

## App & MCP behavior

- **Canvas**: an opted-in component renders a dive affordance (same
  grammar as container→component). L4 uses the existing deterministic
  layout pipeline; derived elements get a `derived` chip and the module
  file name as kicker; `stale: true` adds the staleness badge. The
  inspector shows `path` and offers open-in-editor at `path:line`
  (existing `open_in_editor` command).
- **MCP**: derived elements appear in `find_elements`, `element`, and
  `blast_radius` (marked `derived: true`) — an agent asking for the
  blast radius of a module sees real code-level fan-in. Write tools
  refuse them as above. A `introspect` MCP tool mirrors the CLI
  command so agents can refresh facts.
- **Exports**: derived elements ride along in snapshots and HTML/PNG/SVG
  export automatically (they're ordinary elements by render time).

## Exit criteria (0.3.0 theme 1)

1. Diving below `blastradius.ui.canvas` in the app shows the real
   `ui/js` module/import graph, from committed facts, with click-through
   to source.
2. `blastradius introspect --check` green in CI on the dogfood mapping;
   double-run determinism asserted in tests.
3. C# fixture corpus round-trips through the dotnet extractor with
   byte-exact facts in tests, without any MSBuild/restore step.
4. Derived elements are visibly read-only: `apply_operation` against
   one returns the source-file-pointing error, covered by a test.
