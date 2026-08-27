//! `blastradius init` onboarding extras: git init, per-agent MCP registration,
//! and per-agent skills/instructions. Everything here merges — an existing
//! file is amended only when our entry is absent, and never clobbered when it
//! cannot be parsed. The interactive prompts live in main.rs; this module is
//! the testable part.

use serde_json::{json, Value};
use std::path::{Path, PathBuf};

pub const AGENTS: [&str; 4] = ["claude", "copilot", "cursor", "codex"];

#[derive(Default)]
pub struct SetupOptions {
    pub git_init: bool,
    /// Agent names (from AGENTS) to write MCP config for.
    pub mcp: Vec<String>,
    /// Agent names to write skills/instructions for.
    pub skills: Vec<String>,
    /// How an agent should launch the server. `None` writes the bare name
    /// `blastradius`, which is right for a Store install (its execution alias
    /// puts the CLI on PATH) and for a checkout. The desktop app passes the
    /// absolute path of the CLI sitting beside it: a portable install is on no
    /// PATH at all, and a server that cannot start looks exactly like a server
    /// that was never registered.
    pub command: Option<String>,
}

/// Walk up from `dir` looking for a `.git` entry (dir or worktree file).
pub fn git_root(dir: &Path) -> Option<PathBuf> {
    let mut cur = Some(dir.to_path_buf());
    while let Some(d) = cur {
        if d.join(".git").exists() {
            return Some(d);
        }
        cur = d.parent().map(Path::to_path_buf);
    }
    None
}

/// The workspace folder as an agent will refer to it: relative to the git
/// root it runs from, or `.` when the workspace *is* that root.
pub fn workspace_rel(workspace: &Path) -> String {
    let root = git_root(workspace).unwrap_or_else(|| workspace.to_path_buf());
    workspace
        .strip_prefix(&root)
        .ok()
        .filter(|r| !r.as_os_str().is_empty())
        .map(|r| r.to_string_lossy().replace('\\', "/"))
        .unwrap_or_else(|| ".".to_string())
}

/// What to paste into a coding agent to turn a scaffolded workspace into a
/// real model. Shown by the app after it sets the agents up, because
/// "initialised successfully" is not an answer to "now what?".
pub fn sample_prompt(rel: &str) -> String {
    let loc = if rel == "." { "this folder".to_string() } else { format!("`{rel}`") };
    format!(
        "Read this repository and model its architecture in the Blastradius \
         workspace at {loc}. Use the blastradius MCP tools — call model_format \
         first for the schema, then apply_operations to create the systems, \
         containers and components and the relations between them. Model what \
         a reader needs to reason about, not everything that exists; stop at \
         components. Run validate when you are done."
    )
}

/// Apply the selected extras. `workspace` is the folder holding
/// the manifest; agent config lands at the git root (agents run there),
/// falling back to the workspace folder itself. Returns a human-readable
/// action log; soft failures are logged, not fatal.
pub fn setup(workspace: &Path, opts: &SetupOptions) -> Vec<String> {
    let mut log = Vec::new();

    if opts.git_init {
        // the user's own git tooling, per ADR-0007 — libgit2 stays read-only
        match std::process::Command::new("git").arg("init").current_dir(workspace).output() {
            Ok(out) if out.status.success() => log.push("ran `git init`".to_string()),
            Ok(out) => log.push(format!(
                "`git init` failed: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            )),
            Err(e) => log.push(format!("`git init` unavailable: {e}")),
        }
    }

    let root = git_root(workspace).unwrap_or_else(|| workspace.to_path_buf());
    let rel = workspace_rel(workspace);

    let command = opts.command.as_deref().unwrap_or("blastradius");
    for agent in &opts.mcp {
        match write_mcp(agent, &root, &rel, command) {
            Ok(msg) => log.push(msg),
            Err(e) => log.push(format!("{agent}: {e}")),
        }
    }
    for agent in &opts.skills {
        match write_skill(agent, &root, &rel) {
            Ok(msg) => log.push(msg),
            Err(e) => log.push(format!("{agent}: {e}")),
        }
    }
    log
}

fn server_args(rel: &str) -> Vec<&str> {
    vec!["mcp", rel]
}

/// Merge our server into a JSON config under `key` ("mcpServers"/"servers").
fn merge_json_config(
    path: &Path,
    key: &str,
    entry: Value,
    label: &str,
) -> Result<String, String> {
    let mut doc: Value = if path.is_file() {
        let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        serde_json::from_str(&text)
            .map_err(|_| format!("{label} exists but is not valid JSON — left untouched"))?
    } else {
        json!({})
    };
    let servers = doc
        .as_object_mut()
        .ok_or_else(|| format!("{label} is not a JSON object — left untouched"))?
        .entry(key)
        .or_insert_with(|| json!({}));
    let servers = servers
        .as_object_mut()
        .ok_or_else(|| format!("{label}: {key} is not an object — left untouched"))?;
    if servers.contains_key("blastradius") {
        return Ok(format!("{label}: blastradius already registered"));
    }
    servers.insert("blastradius".to_string(), entry);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let mut text = serde_json::to_string_pretty(&doc).map_err(|e| e.to_string())?;
    text.push('\n');
    std::fs::write(path, text).map_err(|e| e.to_string())?;
    Ok(format!("wrote {label}"))
}

fn write_mcp(agent: &str, root: &Path, rel: &str, command: &str) -> Result<String, String> {
    let args: Vec<Value> = server_args(rel).into_iter().map(Value::from).collect();
    match agent {
        "claude" => merge_json_config(
            &root.join(".mcp.json"),
            "mcpServers",
            json!({"command": command, "args": args}),
            ".mcp.json (Claude Code)",
        ),
        "cursor" => merge_json_config(
            &root.join(".cursor/mcp.json"),
            "mcpServers",
            json!({"command": command, "args": args}),
            ".cursor/mcp.json (Cursor)",
        ),
        "copilot" => merge_json_config(
            &root.join(".vscode/mcp.json"),
            "servers",
            json!({"type": "stdio", "command": command, "args": args}),
            ".vscode/mcp.json (VS Code / Copilot)",
        ),
        "codex" => {
            let path = root.join(".codex/config.toml");
            let existing = if path.is_file() {
                std::fs::read_to_string(&path).map_err(|e| e.to_string())?
            } else {
                String::new()
            };
            if existing.contains("[mcp_servers.blastradius]") {
                return Ok(".codex/config.toml (Codex): blastradius already registered".into());
            }
            let mut text = existing;
            if !text.is_empty() && !text.ends_with('\n') {
                text.push('\n');
            }
            text.push_str(&format!(
                "\n# Blastradius model server — Codex loads project config only for\n\
                 # trusted projects (`codex --trust` or trust_level in ~/.codex/config.toml).\n\
                 [mcp_servers.blastradius]\n\
                 command = \"{cmd}\"\n\
                 args = [\"mcp\", \"{rel}\"]\n",
                // TOML basic strings take C-style escapes, and the app passes
                // a Windows path.
                cmd = command.replace('\\', "\\\\"),
            ));
            std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
            std::fs::write(&path, text).map_err(|e| e.to_string())?;
            Ok("wrote .codex/config.toml (Codex)".into())
        }
        other => Err(format!("unknown agent {other:?} — expected one of {AGENTS:?}")),
    }
}

/// The shared primer every agent gets, in its ecosystem's native format.
///
/// Rewritten after the first outside use (docs/roadmap.md): the old text said
/// to "prefer" `apply_operation` and never said what valid YAML looks like, so
/// an agent read the model through the tools, then hand-wrote files inferred
/// from a sample and looped on validation errors. Hand-editing is allowed —
/// the files are the source of truth — but nothing had told it where the
/// schema lives, and there was no way to make many edits at once.
fn primer(rel: &str) -> String {
    let loc = if rel == "." { "this folder".to_string() } else { format!("`{rel}/`") };
    format!(
        "This repository contains a Blastradius workspace in {loc} — a C4\n\
         architecture model as plain YAML (blastradius.yaml + model/ + views/),\n\
         versioned like source code.\n\
         \n\
         ## Reading the model\n\
         \n\
         Use the `blastradius` MCP tools. Start with `workspace_summary`; call\n\
         `blast_radius` with an element id before changing or deleting anything\n\
         it models; `doc` returns the ADRs and specs governing an element.\n\
         \n\
         ## Editing the model\n\
         \n\
         **Edit through `apply_operation`, or `apply_operations` for several\n\
         changes at once.** Not a style preference: those tools splice the YAML\n\
         in place so comments, key order and formatting survive, they validate\n\
         before writing and refuse anything that would break the workspace, and\n\
         they are undoable. Hand-written YAML has none of that, and the\n\
         operation shapes are published in each tool's input schema.\n\
         \n\
         Modelling a repository from scratch is a single `apply_operations`\n\
         call with the whole list — create parents before children, elements\n\
         before the relations between them. It applies as one transaction: if\n\
         any operation is refused the rest roll back, and one `undo` reverts\n\
         the lot.\n\
         \n\
         **Never guess the schema from an existing file.** Call `model_format`\n\
         (or run `blastradius format`) for the authoritative reference: every\n\
         element kind, what may nest in what, relations, views, docs\n\
         frontmatter, deployment, and a complete example. If you do edit YAML\n\
         by hand, read that first, never re-serialize or re-order a file, and\n\
         run `validate` immediately — before moving on to anything else.\n\
         \n\
         {practice}\n\
         ## Keeping it honest\n\
         \n\
         - Element ids (the YAML keys) are immutable — renaming means changing\n\
         \x20 the `name:` field only.\n\
         - Markdown docs with a `doc:` frontmatter block are part of the model;\n\
         \x20 their `elements:` links must point at real element ids.\n\
         - When you add, remove, or rewire a real component, mirror it in the\n\
         \x20 model in the same change.\n\
         - Components with a `source:` mapping have derived L4 code elements\n\
         \x20 (modules/types extracted from source). They answer in\n\
         \x20 `find_elements`, `element`, and `blast_radius` (code-level\n\
         \x20 fan-in), marked `derived: true` — read-only; edit the source\n\
         \x20 instead, then run the `introspect` tool to refresh the committed\n\
         \x20 facts. `blast_radius` on a derived id shows real code dependents.\n\
         - `git_status` and `git_conflicts` read repository state. A merge\n\
         \x20 conflict in the model resolves per element: read `git_conflicts`\n\
         \x20 (each conflicted element carries ours/theirs field values), then\n\
         \x20 call `resolve_conflicts` with {{elements: {{\"<id>\": \"ours\"|\"theirs\"}}}}\n\
         \x20 — choices splice onto the chosen side (comments survive), files\n\
         \x20 are validated and staged via the user's own git, and the commit\n\
         \x20 stays the user's. Anything undecided keeps ours.\n\
         \n\
         If the `blastradius` MCP tools are not available, say so rather than\n\
         working around it: `blastradius validate {rel}` and `blastradius\n\
         format` still work from the command line.\n",
        practice = crate::format_ref::PRACTICE.replace("# Modelling rules", "## Modelling rules"),
    )
}

/// Create a file and the directories above it. Callers check existence first:
/// nothing here ever overwrites.
fn write_new(path: &Path, text: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(path, text).map_err(|e| e.to_string())
}


fn write_skill(agent: &str, root: &Path, rel: &str) -> Result<String, String> {
    match agent {
        // Claude Code gets three surfaces, because the jobs are different
        // shapes (see crates/blastradius-core/src/workflows.rs): a *skill* is
        // reference and fires on its own, *commands* are user-initiated and so
        // may interview, and a *subagent* gets its own context window for
        // reading a whole repository. Every other agent gets the primer alone,
        // which is why the primer stays self-contained.
        "claude" => {
            let mut wrote: Vec<String> = Vec::new();
            let mut present = 0usize;

            let skill = root.join(".claude/skills/blastradius/SKILL.md");
            if skill.exists() {
                present += 1;
            } else {
                let text = format!(
                    "---\nname: blastradius\ndescription: Query and edit this repo's Blastradius C4 architecture model (YAML workspace). Use when working with the architecture model, ADRs, or when a change affects modelled components.\n---\n\n# Working with the Blastradius model\n\nWorkflows live in slash commands rather than in here, because a skill fires\non its own and an interview should not: `/blastradius:model` builds a model\nby asking first, `/blastradius:sync` brings it back in step with the code,\nand `/blastradius:review` judges it. This file is the reference they lean on.\n\n{}",
                    primer(rel)
                );
                write_new(&skill, &text)?;
                wrote.push("skill".to_string());
            }

            let mut extras = crate::workflows::claude_commands(rel);
            extras.push(crate::workflows::claude_agent());
            let (mut cmds, mut agents) = (0usize, 0usize);
            for (rel_path, text) in &extras {
                let path = root.join(rel_path);
                if path.exists() {
                    present += 1;
                    continue;
                }
                write_new(&path, text)?;
                if rel_path.contains("/agents/") {
                    agents += 1;
                } else {
                    cmds += 1;
                }
            }
            if cmds > 0 {
                wrote.push(format!("{cmds} commands"));
            }
            if agents > 0 {
                wrote.push(format!("{agents} agent"));
            }

            if wrote.is_empty() {
                return Ok(format!(".claude/: already present ({present} files)"));
            }
            Ok(format!("wrote .claude/ — {} (Claude Code)", wrote.join(", ")))
        }
        "cursor" => {
            let path = root.join(".cursor/rules/blastradius.mdc");
            if path.exists() {
                return Ok(".cursor/rules/blastradius.mdc: already present".into());
            }
            std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
            let text = format!(
                "---\n\
                 description: Blastradius C4 architecture model in this repo\n\
                 alwaysApply: false\n\
                 ---\n\n{}",
                primer(rel)
            );
            std::fs::write(&path, text).map_err(|e| e.to_string())?;
            Ok("wrote .cursor/rules/blastradius.mdc (Cursor)".into())
        }
        "codex" => append_instructions(
            &root.join("AGENTS.md"),
            "AGENTS.md (Codex)",
            rel,
        ),
        "copilot" => append_instructions(
            &root.join(".github/copilot-instructions.md"),
            ".github/copilot-instructions.md (Copilot)",
            rel,
        ),
        other => Err(format!("unknown agent {other:?} — expected one of {AGENTS:?}")),
    }
}

fn append_instructions(path: &Path, label: &str, rel: &str) -> Result<String, String> {
    let existing = if path.is_file() {
        std::fs::read_to_string(path).map_err(|e| e.to_string())?
    } else {
        String::new()
    };
    if existing.to_lowercase().contains("blastradius") {
        return Ok(format!("{label}: already mentions blastradius"));
    }
    let mut text = existing;
    if !text.is_empty() && !text.ends_with('\n') {
        text.push('\n');
    }
    if !text.is_empty() {
        text.push('\n');
    }
    text.push_str(&format!("## Blastradius architecture model\n\n{}", primer(rel)));
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(path, text).map_err(|e| e.to_string())?;
    Ok(format!("wrote {label}"))
}
