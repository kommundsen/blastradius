//! Agent workflows: the interview-shaped half of the onboarding.
//!
//! The skill (`format_ref`) is *reference* — it auto-triggers when
//! architecture comes up, and reference is the right shape for that. It is the
//! wrong shape for an interview: a skill that started interrogating you
//! because it fired on its own would be obnoxious, which is why the primer
//! never asked anything.
//!
//! Workflows are therefore **commands** — user-initiated, so they may ask —
//! plus one **subagent** for the read-the-whole-repository pass, where a
//! separate context window genuinely pays. Claude Code has all three surfaces;
//! the other agents get the primer alone, which is why the reference stays one
//! self-contained document rather than being split across them.

/// `(path under the repository root, contents)`.
pub type File = (&'static str, String);

/// Slash commands for Claude Code. `commands/blastradius/model.md` is invoked
/// as `/blastradius:model`.
pub fn claude_commands(rel: &str) -> Vec<File> {
    vec![
        (".claude/commands/blastradius/model.md", model_command(rel)),
        (".claude/commands/blastradius/sync.md", sync_command(rel)),
        (".claude/commands/blastradius/review.md", review_command(rel)),
    ]
}

/// The read-only survey agent. Its own context window, because reading a whole
/// repository to form an opinion is exactly the job that should not crowd out
/// the conversation that follows it.
pub fn claude_agent() -> File {
    (".claude/agents/blastradius-surveyor.md", SURVEYOR.to_string())
}

fn ws(rel: &str) -> String {
    if rel == "." {
        "the workspace in this folder".to_string()
    } else {
        format!("the workspace at `{rel}`")
    }
}

fn model_command(rel: &str) -> String {
    format!(
        "---\n\
         description: Build or extend the C4 architecture model, interviewing the user first\n\
         argument-hint: \"[what to focus on, optional]\"\n\
         ---\n\
         \n\
         # Model this repository\n\
         \n\
         Build the Blastradius model for {loc}. **Interview before you build.** A\n\
         model nobody agreed to is a model nobody maintains, and the questions below\n\
         are the ones whose answers change what you would write.\n\
         \n\
         Call `model_format` first if you have not already this session — it is the\n\
         schema for this build. Everything you create goes through `apply_operations`.\n\
         \n\
         ## 1. Work out which situation you are in\n\
         \n\
         Look before you ask: `workspace_summary`, then the repository itself —\n\
         manifests, entry points, directory shape.\n\
         \n\
         - **There is code here.** Do not interview from a blank page; that spends\n\
         \x20 the user's time on things you could have read. Launch the\n\
         \x20 `blastradius-surveyor` subagent to survey the repository and come back\n\
         \x20 with a *proposal*: candidate containers with the evidence for each,\n\
         \x20 candidate components, external systems, and where source lives. Put that\n\
         \x20 proposal to the user to correct. Corrections are cheaper than answers.\n\
         - **The repository is empty or nearly so.** There is nothing to read, so the\n\
         \x20 model can only come from the user. Grill them — section 2, properly, one\n\
         \x20 topic at a time.\n\
         - **A model already exists.** Say what is in it and ask what they want\n\
         \x20 changed or extended. Never silently restructure someone's model.\n\
         \n\
         ## 2. Ask, in this order\n\
         \n\
         Ask in small batches and *use* the answers — do not collect everything and\n\
         then ignore it. Where the survey already suggests an answer, put it forward\n\
         for confirmation rather than asking cold.\n\
         \n\
         1. **Scope.** Which system is this, and what is deliberately outside it?\n\
         \x20  Things you do not own are `external`, not systems.\n\
         2. **Level of detail.** Containers only (L2), or components too (L3) — and\n\
         \x20  if components, for which containers? Most models want components in one\n\
         \x20  or two containers and nowhere else. Do not offer to go below\n\
         \x20  components: that is what `introspect` is for.\n\
         3. **Audience.** Who reads this, and what decision are they making? That\n\
         \x20  settles most arguments about what to include.\n\
         4. **Documents.** Are there ADRs, specs or design notes to attach to\n\
         \x20  elements? Do they want ADRs written for decisions you found in the\n\
         \x20  code? Attaching is cheap and makes the model worth opening — but do\n\
         \x20  not invent decisions nobody made.\n\
         5. **Code-level detail.** Should components carry a `source:` mapping\n\
         \x20  (`rust`, `typescript`, `csharp`)? That is what makes L4 and drift\n\
         \x20  detection work. Ask which components, and check the paths against what\n\
         \x20  is actually on disk.\n\
         6. **Deployment.** Is there a deployment story worth modelling —\n\
         \x20  environments, what runs where? If the repository has compose files,\n\
         \x20  infrastructure code or CI deploy steps, say what you found and ask\n\
         \x20  whether to model it. If there is nothing, say so and move on rather\n\
         \x20  than inventing one.\n\
         \n\
         ## 3. Build it\n\
         \n\
         - One `apply_operations` call per coherent chunk, in dependency order:\n\
         \x20 systems, then containers, then components, then the relations between\n\
         \x20 them. Not one call per element.\n\
         - **A relation is a dependency, not a data flow.** Ask which one breaks if\n\
         \x20 the other changes, and point from the answer.\n\
         - Technology goes in `tech:`, not in names.\n\
         - `validate` after each chunk, and fix what it reports before continuing.\n\
         \n\
         ## 4. Hand it back\n\
         \n\
         - `validate` once more and report the result honestly.\n\
         - If you added `source:` mappings, run `introspect` and say what it derived.\n\
         - Summarise what you modelled, what you deliberately left out, and every\n\
         \x20 question you had to guess at — that last list is where the user should\n\
         \x20 look first.\n\
         - Tell them to open {loc} in Blastradius to see it.\n\
         \n\
         $ARGUMENTS\n",
        loc = ws(rel)
    )
}

fn sync_command(rel: &str) -> String {
    format!(
        "---\n\
         description: Bring the architecture model back in step with the code\n\
         argument-hint: \"[git ref to compare against, default: the merge-base]\"\n\
         ---\n\
         \n\
         # Sync the model with the code\n\
         \n\
         The model in {loc} is only worth having if it matches reality. Find where it\n\
         has drifted, and fix it — with the user's agreement on anything structural.\n\
         \n\
         1. **See what changed.** `git diff` against the ref in `$ARGUMENTS`, or the\n\
         \x20  merge-base with the default branch. `model_diff` shows what has already\n\
         \x20  changed in the model itself.\n\
         2. **Refresh what is derived.** If any component has a `source:` mapping,\n\
         \x20  run `introspect` — that alone resolves the drift the model never has to\n\
         \x20  be told about.\n\
         3. **Look for structural drift**, treating each as a question rather than a\n\
         \x20  conclusion:\n\
         \x20  - a new deployable or service with no container;\n\
         \x20  - a container or component whose code is gone;\n\
         \x20  - a dependency in the code with no relation in the model, or a relation\n\
         \x20    the code no longer justifies;\n\
         \x20  - technology that changed under a `tech:` field.\n\
         4. **Propose, then apply.** Show the list. Get agreement on anything that\n\
         \x20  adds, removes or re-points an element, then apply with\n\
         \x20  `apply_operations`. Deleting deserves the most care: run `blast_radius`\n\
         \x20  first and show what goes with it.\n\
         5. **Check the documents.** A doc whose `elements:` names something that no\n\
         \x20  longer exists is a model error, not a wiki problem.\n\
         6. `validate`, and report the result plainly.\n",
        loc = ws(rel)
    )
}

fn review_command(rel: &str) -> String {
    format!(
        "---\n\
         description: Review the architecture model against the code and report honestly\n\
         ---\n\
         \n\
         # Review the model\n\
         \n\
         Read-only. Judge {loc} against the repository and say what is wrong with it.\n\
         Do not fix anything — the point is an honest assessment the user can act on.\n\
         \n\
         Check each of these, with evidence for every finding:\n\
         \n\
         - **Validity** — `validate`. Errors first, then warnings.\n\
         - **Drift** — `blastradius validate --strict-drift`, plus `introspect` where\n\
         \x20 components carry `source:` mappings. Undeclared dependencies and\n\
         \x20 relations the code does not back are the two directions it runs in.\n\
         - **Truthfulness of relations** — sample a few and confirm the direction is\n\
         \x20 the dependency and not a data flow. This project got that wrong in its\n\
         \x20 own model until drift detection caught it.\n\
         - **Level of detail** — containers with an unhelpful number of components,\n\
         \x20 or a system modelled at one altitude where the reader needs another.\n\
         \x20 Forty boxes on one diagram is a finding.\n\
         - **Naming** — technology in names instead of `tech:`; ids that no longer\n\
         \x20 describe the thing (ids are immutable — the fix is `name:`).\n\
         - **Documents** — elements that clearly warrant a governing doc and have\n\
         \x20 none, and docs pointing at elements that no longer exist.\n\
         - **Coverage** — parts of the repository the model says nothing about. Say\n\
         \x20 whether that looks deliberate or forgotten.\n\
         \n\
         Report as a short list, worst first, each with the file and the reason. End\n\
         with the single change that would most improve the model.\n",
        loc = ws(rel)
    )
}

/// Read-only, and deliberately narrow: it proposes, someone else decides.
const SURVEYOR: &str = r#"---
name: blastradius-surveyor
description: Reads a repository and proposes a C4 structure — candidate containers and components with the evidence for each, external systems, and where source lives. Use before modelling an existing codebase, so the user corrects a proposal instead of answering from a blank page.
tools: Read, Grep, Glob
---

You survey a repository and propose how it should be modelled in C4. You edit
nothing and you do not write the model — you hand back a proposal that someone
else will put to the user.

Read widely before concluding: build manifests, workspace and package files,
entry points, service definitions, deployment manifests, CI configuration, and
the README. A directory name is a hint, not evidence.

Return exactly these sections and nothing else.

**System** — usually one. What it is called, and what it does in a line.

**Containers** — the separately deployable or runnable things: applications,
services, databases, scheduled jobs. For each, give a name, the technology, one
line of responsibility, and **the evidence** — the file that made you say so. A
container you cannot point at a file for is a guess, and should be labelled one.

**Components** — only for containers whose internal structure is worth a
reader's time, and only down to major parts. Never below that: code-level
detail is derived by `introspect`, not modelled by hand.

**External systems and people** — what this depends on but does not own, and
who uses it.

**Relations** — the dependencies you can evidence, as `from -> to`, each with
what makes it true: an import, a client being constructed, a connection string.
Direction is the dependency — point from the thing that would break.

**Source mappings** — for each component you would model, the repo-relative
path and language (`rust`, `typescript`, `csharp`) for a `source:` mapping, so
code-level detail and drift detection can work. Only where the language is one
of those three.

**Deployment** — environments and what runs where, if the repository shows it:
compose files, infrastructure code, CI deploy steps. If it shows none, say so
rather than inventing one.

**What you are unsure about** — the questions a human has to answer. Be
specific, and do not pad this with things you could have checked yourself. It
is the most useful section you produce.
"#;
