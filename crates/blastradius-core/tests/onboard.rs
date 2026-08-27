//! `blastradius init` extras: git init, per-agent MCP registration, and
//! skills/instructions — all merge-only, never clobbering.

use blastradius_core::onboard::{git_root, setup, SetupOptions, AGENTS};
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

/// The primer is the only thing an agent reads before it starts editing.
/// After the first outside use it has to do three jobs the old one did not:
/// state the edit rule instead of preferring it, point at the schema instead
/// of leaving it to be inferred, and teach enough C4 to model well
/// (docs/roadmap.md, first-user findings).
#[test]
fn the_primer_tells_an_agent_where_the_schema_is() {
    let dir = temp("primer");
    setup(&dir, &SetupOptions { skills: all(), ..Default::default() });
    let skill = std::fs::read_to_string(dir.join(".claude/skills/blastradius/SKILL.md")).unwrap();

    // Where the schema lives, and that guessing is not an option.
    assert!(skill.contains("model_format"), "{skill}");
    assert!(skill.contains("Never guess the schema"), "{skill}");
    // Bootstrapping without dozens of round trips.
    assert!(skill.contains("apply_operations"), "{skill}");
    // Modelling guidance, not just format.
    assert!(skill.contains("dependency, not a data flow"), "{skill}");
    assert!(skill.contains("Stop at components"), "{skill}");
    // And what to do when the tools are missing, rather than improvising.
    assert!(skill.contains("not available"), "{skill}");

    // Every agent gets the same primer, so one check covers the others.
    let agents_md = std::fs::read_to_string(dir.join("AGENTS.md")).unwrap();
    assert!(agents_md.contains("model_format"), "{agents_md}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Claude Code gets three surfaces because the jobs are different shapes: a
/// skill is reference and fires on its own, commands are user-initiated and so
/// may interview, and a subagent gets its own context window. The interview
/// the owner asked for is a *command* — a skill that started interrogating you
/// because it auto-triggered would be obnoxious (2026-08-26).
#[test]
fn claude_gets_a_skill_commands_and_a_survey_agent() {
    let dir = temp("surfaces");
    let log = setup(
        &dir,
        &SetupOptions { skills: vec!["claude".into()], ..Default::default() },
    );

    for rel in [
        ".claude/skills/blastradius/SKILL.md",
        ".claude/commands/blastradius/model.md",
        ".claude/commands/blastradius/sync.md",
        ".claude/commands/blastradius/review.md",
        ".claude/agents/blastradius-surveyor.md",
    ] {
        assert!(dir.join(rel).is_file(), "{rel} missing; log = {log:#?}");
    }

    // The model command must actually interview, and cover the topics asked
    // for: level of detail, documents, introspection, deployment.
    let model = std::fs::read_to_string(dir.join(".claude/commands/blastradius/model.md")).unwrap();
    assert!(model.starts_with("---\ndescription:"), "commands need frontmatter:\n{model}");
    assert!(model.contains("Interview before you build"), "{model}");
    for topic in ["Level of detail", "Documents", "Code-level detail", "Deployment", "Scope"] {
        assert!(model.contains(topic), "the interview never asks about {topic}");
    }
    // An existing codebase is surveyed first; a blank repo is questioned.
    assert!(model.contains("blastradius-surveyor"), "existing code should be surveyed:\n{model}");
    assert!(model.contains("empty or nearly so"), "a clean repo needs its own branch:\n{model}");

    // The agent is read-only: it proposes, it does not model.
    let agent = std::fs::read_to_string(dir.join(".claude/agents/blastradius-surveyor.md")).unwrap();
    assert!(agent.contains("name: blastradius-surveyor"), "{agent}");
    assert!(agent.contains("tools: Read, Grep, Glob"), "the surveyor must not be able to write");

    // The skill points at the commands rather than duplicating them.
    let skill = std::fs::read_to_string(dir.join(".claude/skills/blastradius/SKILL.md")).unwrap();
    assert!(skill.contains("/blastradius:model"), "the skill should name the workflows:\n{skill}");

    // Idempotent, and nothing is overwritten.
    std::fs::write(dir.join(".claude/commands/blastradius/sync.md"), "mine").unwrap();
    let again = setup(&dir, &SetupOptions { skills: vec!["claude".into()], ..Default::default() });
    assert!(again.iter().all(|l| l.contains("already")), "{again:#?}");
    assert_eq!(
        std::fs::read_to_string(dir.join(".claude/commands/blastradius/sync.md")).unwrap(),
        "mine",
        "an edited command was overwritten"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// The other agents have no command or subagent surface, so their primer has
/// to stay self-contained — that is why the reference is not split up.
#[test]
fn the_other_agents_still_get_one_self_contained_primer() {
    let dir = temp("portable");
    setup(
        &dir,
        &SetupOptions {
            skills: vec!["cursor".into(), "copilot".into(), "codex".into()],
            ..Default::default()
        },
    );
    for rel in [".cursor/rules/blastradius.mdc", ".github/copilot-instructions.md", "AGENTS.md"] {
        let text = std::fs::read_to_string(dir.join(rel)).unwrap();
        assert!(text.contains("model_format"), "{rel} lost the schema pointer");
        assert!(text.contains("dependency, not a data flow"), "{rel} lost the C4 guidance");
    }
    assert!(!dir.join(".claude").exists(), "only Claude Code gets .claude/");
    let _ = std::fs::remove_dir_all(&dir);
}
