//! Visual groups (spec §3c): a `group:` label is presentation, not structure.
//! Ids, hierarchy, relations and pins must be untouched by it, and drawing
//! boundaries is opt-in per view.

use blastradius_core::diagnostics::has_errors;
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
    let dir = std::env::temp_dir().join(format!("br-groups-{tag}-{}", std::process::id()));
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
  web:
    name: Web
    group: Storefront
  api:
    name: API
    group: Storefront
    components:
      router: { name: Router, group: Edge }
  ledger:
    name: Ledger
    group: '  '
relations:
  - from: web
    to: api
    label: calls
";

fn load(tag: &str, view: Option<&str>) -> (TempDir, blastradius_core::Workspace) {
    let t = temp(tag);
    write(&t.dir, "blastradius.yaml", MANIFEST);
    write(&t.dir, "model/shop.yaml", SYSTEM);
    if let Some(v) = view {
        write(&t.dir, "views/containers.yaml", v);
    }
    let (ws, diags) = blastradius_core::load_workspace(&t.dir);
    assert!(!has_errors(&diags), "{diags:?}");
    (t, ws)
}

#[test]
fn a_group_labels_without_restructuring() {
    let (_t, ws) = load("parse", None);

    let group = |id: &str| ws.elements[id].group.clone();
    assert_eq!(group("shop.web").as_deref(), Some("Storefront"));
    assert_eq!(group("shop.api").as_deref(), Some("Storefront"));
    // Groups are per-scope: a component's group is its own, unrelated to its
    // container's.
    assert_eq!(group("shop.api.router").as_deref(), Some("Edge"));
    // Whitespace-only is no group at all — an unnamed boundary is not a thing.
    assert_eq!(group("shop.ledger"), None);

    // The point of the decision: nothing structural moved. Ids keep their
    // shape, parents are unchanged, and the relation still resolves.
    assert!(ws.elements.contains_key("shop.web"), "ids must not gain a group segment");
    assert!(ws.elements.keys().all(|k| !k.contains("Storefront")));
    assert_eq!(ws.elements["shop.api.router"].id, "shop.api.router");
    assert!(ws
        .resolved_relations()
        .iter()
        .any(|(f, t, _)| f == "shop.web" && t == "shop.api"));
}

#[test]
fn drawing_boundaries_is_opt_in() {
    let base = "view: containers\nscope: shop\nlevel: L2\n";
    let (_t, ws) = load("default", Some(base));
    assert!(!ws.views[0].show_groups, "grouping must not change existing diagrams");

    let (_t2, ws) = load("on", Some(&format!("{base}show-groups: true\n")));
    assert!(ws.views[0].show_groups);

    let (_t3, ws) = load("off", Some(&format!("{base}show-groups: false\n")));
    assert!(!ws.views[0].show_groups);
}
