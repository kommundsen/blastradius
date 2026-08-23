//! Workspace discovery + manifest naming (ADR-0014): open a repo root and
//! find the workspace(s) inside; `blastradius.yaml` is the manifest,
//! legacy `workspace.yaml` still loads with a deprecation warning.

use blastradius_core::diagnostics::Severity;
use blastradius_core::discover::discover_workspaces;
use std::path::PathBuf;

const MANIFEST: &str = "workspace:\n  name: T\n  version: 1\nmodel:\n  include: [model/*.yaml]\n";
const MODEL: &str = "system: t\nname: T\ncontainers:\n  app:\n    name: App\n";

struct Temp {
    dir: PathBuf,
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}
fn temp(tag: &str) -> Temp {
    let dir = std::env::temp_dir()
        .join(format!("br-discover-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    Temp { dir }
}
fn write(t: &Temp, rel: &str, text: &str) {
    let p = t.dir.join(rel);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, text).unwrap();
}

#[test]
fn base_itself_is_the_only_hit() {
    let t = temp("direct");
    write(&t, "blastradius.yaml", MANIFEST);
    write(&t, "sub/blastradius.yaml", MANIFEST); // shadowed by the direct hit
    assert_eq!(discover_workspaces(&t.dir), vec![t.dir.clone()]);
}

#[test]
fn finds_the_dogfood_layout_from_the_repo_root() {
    let t = temp("nested");
    write(&t, "src/lib.rs", "");
    write(&t, "docs/blastradius.yaml", MANIFEST);
    write(&t, "docs/model/t.yaml", MODEL);
    assert_eq!(discover_workspaces(&t.dir), vec![t.dir.join("docs")]);
}

#[test]
fn legacy_manifest_name_is_discovered_too() {
    let t = temp("legacy");
    write(&t, "docs/workspace.yaml", MANIFEST);
    assert_eq!(discover_workspaces(&t.dir), vec![t.dir.join("docs")]);
}

#[test]
fn monorepo_yields_every_workspace_sorted() {
    let t = temp("multi");
    write(&t, "apps/checkout/docs/blastradius.yaml", MANIFEST);
    write(&t, "apps/billing/docs/blastradius.yaml", MANIFEST);
    assert_eq!(
        discover_workspaces(&t.dir),
        vec![
            t.dir.join("apps").join("billing").join("docs"),
            t.dir.join("apps").join("checkout").join("docs"),
        ]
    );
}

#[test]
fn foreign_and_lookalike_files_are_sniffed_out() {
    let t = temp("sniff");
    // another tool's workspace.yaml: no top-level `workspace:` key
    write(&t, "melos/workspace.yaml", "name: mono\npackages:\n  - apps/*\n");
    // a *model* file that happens to be named blastradius.yaml (our own
    // dogfood does exactly this under model/)
    write(&t, "docs2/blastradius.yaml", "system: b\nname: B\n");
    assert!(discover_workspaces(&t.dir).is_empty());
}

#[test]
fn dependency_and_hidden_dirs_are_never_searched() {
    let t = temp("skips");
    write(&t, "node_modules/pkg/blastradius.yaml", MANIFEST);
    write(&t, "target/debug/blastradius.yaml", MANIFEST);
    write(&t, ".cache/blastradius.yaml", MANIFEST);
    assert!(discover_workspaces(&t.dir).is_empty());
}

#[test]
fn depth_is_bounded() {
    let t = temp("depth");
    write(&t, "a/b/c/d/blastradius.yaml", MANIFEST); // depth 4: found
    let hits = discover_workspaces(&t.dir);
    assert_eq!(hits, vec![t.dir.join("a").join("b").join("c").join("d")]);
    let t2 = temp("depth5");
    write(&t2, "a/b/c/d/e/blastradius.yaml", MANIFEST); // depth 5: too deep
    assert!(discover_workspaces(&t2.dir).is_empty());
}

#[test]
fn legacy_manifest_loads_with_a_deprecation_warning() {
    let t = temp("legacy-load");
    write(&t, "workspace.yaml", MANIFEST);
    write(&t, "model/t.yaml", MODEL);
    let (ws, diags) = blastradius_core::load_workspace(&t.dir);
    assert_eq!(ws.name, "T");
    let warns: Vec<_> = diags.iter().filter(|d| d.severity == Severity::Warning).collect();
    assert_eq!(warns.len(), 1, "{diags:?}");
    assert!(warns[0].message.contains("rename workspace.yaml to blastradius.yaml"));
}

#[test]
fn new_name_wins_when_both_exist_and_the_loser_is_flagged() {
    let t = temp("both");
    write(&t, "blastradius.yaml", MANIFEST.replace("name: T", "name: New").as_str());
    write(&t, "workspace.yaml", MANIFEST.replace("name: T", "name: Old").as_str());
    write(&t, "model/t.yaml", MODEL);
    let (ws, diags) = blastradius_core::load_workspace(&t.dir);
    assert_eq!(ws.name, "New");
    assert!(diags.iter().any(|d| d.severity == Severity::Warning
        && d.file == "workspace.yaml"
        && d.message.contains("takes precedence")));
}
