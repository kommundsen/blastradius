//! Deployment views (ADR-0018, spec §3b): the tree parses into ordinary
//! elements with dotted ids, instances must name a real container, and the
//! whole thing is editable through the sync engine like anything else.

use blastradius_core::diagnostics::{has_errors, Diagnostic, Severity};
use blastradius_core::model::ElementKind;
use blastradius_core::sync::{Operation, SyncEngine};
use std::fs;
use std::path::{Path, PathBuf};

struct TempDir {
    dir: PathBuf,
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

fn temp(tag: &str) -> TempDir {
    let dir = std::env::temp_dir().join(format!("br-deploy-{tag}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    TempDir { dir }
}

fn write(root: &Path, rel: &str, content: &str) {
    let path = root.join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}

const MANIFEST: &str = "workspace:\n  name: Shop\n  version: 1\nmodel:\n  include: [model/*.yaml]\nviews:\n  include: [views/*.yaml]\n";

const SYSTEM: &str = "\
system: shop
name: Shop
containers:
  api:
    name: API
  web:
    name: Web
";

const DEPLOYMENT: &str = "\
environments:
  production:
    name: Production
    nodes:
      eu-west:
        name: EU West
        tech: AWS
        nodes:
          app-server:
            name: App Server
            instances:
              api: { container: shop.api }
              web: { container: shop.web }
      cdn:
        name: CDN
    relations:
      - from: cdn
        to: eu-west
        label: origin
";

/// Builds a workspace; `deployment` replaces the default deployment file.
fn workspace(tag: &str, deployment: &str) -> (TempDir, blastradius_core::Workspace, Vec<Diagnostic>) {
    let t = temp(tag);
    write(&t.dir, "blastradius.yaml", MANIFEST);
    write(&t.dir, "model/shop.yaml", SYSTEM);
    write(&t.dir, "model/deployment.yaml", deployment);
    let (ws, diags) = blastradius_core::load_workspace(&t.dir);
    (t, ws, diags)
}

fn errors(diags: &[Diagnostic]) -> Vec<&Diagnostic> {
    diags.iter().filter(|d| d.severity == Severity::Error).collect()
}

#[test]
fn deployment_tree_parses_into_dotted_elements() {
    let (_t, ws, diags) = workspace("parse", DEPLOYMENT);
    assert!(!has_errors(&diags), "{diags:?}");

    let kind = |id: &str| ws.elements.get(id).unwrap_or_else(|| panic!("missing {id}")).kind;
    assert_eq!(kind("production"), ElementKind::Environment);
    assert_eq!(kind("production.eu-west"), ElementKind::DeploymentNode);
    // Nodes nest arbitrarily — this one is two deep.
    assert_eq!(kind("production.eu-west.app-server"), ElementKind::DeploymentNode);
    assert_eq!(kind("production.eu-west.app-server.api"), ElementKind::ContainerInstance);

    // An instance records the container it runs, and takes its own name.
    let api = &ws.elements["production.eu-west.app-server.api"];
    assert_eq!(api.instance_of.as_deref(), Some("shop.api"));

    // An unnamed instance shows the container's name, not its own titleized
    // key — "Web", not "Web" by accident; the container is the thing running.
    assert_eq!(ws.elements["production.eu-west.app-server.api"].name, "API");
    assert_eq!(ws.elements["production.eu-west.app-server.web"].name, "Web");

    // Environment-scoped relations resolve like system-scoped ones.
    assert!(
        ws.resolved_relations()
            .iter()
            .any(|(from, to, r)| from == "production.cdn" && to == "production.eu-west" && r.label.as_deref() == Some("origin")),
        "relations: {:?}",
        ws.relations
    );
}

#[test]
fn an_explicit_instance_name_wins_over_the_container_name() {
    let (_t, ws, diags) = workspace(
        "named",
        &DEPLOYMENT.replace("api: { container: shop.api }", "api: { container: shop.api, name: API (blue) }"),
    );
    assert!(!has_errors(&diags), "{diags:?}");
    assert_eq!(ws.elements["production.eu-west.app-server.api"].name, "API (blue)");
}

#[test]
fn instance_must_name_a_real_container() {
    // Dangling reference.
    let (_t, _ws, diags) = workspace("dangling", &DEPLOYMENT.replace("container: shop.api", "container: shop.nope"));
    let errs = errors(&diags);
    assert_eq!(errs.len(), 1, "{diags:?}");
    assert!(errs[0].message.contains("dangling `container: shop.nope`"), "{}", errs[0].message);
    assert_eq!(errs[0].file, "model/deployment.yaml");

    // Resolves, but to the wrong kind of thing.
    let (_t2, _ws2, diags) = workspace("wrongkind", &DEPLOYMENT.replace("container: shop.api", "container: shop"));
    let errs = errors(&diags);
    assert_eq!(errs.len(), 1, "{diags:?}");
    assert!(errs[0].message.contains("resolves to a system, not a container"), "{}", errs[0].message);

    // Missing entirely.
    let (_t3, _ws3, diags) = workspace("missing", &DEPLOYMENT.replace("api: { container: shop.api }", "api: {}"));
    let errs = errors(&diags);
    assert_eq!(errs.len(), 1, "{diags:?}");
    assert!(errs[0].message.contains("needs `container:`"), "{}", errs[0].message);
}

#[test]
fn a_deployment_file_stands_alone() {
    let (_t, _ws, diags) = workspace("mixed", &format!("system: other\n{DEPLOYMENT}"));
    let errs = errors(&diags);
    assert!(errs.iter().any(|e| e.message.contains("not a mix")), "{diags:?}");
}

/// Deployment elements are ordinary elements (ADR-0018), so the CST-preserving
/// sync engine addresses them like any other — at whatever nesting depth.
#[test]
fn deployment_elements_are_editable_in_place() {
    let t = temp("edit");
    write(&t.dir, "blastradius.yaml", MANIFEST);
    write(&t.dir, "model/shop.yaml", SYSTEM);
    write(&t.dir, "model/deployment.yaml", DEPLOYMENT);

    let mut engine = SyncEngine::open(&t.dir);
    engine
        .apply(Operation::SetField {
            id: "production.eu-west.app-server".into(),
            field: "tech".into(),
            value: "Ubuntu 24.04".into(),
        })
        .expect("set tech on a nested deployment node");
    engine
        .apply(Operation::Rename { id: "production.cdn".into(), name: "Edge CDN".into() })
        .expect("rename a deployment node");

    let text = fs::read_to_string(t.dir.join("model/deployment.yaml")).unwrap();
    assert!(text.contains("tech: Ubuntu 24.04"), "{text}");
    assert!(text.contains("name: Edge CDN"), "{text}");
    // Comments and the rest of the tree survive the splice.
    assert!(text.contains("instances:"), "{text}");
    assert!(text.contains("container: shop.web"), "{text}");

    let (ws, diags) = blastradius_core::load_workspace(&t.dir);
    assert!(!has_errors(&diags), "{diags:?}");
    assert_eq!(ws.elements["production.eu-west.app-server"].tech.as_deref(), Some("Ubuntu 24.04"));
}

/// Creating deployment elements from the canvas, at each nesting position.
#[test]
fn deployment_elements_can_be_created() {
    let t = temp("create");
    write(&t.dir, "blastradius.yaml", MANIFEST);
    write(&t.dir, "model/shop.yaml", SYSTEM);
    write(&t.dir, "model/deployment.yaml", DEPLOYMENT);

    let mut engine = SyncEngine::open(&t.dir);
    engine
        .apply(Operation::Create {
            id: "staging".into(),
            parent: None,
            name: "Staging".into(),
            kind: "environment".into(),
        })
        .expect("create an environment");
    engine
        .apply(Operation::Create {
            id: "db".into(),
            parent: Some("production.eu-west".into()),
            name: "Database Host".into(),
            kind: "deployment-node".into(),
        })
        .expect("create a nested node");

    // An instance cannot hang off another instance — nothing runs on a
    // running container.
    let err = engine
        .apply(Operation::Create {
            id: "nope".into(),
            parent: Some("production.eu-west.app-server.api".into()),
            name: "Nope".into(),
            kind: "deployment-node".into(),
        })
        .unwrap_err();
    assert!(err.contains("not a container instance"), "{err}");

    let (ws, diags) = blastradius_core::load_workspace(&t.dir);
    assert!(!has_errors(&diags), "{diags:?}");
    assert_eq!(ws.elements["staging"].kind, ElementKind::Environment);
    assert_eq!(ws.elements["production.eu-west.db"].kind, ElementKind::DeploymentNode);
    // The rest of the tree is untouched by the splices.
    assert!(ws.elements.contains_key("production.eu-west.app-server.web"));
}

#[test]
fn ld_is_a_valid_view_level_and_junk_is_not() {
    let t = temp("views");
    write(&t.dir, "blastradius.yaml", MANIFEST);
    write(&t.dir, "model/shop.yaml", SYSTEM);
    write(&t.dir, "model/deployment.yaml", DEPLOYMENT);
    write(
        &t.dir,
        "views/production.yaml",
        "view: production\nname: Production\nscope: production\nlevel: LD\nlayout:\n  eu-west: [2, 2]\n",
    );
    let (ws, diags) = blastradius_core::load_workspace(&t.dir);
    assert!(!has_errors(&diags), "{diags:?}");
    assert_eq!(ws.views.len(), 1);
    assert_eq!(ws.views[0].level, "LD");

    // The overview has no scope: its subject is every environment, so pins
    // are absolute ids. This is the one view that may omit `scope:`.
    write(
        &t.dir,
        "views/production.yaml",
        "view: deployment\nname: Deployment\nlevel: LD\nlayout:\n  production: [1, 1]\n",
    );
    let (ws, diags) = blastradius_core::load_workspace(&t.dir);
    assert!(!has_errors(&diags), "{diags:?}");
    assert_eq!(ws.views[0].scope, "");
    assert!(ws.views[0].layout.contains_key("production"));

    // A pin naming nothing is still an error there.
    write(&t.dir, "views/production.yaml", "view: deployment\nlevel: LD\nlayout:\n  nope: [1, 1]\n");
    let (_, diags) = blastradius_core::load_workspace(&t.dir);
    assert!(diags.iter().any(|d| d.message.contains("layout pins unknown element")), "{diags:?}");

    // Every other level still demands one.
    write(&t.dir, "views/production.yaml", "view: v\nlevel: L2\n");
    let (_, diags) = blastradius_core::load_workspace(&t.dir);
    assert!(diags.iter().any(|d| d.message.contains("needs `scope:`")), "{diags:?}");

    // A bad level is an error *and* the view is withheld from the renderer,
    // rather than arriving as a scene it cannot compute.
    write(&t.dir, "views/production.yaml", "view: production\nscope: production\nlevel: L9\n");
    let (ws, diags) = blastradius_core::load_workspace(&t.dir);
    assert!(has_errors(&diags));
    assert!(ws.views.is_empty(), "an unrenderable view should not reach the snapshot");
}

// ---- replicas (ADR-0018 follow-up) -----------------------------------------
// Three identical app servers are one box saying x3, not three ids: modelling
// them as three elements would put three of everything in every relation
// touching them, and the reader learns nothing from the copies.

const REPLICATED: &str = "\
environments:
  production:
    name: Production
    nodes:
      app-server:
        name: App Server
        replicas: 3
        instances:
          api:
            container: shop.api
            replicas: 2
          web: { container: shop.web }
";

#[test]
fn replicas_are_read_on_nodes_and_on_instances() {
    let (_t, ws, diags) = workspace("replicas", REPLICATED);
    assert!(!has_errors(&diags), "{diags:?}");
    let el = |id: &str| ws.elements.get(id).unwrap_or_else(|| panic!("missing {id}"));
    assert_eq!(el("production.app-server").replicas, Some(3));
    assert_eq!(el("production.app-server.api").replicas, Some(2));
    // One of something is the default; nothing to record.
    assert_eq!(el("production.app-server.web").replicas, None);
    assert_eq!(el("production").replicas, None);
}

#[test]
fn replicas_reach_the_snapshot_the_renderers_read() {
    let (t, ws, diags) = workspace("replicas-snap", REPLICATED);
    let snap = blastradius_core::snapshot::snapshot(
        &blastradius_core::vfs::DiskVfs::new(&t.dir),
        &ws,
        &diags,
    );
    let by_id = |id: &str| snap.elements.iter().find(|e| e.id == id).unwrap();
    assert_eq!(by_id("production.app-server").replicas, Some(3));
    assert_eq!(by_id("production.app-server.web").replicas, None);
}

#[test]
fn replicas_zero_is_refused_rather_than_quietly_accepted() {
    let (_t, _ws, diags) = workspace(
        "replicas-zero",
        "environments:\n  production:\n    name: Production\n    nodes:\n      app:\n        name: App\n        replicas: 0\n",
    );
    assert!(
        errors(&diags).iter().any(|d| d.message.contains("replicas: 0")),
        "an element that runs none of itself should be deleted, not modelled: {diags:?}"
    );
}

#[test]
fn a_non_numeric_replicas_is_an_error_with_a_line() {
    let (_t, _ws, diags) = workspace(
        "replicas-bad",
        "environments:\n  production:\n    name: Production\n    nodes:\n      app:\n        name: App\n        replicas: many\n",
    );
    let e = errors(&diags);
    assert!(e.iter().any(|d| d.message.contains("not a whole number")), "{diags:?}");
    assert!(e.iter().all(|d| d.line > 0), "diagnostics must carry a line: {e:?}");
}

// ---- nested display (ADR-0018 follow-up) -----------------------------------
// C4 conventionally draws deployment as boxes inside boxes; this product draws
// one altitude at a time everywhere else and does not want two ways of saying
// "what is inside". So containment is an opt-in per *view*, and only on the
// one level where it is the convention.

#[test]
fn a_deployment_view_can_ask_for_containment() {
    let t = temp("nested-view");
    write(&t.dir, "blastradius.yaml", MANIFEST);
    write(&t.dir, "model/shop.yaml", SYSTEM);
    write(&t.dir, "model/deployment.yaml", DEPLOYMENT);
    write(
        &t.dir,
        "views/prod.yaml",
        "view: prod\nname: Production\nscope: production\nlevel: LD\nnested: true\n",
    );
    let (ws, diags) = blastradius_core::load_workspace(&t.dir);
    assert!(!has_errors(&diags), "{diags:?}");
    let v = ws.views.iter().find(|v| v.id == "prod").expect("view");
    assert!(v.nested, "`nested: true` did not reach the view");
}

#[test]
fn nesting_outside_deployment_is_a_warning_not_a_silent_no_op() {
    let t = temp("nested-wrong-level");
    write(&t.dir, "blastradius.yaml", MANIFEST);
    write(&t.dir, "model/shop.yaml", SYSTEM);
    write(&t.dir, "model/deployment.yaml", DEPLOYMENT);
    write(
        &t.dir,
        "views/containers.yaml",
        "view: containers\nscope: shop\nlevel: L2\nnested: true\n",
    );
    let (ws, diags) = blastradius_core::load_workspace(&t.dir);
    // Still a valid workspace — the key is ignored, not fatal.
    assert!(!has_errors(&diags), "{diags:?}");
    assert!(
        diags.iter().any(|d| d.message.contains("deployment-only")),
        "an ignored key must say so: {diags:?}"
    );
    let v = ws.views.iter().find(|v| v.id == "containers").expect("view");
    assert!(!v.nested, "nesting leaked onto an L2 view");
}
