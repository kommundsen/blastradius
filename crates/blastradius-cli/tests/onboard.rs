//! `blastradius init` extras: git init, per-agent MCP registration, and
//! skills/instructions — all merge-only, never clobbering.

use blastradius_cli::onboard::{git_root, setup, SetupOptions, AGENTS};
use serde_json::Value;
use std::path::PathBuf;

fn temp(name: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("blastradius-onboard-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn all() -> Vec<String> {
    AGENTS.iter().map(|s| s.to_string()).collect()
}

#[test]
fn git_init_creates_a_repository() {
    let dir = temp("git");
    assert!(git_root(&dir).is_none() || !dir.join(".git").exists());
    let log = setup(&dir, &SetupOptions { git_init: true, ..Default::default() });
    assert!(dir.join(".git").exists(), "{log:?}");
    assert_eq!(git_root(&dir), Some(dir.clone()));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn mcp_config_written_for_every_agent() {
    let dir = temp("mcp-all");
    let log = setup(&dir, &SetupOptions { mcp: all(), ..Default::default() });
    for f in [".mcp.json", ".cursor/mcp.json", ".vscode/mcp.json", ".codex/config.toml"] {
        assert!(dir.join(f).is_file(), "{f} missing: {log:?}");
    }
    let claude: Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join(".mcp.json")).unwrap()).unwrap();
    assert_eq!(claude["mcpServers"]["blastradius"]["command"], "blastradius");
    assert_eq!(claude["mcpServers"]["blastradius"]["args"][0], "mcp");
    let vscode: Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join(".vscode/mcp.json")).unwrap())
            .unwrap();
    assert_eq!(vscode["servers"]["blastradius"]["type"], "stdio");
    let codex = std::fs::read_to_string(dir.join(".codex/config.toml")).unwrap();
    assert!(codex.contains("[mcp_servers.blastradius]"), "{codex}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn existing_configs_are_merged_not_clobbered() {
    let dir = temp("merge");
    std::fs::write(
        dir.join(".mcp.json"),
        r#"{"mcpServers": {"other": {"command": "x"}}}"#,
    )
    .unwrap();
    setup(&dir, &SetupOptions { mcp: vec!["claude".into()], ..Default::default() });
    let doc: Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join(".mcp.json")).unwrap()).unwrap();
    assert_eq!(doc["mcpServers"]["other"]["command"], "x", "existing entry preserved");
    assert!(doc["mcpServers"]["blastradius"].is_object());

    // second run: idempotent
    let log = setup(&dir, &SetupOptions { mcp: vec!["claude".into()], ..Default::default() });
    assert!(log[0].contains("already registered"), "{log:?}");

    // invalid JSON is left untouched
    std::fs::write(dir.join(".cursor/mcp.json"), "{ not json").ok();
    std::fs::create_dir_all(dir.join(".cursor")).unwrap();
    std::fs::write(dir.join(".cursor/mcp.json"), "{ not json").unwrap();
    let log = setup(&dir, &SetupOptions { mcp: vec!["cursor".into()], ..Default::default() });
    assert!(log[0].contains("left untouched"), "{log:?}");
    assert_eq!(std::fs::read_to_string(dir.join(".cursor/mcp.json")).unwrap(), "{ not json");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn skills_and_instructions_written_and_appended() {
    let dir = temp("skills");
    std::fs::write(dir.join("AGENTS.md"), "# Existing guidance\n\nKeep me.\n").unwrap();
    let log = setup(&dir, &SetupOptions { skills: all(), ..Default::default() });
    assert!(dir.join(".claude/skills/blastradius/SKILL.md").is_file(), "{log:?}");
    assert!(dir.join(".cursor/rules/blastradius.mdc").is_file());
    assert!(dir.join(".github/copilot-instructions.md").is_file());
    let agents_md = std::fs::read_to_string(dir.join("AGENTS.md")).unwrap();
    assert!(agents_md.contains("Keep me."), "existing content preserved");
    assert!(agents_md.contains("## Blastradius architecture model"));
    let skill = std::fs::read_to_string(dir.join(".claude/skills/blastradius/SKILL.md")).unwrap();
    assert!(skill.starts_with("---\nname: blastradius\n"), "{skill}");
    assert!(skill.contains("blast_radius"), "{skill}");
    // idempotent
    let log = setup(&dir, &SetupOptions { skills: all(), ..Default::default() });
    assert!(log.iter().all(|l| l.contains("already")), "{log:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn config_lands_at_the_git_root_with_relative_workspace_path() {
    let repo = temp("nested");
    setup(&repo, &SetupOptions { git_init: true, ..Default::default() });
    let ws = repo.join("docs");
    std::fs::create_dir_all(&ws).unwrap();
    setup(&ws, &SetupOptions { mcp: vec!["claude".into()], ..Default::default() });
    let doc: Value =
        serde_json::from_str(&std::fs::read_to_string(repo.join(".mcp.json")).unwrap()).unwrap();
    assert_eq!(doc["mcpServers"]["blastradius"]["args"][1], "docs");
    assert!(!ws.join(".mcp.json").exists(), "config belongs at the repo root");
    let _ = std::fs::remove_dir_all(&repo);
}

/// The CLI flags end to end: non-interactive, no prompts, extras applied.
#[test]
fn init_flags_drive_the_extras() {
    let dir = temp("cli-flags");
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_blastradius"))
        .args(["init", dir.to_str().unwrap(), "--name", "Acme", "--no-git",
               "--agents", "claude,codex", "--skills", "claude"])
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert!(dir.join("workspace.yaml").is_file());
    assert!(!dir.join(".git").exists(), "--no-git respected");
    assert!(dir.join(".mcp.json").is_file());
    assert!(dir.join(".codex/config.toml").is_file());
    assert!(!dir.join(".vscode").exists(), "unselected agents untouched");
    assert!(dir.join(".claude/skills/blastradius/SKILL.md").is_file());

    // rerun on the existing workspace: scaffold skipped, extras still work
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_blastradius"))
        .args(["init", dir.to_str().unwrap(), "--no-git", "--agents", "copilot",
               "--skills", "none"])
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert!(String::from_utf8_lossy(&out.stdout).contains("scaffold skipped"));
    assert!(dir.join(".vscode/mcp.json").is_file());

    // bad agent name errors out
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_blastradius"))
        .args(["init", dir.to_str().unwrap(), "--agents", "clippy"])
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("unknown agent"));
    let _ = std::fs::remove_dir_all(&dir);
}
