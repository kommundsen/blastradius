//! The seeded-fault half of the Phase 0 exit criterion (docs/roadmap.md):
//! each fault class must fail with the correct file and line.

use blastradius_core::diagnostics::{has_errors, Diagnostic, Severity};
use std::path::PathBuf;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name)
}

fn load(name: &str) -> (blastradius_core::Workspace, Vec<Diagnostic>) {
    blastradius_core::load_workspace(&fixture(name))
}

fn errors(diags: &[Diagnostic]) -> Vec<&Diagnostic> {
    diags.iter().filter(|d| d.severity == Severity::Error).collect()
}

#[test]
fn valid_fixture_passes() {
    let (ws, diags) = load("valid");
    assert!(!has_errors(&diags), "unexpected errors: {diags:?}");
    // 2 context + shop + web + api + router = 6 elements
    assert_eq!(ws.elements.len(), 6);
    assert_eq!(ws.relations.len(), 3);
    assert_eq!(ws.docs.len(), 1);
    // notes.md without frontmatter -> exactly one info, no warnings
    assert_eq!(diags.iter().filter(|d| d.severity == Severity::Info).count(), 1);
    assert_eq!(diags.iter().filter(|d| d.severity == Severity::Warning).count(), 0);
}

#[test]
fn dangling_relation_reference() {
    let (_, diags) = load("dangling-relation");
    let errs = errors(&diags);
    assert_eq!(errs.len(), 1, "{diags:?}");
    let e = errs[0];
    assert_eq!(e.file, "model/shop.yaml");
    assert_eq!(e.line, 6, "relation item starts on line 6");
    assert!(e.message.contains("dangling reference \"apii\""), "{}", e.message);
}

#[test]
fn duplicate_id_reports_both_sites() {
    let (_, diags) = load("duplicate-id");
    let errs = errors(&diags);
    assert_eq!(errs.len(), 1, "{diags:?}");
    let e = errs[0];
    assert_eq!(e.file, "model/context.yaml");
    assert_eq!(e.line, 5, "second declaration of `user` is on line 5");
    assert!(e.message.contains("duplicate id \"user\""), "{}", e.message);
    assert!(e.message.contains("model/context.yaml:2"), "must point at first site: {}", e.message);
}

#[test]
fn bad_frontmatter_status_and_dangling_doc_link() {
    let (_, diags) = load("bad-status");
    let errs = errors(&diags);
    assert_eq!(errs.len(), 2, "{diags:?}");
    assert!(errs.iter().all(|e| e.file == "decision.md"));
    // doc id is on file line 2 (line 1 is the `---` fence)
    assert!(errs.iter().all(|e| e.line == 2), "{errs:?}");
    assert!(errs.iter().any(|e| e.message.contains("\"acceptedd\" invalid for type \"adr\"")));
    assert!(errs.iter().any(|e| e.message.contains("dangling: \"ghost\"")));
}

#[test]
fn unknown_version_refuses_with_upgrade_message() {
    let (ws, diags) = load("unknown-version");
    let errs = errors(&diags);
    assert_eq!(errs.len(), 1, "{diags:?}");
    assert_eq!(errs[0].file, "blastradius.yaml");
    assert_eq!(errs[0].line, 3, "version: field is on line 3");
    assert!(errs[0].message.contains("newer than this build"), "{}", errs[0].message);
    // version gate = no partial parse (spec §1)
    assert!(ws.elements.is_empty());
}

#[test]
fn malformed_yaml_fails_with_location() {
    let (_, diags) = load("malformed");
    let errs = errors(&diags);
    assert!(!errs.is_empty());
    let e = errs[0];
    assert_eq!(e.file, "model/context.yaml");
    assert_eq!(e.line, 4, "bad indent is on line 4");
    assert!(e.message.contains("malformed YAML"), "{}", e.message);
}

#[test]
fn missing_workspace_yaml_is_not_a_workspace() {
    let (_, diags) = blastradius_core::load_workspace(&fixture("does-not-exist"));
    let errs = errors(&diags);
    assert_eq!(errs.len(), 1);
    assert!(errs[0].message.contains("not a workspace"));
}
