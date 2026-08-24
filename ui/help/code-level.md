# Code-level detail (L4)

Below components lies the code. Point a component at a source directory and
Blastradius derives its real modules and types — and the imports between them —
from the code itself.

This is **opt-in per component**. Nothing is derived until you ask.

## Opting in

Add a `source:` mapping to a component:

```yaml
components:
  canvas:
    name: Canvas
    source:
      language: typescript   # or csharp, rust
      root: ui/js            # from the repo root
      include: ["**/*.mjs"]  # optional
      exclude: ["**/*.test.mjs"]
```

`root` is relative to the **repository** root, not the workspace folder — code
lives beside the model, not inside it.

## Extracting

```
blastradius introspect .
```

writes one facts file per opted-in component under `model/derived/`. **Commit
those files.** They are what the app reads, so anyone can browse the code graph
without your toolchain installed.

To check them in CI:

```
blastradius introspect . --check
```

which fails when the committed facts no longer match the source — the same
staleness gate that stops the diagram drifting from the code.

## What you get

Modules and the types inside them (classes, interfaces, enums, records), with
`imports`, `references`, `extends`, and `implements` between them. Dive into an
opted-in component to see the graph; dive again to go from a module to its
types. The inspector links straight to the file and line.

**External dependencies** roll up to one node per package — `serde`, `react`,
`Newtonsoft` — so a module that leans on a library looks like it does, rather
than appearing self-contained. Standard libraries are left out: they are in
every file and say nothing about your architecture.

Derived elements are **read-only** in the app. The source is the truth; edit
the code and re-run `introspect`.

## Languages

- **TypeScript / JavaScript** — the TypeScript compiler's own module
  resolution, so `tsconfig` paths and index files resolve the way your build
  resolves them.
- **Rust** — built in, no toolchain needed at extraction time. `pub use`
  re-exports are followed to whatever actually defines the type.
- **C#** — Roslyn syntax analysis by default: no restore, no build, works on
  any checkout. Add `mode: semantic` to load the real solution through MSBuild
  and resolve symbols properly, which catches what name-matching cannot —
  same-named types in different projects, global usings, cross-project
  references. If the solution will not load, it falls back to the syntax pass
  with a warning; it is never worse than the default.

Where a language cannot be certain, it drops the edge rather than guessing.
Under-reporting beats a diagram full of false arrows.

## Hand-modelled components still work

A component with no `source:` mapping is not lesser. Model the components you
want to reason about by hand, and derive the ones where the code is the better
authority.
