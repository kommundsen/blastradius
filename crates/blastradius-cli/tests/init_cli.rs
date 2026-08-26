//! `blastradius init` driven as the user drives it — the binary, with flags.
//! The onboarding logic itself lives in core (tests/onboard.rs); this covers
//! only the CLI surface over it.

use std::path::PathBuf;

fn temp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("blastradius-initcli-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
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
    assert!(dir.join("blastradius.yaml").is_file());
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

/// `blastradius init` in a repository that already has a README used to write
/// four files, print "refusing to overwrite", exit 2, and skip the agent
/// setup — a half-initialised repo and a non-zero exit (reported 2026-08-26).
#[test]
fn init_keeps_existing_files_and_still_wires_agents() {
    let dir = temp("existing");
    let mine = "# Mine\n";
    std::fs::write(dir.join("README.md"), mine).unwrap();

    let out = std::process::Command::new(env!("CARGO_BIN_EXE_blastradius"))
        .args(["init", dir.to_str().unwrap(), "--name", "Acme", "--no-git",
               "--agents", "claude", "--skills", "claude"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "exited {:?}\n{stdout}\n{stderr}", out.status.code());

    assert!(stdout.contains("kept README.md"), "no mention of the kept file:\n{stdout}");
    assert_eq!(std::fs::read_to_string(dir.join("README.md")).unwrap(), mine);
    assert!(dir.join("blastradius.yaml").is_file());
    // The part that used to be unreachable.
    assert!(dir.join(".mcp.json").is_file(), "agents skipped:\n{stdout}\n{stderr}");
    assert!(dir.join(".claude/skills/blastradius/SKILL.md").is_file());
    let _ = std::fs::remove_dir_all(&dir);
}

/// `--into` puts the workspace in a subfolder, and the MCP registration that
/// follows must point at *that* folder, not the project root.
#[test]
fn init_into_a_subfolder_registers_the_right_path() {
    let dir = temp("into");
    std::process::Command::new("git").arg("init").current_dir(&dir).output().unwrap();

    let out = std::process::Command::new(env!("CARGO_BIN_EXE_blastradius"))
        .args(["init", dir.to_str().unwrap(), "--into", "docs", "--no-git",
               "--agents", "claude", "--skills", "none"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success(), "{stdout}{}", String::from_utf8_lossy(&out.stderr));

    assert!(dir.join("docs/blastradius.yaml").is_file(), "not in docs/:\n{stdout}");
    assert!(!dir.join("blastradius.yaml").exists(), "leaked into the project root");

    // Agent config lands at the git root, pointing into the workspace.
    let mcp: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join(".mcp.json")).unwrap()).unwrap();
    assert_eq!(mcp["mcpServers"]["blastradius"]["args"][1], "docs");

    // And the model is named after the project, not after "docs".
    let sys = std::fs::read_to_string(dir.join("docs/blastradius.yaml")).unwrap();
    assert!(!sys.contains("name: Docs"), "the system got named after its folder:\n{sys}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Scripted runs without `--into` keep writing to the project folder itself:
/// the recommendation is something the interactive prompt offers, not a
/// silent relocation of everybody's existing scripts.
#[test]
fn without_into_a_scripted_init_is_unchanged() {
    let dir = temp("noninteractive");
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_blastradius"))
        .args(["init", dir.to_str().unwrap(), "--no-git", "--agents", "none", "--skills", "none"])
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert!(dir.join("blastradius.yaml").is_file());
    assert!(!dir.join("docs").exists(), "created docs/ without being asked");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_location_outside_the_project_is_refused() {
    let dir = temp("escape");
    for bad in ["../evil", "/etc/blastradius"] {
        let out = std::process::Command::new(env!("CARGO_BIN_EXE_blastradius"))
            .args(["init", dir.to_str().unwrap(), "--into", bad, "--no-git"])
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(2), "{bad:?} was not refused");
    }
    let _ = std::fs::remove_dir_all(&dir);
}
