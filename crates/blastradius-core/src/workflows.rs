//! Agent workflows: the interview-shaped half of the onboarding.
//!
//! The skill (`format_ref`) is *reference* — it loads when architecture comes
//! up, and reference is the right shape for that. It is the wrong shape for an
//! interview: something that auto-triggers must not start interrogating you,
//! which is why the primer never asked anything.
//!
//! Workflows are therefore invoked deliberately, and every one of these agents
//! has a surface for that — a correction from 2026-08-26, when this module
//! shipped claiming only Claude Code did. Verified against each vendor's docs
//! rather than from memory, because a file written to the wrong path with the
//! wrong extension does nothing at all and says nothing about it:
//!
//! | agent   | reference                                    | workflows                     | subagent                    |
//! |---------|----------------------------------------------|-------------------------------|-----------------------------|
//! | claude  | `.claude/skills/blastradius/SKILL.md`        | `.claude/commands/**/*.md`    | `.claude/agents/*.md`       |
//! | copilot | `.github/instructions/*.instructions.md`     | `.github/prompts/*.prompt.md` | `.github/agents/*.agent.md` |
//! | cursor  | `.cursor/rules/*.mdc`                        | `.agents/skills/*/SKILL.md`   | —                           |
//! | codex   | `.agents/blastradius.md` + AGENTS.md pointer | `.agents/skills/*/SKILL.md`   | —                           |
//!
//! Every reference is a file of our own, so nothing of the project's is
//! rewritten to make room for it — Copilot moved off an append into
//! `copilot-instructions.md` for exactly that reason. Codex has no per-repo
//! instructions file other than `AGENTS.md`, which is the project's, so it
//! gets the one thing that has to auto-load: a delimited five-line pointer at
//! `.agents/blastradius.md`, where the reference itself lives. A repo set up
//! by 0.6.x has the old pasted-in primer and is left exactly as it is.
//!
//! `.agents/skills/` is the shared convention Cursor and Codex both discover,
//! so one set of files serves both; whichever is selected second finds them
//! already present. Cursor and Copilot also read `.claude/` directly, so a
//! repo set up for Claude Code is partly set up for them anyway.

/// `(path under the repository root, contents)`.
pub type File = (String, String);

/// The workflows, in the order a person meets them. One list, so the files
/// written and the hand-off that tells you about them cannot disagree about
/// which workflows exist or what they do.
pub const CATALOGUE: [(&str, &str); 3] = [
    ("model", "build or extend the model, interviewing you first"),
    ("sync", "bring the model back in step with the code since a commit"),
    ("review", "judge the model against the repository, changing nothing"),
];

/// How a person actually invokes one of these workflows, per agent. Derived
/// from the same table `files_for` writes, and pinned by a test, because the
/// onboarding hand-off quotes these strings and a wrong one is worse than
/// none: it sends someone to a command that does not exist.
///
/// `None` for an agent with no workflow surface.
pub fn invocation(agent: &str, workflow: &str) -> Option<String> {
    match agent {
        "claude" => Some(format!("/blastradius:{workflow}")),
        "copilot" => Some(format!("/blastradius-{workflow}")),
        // Cursor and Codex invoke a skill by name rather than with a prefix.
        "cursor" | "codex" => Some(format!("the `blastradius-{workflow}` skill")),
        _ => None,
    }
}

/// The label a person would recognise, for the same hand-off.
pub fn agent_label(agent: &str) -> &'static str {
    match agent {
        "claude" => "Claude Code",
        "copilot" => "GitHub Copilot",
        "cursor" => "Cursor",
        "codex" => "Codex",
        _ => "your agent",
    }
}

/// The workflow files for one agent. Empty for an agent with no surface for
/// them — never a file written somewhere nothing will look.
pub fn files_for(agent: &str, rel: &str) -> Vec<File> {
    // Descriptions here are the frontmatter an agent reads; CATALOGUE carries
    // the shorter phrasing a person reads. Names come from CATALOGUE so the
    // two lists cannot drift apart on *which* workflows exist.
    let w = [
        (CATALOGUE[0].0, "Build or extend the C4 architecture model, interviewing you first", "[what to focus on, optional]", model_body(rel)),
        (CATALOGUE[1].0, "Bring the architecture model back in step with the code", "[git ref to compare against, default: the merge-base]", sync_body(rel)),
        (CATALOGUE[2].0, "Review the architecture model against the code and report honestly", "", review_body(rel)),
    ];
    match agent {
        "claude" => {
            let mut out: Vec<File> = w
                .iter()
                .map(|(name, desc, hint, body)| {
                    let hint = if hint.is_empty() {
                        String::new()
                    } else {
                        format!("argument-hint: \"{hint}\"\n")
                    };
                    (
                        format!(".claude/commands/blastradius/{name}.md"),
                        // Claude commands substitute $ARGUMENTS.
                        format!("---\ndescription: {desc}\n{hint}---\n\n{body}\n$ARGUMENTS\n"),
                    )
                })
                .collect();
            out.push((".claude/agents/blastradius-surveyor.md".into(), surveyor("claude")));
            out
        }
        "copilot" => {
            let mut out: Vec<File> = w
                .iter()
                .map(|(name, desc, hint, body)| {
                    let hint = if hint.is_empty() {
                        String::new()
                    } else {
                        format!("argument-hint: \"{hint}\"\n")
                    };
                    (
                        format!(".github/prompts/blastradius-{name}.prompt.md"),
                        // No $ARGUMENTS: Copilot prompt files use ${input:…}
                        // and would render the literal text instead.
                        format!("---\ndescription: {desc}\n{hint}agent: agent\n---\n\n{body}"),
                    )
                })
                .collect();
            out.push((
                ".github/agents/blastradius-surveyor.agent.md".into(),
                surveyor("copilot"),
            ));
            out
        }
        // Cursor and Codex both discover `.agents/skills/`, and both invoke a
        // skill by name, so the workflows are skills there. Same files for
        // either; the second agent selected finds them already written.
        "cursor" | "codex" => w
            .iter()
            .zip(["model", "update", "review"])
            .map(|((name, desc, _, body), verb)| {
                (
                    format!(".agents/skills/blastradius-{name}/SKILL.md"),
                    format!(
                        "---
name: blastradius-{name}
description: {desc}. Use when asked to {verb} this repository's architecture model.
---

{body}"
                    ),
                )
            })
            .collect(),
        _ => Vec::new(),
    }
}

fn ws(rel: &str) -> String {
    if rel == "." {
        "the workspace in this folder".to_string()
    } else {
        format!("the workspace at `{rel}`")
    }
}

fn model_body(rel: &str) -> String {
    format!(
        r#"# Model this repository

Build the Blastradius model for {loc}. **Interview before you build.** A model
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
- Tell them to open {loc} in Blastradius to see it.
"#,
        loc = ws(rel)
    )
}

fn sync_body(rel: &str) -> String {
    format!(
        r#"# Sync the model with the code

The model in {loc} is only worth having if it matches reality. Find where it
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
"#,
        loc = ws(rel)
    )
}

fn review_body(rel: &str) -> String {
    format!(
        r#"# Review the model

Read-only. Judge {loc} against the repository and say what is wrong with it. Do
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
"#,
        loc = ws(rel)
    )
}

/// The read-only survey agent. Its own context window, because reading a whole
/// repository to form an opinion should not crowd out the conversation that
/// follows it. Claude and Copilot spell the frontmatter differently.
fn surveyor(agent: &str) -> String {
    let front = match agent {
        "copilot" => "---\nname: blastradius-surveyor\ndescription: Reads a repository and proposes a C4 structure — candidate containers and components with the evidence for each, external systems, and where source lives.\ntools: ['search', 'codebase']\n---\n\n",
        _ => "---\nname: blastradius-surveyor\ndescription: Reads a repository and proposes a C4 structure — candidate containers and components with the evidence for each, external systems, and where source lives. Use before modelling an existing codebase, so the user corrects a proposal instead of answering from a blank page.\ntools: Read, Grep, Glob\n---\n\n",
    };
    format!("{front}{SURVEYOR_BODY}")
}

const SURVEYOR_BODY: &str = r#"You survey a repository and propose how it should be modelled in C4. You edit
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

**External systems and people** — what this depends on but does not own, and who
uses it.

**Relations** — the dependencies you can evidence, as `from -> to`, each with
what makes it true: an import, a client being constructed, a connection string.
Direction is the dependency — point from the thing that would break.

**Source mappings** — for each component you would model, the repo-relative path
and language (`rust`, `typescript`, `csharp`) for a `source:` mapping, so
code-level detail and drift detection can work. Only where the language is one
of those three.

**Deployment** — environments and what runs where, if the repository shows it:
compose files, infrastructure code, CI deploy steps. If it shows none, say so
rather than inventing one.

**What you are unsure about** — the questions a human has to answer. Be
specific, and do not pad this with things you could have checked yourself. It is
the most useful section you produce.
"#;
