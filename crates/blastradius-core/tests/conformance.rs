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
    // 25 logical + 20 deployment (ADR-0018): 3 environments, 8 nodes, 9 instances.
    assert_eq!(ws.elements.len(), 45, "element count changed — update this test with the model");
    assert_eq!(ws.docs.len(), 29, "registered doc count changed");
    // L1 is implicit; the four files are containers (L2), core components
    // (L3), the deployment overview, and the nested developer machine.
    assert_eq!(ws.views.len(), 4);
    assert_eq!(
        ws.views.iter().filter(|v| v.nested).count(),
        1,
        "the dogfood model exercises the nested deployment view (ADR-0018)"
    );
    assert_eq!(
        ws.elements.values().filter(|e| e.kind.is_deployment()).count(),
        20,
        "deployment element count changed"
    );
    // The dogfood workspace introspects itself (spec/l4-introspection.md):
    // four committed facts graphs — one TypeScript, three Rust. The three Rust
    // mappings are what give drift detection something to compare (ADR-0019).
    assert_eq!(ws.derived.len(), 4, "dogfood derived graph count changed");
    // And the model agrees with the code: no drift in our own architecture.
    assert!(blastradius_core::drift::detect(&ws).is_empty(), "dogfood drift: {:?}", blastradius_core::drift::detect(&ws));
    // exactly one frontmatter-less file (docs/README.md), no warnings
    assert_eq!(diags.iter().filter(|d| d.severity == Severity::Info).count(), 1);
    assert_eq!(diags.iter().filter(|d| d.severity == Severity::Warning).count(), 0);

    // spot-check doc <-> element links resolved
    assert!(ws.docs.iter().any(|d| d.id == "adr-0007"
        && d.elements.iter().any(|e| e == "blastradius.core.git-service")));
}
