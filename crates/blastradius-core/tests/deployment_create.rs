//! Creating and re-pointing a container instance (0.11.0, found in use).
//!
//! Reported from a real modelling session driving the MCP server: "the create
//! op can't set a container-instance's `container:` reference, so I'll write
//! this one by hand". It was worse than that — `container:` is required, so the
//! invalidation guard refused *every* create of an instance, and `set-field`
//! refused the field too. A whole element kind was unreachable through any
//! operation, in the app and over MCP alike.

use blastradius_core::sync::{Operation, SyncEngine};
use std::fs;
use std::path::{Path, PathBuf};

struct TempWs(PathBuf);
impl Drop for TempWs {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn write(root: &Path, rel: &str, content: &str) {
    let path = root.join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}

fn read(root: &Path, rel: &str) -> String {
    fs::read_to_string(root.join(rel)).unwrap()
}

const MANIFEST: &str =
    "workspace:\n  name: T\n  version: 1\nmodel:\n  include: [model/*.yaml]\nviews:\n  include: [views/*.yaml]\n";

const SHOP: &str = "\
system: shop
name: Shop

containers:
  web: { name: Web App }
  api: { name: API }
  db:  { name: Database, components: { pool: { name: Pool } } }
";

const DEPLOY: &str = "\
environments:
  prod:
    name: Production
    nodes:
      box:
        name: App Server
        instances:
          web: { container: shop.web }
";

fn create(parent: &str, id: &str, name: &str, kind: &str, container: Option<&str>) -> Operation {
    Operation::Create {
        parent: Some(parent.into()),
        id: id.into(),
        name: name.into(),
        kind: kind.into(),
        container: container.map(str::to_string),
    }
}

fn setup(name: &str) -> (TempWs, SyncEngine) {
    let dir = std::env::temp_dir().join(format!("br-depcreate-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let t = TempWs(dir);
    write(&t.0, "blastradius.yaml", MANIFEST);
    write(&t.0, "model/shop.yaml", SHOP);
    write(&t.0, "model/deployment.yaml", DEPLOY);
    let e = SyncEngine::open(&t.0);
    assert!(e.stale.is_empty(), "{:?}", e.diagnostics);
    (t, e)
}

#[test]
fn a_container_instance_can_be_created() {
    let (t, mut e) = setup("create");
    e.apply(create("prod.box", "api", "API", "container-instance", Some("shop.api"))).unwrap();
    assert_eq!(
        read(&t.0, "model/deployment.yaml"),
        DEPLOY.replace(
            "          web: { container: shop.web }\n",
            "          web: { container: shop.web }\n          api:\n            container: shop.api\n"
        ),
    );
    // And the workspace it produced actually loads, which is the whole thing
    // that was impossible before: the guard refused every attempt.
    let reopened = SyncEngine::open(&t.0);
    assert!(reopened.diagnostics.is_empty(), "{:?}", reopened.diagnostics);
    assert!(reopened.model.elements.contains_key("prod.box.api"));
}

#[test]
fn the_name_is_not_written_when_the_container_already_says_it() {
    let (t, mut e) = setup("inherited");
    // An unnamed instance takes its container's name (spec §3b), so writing
    // that same name states nothing — the rule every other default follows.
    e.apply(create("prod.box", "api", "API", "container-instance", Some("shop.api"))).unwrap();
    let text = read(&t.0, "model/deployment.yaml");
    assert!(!text.contains("name: API"), "{text}");
    assert_eq!(SyncEngine::open(&t.0).model.elements["prod.box.api"].name, "API");
}

#[test]
fn a_name_of_its_own_is_written() {
    let (t, mut e) = setup("named");
    e.apply(create("prod.box", "api", "Public API", "container-instance", Some("shop.api"))).unwrap();
    assert!(read(&t.0, "model/deployment.yaml").contains("name: Public API"));
}

#[test]
fn an_instance_without_a_container_is_refused_by_name() {
    let (_t, mut e) = setup("nocontainer");
    let err = e.apply(create("prod.box", "api", "API", "container-instance", None)).unwrap_err();
    // The old failure said the file would not load. This says what to pass.
    assert!(err.contains("needs `container`"), "{err}");
}

#[test]
fn the_container_has_to_be_one() {
    let (_t, mut e) = setup("notacontainer");
    let err = e
        .apply(create("prod.box", "p", "P", "container-instance", Some("shop.db.pool")))
        .unwrap_err();
    assert!(err.contains("is a component"), "{err}");

    let err = e
        .apply(create("prod.box", "g", "G", "container-instance", Some("shop.ghost")))
        .unwrap_err();
    assert!(err.contains("unknown container"), "{err}");
}

#[test]
fn container_is_refused_on_a_kind_that_does_not_run_one() {
    let (_t, mut e) = setup("wrongkind");
    let err = e
        .apply(create("prod", "box2", "Box 2", "deployment-node", Some("shop.api")))
        .unwrap_err();
    assert!(err.contains("is not one"), "{err}");
}

#[test]
fn an_instance_is_re_pointed_at_another_container() {
    let (t, mut e) = setup("repoint");
    e.apply(Operation::SetField {
        id: "prod.box.web".into(),
        field: "container".into(),
        value: "shop.api".into(),
    })
    .unwrap();
    assert_eq!(
        read(&t.0, "model/deployment.yaml"),
        DEPLOY.replace("container: shop.web", "container: shop.api")
    );
}

#[test]
fn the_container_reference_is_never_cleared() {
    let (_t, mut e) = setup("noclear");
    // Emptying every other scalar removes the key; this one would delete the
    // element instead, because the instance does not parse without it.
    let err = e
        .apply(Operation::SetField {
            id: "prod.box.web".into(),
            field: "container".into(),
            value: String::new(),
        })
        .unwrap_err();
    assert!(err.contains("re-point it, or delete it"), "{err}");
}

#[test]
fn the_snapshot_says_which_container_an_instance_runs() {
    let (t, _e) = setup("snapshot");
    // It never did, so no surface could show the field and none could edit it —
    // half of why this went unnoticed until someone modelled a deployment.
    let vfs = blastradius_core::vfs::DiskVfs::new(&t.0);
    let (ws, diags) = blastradius_core::load_workspace(&t.0);
    let snap = blastradius_core::snapshot::snapshot(&vfs, &ws, &diags);
    let inst = snap.elements.iter().find(|e| e.id == "prod.box.web").unwrap();
    assert_eq!(inst.container.as_deref(), Some("shop.web"));
    let node = snap.elements.iter().find(|e| e.id == "prod.box").unwrap();
    assert_eq!(node.container, None, "only an instance runs a container");
}
