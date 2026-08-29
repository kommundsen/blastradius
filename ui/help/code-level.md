# Code-level detail (L4)

Below components lies the code. Point a component at a source directory and
Blastradius derives its real modules and types — and the imports between them —
from the code itself.

This is **opt-in per component**. Nothing is derived until you ask.

## Opting in

Select a component and use **Code level** in the inspector — or right-click it
and pick *Point at its code…*. Choose the language and the folder its code
lives in, then **Run introspection**. That writes the same mapping and the same
facts as the command line below; nothing about it is app-only.

The mapping it writes is an ordinary `source:` block, and hand-writing one is
still a first-class route:

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

**Run introspection** in the inspector does this for the selected component.
For every component at once, or in CI:

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

## Drift: the model checked against the code

Once two components are introspected, Blastradius can compare what their code
actually does against what the model declares, and show you the disagreements
where you are looking:

- **A dashed ghost line** is a dependency the code has and the model does not
  declare. Click it: the inspector names the file that proves it, and
  **Declare this relation** turns it into a real one.
- **A marked line** is the opposite — a relation you declared with no code
  reference behind it. Most often the dependency runs the other way, so the
  inspector offers **Reverse it**, which is one action and one undo.

Neither is shown while you are diffing or time-travelling: drift is a fact
about the code as it is now.

In CI, `blastradius validate --strict-drift` turns the same findings into a
failing build. They are warnings by default, so adopting this on an existing
repository does not hand you a red build on day one.

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
