---
description: Review the architecture model against the code and report honestly
---

# Review the model

Read-only. Judge the workspace at `docs` against the repository and say what is wrong with it. Do
not fix anything — the point is an honest assessment the user can act on.

Check each of these, with evidence for every finding:

- **Validity** — `validate`. Errors first, then warnings.
- **Drift** — `blastradius validate --strict-drift`, plus `introspect` where
  components carry `source:` mappings. Undeclared dependencies and relations
  the code does not back are the two directions it runs in.
- **Truthfulness of relations** — sample a few and confirm the direction is the
  dependency and not a data flow. This project got that wrong in its own model
  until drift detection caught it.
- **Level of detail** — containers with an unhelpful number of components, or a
  system modelled at one altitude where the reader needs another. Forty boxes
  on one diagram is a finding.
- **Naming** — technology in names instead of `tech:`; ids that no longer
  describe the thing (ids are immutable — the fix is `name:`).
- **Documents** — elements that clearly warrant a governing doc and have none,
  and docs pointing at elements that no longer exist.
- **Coverage** — parts of the repository the model says nothing about. Say
  whether that looks deliberate or forgotten.

Report as a short list, worst first, each with the file and the reason. End with
the single change that would most improve the model.

$ARGUMENTS
