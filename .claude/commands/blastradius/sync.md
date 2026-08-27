---
description: Bring the architecture model back in step with the code
argument-hint: "[git ref to compare against, default: the merge-base]"
---

# Sync the model with the code

The model in the workspace at `docs` is only worth having if it matches reality. Find where it
has drifted, and fix it — with the user's agreement on anything structural.

1. **See what changed.** `git diff` against the ref the user gave, or the
   merge-base with the default branch. `model_diff` shows what has already
   changed in the model itself.
2. **Refresh what is derived.** If any component has a `source:` mapping, run
   `introspect` — that alone resolves the drift the model never has to be told
   about.
3. **Look for structural drift**, treating each as a question rather than a
   conclusion:
   - a new deployable or service with no container;
   - a container or component whose code is gone;
   - a dependency in the code with no relation in the model, or a relation the
     code no longer justifies;
   - technology that changed under a `tech:` field.
4. **Propose, then apply.** Show the list. Get agreement on anything that adds,
   removes or re-points an element, then apply with `apply_operations`.
   Deleting deserves the most care: run `blast_radius` first and show what goes
   with it.
5. **Check the documents.** A doc whose `elements:` names something that no
   longer exists is a model error, not a wiki problem.
6. `validate`, and report the result plainly.

$ARGUMENTS
