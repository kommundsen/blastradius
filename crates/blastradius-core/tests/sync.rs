//! Sync engine tests (ADR-0008, spec/sync-engine.md), ending in the Phase 3
//! exit criterion: a scripted torture session — interleaved canvas operations,
//! external edits, and malformed intermediate states — that must end
//! byte-identical to the expected files, comments and formatting intact.

use blastradius_core::sync::{Operation, SourceInput, SyncEngine};
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
        name: "Cache".into(), kind: "container".into(), container: None,
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

/// A workspace with the shapes `SHOP` has not got: a component to introspect,
/// and a deployment node to count. Two files, so the two halves stay legible.
const DEPOT: &str = "\
system: depot
name: Depot

containers:
  api:
    name: API
    tech: Rust
    components:
      router:
        name: Router
        tech: axum
      store:
        name: Store
";

const DEPOT_DEPLOY: &str = "\
environments:
  prod:
    name: Production
    nodes:
      web-tier:
        name: Web Tier
        instances:
          api: { container: depot.api }
";

fn depot(name: &str) -> (TempWs, SyncEngine) {
    let t = temp_ws(name);
    write(&t.dir, "blastradius.yaml", MANIFEST);
    write(&t.dir, "model/depot.yaml", DEPOT);
    write(&t.dir, "model/deployment.yaml", DEPOT_DEPLOY);
    let engine = SyncEngine::open(&t.dir);
    assert!(engine.stale.is_empty(), "{:?}", engine.diagnostics);
    (t, engine)
}

fn set(e: &mut SyncEngine, id: &str, field: &str, value: &str) -> Result<(), String> {
    e.apply(Operation::SetField { id: id.into(), field: field.into(), value: value.into() }).map(|_| ())
}

#[test]
fn the_whole_element_is_writable_not_just_its_name() {
    let (t, mut e) = setup("fields");
    set(&mut e, "shop.web", "tech", "Preact").unwrap();
    // §3c grouping had no operation at all before 0.9.0: a group could only be
    // hand-written or imported from a Structurizr workspace that had one.
    set(&mut e, "shop.web", "group", "Storefront").unwrap();
    set(&mut e, "shop.api", "group", "Storefront").unwrap();
    let text = read(&t.dir, "model/shop.yaml");
    assert!(text.contains("tech: Preact # SPA"), "comment survives: {text}");
    assert!(text.contains("group: Storefront"), "{text}");
    assert_eq!(e.model.elements["shop.web"].group.as_deref(), Some("Storefront"));
    assert_eq!(e.model.elements["shop.api"].group.as_deref(), Some("Storefront"));

    // A system in its own file is the one element whose `group:` the parser
    // hardcoded to None — so it could be written and never drawn. At L1 its
    // siblings are the people and externals, which have always grouped.
    set(&mut e, "shop", "group", "Retail").unwrap();
    assert_eq!(e.model.elements["shop"].group.as_deref(), Some("Retail"));
}

#[test]
fn an_emptied_field_is_removed_rather_than_blanked() {
    let (t, mut e) = setup("clear");
    set(&mut e, "shop.web", "description", "The storefront SPA").unwrap();
    assert!(read(&t.dir, "model/shop.yaml").contains("description: The storefront SPA"));
    set(&mut e, "shop.web", "description", "").unwrap();
    let text = read(&t.dir, "model/shop.yaml");
    // Not `description: ""` — that is a description saying nothing, which is
    // what both the MCP schema and the inspector had always claimed not to do.
    assert!(!text.contains("description"), "{text}");
    assert_eq!(text, SHOP, "and nothing else moved");
    assert_eq!(e.model.elements["shop.web"].description, None);

    // Clearing what is already absent is not an edit, so it leaves no undo
    // entry behind to puzzle over.
    let tx = e
        .apply(Operation::SetField {
            id: "shop.web".into(), field: "tech".into(), value: "".into(),
        })
        .unwrap();
    assert!(!tx.changes.is_empty(), "the first clear writes");
    let tx = e
        .apply(Operation::SetField {
            id: "shop.web".into(), field: "tech".into(), value: "".into(),
        })
        .unwrap();
    assert!(tx.changes.is_empty(), "second clear is a no-op");
}

#[test]
fn a_field_clears_out_of_a_one_line_mapping_too() {
    let (t, mut e) = setup("flow-clear");
    // `db: { name: Database, tech: Postgres }` — removing a whole line here
    // would take the element with it.
    set(&mut e, "shop.db", "tech", "").unwrap();
    let text = read(&t.dir, "model/shop.yaml");
    assert!(text.contains("db: { name: Database }"), "{text}");
    assert_eq!(e.model.elements["shop.db"].tech, None);
    assert_eq!(e.model.elements["shop.db"].name, "Database");

    // And the last one out leaves a well-formed empty mapping rather than
    // braces with a comma in them.
    set(&mut e, "shop.db", "name", "").unwrap();
    let text = read(&t.dir, "model/shop.yaml");
    assert!(text.contains("db: {}"), "{text}");
}

#[test]
fn replicas_is_checked_against_the_kind_and_the_number() {
    let (t, mut e) = depot("replicas");
    set(&mut e, "prod.web-tier", "replicas", "3").unwrap();
    assert!(read(&t.dir, "model/deployment.yaml").contains("replicas: 3"));
    assert_eq!(e.model.elements["prod.web-tier"].replicas, Some(3));

    // 1 is the default and is never drawn, so writing it says nothing.
    set(&mut e, "prod.web-tier", "replicas", "1").unwrap();
    assert!(!read(&t.dir, "model/deployment.yaml").contains("replicas"));

    let err = set(&mut e, "prod.web-tier", "replicas", "0").unwrap_err();
    assert!(err.contains("delete the element"), "{err}");
    let err = set(&mut e, "prod.web-tier", "replicas", "many").unwrap_err();
    assert!(err.contains("whole number"), "{err}");
    // A container does not have replicas — the thing that runs it does.
    let err = set(&mut e, "depot.api", "replicas", "2").unwrap_err();
    assert!(err.contains("deployment node or container instance"), "{err}");
}

#[test]
fn external_is_a_system_flag_and_false_is_no_flag() {
    let (t, mut e) = setup("external");
    set(&mut e, "shop", "external", "true").unwrap();
    assert!(read(&t.dir, "model/shop.yaml").contains("external: true"));
    assert!(e.model.elements["shop"].external);

    set(&mut e, "shop", "external", "false").unwrap();
    assert!(!read(&t.dir, "model/shop.yaml").contains("external"));
    assert!(!e.model.elements["shop"].external);

    let err = set(&mut e, "shop.web", "external", "true").unwrap_err();
    assert!(err.contains("not a container"), "{err}");
    let err = set(&mut e, "shop", "external", "sometimes").unwrap_err();
    assert!(err.contains("true or false"), "{err}");
}

#[test]
fn a_source_mapping_can_be_written_without_opening_the_file() {
    let (t, mut e) = depot("source");
    e.apply(Operation::SetSource {
        id: "depot.api.router".into(),
        source: Some(SourceInput {
            language: "rust".into(),
            root: "crates/api/src\\".into(), // backslashes and trailing slash normalise
            include: vec!["router.rs".into(), "  ".into()],
            exclude: vec![],
            mode: None,
            extractor: None,
        }),
    })
    .unwrap();
    let text = read(&t.dir, "model/depot.yaml");
    assert!(
        text.contains("      router:\n        name: Router\n        tech: axum\n        source:\n          language: rust\n          root: crates/api/src\n          include: [router.rs]\n"),
        "{text}"
    );
    let mapping = e.model.elements["depot.api.router"].source.as_ref().expect("mapping");
    assert_eq!(mapping.root, "crates/api/src");
    assert_eq!(mapping.include, vec!["router.rs".to_string()]);

    // Replacing rewrites the block rather than stacking a second one.
    e.apply(Operation::SetSource {
        id: "depot.api.router".into(),
        source: Some(SourceInput {
            language: "typescript".into(),
            root: "web/src".into(),
            include: vec![],
            exclude: vec!["**/*.test.ts".into()],
            mode: None,
            extractor: None,
        }),
    })
    .unwrap();
    let text = read(&t.dir, "model/depot.yaml");
    assert_eq!(text.matches("source:").count(), 1, "{text}");
    assert!(text.contains("language: typescript") && text.contains("exclude: [\"**/*.test.ts\"]"), "{text}");
    assert!(!text.contains("include:"), "an absent include means the language default: {text}");

    // And removing it takes the whole block, leaving the component behind.
    e.apply(Operation::SetSource { id: "depot.api.router".into(), source: None }).unwrap();
    let text = read(&t.dir, "model/depot.yaml");
    assert_eq!(text, DEPOT, "byte-identical to before it was ever mapped");
    assert!(e.model.elements["depot.api.router"].source.is_none());
}

#[test]
fn a_source_mapping_is_refused_before_it_reaches_the_file() {
    let (t, mut e) = depot("source-refused");
    let attempt = |e: &mut SyncEngine, id: &str, language: &str, root: &str, mode: Option<&str>| {
        e.apply(Operation::SetSource {
            id: id.into(),
            source: Some(SourceInput {
                language: language.into(),
                root: root.into(),
                include: vec![],
                exclude: vec![],
                mode: mode.map(str::to_string),
                extractor: None,
            }),
        })
        .map(|_| ())
    };
    // Introspection is per component (spec/l4-introspection.md) — a `source:`
    // on a container was silently ignored until 0.6.2 and is a warning now;
    // proposing one is simply refused.
    let err = attempt(&mut e, "depot.api", "rust", "crates/api/src", None).unwrap_err();
    assert!(err.contains("per component"), "{err}");
    let err = attempt(&mut e, "depot.api.router", "cobol", "src", None).unwrap_err();
    assert!(err.contains("unknown source language"), "{err}");
    let err = attempt(&mut e, "depot.api.router", "rust", "src", Some("deep")).unwrap_err();
    assert!(err.contains("unknown source mode"), "{err}");
    // Repo-root-relative, per ADR-0014.
    for bad in ["", "/abs/path", "../sibling", "C:/repo/src"] {
        let err = attempt(&mut e, "depot.api.router", "rust", bad, None).unwrap_err();
        assert!(err.contains("root"), "{bad}: {err}");
    }
    assert_eq!(read(&t.dir, "model/depot.yaml"), DEPOT, "nothing reached the file");
}

fn flag(e: &mut SyncEngine, level: &str, scope: Option<&str>, flag: &str, value: bool) -> Result<(), String> {
    e.apply(Operation::SetViewFlag {
        view: None,
        level: level.into(),
        scope: scope.map(str::to_string),
        flag: flag.into(),
        value,
    })
    .map(|_| ())
}

#[test]
fn a_view_flag_authors_the_file_it_needs() {
    let (t, mut e) = setup("view-flag-new");
    // Nothing in views/ at all: turning a flag on writes the view, the way
    // pinning does — and with the same shape, so the two cannot disagree.
    flag(&mut e, "L2", Some("shop"), "show-groups", true).unwrap();
    assert_eq!(
        read(&t.dir, "views/shop-l2.yaml"),
        "view: shop-l2\nscope: shop\nlevel: L2\nshow-groups: true\n"
    );
    assert!(e.model.views.iter().any(|v| v.id == "shop-l2" && v.show_groups));

    // Turning it back off removes the key rather than writing the default.
    flag(&mut e, "L2", Some("shop"), "show-groups", false).unwrap();
    assert_eq!(read(&t.dir, "views/shop-l2.yaml"), "view: shop-l2\nscope: shop\nlevel: L2\n");
}

#[test]
fn a_default_is_never_written_and_never_creates_a_file() {
    let (t, mut e) = setup("view-flag-default");
    // `show-groups: false` and `include-context: true` say exactly what their
    // absence says, so a file stating them is a file to keep in step with a
    // default that might move.
    let tx = e
        .apply(Operation::SetViewFlag {
            view: None, level: "L2".into(), scope: Some("shop".into()),
            flag: "show-groups".into(), value: false,
        })
        .unwrap();
    assert!(tx.changes.is_empty());
    assert!(!t.dir.join("views/shop-l2.yaml").exists(), "no file authored to say nothing");

    // The one that is on by default writes only when turned off.
    flag(&mut e, "L2", Some("shop"), "include-context", false).unwrap();
    assert!(read(&t.dir, "views/shop-l2.yaml").contains("include-context: false"));
    flag(&mut e, "L2", Some("shop"), "include-context", true).unwrap();
    assert!(!read(&t.dir, "views/shop-l2.yaml").contains("include-context"));
}

#[test]
fn a_view_flag_lands_in_the_file_that_exists_keeping_its_comments() {
    let (t, mut e) = setup("view-flag-existing");
    write(&t.dir, "views/shop-l2.yaml", SHOP_VIEW);
    assert!(e.external_scan());
    flag(&mut e, "L2", Some("shop"), "show-groups", true).unwrap();
    let text = read(&t.dir, "views/shop-l2.yaml");
    assert!(text.starts_with("# L2 — the shop's containers.\n"), "{text}");
    assert!(text.contains("show-groups: true"), "{text}");
    assert!(text.contains("web: [4, 2]     # the front door"), "pins and comments untouched: {text}");
}

#[test]
fn nested_is_a_deployment_view_flag_and_says_so() {
    let (t, mut e) = setup("view-flag-nested");
    let err = flag(&mut e, "L2", Some("shop"), "nested", true).unwrap_err();
    assert!(err.contains("dives instead"), "{err}");
    let err = flag(&mut e, "L2", Some("shop"), "show-descriptions", true).unwrap_err();
    assert!(err.contains("not a view flag"), "{err}");
    assert!(!t.dir.join("views/shop-l2.yaml").exists(), "nothing reached a file");
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
        name: "Dup".into(), kind: "container".into(), container: None,
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
