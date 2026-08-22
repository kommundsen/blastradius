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

/// Apply the selected extras. `workspace` is the folder holding
/// workspace.yaml; agent config lands at the git root (agents run there),
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
    let rel = workspace
        .strip_prefix(&root)
        .ok()
        .filter(|r| !r.as_os_str().is_empty())
        .map(|r| r.to_string_lossy().replace('\\', "/"))
        .unwrap_or_else(|| ".".to_string());

    for agent in &opts.mcp {
        match write_mcp(agent, &root, &rel) {
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

fn write_mcp(agent: &str, root: &Path, rel: &str) -> Result<String, String> {
    let args: Vec<Value> = server_args(rel).into_iter().map(Value::from).collect();
    match agent {
        "claude" => merge_json_config(
            &root.join(".mcp.json"),
            "mcpServers",
            json!({"command": "blastradius", "args": args}),
            ".mcp.json (Claude Code)",
        ),
        "cursor" => merge_json_config(
            &root.join(".cursor/mcp.json"),
            "mcpServers",
            json!({"command": "blastradius", "args": args}),
            ".cursor/mcp.json (Cursor)",
        ),
        "copilot" => merge_json_config(
            &root.join(".vscode/mcp.json"),
            "servers",
            json!({"type": "stdio", "command": "blastradius", "args": args}),
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
                 command = \"blastradius\"\n\
                 args = [\"mcp\", \"{rel}\"]\n"
            ));
            std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
            std::fs::write(&path, text).map_err(|e| e.to_string())?;
            Ok("wrote .codex/config.toml (Codex)".into())
        }
        other => Err(format!("unknown agent {other:?} — expected one of {AGENTS:?}")),
    }
}

/// The shared primer every agent gets, in its ecosystem's native format.
fn primer(rel: &str) -> String {
    let loc = if rel == "." { "this folder".to_string() } else { format!("`{rel}/`") };
    format!(
        "This repository contains a Blastradius workspace in {loc} — a C4\n\
         architecture model as plain YAML (workspace.yaml + model/ + views/),\n\
         versioned like source code.\n\
         \n\
         When architecture is relevant:\n\
         \n\
         - Query the model through the `blastradius` MCP tools. Start with\n\
         \x20 `workspace_summary`; call `blast_radius` with an element id before\n\
         \x20 changing or deleting anything it models; `doc` returns the ADRs and\n\
         \x20 specs governing an element.\n\
         - Prefer the `apply_operation` tool for model edits — it splices the\n\
         \x20 YAML in place (comments and formatting survive), validates before\n\
         \x20 writing, and is undoable. If you edit the YAML by hand instead:\n\
         \x20 never re-serialize or re-order keys, and run `blastradius validate\n\
         \x20 {rel}` (or the `validate` tool) afterwards.\n\
         - Element ids (the YAML keys) are immutable — renaming means changing\n\
         \x20 the `name:` field only.\n\
         - Markdown docs with a `doc:` frontmatter block are part of the model;\n\
         \x20 their `elements:` links must point at real element ids.\n\
         - Keep the model in sync with reality: when you add, remove, or rewire\n\
         \x20 a real component, mirror it in the model in the same change.\n"
    )
}

fn write_skill(agent: &str, root: &Path, rel: &str) -> Result<String, String> {
    match agent {
        "claude" => {
            let path = root.join(".claude/skills/blastradius/SKILL.md");
            if path.exists() {
                return Ok(".claude/skills/blastradius: already present".into());
            }
            std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
            let text = format!(
                "---\n\
                 name: blastradius\n\
                 description: Query and edit this repo's Blastradius C4 architecture model (YAML workspace). Use when working with the architecture model, ADRs, or when a change affects modelled components.\n\
                 ---\n\n\
                 # Working with the Blastradius model\n\n{}",
                primer(rel)
            );
            std::fs::write(&path, text).map_err(|e| e.to_string())?;
            Ok("wrote .claude/skills/blastradius/SKILL.md (Claude Code)".into())
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
