---
description: Build or extend the C4 architecture model, interviewing you first
argument-hint: "[what to focus on, optional]"
---

# Model this repository

Build the Blastradius model for the workspace at `docs`. **Interview before you build.** A model
nobody agreed to is a model nobody maintains, and the questions below are the
ones whose answers change what you would write.

Call `model_format` first if you have not already this session — it is the
schema for this build. Everything you create goes through `apply_operations`.

## 1. Work out which situation you are in

Look before you ask: `workspace_summary`, then the repository itself —
manifests, entry points, directory shape.

- **There is code here.** Do not interview from a blank page; that spends the
  user's time on things you could have read. Survey the repository first — with
  the `blastradius-surveyor` subagent if you have one — and come back with a
  *proposal*: candidate containers with the evidence for each, candidate
  components, external systems, and where source lives. Put that to the user to
  correct. Corrections are cheaper than answers.
- **The repository is empty or nearly so.** There is nothing to read, so the
  model can only come from the user. Interview them properly — section 2, one
  topic at a time.
- **A model already exists.** Say what is in it and ask what they want changed
  or extended. Never silently restructure someone's model.

## 2. Ask, in this order

Ask in small batches and *use* the answers — do not collect everything and then
ignore it. Where a survey already suggests an answer, put it forward for
confirmation rather than asking cold.

1. **Scope.** Which system is this, and what is deliberately outside it? Things
   you do not own are `external`, not systems.
2. **Level of detail.** Containers only (L2), or components too (L3) — and if
   components, for which containers? Most models want components in one or two
   containers and nowhere else. Do not offer to go below components: that is
   what `introspect` is for.
3. **Audience.** Who reads this, and what decision are they making? That settles
   most arguments about what to include.
4. **Documents.** Are there ADRs, specs or design notes to attach to elements?
   Do they want ADRs written for decisions you found in the code? Attaching is
   cheap and makes the model worth opening — but do not invent decisions nobody
   made.
5. **Code-level detail.** Should components carry a `source:` mapping (`rust`,
   `typescript`, `csharp`)? That is what makes L4 and drift detection work. Ask
   which components, and check the paths against what is actually on disk.
6. **Deployment.** Is there a deployment story worth modelling — environments,
   what runs where? If the repository has compose files, infrastructure code or
   CI deploy steps, say what you found and ask whether to model it. If there is
   nothing, say so and move on rather than inventing one.

## 3. Build it

- One `apply_operations` call per coherent chunk, in dependency order: systems,
  then containers, then components, then the relations between them. Not one
  call per element.
- **A relation is a dependency, not a data flow.** Ask which one breaks if the
  other changes, and point from the answer.
- Technology goes in `tech:`, not in names.
- `validate` after each chunk, and fix what it reports before continuing.

## 4. Hand it back

- `validate` once more and report the result honestly.
- If you added `source:` mappings, run `introspect` and say what it derived.
- Summarise what you modelled, what you deliberately left out, and every
  question you had to guess at — that last list is where the user should look
  first.
- Tell them to open the workspace at `docs` in Blastradius to see it.

$ARGUMENTS
