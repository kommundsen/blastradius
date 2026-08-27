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

/// Structurizr `group` used to be flattened and discarded with a "groups are
/// not modelled" note. Now that grouping exists (spec §3c) the label survives
/// the round trip — while staying presentation only, so the flattening of the
/// *structure* is unchanged.
#[test]
fn groups_survive_import_as_labels() {
    let dsl = r#"
workspace "Shop" {
    model {
        shop = softwareSystem "Shop" {
            group "Storefront" {
                web = container "Web App" "" "React"
                bff = container "BFF" "" "Node"
            }
            group "Finance" {
                ledger = container "Ledger" "" "Go"
            }
            ops = container "Ops" "" "Go"
        }
    }
}
"#;
    let imported = import_dsl(dsl).unwrap();
    let shop = &imported.files["model/shop.yaml"];
    assert!(shop.contains("group: Storefront"), "group label must be emitted: {shop}");
    assert!(shop.contains("group: Finance"), "{shop}");

    // Presentation only: grouped containers stay siblings of the ungrouped
    // one, at the same depth, with no group segment in any id.
    for id in ["web-app:", "bff:", "ledger:", "ops:"] {
        assert!(shop.contains(id), "{id} missing — grouping must not restructure: {shop}");
    }
    assert!(!shop.contains("storefront:"), "a group must not become an element: {shop}");

    // The group closes: `ops` sits outside both blocks.
    let ops_at = shop.find("  ops:").expect("ops container");
    let after_ops = &shop[ops_at..];
    assert!(!after_ops.contains("group:"), "group leaked past its block: {after_ops}");

    // And it no longer reports the loss it used to.
    assert!(
        !imported.report.contains("groups are not modelled"),
        "stale fidelity note: {}",
        imported.report
    );
    import_and_validate(dsl).unwrap();
}

// ---- deployment (ADR-0018 follow-up) ---------------------------------------
// `deploymentEnvironment` and `deploymentNode` were parsed and discarded
// through 0.6.x: a DSL that says where its containers run is telling you
// something the logical model cannot, and dropping it silently was the worst
// of both.

const DEPLOYED: &str = r#"
workspace "Shop" {
  model {
    api = softwareSystem "Shop" {
      web = container "Web" "" "React"
      svc = container "Service" "" "Go"
    }
    prod = deploymentEnvironment "Production" {
      aws = deploymentNode "AWS" "the account" "us-east-1" {
        lb = infrastructureNode "Load Balancer" "" "ALB"
        app = deploymentNode "App Server" "" "EC2" "" 3 {
          containerInstance svc
        }
        edge = deploymentNode "CDN" {
          containerInstance web
        }
      }
      lb -> app "routes to"
    }
  }
}
"#;

#[test]
fn a_deployment_environment_becomes_a_deployment_file() {
    let imported = import_dsl(DEPLOYED).expect("import");
    let file = imported
        .files
        .get("model/deployment.yaml")
        .expect("no deployment file was written");

    assert!(file.contains("environments:"), "{file}");
    assert!(file.contains("  production:"), "{file}");
    // Nodes nest the way the DSL nested them.
    assert!(file.contains("      aws:"), "{file}");
    assert!(file.contains("          app-server:"), "{file}");
    // Structurizr's trailing instance count is our `replicas`.
    assert!(file.contains("replicas: 3"), "{file}");
    // An instance points at the container it runs, by resolved id.
    assert!(file.contains("container: shop.service"), "{file}");
    assert!(file.contains("container: shop.web"), "{file}");
    // An infrastructure node has no kind of its own here; it is a node.
    assert!(file.contains("load-balancer:"), "{file}");
    // Relations inside an environment stay inside it, relative to it.
    assert!(file.contains("    relations:"), "{file}");
    assert!(file.contains("routes to"), "{file}");
}

#[test]
fn the_imported_deployment_actually_loads() {
    let (elements, errors) = import_and_validate(DEPLOYED).expect("import");
    assert_eq!(errors, 0, "the imported workspace does not validate");
    // 1 system + 2 containers + 1 environment + 3 nodes + 1 infra node
    // + 2 instances.
    assert!(elements >= 10, "only {elements} elements survived");
}

#[test]
fn deployment_is_counted_as_mapped_rather_than_skipped() {
    let imported = import_dsl(DEPLOYED).expect("import");
    let m = &imported.fidelity.mapped;
    assert_eq!(m.get("deploymentEnvironment"), Some(&1));
    assert_eq!(m.get("deploymentNode"), Some(&3));
    assert_eq!(m.get("infrastructureNode"), Some(&1));
    assert_eq!(m.get("containerInstance"), Some(&2));
    assert!(
        !imported.fidelity.skipped.iter().any(|(_, what, _)| what.contains("deployment")),
        "{:?}",
        imported.fidelity.skipped
    );
}

/// Not just the hand-written case: the real-world corpus contains DSLs with
/// deployment blocks, and those blocks are where a `tags` line sits between
/// two `deploymentNode`s. Skipping an unknown keyword there must not swallow
/// the keyword after it.
#[test]
fn real_world_deployment_blocks_import() {
    let mut with_deployment = 0;
    for entry in fs::read_dir(corpus_dir()).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) != Some("dsl") {
            continue;
        }
        let src = fs::read_to_string(&path).unwrap();
        if !src.contains("deploymentEnvironment") {
            continue;
        }
        with_deployment += 1;
        let imported = import_dsl(&src)
            .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        assert!(
            imported.files.contains_key("model/deployment.yaml"),
            "{} declares a deployment environment but produced no deployment file",
            path.display()
        );
    }
    assert!(with_deployment >= 2, "the corpus lost its deployment examples");
}
