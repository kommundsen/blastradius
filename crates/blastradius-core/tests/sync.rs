//! Sync engine tests (ADR-0008, spec/sync-engine.md), ending in the Phase 3
//! exit criterion: a scripted torture session — interleaved canvas operations,
//! external edits, and malformed intermediate states — that must end
//! byte-identical to the expected files, comments and formatting intact.

use blastradius_core::sync::{Operation, SyncEngine};
use std::fs;
use std::path::{Path, PathBuf};

struct TempWs {
    dir: PathBuf,
}

impl Drop for TempWs {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

fn temp_ws(name: &str) -> TempWs {
    let dir = std::env::temp_dir().join(format!("blastradius-sync-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
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

/// Deliberately quirky formatting: comments, blank lines, flow mappings,
/// an aligned inline comment — all of it must survive every edit untouched.
const SHOP: &str = "\
# The shop system — hand-maintained, mind the comments.
system: shop
name: Shop

containers:
  web:
    name: Web App
    tech: React   # SPA
  api:
    name: API
    tech: Go
  db: { name: Database, tech: Postgres }

# relations live at the bottom
relations:
  - from: web
    to: api
    label: calls
    protocol: HTTPS
  - from: api
    to: db          # keep this comment
    label: reads
";

fn setup(name: &str) -> (TempWs, SyncEngine) {
    let t = temp_ws(name);
    write(&t.dir, "blastradius.yaml", MANIFEST);
    write(&t.dir, "model/shop.yaml", SHOP);
    let engine = SyncEngine::open(&t.dir);
    assert!(engine.stale.is_empty(), "{:?}", engine.diagnostics);
    (t, engine)
}

#[test]
fn rename_touches_exactly_one_value() {
    let (t, mut e) = setup("rename");
    e.apply(Operation::Rename { id: "shop.web".into(), name: "Storefront".into() }).unwrap();
    let text = read(&t.dir, "model/shop.yaml");
    assert_eq!(text, SHOP.replace("name: Web App", "name: Storefront"),
        "everything except the one value is byte-identical");
}

#[test]
fn rename_inside_flow_mapping() {
    let (t, mut e) = setup("flow");
    e.apply(Operation::Rename { id: "shop.db".into(), name: "Main DB".into() }).unwrap();
    let text = read(&t.dir, "model/shop.yaml");
    assert!(text.contains("db: { name: Main DB, tech: Postgres }"), "{text}");
}

#[test]
fn rename_preserves_inline_comment() {
    let (t, mut e) = setup("comment");
    // tech is not name — but set_field on name of api leaves web's `# SPA` alone,
    // and a a second rename on web keeps its own comment intact
    e.apply(Operation::Rename { id: "shop.api".into(), name: "Payments API".into() }).unwrap();
    let text = read(&t.dir, "model/shop.yaml");
    assert!(text.contains("tech: React   # SPA"), "inline comment survives: {text}");
    assert!(text.contains("# relations live at the bottom"), "section comment survives");
}

#[test]
fn create_and_delete_container_with_cascade() {
    let (t, mut e) = setup("cascade");
    e.apply(Operation::Create {
        parent: Some("shop".into()), id: "cache".into(),
        name: "Cache".into(), kind: "container".into(),
    }).unwrap();
    assert!(e.model.elements.contains_key("shop.cache"));
    e.apply(Operation::AddRelation {
        from: "shop.api".into(), to: "shop.cache".into(),
        label: Some("reads".into()), protocol: Some("RESP".into()),
    }).unwrap();
    assert!(read(&t.dir, "model/shop.yaml").contains("- from: api\n    to: cache"));

    // delete cascades: the element AND the relation vanish in one transaction
    let tx = e.apply(Operation::Delete { id: "shop.cache".into() }).unwrap();
    assert_eq!(tx.changes.len(), 1, "one file touched");
    let text = read(&t.dir, "model/shop.yaml");
    assert!(!text.contains("cache"), "{text}");
    // the untouched relation with its comment survives
    assert!(text.contains("to: db          # keep this comment"));
}

#[test]
fn pin_creates_and_updates_view_file() {
    let (t, mut e) = setup("pin");
    // no view file exists: pin creates one
    e.apply(Operation::Pin {
        view: None, level: "L2".into(), scope: Some("shop".into()),
        id: "shop.web".into(), x: 4, y: 2,
    }).unwrap();
    let rel = "views/shop-l2.yaml";
    let text = read(&t.dir, rel);
    assert!(text.contains("layout:\n  web: [4, 2]"), "{text}");

    // second pin upserts into the same file
    e.apply(Operation::Pin {
        view: None, level: "L2".into(), scope: Some("shop".into()),
        id: "shop.api".into(), x: 8, y: 2,
    }).unwrap();
    let text = read(&t.dir, rel);
    assert!(text.contains("web: [4, 2]") && text.contains("api: [8, 2]"), "{text}");

    // re-pin replaces in place
    e.apply(Operation::Pin {
        view: None, level: "L2".into(), scope: Some("shop".into()),
        id: "shop.web".into(), x: 5, y: 3,
    }).unwrap();
    let text = read(&t.dir, rel);
    assert!(text.contains("web: [5, 3]") && !text.contains("[4, 2]"), "{text}");
}

/// A hand-written view file, commented like a real one: unpinning must leave
/// everything it did not remove byte-identical.
const SHOP_VIEW: &str = "\
# L2 — the shop's containers.
view: shop-l2
scope: shop
level: L2
layout:
  web: [4, 2]     # the front door
  api: [8, 2]
  db: [12, 2]
include-context: true
";

#[test]
fn unpin_removes_one_pin_and_leaves_the_rest_alone() {
    let (t, mut e) = setup("unpin-one");
    write(&t.dir, "views/shop-l2.yaml", SHOP_VIEW);
    assert!(e.external_scan());
    e.apply(Operation::Unpin {
        view: None, level: "L2".into(), scope: Some("shop".into()),
        id: Some("shop.api".into()),
    })
    .unwrap();
    let text = read(&t.dir, "views/shop-l2.yaml");
    assert_eq!(text, SHOP_VIEW.replace("  api: [8, 2]\n", ""), "only that line goes");
}

/// The last pin takes `layout:` with it: a key standing over nothing is worse
/// than no key, which is the rule `descriptions:` already follows.
#[test]
fn unpinning_everything_removes_the_layout_key() {
    let (t, mut e) = setup("unpin-all");
    write(&t.dir, "views/shop-l2.yaml", SHOP_VIEW);
    assert!(e.external_scan());
    let tx = e
        .apply(Operation::Unpin {
            view: None, level: "L2".into(), scope: Some("shop".into()), id: None,
        })
        .unwrap();
    assert_eq!(tx.changes.len(), 1, "one file touched");
    assert_eq!(tx.label, "reset the L2 layout of shop");
    let text = read(&t.dir, "views/shop-l2.yaml");
    assert!(!text.contains("layout"), "{text}");
    assert_eq!(
        text,
        "# L2 — the shop's containers.\nview: shop-l2\nscope: shop\nlevel: L2\ninclude-context: true\n",
        "the view survives; only its pins do not"
    );
    // The view still parses, and is now fully auto-laid-out.
    assert!(e.model.views.iter().any(|v| v.id == "shop-l2" && v.layout.is_empty()));

    // And one undo brings the whole arrangement back.
    e.undo().unwrap();
    assert_eq!(read(&t.dir, "views/shop-l2.yaml"), SHOP_VIEW);
}

/// Removing the pins one at a time reaches the same place as removing them
/// all at once — the last one out turns off the light.
#[test]
fn the_last_single_unpin_also_takes_the_key() {
    let (t, mut e) = setup("unpin-last");
    write(&t.dir, "views/shop-l2.yaml", SHOP_VIEW);
    assert!(e.external_scan());
    for id in ["shop.web", "shop.api", "shop.db"] {
        e.apply(Operation::Unpin {
            view: None, level: "L2".into(), scope: Some("shop".into()), id: Some(id.into()),
        })
        .unwrap();
    }
    let text = read(&t.dir, "views/shop-l2.yaml");
    assert!(!text.contains("layout"), "{text}");
}

/// Nothing is pinned in a view with no file, so there is nothing to write —
/// and in particular no file to author, which is where this differs from pin.
#[test]
fn unpinning_what_is_not_pinned_is_not_an_edit() {
    let (t, mut e) = setup("unpin-nothing");
    let tx = e
        .apply(Operation::Unpin {
            view: None, level: "L2".into(), scope: Some("shop".into()), id: None,
        })
        .unwrap();
    assert!(tx.changes.is_empty());
    assert!(!t.dir.join("views/shop-l2.yaml").exists(), "no view file authored");
    assert_eq!(e.undo().unwrap(), None, "and no empty undo entry on the stack");

    // Same for an element the view does not pin.
    write(&t.dir, "views/shop-l2.yaml", SHOP_VIEW);
    assert!(e.external_scan());
    let tx = e
        .apply(Operation::Unpin {
            view: None, level: "L2".into(), scope: Some("shop".into()),
            id: Some("shop.checkout".into()),
        })
        .unwrap();
    assert!(tx.changes.is_empty());
    assert_eq!(read(&t.dir, "views/shop-l2.yaml"), SHOP_VIEW);
}

#[test]
fn stale_blocks_operations_and_recovers() {
    let (t, mut e) = setup("stale");
    // external malformed write
    write(&t.dir, "model/shop.yaml", "system: shop\n   bad: indent\n  worse:\n");
    assert!(e.external_scan(), "change detected");
    assert!(!e.stale.is_empty());
    // canvas is read-only while stale (spec)
    let err = e.apply(Operation::Rename { id: "shop.web".into(), name: "X".into() }).unwrap_err();
    assert!(err.contains("stale"), "{err}");
    // the last valid model is still served
    assert!(e.model.elements.contains_key("shop.web"));
    // external fix
    write(&t.dir, "model/shop.yaml", SHOP);
    assert!(e.external_scan());
    assert!(e.stale.is_empty());
    e.apply(Operation::Rename { id: "shop.web".into(), name: "X".into() }).unwrap();
}

#[test]
fn race_abort_never_merges() {
    let (t, mut e) = setup("race");
    // disk changes under the engine without a watcher event reaching it
    let sneaky = SHOP.replace("tech: Go", "tech: Rust");
    write(&t.dir, "model/shop.yaml", &sneaky);
    let err = e.apply(Operation::Rename { id: "shop.web".into(), name: "X".into() }).unwrap_err();
    assert!(err.contains("changed on disk"), "{err}");
    // nothing was written: the sneaky edit is intact
    assert_eq!(read(&t.dir, "model/shop.yaml"), sneaky);
}

#[test]
fn shared_undo_spans_ops_and_external_edits() {
    let (t, mut e) = setup("undo");
    e.apply(Operation::Rename { id: "shop.web".into(), name: "One".into() }).unwrap();
    // external edit enters history
    let ext = read(&t.dir, "model/shop.yaml").replace("tech: Go", "tech: Rust");
    write(&t.dir, "model/shop.yaml", &ext);
    assert!(e.external_scan());
    e.apply(Operation::Rename { id: "shop.api".into(), name: "Two".into() }).unwrap();

    // undo op -> undo external -> undo op, exactly like a text editor
    assert!(e.undo().unwrap().unwrap().contains("rename shop.api"));
    assert!(!read(&t.dir, "model/shop.yaml").contains("Two"));
    assert!(e.undo().unwrap().unwrap().contains("external change"));
    assert!(read(&t.dir, "model/shop.yaml").contains("tech: Go"), "external edit reverted");
    assert!(e.undo().unwrap().unwrap().contains("rename shop.web"));
    assert_eq!(read(&t.dir, "model/shop.yaml"), SHOP, "back to the original bytes");

    // redo forward again
    assert!(e.redo().unwrap().is_some());
    assert!(read(&t.dir, "model/shop.yaml").contains("One"));
}

#[test]
fn operation_that_would_invalidate_is_refused() {
    let (_t, mut e) = setup("invalid");
    let err = e.apply(Operation::Create {
        parent: Some("shop".into()), id: "web".into(), // duplicate id
        name: "Dup".into(), kind: "container".into(),
    }).unwrap_err();
    assert!(err.contains("already exists"), "{err}");
    // and a relation to a missing element fails candidate validation
    let err = e.apply(Operation::AddRelation {
        from: "shop.web".into(), to: "shop.ghost".into(), label: None, protocol: None,
    }).unwrap_err();
    assert!(err.contains("invalidate") || err.contains("dangling"), "{err}");
}

#[test]
fn duplicate_yaml_key_reports_exact_line() {
    let t = temp_ws("dupkey");
    write(&t.dir, "blastradius.yaml", MANIFEST);
    write(
        &t.dir,
        "model/shop.yaml",
        "system: shop\ncontainers:\n  web:\n    name: A\n  web:\n    name: B\n",
    );
    let (_, diags) = blastradius_core::load_workspace(&t.dir);
    let dup = diags
        .iter()
        .find(|d| d.message.contains("malformed YAML"))
        .expect("duplicate key is a parse error");
    assert_eq!(dup.line, 5, "the second `web:` is on line 5 — the carried Phase 3 requirement");
}

/// THE EXIT CRITERION (docs/roadmap.md Phase 3): a scripted torture session.
#[test]
fn torture_session_ends_byte_identical() {
    let (t, mut e) = setup("torture");

    // 1. canvas: pin two nodes (creates the view file)
    e.apply(Operation::Pin { view: None, level: "L2".into(), scope: Some("shop".into()), id: "shop.web".into(), x: 2, y: 1 }).unwrap();
    e.apply(Operation::Pin { view: None, level: "L2".into(), scope: Some("shop".into()), id: "shop.api".into(), x: 8, y: 1 }).unwrap();

    // 2. external: the user adds a comment and a container in their editor
    let external1 = read(&t.dir, "model/shop.yaml").replace(
        "  db: { name: Database, tech: Postgres }",
        "  db: { name: Database, tech: Postgres }\n  # added by hand:\n  worker:\n    name: Worker",
    );
    write(&t.dir, "model/shop.yaml", &external1);
    assert!(e.external_scan());
    assert!(e.model.elements.contains_key("shop.worker"));

    // 3. canvas: rename + add a relation to the hand-added element
    e.apply(Operation::Rename { id: "shop.api".into(), name: "Payments API".into() }).unwrap();
    e.apply(Operation::AddRelation { from: "shop.api".into(), to: "shop.worker".into(), label: Some("enqueues".into()), protocol: None }).unwrap();

    // 4. external: malformed intermediate state (user mid-edit)
    let good = read(&t.dir, "model/shop.yaml");
    write(&t.dir, "model/shop.yaml", &format!("{good}  broken: [unclosed\n"));
    assert!(e.external_scan());
    assert!(!e.stale.is_empty(), "malformed state is stale");
    assert!(e.apply(Operation::Rename { id: "shop.web".into(), name: "Nope".into() }).is_err());

    // 5. external: the user finishes their edit (removes the breakage)
    write(&t.dir, "model/shop.yaml", &good);
    assert!(e.external_scan());
    assert!(e.stale.is_empty());

    // 6. canvas: delete the db container (cascades its relation), then undo it
    e.apply(Operation::Delete { id: "shop.db".into() }).unwrap();
    assert!(!read(&t.dir, "model/shop.yaml").contains("db:"));
    e.undo().unwrap();

    // 7. canvas: set a protocol on the new relation
    e.apply(Operation::SetRelationField { from: "shop.api".into(), to: "shop.worker".into(), label: Some("enqueues".into()), field: "protocol".into(), value: "AMQP".into() }).unwrap();

    // ---- expected end state, byte for byte ---------------------------------
    let expected_model = "\
# The shop system — hand-maintained, mind the comments.
system: shop
name: Shop

containers:
  web:
    name: Web App
    tech: React   # SPA
  api:
    name: Payments API
    tech: Go
  db: { name: Database, tech: Postgres }
  # added by hand:
  worker:
    name: Worker

# relations live at the bottom
relations:
  - from: web
    to: api
    label: calls
    protocol: HTTPS
  - from: api
    to: db          # keep this comment
    label: reads
  - from: api
    to: worker
    label: enqueues
    protocol: AMQP
";
    let expected_view = "\
view: shop-l2
scope: shop
level: L2
layout:
  web: [2, 1]
  api: [8, 1]
";
    assert_eq!(read(&t.dir, "model/shop.yaml"), expected_model, "model file byte-identical");
    assert_eq!(read(&t.dir, "views/shop-l2.yaml"), expected_view, "view file byte-identical");

    // and the workspace is valid at the end
    let (_, diags) = blastradius_core::load_workspace(&t.dir);
    assert!(!blastradius_core::diagnostics::has_errors(&diags));
}

#[test]
fn rename_a_system_touches_the_root_mapping() {
    // regression: system rename went through set_field with an empty chain,
    // which errored "[] not found" — masked by the frontend mock until the
    // MCP work exercised the real path
    let (t, mut e) = setup("sys-rename");
    e.apply(Operation::Rename { id: "shop".into(), name: "Shop 2".into() }).unwrap();
    let text = read(&t.dir, "model/shop.yaml");
    assert!(text.contains("name: Shop 2"), "{text}");
    assert!(text.contains("# The shop system — hand-maintained, mind the comments."),
        "comments survive: {text}");
    e.undo().unwrap();
    assert_eq!(read(&t.dir, "model/shop.yaml"), SHOP);
}

#[test]
fn set_field_writes_description_and_tech() {
    let (t, mut e) = setup("set-field");
    // insert a missing field on a container...
    e.apply(Operation::SetField {
        id: "shop.web".into(), field: "description".into(),
        value: "The storefront SPA".into(),
    }).unwrap();
    // ...replace an existing one, preserving the aligned inline comment
    e.apply(Operation::SetField {
        id: "shop.web".into(), field: "tech".into(), value: "Preact".into(),
    }).unwrap();
    let text = read(&t.dir, "model/shop.yaml");
    assert!(text.contains("description: The storefront SPA"), "{text}");
    assert!(text.contains("tech: Preact # SPA"), "comment survives: {text}");
    // whitelist enforced
    let err = e.apply(Operation::SetField {
        id: "shop.web".into(), field: "id".into(), value: "nope".into(),
    }).unwrap_err();
    assert!(err.contains("not editable"), "{err}");
    // description on a *system* lands in the root header block
    e.apply(Operation::SetField {
        id: "shop".into(), field: "description".into(), value: "Sells things".into(),
    }).unwrap();
    let text = read(&t.dir, "model/shop.yaml");
    assert!(text.contains("name: Shop\ndescription: Sells things"), "{text}");
}
