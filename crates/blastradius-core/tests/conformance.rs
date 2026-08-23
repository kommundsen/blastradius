//! The dogfood gate (docs/prd.md): the repository's own docs/ folder is the
//! conformance workspace. If this test fails, either the parser or the docs
//! are wrong — and either way it must not merge.

use blastradius_core::diagnostics::{has_errors, Severity};
use std::path::PathBuf;

#[test]
fn repo_docs_workspace_is_valid() {
    let docs = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs");
    let (ws, diags) = blastradius_core::load_workspace(&docs);

    for d in &diags {
        eprintln!("{d}");
    }
    assert!(!has_errors(&diags));
    assert_eq!(ws.elements.len(), 25, "element count changed — update this test with the model");
    assert_eq!(ws.docs.len(), 26, "registered doc count changed");
    assert_eq!(ws.views.len(), 2);
    // The dogfood workspace introspects itself (spec/l4-introspection.md):
    // two committed facts graphs, TypeScript and Rust.
    assert_eq!(ws.derived.len(), 2, "dogfood derived graph count changed");
    // exactly one frontmatter-less file (docs/README.md), no warnings
    assert_eq!(diags.iter().filter(|d| d.severity == Severity::Info).count(), 1);
    assert_eq!(diags.iter().filter(|d| d.severity == Severity::Warning).count(), 0);

    // spot-check doc <-> element links resolved
    assert!(ws.docs.iter().any(|d| d.id == "adr-0007"
        && d.elements.iter().any(|e| e == "blastradius.core.git-service")));
}
