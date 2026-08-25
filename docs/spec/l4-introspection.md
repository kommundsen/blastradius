---
doc: spec-l4-introspection
type: spec
status: draft
elements: [blastradius.core.introspector, blastradius.cli, blastradius.ui.canvas]
---

# Spec: L4 code introspection

Implements ADR-0016. Components opt in to a source mapping; per-language
extractors emit a common facts file; core derives read-only L4 elements
from it. Languages in scope: TypeScript/JavaScript and C#/.NET (the
priority stack) plus Rust (built-in, dogfooding this repo's own crates).

**Introspection is optional, per component.** L4 is an ordinary model
level first: users can hand-model code-level children in the YAML like
any other element — editable, sync-engine-managed, no toolchain
involved. A component only gets derived elements if it carries a
`source:` mapping; without one, nothing runs and nothing is generated.
The two styles coexist even on the same component: derived elements
live under the reserved `.src.` id segment, hand-modeled children
outside it, so neither can collide with or overwrite the other.

## The `source:` mapping

On a component (L3), in the model YAML:

```yaml
components:
  canvas:
    name: Canvas
    source:
      language: typescript        # typescript | csharp | rust
      root: ui/js                 # repo-root-relative (ADR-0014 anchor)
      include: ["**/*.mjs"]       # optional, extractor defaults apply
      exclude: ["**/*.test.mjs"]  # optional
      mode: syntax                # optional: syntax (default) | semantic
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
  `record`, `enum`, `dependency`. Type elements carry `parent` (the
  containing module or namespace id). `path` is repo-root-relative with
  forward slashes; type elements add `line` (1-based declaration line)
  for click-through.
- `edges[].kind`: `imports` (module→module, and module/namespace→
  dependency), `references` (type→type, name-based), `extends`,
  `implements`.

### Outbound references and drift (ADR-0019)

A reference that leaves a component's mapped corpus but stays inside the
repository is recorded in `outbound` as `{from, path}` — the element holding
it, and the repo-relative file it points at:

```json
"outbound": [
  { "from": "git", "path": "crates/blastradius-core/src/model.rs" }
]
```

Deliberately a **file**, not a component: a per-component extractor cannot know
who owns that file, and guessing would bake one component's view into its own
facts. The workspace resolves ownership at load time by matching the path
against every `source:` mapping, which turns the reference into a code
dependency between components — the input to drift detection.

- **Rust** records any `crate::`/`self::`/`super::` path that its corpus cannot
  resolve, mapping the module path to `<root>/<a>/<b>.rs` or `.../mod.rs`
  (longest prefix that exists on disk wins, since trailing segments may be
  types). Bare unanchored paths remain external-crate rollups.
- **TypeScript** records anything the compiler resolves to a real file outside
  the mapped root but inside the repository — the resolver already knows the
  exact path, so there is no inference.
- **C#** records nothing yet: at syntax level it resolves namespaces, not
  paths, so there is no file to name. Recorded, not solved.

Empty is omitted, and facts written before 0.5.0 simply have none.

### External dependency rollups

Imports that leave the mapped tree used to vanish, which made a module
that leans on `serde` or `react` look self-contained. They now roll up
to **one node per package**:

- Id `dep.<package>` with the package name verbatim — `dep.serde`,
  `dep.left-pad`, `dep.@scope/widgets`, `dep.Newtonsoft`. Kind
  `dependency`, no `parent` (they sit beside the modules at the top of
  the derived scene), and `path` empty: there is no file to open,
  because the package is not part of the mapped tree.
- Nothing derives hierarchy from dots in fact ids — nesting comes from
  the explicit `parent` field — so dotted and scoped package names are
  inert. The TypeScript and C# extractors delimit their internal edge
  encoding with NUL, not spaces, so no package name can break it.
- **Collision rule**: if a corpus element already owns the id (a module
  literally named `dep.foo`), the rollup is skipped rather than
  colliding — under-reporting over a silent clash, as everywhere else.
- Per-language detection and exclusions are in each extractor's section
  below. Each excludes the language's own standard library: it ships
  with the toolchain, appears in nearly every file, and carries no
  architectural signal.
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
blastradius introspect [dir] [component-id] [--check]
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
  pattern as the snapshot gate. CI's validate job runs `--check` for
  both dogfood mappings (Rust is built in; TypeScript uses the runner's
  Node plus the `typescript` dev dependency). Both out-of-process
  extractors additionally carry a byte-exact fixture gate —
  `extractors/dotnet/test.sh` and `extractors/typescript/test.sh` — run
  in the same job. The C# extractor has no dogfood mapping, so its
  fixture gate is its only coverage; the TypeScript one covers cases the
  dogfood corpus lacks (external packages, scoped names, builtins).

Extractor commands (overridable per mapping with `extractor:`, for
monorepos with pinned toolchains):

| language   | default command                                   |
|------------|---------------------------------------------------|
| typescript | `node <dir>/extractors/typescript/extract.mjs`    |
| csharp     | `dotnet <dir>/extractors/dotnet/BlastradiusExtract.dll`, else `dotnet run --project <dir>/extractors/dotnet -c Release` |
| rust       | built into core (`syn`) — no external process     |

The defaults resolve against the Blastradius install dir first, then
the repo, so users don't need the extractors vendored in their repo.

**Installed layouts ship the C# extractor published, not as a project**
(`tools/stage-extractors.mjs`, used by both the MSIX and the portable
archive). `dotnet run` writes `bin/` and `obj/` beside the project, which
an install directory does not allow — MSIX makes it read-only outright —
and publishing also means the machine needs the .NET **runtime** only,
with no first-run NuGet restore. A checkout has no published build and
falls back to the project, which is what `test.sh` and development use.

Every Store build from 0.1.0 to 0.5.0 packaged the two executables and
nothing else, so no installed copy could introspect TypeScript or C# at
all; only Rust worked, being built into core. Found by the first outside
user (docs/roadmap.md). Two guards now: the packer fails if the staged
tree has no extractor in it, and the release smoke test pipes a fixture
through the staged extractor before the archive is published.

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
  but still inside the repo are dropped. Type-only imports collapse into
  the same `imports` edge. `extends`/`implements` edges between
  extracted types via the checker's declared heritage.
- **Dependencies**: a specifier that resolves into `node_modules` rolls
  up under the resolver's own `packageId.name`; one that does not
  resolve at all still rolls up, using the specifier read lexically
  (`pkg/sub` → `pkg`, `@scope/pkg/sub` → `@scope/pkg`). Facts therefore
  do not depend on whether `node_modules` happens to be installed on the
  machine running the extractor. Excluded: `node:`-prefixed builtins.
  Deliberately *not* consulted: `module.builtinModules` — that list
  grows with the Node version, so an unprefixed `import fs from 'fs'`
  rolls up like any other package rather than making facts depend on
  which Node ran.
- Dogfood: `blastradius.ui.canvas` maps `root: ui/js` — diving below
  the Canvas component shows the real `app/layout/svg/…` module graph.
  Determinism proven by double-run byte-compare in the node test suite.

## C# extractor

`extractors/dotnet/` — a small dotnet console project on
`Microsoft.CodeAnalysis.CSharp`, **syntax-level only** in v1
(ADR-0016): no MSBuild, no restore, works on any checkout regardless of
whether the solution builds. Shipped published (see above), so a user
needs the .NET runtime rather than an SDK; opt-in semantic mode is the
one exception, since MSBuild has to load their solution.

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
- **Dependencies**: at syntax level the honest proxy for a package is
  the **namespace root** of a plain `using` that no corpus namespace
  claims (`using Newtonsoft.Json` → `dep.Newtonsoft`); `System` is
  excluded as the BCL. The edge is owned by the namespace the file
  declares types into, not by each type: a using directive is
  file-scoped, so attributing it per type would invent precision the
  syntax does not carry. A file that declares no namespaced type emits
  no dependency edge. Semantic mode can do better — assembly identity
  rather than a namespace-root proxy — which is a recorded follow-up.
- Tested against a fixture corpus (`extractors/dotnet/fixtures/`)
  covering file-scoped namespaces, partials, records, and a
  cross-namespace reference — asserting exact facts bytes.
### Semantic mode (C#)

Opt in per mapping with `mode: semantic` (the extractor also takes a
`--semantic` flag, which is how `test.sh` drives it). Default stays
`syntax`; `mode:` on a language with no semantic pass warns.

- **What it buys**: real symbol resolution instead of name matching —
  same-named types in different projects, global usings, cross-project
  references. Everything the syntax pass cannot know from one file.
- **What it does not change**: pass 1 and `sourceDigest`. The element
  set and the staleness probe are defined by the mapping's file
  collection, never by MSBuild; only *edges* differ between modes.
- **Discovery**: one `.sln` under the source root (sorted first, with a
  note, if several), else every `.csproj`; `bin`/`obj`/dotfile
  directories skipped. Projects must be restored.
- **Fallback contract**: any failure — no MSBuild instance, no solution,
  a project that will not open, no mapped file in a loaded project — is
  reported on stderr and degrades to the syntax pass, exit 0. Semantic
  mode is never worse than syntax mode.
- **Effective mode is recorded** in the facts' `extractor` string:
  `blastradius-extract-cs 0.3.0 (semantic)` versus `(syntax-fallback)`.
  This is what lets `introspect --check` tell a machine that *cannot*
  run the semantic pass apart from facts that are genuinely stale: the
  former reports "NOT VERIFIED" and passes, the latter fails. Repos
  using semantic mode should regenerate facts somewhere with a known
  SDK — CI — rather than from whichever developer machine ran last.
- **SDK resolution**: extractors are spawned from their own directory,
  not the target repo, so a repo pinning an old SDK in `global.json`
  cannot break the extractor's own build. Semantic mode then switches
  to the repo root before registering the MSBuild locator, so the
  target solution still loads under the SDK it pins. Extractors receive
  an absolute `repoRoot` and must not depend on the working directory.
- **Dependency identity**: rollups still come from the using-directive
  scan in both modes. Semantic mode could name packages by assembly
  instead of namespace root — recorded follow-up, not done.

## Rust extractor

Built into core as a module on `syn` (compile-time dependency; no
external process, no toolchain requirement at extraction time — the one
language where "spawn the extractor" is a function call). Syntax-level,
same honesty rules as C#.

- Input: `**/*.rs` under `root`, minus `target/` and the mapping's
  excludes. Files parse with `syn::parse_file`; a file that fails to
  parse is reported as a warning and skipped, never fatal.
- **Elements**: modules from the file tree and inline `mod` blocks
  (id = crate-relative module path, `src/git.rs` → `git`,
  `src/model/loader.rs` → `model.loader`); `struct`/`enum`/`trait`
  declarations become children of their module. `impl` blocks are not
  elements; they contribute edges.
- **Edges**: `use` declarations resolved syntactically against the
  extracted corpus (`crate::`/`self::`/`super::` paths; glob imports
  dropped) produce `imports`; `impl Trait for Type` where both sides
  are in the corpus produces `implements`; path references in
  signatures and bodies matched against the corpus produce
  `references`. Ambiguous or external names are dropped, not guessed.
- **Re-exports**: `pub use` (and restricted forms like `pub(crate) use`,
  visible to the rest of the corpus) build a per-module export table —
  the name a façade re-exports mapped to the type that defines it. The
  table is built by fixpoint, so a name forwarded through a chain of
  façades resolves **transitively**, and `as` renames are honored. Edges
  then point at the *defining* module, never the façade that forwarded
  the name: `use crate::facade::Engine` in module `user`, where `facade`
  re-exports from `engine`, yields `user → engine`. Not followed: glob
  re-exports (`pub use x::*`) and module re-exports (`pub use crate::a;`)
  — dropped, in keeping with the ambiguity rule. A re-export cycle
  resolves nothing and is dropped rather than guessed.
- **Dependencies**: a `use` path that resolves to nothing and is not
  anchored with `crate::`/`self::`/`super::` names an external crate in
  its first segment (`use serde::Serialize` → `dep.serde`). Excluded:
  the sysroot crates `std`, `core`, `alloc`, `proc_macro`, `test`.
  Detection is from `use` declarations only — an inline `git2::Repository`
  path in an expression stays dropped. Two honest limits: a crate renamed
  in `Cargo.toml` (`foo = { package = "bar" }`) shows as `dep.foo`,
  because the extractor reads syntax and the name in the code is what it
  sees; and a bare path to a sibling module excluded by the mapping's
  include globs would be misattributed as a dependency — the `crate::`
  convention avoids this in practice.
- **Honest limit**: `syn` sees surface syntax only — macro-generated
  items (`#[derive]`, proc-macros) are invisible. Good enough for
  module/type structure, which is what L4 draws.
- Dogfood: a mapping on a Core component using `include` globs (e.g.
  `blastradius.core.git-service` ← `crates/blastradius-core/src` with
  `include: [git.rs, resolve.rs]`) — this also exercises the glob
  semantics that the whole-directory TypeScript dogfood doesn't.

## Other languages

The facts schema is the extension point: any tool that emits valid
facts is a Blastradius extractor, which is also the future SCIP/LSIF
import path (ADR-0016 option 3).

## App & MCP behavior

- **Canvas**: an opted-in component dives like a container (same
  grammar); one more dive steps from a module into its types, and
  Escape climbs back out. The header's L4 segment is enabled whenever
  the model has derived graphs and jumps to the nearest introspected
  component (selection first, then current scope, then the first
  graph); with no `source:` mappings it stays disabled. The model
  explorer lists derived elements under their component (modules, then
  types), styled as code; clicking a row jumps the canvas to that
  element's code altitude. L4 uses the existing deterministic layout
  pipeline with no pinning (derived layouts are pure auto-layout);
  derived nodes carry a kind kicker with a derived marker ("Module ·
  derived") and monospace, case-preserving titles; a stale graph badges
  the breadcrumb and dashes the nodes. Dependency rollups read as
  external rather than derived — outline-only nodes, kicker
  "Dependency · external" — and are leaves, so a dive does nothing. The
  inspector shows `path:line` and opens the file via the `open_source`
  command (repo-root-relative, unlike `open_in_editor`'s
  workspace-relative paths); for an element with no path — a dependency
  rollup, or a C# namespace, which owns no single file — it says so
  instead of offering a button that cannot work.
- **MCP**: derived elements appear in `find_elements`, `element`, and
  `blast_radius` (marked `derived: true`) — an agent asking for the
  blast radius of a module sees real code-level fan-in. Write tools
  refuse them as above. A `introspect` MCP tool mirrors the CLI
  command so agents can refresh facts.
- **Exports**: derived elements ride along in snapshots and HTML/PNG/SVG
  export automatically (they're ordinary elements by render time).
  **Recorded gap**: the standalone exported viewer (`ui/js/viewer.js`)
  has no derived handling of its own — its kind labels and node classes
  know only authored kinds, so an exported page cannot browse L4 the way
  the app does. Not a regression from dependency rollups (nothing L4
  rendered there before); worth closing when the viewer next gets
  attention.

## Exit criteria (0.3.0 theme 1)

1. Diving below `blastradius.ui.canvas` in the app shows the real
   `ui/js` module/import graph, from committed facts, with click-through
   to source.
2. Diving below the Rust-mapped Core component shows its real
   module/type graph via the built-in extractor.
3. `blastradius introspect --check` green in CI on both dogfood
   mappings; double-run determinism asserted in tests.
4. C# fixture corpus round-trips through the dotnet extractor with
   byte-exact facts in tests, without any MSBuild/restore step.
5. Derived elements are visibly read-only: `apply_operation` against
   one returns the source-file-pointing error, covered by a test.
6. A component with no `source:` mapping is completely untouched by
   the feature — hand-modeled L4 children remain editable as ordinary
   elements (covered by a test).
