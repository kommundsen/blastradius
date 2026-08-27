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
    // Copilot now gets its own file rather than an append into the shared one.
    assert!(dir.join(".github/instructions/blastradius.instructions.md").is_file());
    // Codex's reference is our own file; AGENTS.md, which belongs to the
    // project, gets a delimited pointer at it and nothing else.
    assert!(dir.join(".agents/blastradius.md").is_file(), "{log:?}");
    let agents_md = std::fs::read_to_string(dir.join("AGENTS.md")).unwrap();
    assert!(agents_md.contains("Keep me."), "existing content preserved");
    assert!(agents_md.contains("<!-- blastradius:begin -->"));
    assert!(agents_md.contains("<!-- blastradius:end -->"));
    assert!(agents_md.contains(".agents/blastradius.md"), "the pointer names the reference");
    assert!(
        agents_md.lines().count() < 20,
        "the whole primer is back in somebody else's file:
{agents_md}"
    );
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
    let codex = std::fs::read_to_string(dir.join(".agents/blastradius.md")).unwrap();
    assert!(codex.contains("model_format"), "{codex}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Workflows reach every agent, not just Claude Code.
///
/// This shipped claiming only Claude Code had a command surface, which was
/// wrong on three counts — asserted from memory rather than checked. The paths
/// and extensions below come from each vendor's documentation, and they matter:
/// a file in the wrong place with the wrong extension does nothing and says
/// nothing about it (corrected 2026-08-26).
#[test]
fn every_agent_gets_the_workflows_in_its_own_format() {
    let dir = temp("surfaces");
    let log = setup(&dir, &SetupOptions { skills: all(), ..Default::default() });

    for rel in [
        // Claude Code: commands, and a subagent.
        ".claude/skills/blastradius/SKILL.md",
        ".claude/commands/blastradius/model.md",
        ".claude/commands/blastradius/sync.md",
        ".claude/commands/blastradius/review.md",
        ".claude/agents/blastradius-surveyor.md",
        // Copilot: prompt files, and a custom agent.
        ".github/instructions/blastradius.instructions.md",
        ".github/prompts/blastradius-model.prompt.md",
        ".github/prompts/blastradius-sync.prompt.md",
        ".github/prompts/blastradius-review.prompt.md",
        ".github/agents/blastradius-surveyor.agent.md",
        // Cursor and Codex share the .agents/skills convention.
        ".cursor/rules/blastradius.mdc",
        ".agents/blastradius.md",
        "AGENTS.md",
        ".agents/skills/blastradius-model/SKILL.md",
        ".agents/skills/blastradius-sync/SKILL.md",
        ".agents/skills/blastradius-review/SKILL.md",
    ] {
        assert!(dir.join(rel).is_file(), "{rel} missing; log = {log:#?}");
    }
    assert!(!dir.join(".claude/prompts").exists(), "invented a path nothing reads");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Each vendor spells its frontmatter differently, and getting it wrong is
/// silent. These are the keys their docs specify.
#[test]
fn each_workflow_carries_the_frontmatter_its_agent_expects() {
    let dir = temp("frontmatter");
    setup(&dir, &SetupOptions { skills: all(), ..Default::default() });
    let read = |rel: &str| std::fs::read_to_string(dir.join(rel)).unwrap();

    // Claude: description + argument-hint, and $ARGUMENTS is substituted.
    let claude = read(".claude/commands/blastradius/model.md");
    assert!(claude.starts_with("---\ndescription:"), "{claude}");
    assert!(claude.contains("argument-hint:"), "{claude}");
    assert!(claude.contains("$ARGUMENTS"), "{claude}");

    // Copilot prompt files: same keys, but no $ARGUMENTS — it would render as
    // literal text, since Copilot uses ${input:...}.
    let copilot = read(".github/prompts/blastradius-model.prompt.md");
    assert!(copilot.starts_with("---\ndescription:"), "{copilot}");
    assert!(copilot.contains("agent: agent"), "{copilot}");
    assert!(!copilot.contains("$ARGUMENTS"), "a literal $ARGUMENTS would leak into the chat");

    // Skills, for Cursor and Codex: name + description, both required.
    let skill = read(".agents/skills/blastradius-model/SKILL.md");
    assert!(skill.starts_with("---\nname: blastradius-model\n"), "{skill}");
    assert!(skill.contains("description:"), "{skill}");

    // Subagents, where the agent has them.
    let surveyor = read(".claude/agents/blastradius-surveyor.md");
    assert!(surveyor.contains("tools: Read, Grep, Glob"), "the surveyor must not write");
    let gh_surveyor = read(".github/agents/blastradius-surveyor.agent.md");
    assert!(gh_surveyor.contains("name: blastradius-surveyor"), "{gh_surveyor}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// The interview is the point: it must cover the topics that were asked for,
/// and treat an existing codebase differently from an empty one.
#[test]
fn the_model_workflow_interviews_before_it_builds() {
    let dir = temp("interview");
    setup(&dir, &SetupOptions { skills: vec!["claude".into()], ..Default::default() });
    let model = std::fs::read_to_string(dir.join(".claude/commands/blastradius/model.md")).unwrap();

    assert!(model.contains("Interview before you build"), "{model}");
    for topic in ["Scope", "Level of detail", "Documents", "Code-level detail", "Deployment"] {
        assert!(model.contains(topic), "the interview never asks about {topic}");
    }
    assert!(model.contains("blastradius-surveyor"), "existing code should be surveyed first");
    assert!(model.contains("empty or nearly so"), "a clean repo needs its own branch");

    // And the reference points at the workflows instead of duplicating them.
    let skill = std::fs::read_to_string(dir.join(".claude/skills/blastradius/SKILL.md")).unwrap();
    assert!(skill.contains("/blastradius:model"), "{skill}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Nothing is ever overwritten, and a second run is quiet.
#[test]
fn workflow_files_are_never_clobbered() {
    let dir = temp("clobber");
    setup(&dir, &SetupOptions { skills: all(), ..Default::default() });
    std::fs::write(dir.join(".claude/commands/blastradius/sync.md"), "mine").unwrap();
    std::fs::write(dir.join(".github/prompts/blastradius-sync.prompt.md"), "also mine").unwrap();

    let again = setup(&dir, &SetupOptions { skills: all(), ..Default::default() });
    assert!(again.iter().all(|l| l.contains("already")), "{again:#?}");
    assert_eq!(
        std::fs::read_to_string(dir.join(".claude/commands/blastradius/sync.md")).unwrap(),
        "mine"
    );
    assert_eq!(
        std::fs::read_to_string(dir.join(".github/prompts/blastradius-sync.prompt.md")).unwrap(),
        "also mine"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// The reference stays one self-contained document per agent — an agent reads
/// it before doing anything, and splitting it only makes half of it easy to
/// miss.
#[test]
fn every_reference_still_carries_the_schema_pointer_and_the_c4_rules() {
    let dir = temp("portable");
    setup(&dir, &SetupOptions { skills: all(), ..Default::default() });
    for rel in [
        ".claude/skills/blastradius/SKILL.md",
        ".cursor/rules/blastradius.mdc",
        ".github/instructions/blastradius.instructions.md",
        ".agents/blastradius.md",
    ] {
        let text = std::fs::read_to_string(dir.join(rel)).unwrap();
        assert!(text.contains("model_format"), "{rel} lost the schema pointer");
        assert!(text.contains("dependency, not a data flow"), "{rel} lost the C4 guidance");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// Copilot gets its own instructions file rather than an append into
/// `.github/copilot-instructions.md`, which belongs to the project (owner's
/// point, 2026-08-26): ours is removable on its own and never mixes into
/// someone's house rules.
#[test]
fn copilot_gets_its_own_instructions_file_and_leaves_the_shared_one_alone() {
    let dir = temp("copilot-own");
    std::fs::create_dir_all(dir.join(".github")).unwrap();
    let theirs = "# House rules

Use tabs. Never mention semicolons.
";
    std::fs::write(dir.join(".github/copilot-instructions.md"), theirs).unwrap();

    setup(&dir, &SetupOptions { skills: vec!["copilot".into()], ..Default::default() });

    let ours = dir.join(".github/instructions/blastradius.instructions.md");
    assert!(ours.is_file(), "no instructions file written");
    let text = std::fs::read_to_string(&ours).unwrap();
    assert!(text.starts_with("---
description:"), "{text}");
    // Always-on: the model has to stay in step when *code* changes too, not
    // only when the workspace happens to be open.
    assert!(text.contains("applyTo: '**'"), "{text}");
    assert!(text.contains("model_format"), "{text}");

    assert_eq!(
        std::fs::read_to_string(dir.join(".github/copilot-instructions.md")).unwrap(),
        theirs,
        "the project's own instructions were modified"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// A repo set up by an earlier version has our text appended to the shared
/// file. Do not say the same thing twice.
#[test]
fn an_earlier_appended_copilot_setup_is_left_as_it_is() {
    let dir = temp("copilot-legacy");
    std::fs::create_dir_all(dir.join(".github")).unwrap();
    std::fs::write(
        dir.join(".github/copilot-instructions.md"),
        "## Blastradius architecture model

older setup
",
    )
    .unwrap();

    let log = setup(&dir, &SetupOptions { skills: vec!["copilot".into()], ..Default::default() });
    assert!(
        !dir.join(".github/instructions/blastradius.instructions.md").exists(),
        "duplicated the reference; log = {log:#?}"
    );
    // The workflows are new either way, so those still land.
    assert!(dir.join(".github/prompts/blastradius-model.prompt.md").is_file(), "{log:#?}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Codex has no per-repo instructions file but `AGENTS.md`, which belongs to
/// the project — so the reference is our own file and `AGENTS.md` gets a
/// delimited pointer at it. A repository set up by 0.6.x has the whole primer
/// pasted in, unmarked: leave it. Rewriting somebody's AGENTS.md to tidy our
/// own history is not our call, and it still says the right things.
#[test]
fn a_legacy_agents_md_is_left_exactly_as_it_is() {
    let dir = temp("codex-legacy");
    let theirs = "# House rules\n\n## Blastradius architecture model\n\nThe old pasted-in primer.\n";
    std::fs::write(dir.join("AGENTS.md"), theirs).unwrap();

    let log = setup(&dir, &SetupOptions { skills: all(), ..Default::default() });
    assert_eq!(
        std::fs::read_to_string(dir.join("AGENTS.md")).unwrap(),
        theirs,
        "AGENTS.md was rewritten"
    );
    assert!(
        log.iter().any(|l| l.contains("already mentions blastradius")),
        "{log:#?}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
