---
doc: adr-0016
type: adr
status: accepted
elements: [blastradius.core.model-service, blastradius.cli, blastradius.ui.canvas]
---

# ADR-0016: L4 extraction — compiler-API extractors emitting committed facts

## Status
Accepted — 2026-08-23

## Context

0.3.0 theme 1 derives L4 (code-level) elements beneath components that
opt in with a `source:` mapping, with C#/.NET and JavaScript/TypeScript
as the priority languages (the user's stack), plus Rust so this repo's
own crates dogfood the feature. The roadmap
left the extraction mechanism open: vendored tree-sitter grammars vs
shallow heuristic parsing. A third family was raised during planning:
the language servers (tsserver, the C# Dev Kit language service,
rust-analyzer) already compute exactly the information L4 needs.

The insight that settles it: the valuable part of a language server is
its engine, and for our priority languages both engines are available as
embeddable libraries under permissive licenses — the TypeScript compiler
API (the brain inside tsserver, MIT) and Roslyn
(`Microsoft.CodeAnalysis`, the analysis platform beneath C# Dev Kit,
MIT). The LSP *protocol* wrapper around them is the wrong surface for
batch extraction, and in C#'s case the Dev Kit language server binary is
proprietary and licensed only for use with VS Code — the library
underneath is the open part.

## Options considered

1. **LSP protocol clients** (spawn tsserver / a C# LS, speak JSON-RPC).
   Rejected: the protocol is shaped around interactive editing sessions
   (didOpen/didChange, diagnostics push, positional queries one symbol
   at a time). Batch-walking a codebase through it is slow, stateful,
   and fragile; C# Dev Kit's server is additionally license-restricted.
2. **Vendored tree-sitter grammars in core.** Rejected: syntax only —
   no module resolution (tsconfig `paths`, index files, `node_modules`
   boundaries) and no namespace merging (C# partial classes, file-scoped
   namespaces); adds C toolchain + grammar-version drift to every core
   build; every language costs us bespoke query logic that the compiler
   APIs give away.
3. **SCIP/LSIF indexes** (consume scip-typescript / scip-dotnet output).
   Deferred, not rejected: the occurrence-oriented shape needs heavy
   post-processing into an element graph, and indexer maturity is
   uneven. A future `blastradius import --scip` remains compatible with
   the decision below because ingestion is schema-driven.
4. **Per-language extractors on the native compiler APIs** — chosen.

## Decision

- **Thin extractors, one per language**, each written against its
  native compiler platform. TypeScript and C# run out-of-process in
  their own runtimes: `extractors/typescript/` (Node, TypeScript
  compiler API) and `extractors/dotnet/` (dotnet tool, Roslyn
  **syntax-level** — no MSBuildWorkspace in v1; semantic mode is a
  recorded follow-up). **Rust is the exception that proves the schema**:
  `syn` is an ordinary Rust library, so the Rust extractor is built
  into core itself (a compile-time dependency, no runtime toolchain) —
  it speaks the same facts schema internally, chosen over rustdoc JSON
  (nightly-only) and rust-analyzer (the rejected LSP shape).
- **One common facts schema** (JSON, versioned) is the only contract
  between extractors and core. Core spawns the extractor, validates the
  facts, and derives read-only elements; **core itself stays free of
  language toolchains** — no Node, no .NET SDK in any cargo build.
- **Facts are committed** to `docs/model/derived/` like the view
  snapshots: the app, CLI, exports, MCP, and the PR diff bot all read
  L4 without any toolchain installed, PRs show code-structure drift as
  reviewable diffs, and CI regenerates-and-compares as a staleness gate
  (the established snapshot pattern). Extractors must therefore be
  byte-deterministic: sorted output, forward-slash repo-relative paths,
  LF, no timestamps or machine paths.
- **Granularity: modules + types.** TS/JS: modules (files) and their
  import edges. C#: namespaces and type declarations with name-based
  reference edges. Member-level (methods/functions) is out of scope for
  the first release.
- **Derived elements are read-only** (never written into workspace
  YAML) and carry ids computed from stable source facts (relative path;
  fully-qualified type name) per ADR-0003's identity rules, namespaced
  under the owning component's id.

## Consequences

- Toolchains are needed only by whoever runs extraction; everyone else
  consumes committed facts. The dogfood workspace extracts `ui/js`
  (Node, already in CI) **and our own Rust crates** (built-in, always
  available), so two of three extractors run against real production
  code in the staleness gate; the C# path is exercised by a fixture
  corpus.
- The conformance suite's element/doc pins are unaffected: derived
  elements live outside the workspace files and are counted separately.
- Facts edges that cross component boundaries are recorded but not yet
  judged — comparing them against declared L3 relations (architecture
  drift detection) is the natural follow-up this design enables.
- Details, schema, and per-language behavior: `spec/l4-introspection.md`.
