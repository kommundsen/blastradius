//! Descriptions on the box (spec §4): an element's `description:` is a model
//! field, and `descriptions:` in a view file says which boxes draw it. Per
//! view and off by default, for the same reason groups are — writing a
//! description must not silently reshape every existing diagram.

use blastradius_core::diagnostics::has_errors;
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
    let dir = std::env::temp_dir().join(format!("br-desc-{tag}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    TempDir { dir }
}
fn write(root: &Path, rel: &str, content: &str) {
    let path = root.join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}
fn read(root: &Path, rel: &str) -> String {
    fs::read_to_string(root.join(rel)).unwrap()
}

const MANIFEST: &str = "workspace:\n  name: Shop\n  version: 1\nmodel:\n  include: [model/*.yaml]\nviews:\n  include: [views/*.yaml]\n";

const SYSTEM: &str = "\
system: shop
name: Shop

containers:
  web:
    name: Web
    description: The storefront customers actually see.
  api:
    name: API
    description: Orders, pricing, and the checkout state machine.
";

/// A view file with comments and an aligned trailing comment, so the tests
/// below prove the splice is format-preserving and not a re-serialization.
const VIEW: &str = "\
# The container view — hand-maintained.
view: containers
scope: shop
level: L2

layout:
  web: [4, 2]   # top-left on purpose
";

fn workspace(tag: &str, view: Option<&str>) -> (TempDir, blastradius_core::Workspace) {
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

fn engine(tag: &str, view: Option<&str>) -> (TempDir, SyncEngine) {
    let t = temp(tag);
    write(&t.dir, "blastradius.yaml", MANIFEST);
    write(&t.dir, "model/shop.yaml", SYSTEM);
    if let Some(v) = view {
        write(&t.dir, "views/containers.yaml", v);
    }
    let engine = SyncEngine::open(&t.dir);
    (t, engine)
}

fn show(id: &str, show: bool) -> Operation {
    Operation::ShowDescription {
        view: None,
        level: "L2".to_string(),
        scope: Some("shop".to_string()),
        id: id.to_string(),
        show,
    }
}

#[test]
fn drawing_a_description_is_opt_in_per_view() {
    // The elements carry descriptions either way — the field is the text, not
    // the decision to draw it.
    let (_t, ws) = workspace("default", Some(VIEW));
    assert!(ws.elements["shop.web"].description.is_some());
    assert!(
        ws.views[0].descriptions.is_empty(),
        "a description must not change an existing diagram's shape"
    );

    let (_t2, ws) = workspace("on", Some(&format!("{VIEW}descriptions: [web]\n")));
    assert!(ws.views[0].descriptions.contains("web"));
    assert!(!ws.views[0].descriptions.contains("api"));

    // Scope-relative and absolute keys are both accepted, exactly as pins are.
    let (_t3, ws) = workspace("absolute", Some(&format!("{VIEW}descriptions: [shop.api]\n")));
    assert!(ws.views[0].descriptions.contains("shop.api"));
}

#[test]
fn a_description_for_an_element_the_view_cannot_show_is_an_error() {
    let t = temp("unknown");
    write(&t.dir, "blastradius.yaml", MANIFEST);
    write(&t.dir, "model/shop.yaml", SYSTEM);
    write(&t.dir, "views/containers.yaml", &format!("{VIEW}descriptions: [ghost]\n"));
    let (_ws, diags) = blastradius_core::load_workspace(&t.dir);
    assert!(has_errors(&diags), "a dangling id must not pass validation");
    assert!(
        diags.iter().any(|d| d.message.contains("ghost")),
        "the diagnostic must name the id: {diags:?}"
    );
}

#[test]
fn toggling_writes_the_view_file_and_leaves_the_rest_alone() {
    let (t, mut engine) = engine("toggle", Some(VIEW));

    engine.apply(show("shop.web", true)).expect("show web");
    let after = read(&t.dir, "views/containers.yaml");
    assert!(after.contains("descriptions: [web]"), "{after}");
    // Everything that was already in the file is still exactly as it was.
    assert!(after.starts_with("# The container view — hand-maintained.\n"), "{after}");
    assert!(after.contains("  web: [4, 2]   # top-left on purpose"), "{after}");

    // A second element joins the same list rather than starting another.
    engine.apply(show("shop.api", true)).expect("show api");
    let after = read(&t.dir, "views/containers.yaml");
    assert_eq!(after.matches("descriptions:").count(), 1, "{after}");
    assert!(after.contains("descriptions: [api, web]"), "{after}");

    // Hiding the last one takes the key away rather than leaving `[]` behind.
    engine.apply(show("shop.web", false)).expect("hide web");
    engine.apply(show("shop.api", false)).expect("hide api");
    let after = read(&t.dir, "views/containers.yaml");
    assert!(!after.contains("descriptions"), "{after}");
    assert_eq!(after, VIEW, "the file must come back byte-identical");
}

#[test]
fn toggling_to_the_state_it_is_already_in_is_not_an_edit() {
    let (t, mut engine) = engine("noop", Some(VIEW));
    engine.apply(show("shop.web", true)).unwrap();
    let before = read(&t.dir, "views/containers.yaml");

    let tx = engine.apply(show("shop.web", true)).expect("a redundant toggle is not an error");
    assert!(tx.changes.is_empty(), "nothing to write, so nothing to undo");
    assert_eq!(read(&t.dir, "views/containers.yaml"), before);

    // Hiding what was never shown is the same story, in the other direction.
    let tx = engine.apply(show("shop.api", false)).expect("redundant hide");
    assert!(tx.changes.is_empty());
    assert_eq!(read(&t.dir, "views/containers.yaml"), before);
}

#[test]
fn the_first_toggle_authors_a_view_file_when_there_is_none() {
    // Most workspaces have no view file for most levels; pinning learned to
    // write one, and this has to as well or the toggle silently does nothing.
    let (t, mut engine) = engine("newfile", None);
    engine.apply(show("shop.web", true)).expect("show web");

    let written = read(&t.dir, "views/shop-l2.yaml");
    assert_eq!(written, "view: shop-l2\nscope: shop\nlevel: L2\ndescriptions: [web]\n");

    // And the workspace it just wrote still loads clean.
    let (ws, diags) = blastradius_core::load_workspace(&t.dir);
    assert!(!has_errors(&diags), "{diags:?}");
    assert!(ws.views[0].descriptions.contains("web"));
}

#[test]
fn hiding_with_no_view_file_writes_nothing() {
    let (t, mut engine) = engine("nofile-hide", None);
    let tx = engine.apply(show("shop.web", false)).expect("hiding the already-hidden");
    assert!(tx.changes.is_empty());
    assert!(!t.dir.join("views/shop-l2.yaml").exists(), "no file should have been authored");
}

#[test]
fn a_toggle_is_undoable_like_any_other_edit() {
    let (t, mut engine) = engine("undo", Some(VIEW));
    engine.apply(show("shop.web", true)).unwrap();
    assert!(read(&t.dir, "views/containers.yaml").contains("descriptions: [web]"));

    engine.undo().expect("undo");
    assert_eq!(read(&t.dir, "views/containers.yaml"), VIEW);

    engine.redo().expect("redo");
    assert!(read(&t.dir, "views/containers.yaml").contains("descriptions: [web]"));
}

#[test]
fn the_snapshot_carries_the_list_to_the_renderer() {
    let t = temp("snapshot");
    write(&t.dir, "blastradius.yaml", MANIFEST);
    write(&t.dir, "model/shop.yaml", SYSTEM);
    write(&t.dir, "views/containers.yaml", &format!("{VIEW}descriptions: [web]\n"));
    let (ws, diags) = blastradius_core::load_workspace(&t.dir);
    let snap = blastradius_core::snapshot::snapshot(
        &blastradius_core::vfs::DiskVfs::new(&t.dir),
        &ws,
        &diags,
    );
    let view = snap.views.iter().find(|v| v.id == "containers").expect("the view");
    assert_eq!(view.descriptions, vec!["web".to_string()]);
}
