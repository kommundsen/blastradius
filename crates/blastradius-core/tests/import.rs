//! Structurizr importer tests (ADR-0002), including the Phase 4 exit bar:
//! the real-world corpus (tests/fixtures/structurizr — public workspace.dsl
//! files from actual projects) must import cleanly at >= 80% (PRD metric).

use blastradius_core::import::import_dsl;
use std::fs;
use std::path::PathBuf;

fn corpus_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/structurizr")
}

/// Import into a temp dir and validate with the real loader.
fn import_and_validate(src: &str) -> Result<(usize, usize), String> {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static SEQ: AtomicUsize = AtomicUsize::new(0);
    let imported = import_dsl(src)?;
    let dir = std::env::temp_dir().join(format!(
        "blastradius-import-{}-{}",
        std::process::id(),
        SEQ.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = fs::remove_dir_all(&dir);
    for (rel, text) in &imported.files {
        let path = dir.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
    let (ws, diags) = blastradius_core::load_workspace(&dir);
    let errors = diags
        .iter()
        .filter(|d| d.severity == blastradius_core::diagnostics::Severity::Error)
        .count();
    let elements = ws.elements.len();
    let _ = fs::remove_dir_all(&dir);
    if errors > 0 {
        Err(format!("{errors} validation errors"))
    } else {
        Ok((elements, imported.fidelity.skipped.len()))
    }
}

/// THE EXIT BAR (docs/roadmap.md Phase 4): >= 80% of the sampled public
/// corpus imports without manual fixes.
#[test]
fn corpus_meets_the_clean_import_bar() {
    let mut total = 0;
    let mut clean = 0;
    let mut failures = Vec::new();
    for entry in fs::read_dir(corpus_dir()).unwrap().flatten() {
        if entry.path().extension().and_then(|e| e.to_str()) != Some("dsl") {
            continue;
        }
        total += 1;
        let src = fs::read_to_string(entry.path()).unwrap();
        match import_and_validate(&src) {
            Ok((elements, _)) => {
                assert!(elements > 0, "{:?}: imported nothing", entry.file_name());
                clean += 1;
            }
            Err(e) => failures.push(format!("{:?}: {e}", entry.file_name())),
        }
    }
    assert!(total >= 10, "corpus shrank below 10 workspaces");
    let pct = clean * 100 / total;
    eprintln!("corpus: {clean}/{total} clean ({pct}%); failures: {failures:?}");
    assert!(pct >= 80, "clean-import bar missed: {clean}/{total} ({pct}%) — {failures:?}");
}

#[test]
fn basic_mapping_semantics() {
    let dsl = r#"
workspace "Shop" {
    model {
        user = person "Customer" "Buys things"
        legacy = softwareSystem "Old ERP" "..." "External"
        shop = softwareSystem "Shop" {
            web = container "Web App" "storefront" "React"
            api = container "API" "" "Go" {
                router = component "Router" "" "chi"
            }
        }
        user -> web "shops via" "HTTPS"
        api -> legacy "syncs orders"
    }
    views {
        systemContext shop { include * }
    }
}
"#;
    let imported = import_dsl(dsl).unwrap();
    assert_eq!(imported.workspace_name, "Shop");
    let ctx = &imported.files["model/context.yaml"];
    assert!(ctx.contains("customer:"), "{ctx}");
    assert!(ctx.contains("old-erp:"), "external tag routes to context: {ctx}");
    let shop = &imported.files["model/shop.yaml"];
    assert!(shop.contains("tech: React"), "{shop}");
    assert!(shop.contains("router:"), "components nest: {shop}");
    assert!(shop.contains("protocol: HTTPS"), "relationship technology maps: {shop}");
    // views are reported, never silently dropped
    assert!(imported.report.contains("views"), "{}", imported.report);
    // and the whole thing validates
    import_and_validate(dsl).unwrap();
}

#[test]
fn external_system_internals_lift_to_the_system() {
    let dsl = r#"
workspace {
    model {
        dev = person "Developer"
        ext = softwareSystem "Identity Provider" {
            tags "External"
            idp_core = container "IdP Core"
        }
        app = softwareSystem "App" {
            api = container "API"
        }
        idp_core -> api "notifies"
        api -> idp_core "authenticates via"
    }
}
"#;
    let imported = import_dsl(dsl).unwrap();
    let ctx = &imported.files["model/context.yaml"];
    assert!(ctx.contains("identity-provider:"), "{ctx}");
    assert!(!ctx.contains("idp-core"), "external internals are folded: {ctx}");
    let app = &imported.files["model/app.yaml"];
    assert!(app.contains("to: identity-provider") || app.contains("from: identity-provider"),
        "relations lift to the external root: {app}");
    assert!(imported.fidelity.notes.iter().any(|n| n.contains("opaque")),
        "folding is reported: {:?}", imported.fidelity.notes);
    import_and_validate(dsl).unwrap();
}

#[test]
fn keyword_shadowing_identifiers_parse() {
    // seen in the wild: an identifier named after the keyword itself
    let dsl = r#"
workspace {
    model {
        user = person "User"
        softwareSystem = softwareSystem "Software System"
        user -> softwareSystem "Uses"
    }
}
"#;
    let imported = import_dsl(dsl).unwrap();
    assert!(imported.files.contains_key("model/software-system.yaml"));
    import_and_validate(dsl).unwrap();
}
