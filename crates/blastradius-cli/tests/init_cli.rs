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
