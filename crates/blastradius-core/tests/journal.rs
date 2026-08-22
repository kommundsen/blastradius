//! Phase 5 sync-engine debts: journal crash recovery (replay across restarts,
//! torn-write roll-forward) and granular staleness (a stale views file
//! disables only pinning; model semantics keep flowing).

use blastradius_core::sync::{journal_path, Operation, SyncEngine};
use std::fs;
use std::path::{Path, PathBuf};

struct TempWs {
    dir: PathBuf,
}

impl Drop for TempWs {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
        if let Some(j) = journal_path(&self.dir) {
            let _ = fs::remove_file(j);
        }
    }
}

fn temp_ws(name: &str) -> TempWs {
    let dir =
        std::env::temp_dir().join(format!("blastradius-journal-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    // a stale journal from a previous run must never leak into a fresh test
    if let Some(j) = journal_path(&dir) {
        let _ = fs::remove_file(j);
    }
    TempWs { dir }
}

fn write(root: &Path, rel: &str, content: &str) {
    let path = root.join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}

fn read(root: &Path, rel: &str) -> String {
    fs::read_to_string(root.join(rel)).unwrap()
}

const MANIFEST: &str = "workspace:\n  name: T\n  version: 1\nmodel:\n  include: [model/*.yaml]\nviews:\n  include: [views/*.yaml]\n";
const SHOP: &str = "system: shop\nname: Shop\n\ncontainers:\n  web:\n    name: Web App\n  api:\n    name: API\n";
const VIEW: &str = "view: containers\nscope: shop\nlevel: L2\nlayout:\n  web: [2, 2]\n";

fn setup(name: &str) -> (TempWs, SyncEngine) {
    let t = temp_ws(name);
    write(&t.dir, "workspace.yaml", MANIFEST);
    write(&t.dir, "model/shop.yaml", SHOP);
    write(&t.dir, "views/containers.yaml", VIEW);
    let engine = SyncEngine::open(&t.dir);
    assert!(engine.stale.is_empty(), "{:?}", engine.diagnostics);
    (t, engine)
}

// ---- journal crash recovery -------------------------------------------------

#[test]
fn undo_survives_a_restart() {
    let (t, mut e) = setup("restart");
    e.apply(Operation::Rename { id: "shop.web".into(), name: "Storefront".into() }).unwrap();
    let renamed = read(&t.dir, "model/shop.yaml");
    drop(e);

    let mut e2 = SyncEngine::open(&t.dir);
    let (labels, cursor) = e2.history_labels();
    assert_eq!(cursor, 1, "history replayed from the journal: {labels:?}");
    assert!(e2.undo().unwrap().is_some());
    assert_eq!(read(&t.dir, "model/shop.yaml"), SHOP, "undo across restart is byte-exact");
    // and redo comes back too, after yet another restart
    drop(e2);
    let mut e3 = SyncEngine::open(&t.dir);
    assert!(e3.redo().unwrap().is_some());
    assert_eq!(read(&t.dir, "model/shop.yaml"), renamed);
}

#[test]
fn torn_write_rolls_forward_on_open() {
    let (t, mut e) = setup("torn");
    e.apply(Operation::Rename { id: "shop.web".into(), name: "Storefront".into() }).unwrap();
    let before = read(&t.dir, "model/shop.yaml");
    drop(e);

    // simulate a crash after the intent hit the journal but before the write
    // reached disk: append an uncommitted intent; leave the file at `before`
    let after = before.replace("Storefront", "Shopfront");
    let tx = serde_json::json!({
        "event": "intent",
        "tx": {
            "label": "rename shop.web to \"Shopfront\"",
            "source": "canvas",
            "changes": [{ "rel": "model/shop.yaml", "before": before, "after": after }],
        }
    });
    let journal = journal_path(&t.dir).unwrap();
    let mut log = fs::read_to_string(&journal).unwrap();
    log.push_str(&tx.to_string());
    log.push('\n');
    fs::write(&journal, log).unwrap();

    let mut e2 = SyncEngine::open(&t.dir);
    assert_eq!(read(&t.dir, "model/shop.yaml"), after, "torn transaction completed");
    let (_, cursor) = e2.history_labels();
    assert_eq!(cursor, 2, "the rolled-forward transaction is undoable");
    assert!(e2.undo().unwrap().is_some());
    assert_eq!(read(&t.dir, "model/shop.yaml"), before);
}

#[test]
fn external_edits_while_closed_discard_the_journal() {
    let (t, mut e) = setup("discard");
    e.apply(Operation::Rename { id: "shop.web".into(), name: "Storefront".into() }).unwrap();
    drop(e);
    // the user edited the file with the app closed — recovery must not guess
    write(&t.dir, "model/shop.yaml", &read(&t.dir, "model/shop.yaml").replace("API", "Backend"));

    let mut e2 = SyncEngine::open(&t.dir);
    let (labels, cursor) = e2.history_labels();
    assert_eq!((labels.len(), cursor), (0, 0), "no history adopted: {labels:?}");
    assert!(e2.undo().unwrap().is_none());
    assert!(read(&t.dir, "model/shop.yaml").contains("Backend"), "files untouched");
}

// ---- granular staleness -----------------------------------------------------

#[test]
fn stale_view_file_disables_only_pinning() {
    let (t, mut e) = setup("granular");
    write(&t.dir, "views/containers.yaml", "view: containers\n  broken: [indent\n");
    assert!(e.external_scan());
    assert_eq!(e.stale.len(), 1, "{:?}", e.diagnostics);
    assert!(e.stale_model().is_empty(), "a views file is not a model file");
    assert_eq!(e.stale_view_ids(), vec!["containers".to_string()]);

    // semantics still editable
    e.apply(Operation::Rename { id: "shop.web".into(), name: "Storefront".into() })
        .expect("model edits flow while a views file is stale");

    // pinning into the stale view is refused
    let err = e
        .apply(Operation::Pin {
            view: Some("containers".into()),
            level: "L2".into(),
            scope: Some("shop".into()),
            id: "shop.api".into(),
            x: 4,
            y: 4,
        })
        .unwrap_err();
    assert!(err.contains("pinning is disabled"), "{err}");

    // the view's last-known pins are retained while its file is broken
    assert!(e.model.views.iter().any(|v| v.id == "containers"), "view kept from last valid parse");

    // external fix: pinning works again
    write(&t.dir, "views/containers.yaml", VIEW);
    assert!(e.external_scan());
    assert!(e.stale.is_empty());
    e.apply(Operation::Pin {
        view: Some("containers".into()),
        level: "L2".into(),
        scope: Some("shop".into()),
        id: "shop.api".into(),
        x: 4,
        y: 4,
    })
    .unwrap();
}

#[test]
fn stale_model_file_still_blocks_everything() {
    let (t, mut e) = setup("model-stale");
    write(&t.dir, "model/shop.yaml", "system: shop\n   bad: indent\n  worse:\n");
    assert!(e.external_scan());
    assert!(!e.stale_model().is_empty());
    let err =
        e.apply(Operation::Rename { id: "shop.web".into(), name: "X".into() }).unwrap_err();
    assert!(err.contains("stale"), "{err}");
}

#[test]
fn model_semantics_flow_while_view_is_stale() {
    let (t, mut e) = setup("flow");
    write(&t.dir, "views/containers.yaml", "view: containers\n  broken: [indent\n");
    assert!(e.external_scan());
    // an external *model* edit must be adopted even though a views file is stale
    write(&t.dir, "model/shop.yaml", &SHOP.replace("Web App", "Webshop"));
    assert!(e.external_scan());
    assert_eq!(
        e.model.elements.get("shop.web").map(|el| el.name.as_str()),
        Some("Webshop"),
        "granular staleness: view breakage must not freeze the model"
    );
}
