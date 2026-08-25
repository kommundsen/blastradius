//! The format reference an agent is handed must describe *this* build.
//!
//! An example that does not load is worse than no example: it is exactly what
//! someone with no other reference will imitate, and the first agent to model
//! a repository with these tools did precisely that with a sample file
//! (docs/roadmap.md, first-user findings).

use blastradius_core::format_ref;
use blastradius_core::diagnostics::has_errors;
use std::path::PathBuf;

fn temp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("blastradius-formatref-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

#[test]
fn the_worked_example_is_a_workspace_that_loads_clean() {
    let dir = temp("example");
    for (rel, text) in format_ref::EXAMPLE_FILES {
        let path = dir.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    let (ws, diags) = blastradius_core::load_workspace(&dir);
    assert!(!has_errors(&diags), "the example does not validate: {:?}", diags);

    // The shapes the reference teaches must actually be in it.
    for id in ["shopper", "stripe", "shop", "shop.api", "shop.api.billing", "shop.db"] {
        assert!(ws.elements.contains_key(id), "example is missing {id}");
    }
    assert!(!ws.views.is_empty(), "example has no view");
    assert!(ws.docs.iter().any(|d| d.id == "adr-0001"), "example has no doc");

    let _ = std::fs::remove_dir_all(&dir);
}

/// The reference is served over MCP and embedded in the generated skill; both
/// copies come from here, so this is the one place it can be checked.
#[test]
fn the_reference_covers_every_element_kind() {
    let text = format_ref::full_reference();
    for kind in [
        "person", "external", "system", "container", "component",
        "environment", "deployment-node", "container-instance",
    ] {
        assert!(text.contains(kind), "the format reference never mentions {kind}");
    }
    for language in ["rust", "typescript", "csharp"] {
        assert!(text.contains(language), "introspection languages are incomplete: {language}");
    }
}
