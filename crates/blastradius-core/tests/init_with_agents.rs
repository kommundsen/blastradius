//! The app's "Start a model here" path, exercised the way the app drives it.
//!
//! The desktop app hands `workspace_init` the string it got back from
//! `workspace_open`, which is a *canonicalized* path — on Windows that means
//! the `\\?\C:\...` verbatim form. Reported after 0.6.0: taking the offer
//! left the dialog open and wrote no agent files.

use blastradius_core::onboard::{setup, SetupOptions, AGENTS};
use std::path::PathBuf;

fn temp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("br-initagents-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn all() -> Vec<String> {
    AGENTS.iter().map(|s| s.to_string()).collect()
}

#[test]
fn agent_setup_survives_a_canonicalized_path() {
    let dir = temp("verbatim");
    // What the app passes: the canonicalized form, verbatim prefix and all.
    let canonical = dir.canonicalize().unwrap();
    println!("canonical = {}", canonical.display());

    for (rel, text) in blastradius_core::scaffold::starter_workspace("Acme") {
        let path = canonical.join(&rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    let log = setup(
        &canonical,
        &SetupOptions { git_init: false, mcp: all(), skills: all(), command: None },
    );
    println!("log = {log:#?}");

    for rel in [
        ".mcp.json",
        ".cursor/mcp.json",
        ".vscode/mcp.json",
        ".codex/config.toml",
        ".claude/skills/blastradius/SKILL.md",
    ] {
        assert!(dir.join(rel).is_file(), "{rel} was not written; log = {log:#?}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// The server command the app writes must be usable as JSON *and* as a
/// process argument — a Windows absolute path is neither by accident.
#[test]
fn an_absolute_command_round_trips_into_the_configs() {
    let dir = temp("abscmd");
    let exe = r"C:\Program Files\WindowsApps\Blastradius\blastradius.exe";
    let log = setup(
        &dir,
        &SetupOptions {
            git_init: false,
            mcp: all(),
            skills: vec![],
            command: Some(exe.to_string()),
        },
    );
    println!("log = {log:#?}");

    let mcp: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join(".mcp.json")).unwrap()).unwrap();
    assert_eq!(mcp["mcpServers"]["blastradius"]["command"], exe);

    // TOML basic strings take C escapes: an unescaped backslash would make
    // this unparseable, and Codex would silently ignore the server.
    let toml = std::fs::read_to_string(dir.join(".codex/config.toml")).unwrap();
    assert!(toml.contains(r"C:\\Program Files\\WindowsApps"), "unescaped path in TOML:\n{toml}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// The bug the 0.6.0 app shipped with: the starter set includes README.md,
/// and an existing file was fatal — so "Start a model here" failed on any
/// repository that had one, which is essentially all of them. It left the
/// dialog open having written nothing and never reached the agent setup.
#[test]
fn an_existing_readme_is_kept_not_a_failure() {
    use blastradius_core::scaffold;

    let dir = temp("readme");
    let mine = "# My Project\n\nnot yours to overwrite\n";
    std::fs::write(dir.join("README.md"), mine).unwrap();

    let done = scaffold::scaffold_into(&dir, "My Project").expect("scaffolding must not fail");

    assert_eq!(done.skipped, vec!["README.md".to_string()], "the user's file must be kept");
    assert!(done.created.contains(&"blastradius.yaml".to_string()), "{:?}", done.created);
    assert_eq!(
        std::fs::read_to_string(dir.join("README.md")).unwrap(),
        mine,
        "README.md was modified"
    );

    // And what is left is a workspace that loads clean — a skipped README
    // costs nothing, because it is a pointer and not part of the model.
    let (_ws, diags) = blastradius_core::load_workspace(&dir);
    assert!(
        !blastradius_core::diagnostics::has_errors(&diags),
        "partial scaffold does not validate: {diags:?}"
    );

    // The agent setup runs afterwards, which is exactly what used to be
    // skipped when scaffolding bailed.
    let log = setup(
        &dir,
        &SetupOptions { git_init: false, mcp: all(), skills: all(), command: None },
    );
    assert!(dir.join(".mcp.json").is_file(), "agents were not set up; log = {log:#?}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Scaffolding twice must be a no-op, not a pile of errors.
#[test]
fn scaffolding_over_itself_creates_nothing_and_breaks_nothing() {
    use blastradius_core::scaffold;
    let dir = temp("twice");
    let first = scaffold::scaffold_into(&dir, "Acme").unwrap();
    assert!(!first.created.is_empty());
    let second = scaffold::scaffold_into(&dir, "Acme").unwrap();
    assert!(second.created.is_empty(), "created on the second pass: {:?}", second.created);
    assert_eq!(second.skipped.len(), first.created.len());
    let _ = std::fs::remove_dir_all(&dir);
}

// ---- where the workspace goes (owner decision, 2026-08-26) ----------------
//
// A repository root is for source. The model is documentation and belongs
// with the documentation — this repository keeps its own in docs/. Both
// surfaces ask; these cover what they recommend.

#[test]
fn a_project_with_no_docs_folder_is_recommended_docs() {
    use blastradius_core::scaffold::suggested_location;
    let dir = temp("suggest-none");
    std::fs::create_dir_all(dir.join("src")).unwrap();
    assert_eq!(suggested_location(&dir), "docs");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_existing_docs_folder_is_used_rather_than_duplicated() {
    use blastradius_core::scaffold::suggested_location;
    let dir = temp("suggest-docs");
    std::fs::create_dir_all(dir.join("docs")).unwrap();
    assert_eq!(suggested_location(&dir), "docs");

    // A project that spells it `doc` gets `doc` — creating docs/ beside an
    // existing doc/ is exactly the mess this avoids.
    let other = temp("suggest-doc");
    std::fs::create_dir_all(other.join("doc")).unwrap();
    assert_eq!(suggested_location(&other), "doc");

    // Both present: the conventional one wins.
    let both = temp("suggest-both");
    std::fs::create_dir_all(both.join("doc")).unwrap();
    std::fs::create_dir_all(both.join("docs")).unwrap();
    assert_eq!(suggested_location(&both), "docs");

    // A *file* called docs is not a docs folder.
    let filey = temp("suggest-file");
    std::fs::write(filey.join("docs"), "not a folder").unwrap();
    assert_eq!(suggested_location(&filey), "docs");

    for d in [dir, other, both, filey] {
        let _ = std::fs::remove_dir_all(&d);
    }
}

#[test]
fn a_location_cannot_climb_out_of_the_project() {
    use blastradius_core::scaffold::check_location;
    assert!(check_location(".").is_ok());
    assert!(check_location("docs").is_ok());
    assert!(check_location("docs/architecture").is_ok());

    // Refused on *every* platform, not just the one that calls them absolute:
    // a location is written into .mcp.json and read on other machines.
    for bad in ["..", "../evil", "docs/../..", "/etc", r"C:\Windows", "c:/users", "", "   "] {
        assert!(check_location(bad).is_err(), "{bad:?} should be refused");
    }
}
