//! Relation repair (0.11.0 item 5): the fields a relation has, and moving an
//! endpoint without losing the rest of it.
//!
//! The shape under test is that re-pointing is a *splice of one value*. Every
//! assertion here is therefore a byte comparison against the original file with
//! one substring changed — because "nothing else moved" is the whole claim, and
//! a field-by-field comparison would not catch a relation that quietly changed
//! places in the list or lost the comment beside it.

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
    let dir = std::env::temp_dir().join(format!("blastradius-rel-{name}-{}", std::process::id()));
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

const MANIFEST: &str =
    "workspace:\n  name: T\n  version: 1\nmodel:\n  include: [model/*.yaml]\nviews:\n  include: [views/*.yaml]\n";

/// Comments, an aligned trailing comment, a `direction:` already in place, and
/// a container-nested `relations:` block whose `from` is implied by the
/// container it sits under (spec §3) — none of which may move.
const SHOP: &str = "\
# The shop system — mind the comments.
system: shop
name: Shop

containers:
  web:
    name: Web App
    relations:
      - to: api
        label: renders from
  api:
    name: API
  db: { name: Database }
  cache: { name: Cache }

# relations live at the bottom
relations:
  - from: web
    to: api
    label: calls
    protocol: HTTPS
  - from: api
    to: db          # keep this comment
    label: reads
    direction: both
";

const WAREHOUSE: &str = "\
system: warehouse
name: Warehouse

containers:
  picker: { name: Picker }
";

fn setup(name: &str) -> (TempWs, SyncEngine) {
    let t = temp_ws(name);
    write(&t.dir, "blastradius.yaml", MANIFEST);
    write(&t.dir, "model/shop.yaml", SHOP);
    write(&t.dir, "model/warehouse.yaml", WAREHOUSE);
    let engine = SyncEngine::open(&t.dir);
    assert!(engine.stale.is_empty(), "{:?}", engine.diagnostics);
    (t, engine)
}

fn set(from: &str, to: &str, label: Option<&str>, field: &str, value: &str) -> Operation {
    Operation::SetRelationField {
        from: from.into(),
        to: to.into(),
        label: label.map(str::to_string),
        field: field.into(),
        value: value.into(),
    }
}

// ---- endpoints --------------------------------------------------------------

#[test]
fn re_pointing_an_endpoint_moves_that_value_and_nothing_else() {
    let (t, mut e) = setup("repoint-to");
    e.apply(set("shop.web", "shop.api", Some("calls"), "to", "shop.cache")).unwrap();
    assert_eq!(
        read(&t.dir, "model/shop.yaml"),
        SHOP.replace("  - from: web\n    to: api\n", "  - from: web\n    to: cache\n"),
        "the endpoint changed; the label, the protocol, the position in the list \
         and every comment in the file did not"
    );
}

#[test]
fn re_pointing_from_keeps_the_trailing_comment_and_the_direction() {
    let (t, mut e) = setup("repoint-from");
    // The `api -> db` item is the one carrying an aligned trailing comment and
    // a `direction:`, which is exactly what the delete-and-re-add shape lost.
    e.apply(set("shop.api", "shop.db", Some("reads"), "from", "shop.cache")).unwrap();
    let text = read(&t.dir, "model/shop.yaml");
    assert_eq!(text, SHOP.replace("  - from: api\n    to: db", "  - from: cache\n    to: db"));
    assert!(text.contains("to: db          # keep this comment"), "{text}");
    assert!(text.contains("direction: both"), "{text}");
}

#[test]
fn an_endpoint_in_another_system_is_written_as_an_absolute_path() {
    let (t, mut e) = setup("repoint-cross");
    // Relations resolve absolute-first (`Workspace::resolve`), so a cross-system
    // endpoint is legal where the relation already sits and needs no file move.
    e.apply(set("shop.web", "shop.api", Some("calls"), "to", "warehouse.picker")).unwrap();
    assert_eq!(
        read(&t.dir, "model/shop.yaml"),
        SHOP.replace("  - from: web\n    to: api\n", "  - from: web\n    to: warehouse.picker\n")
    );
    assert!(SyncEngine::open(&t.dir).diagnostics.is_empty(), "the reference resolves");
}

#[test]
fn re_pointing_an_endpoint_at_the_other_one_is_refused() {
    let (_t, mut e) = setup("repoint-self");
    let err = e.apply(set("shop.web", "shop.api", Some("calls"), "to", "shop.web")).unwrap_err();
    assert!(err.contains("same element"), "{err}");
}

#[test]
fn re_pointing_at_something_that_is_not_an_element_is_refused() {
    let (_t, mut e) = setup("repoint-unknown");
    let err = e.apply(set("shop.web", "shop.api", Some("calls"), "to", "shop.ghost")).unwrap_err();
    assert!(err.contains("unknown element"), "{err}");
}

// ---- direction --------------------------------------------------------------

#[test]
fn direction_is_written_where_there_was_none() {
    let (t, mut e) = setup("dir-add");
    e.apply(set("shop.web", "shop.api", Some("calls"), "direction", "none")).unwrap();
    assert_eq!(
        read(&t.dir, "model/shop.yaml"),
        SHOP.replace("    protocol: HTTPS\n", "    protocol: HTTPS\n    direction: none\n")
    );
}

#[test]
fn setting_direction_back_to_forward_removes_the_key() {
    let (t, mut e) = setup("dir-default");
    // Forward is the absence of the key (spec §3), so writing `direction:
    // forward` would say the same thing twice and leave noise in the file.
    e.apply(set("shop.api", "shop.db", Some("reads"), "direction", "forward")).unwrap();
    assert_eq!(read(&t.dir, "model/shop.yaml"), SHOP.replace("    direction: both\n", ""));
}

#[test]
fn removing_a_direction_that_is_not_there_is_not_an_edit() {
    let (t, mut e) = setup("dir-noop");
    e.apply(set("shop.web", "shop.api", Some("calls"), "direction", "forward")).unwrap();
    assert_eq!(read(&t.dir, "model/shop.yaml"), SHOP);
}

#[test]
fn a_direction_the_model_does_not_have_is_refused() {
    let (_t, mut e) = setup("dir-bad");
    let err = e.apply(set("shop.web", "shop.api", Some("calls"), "direction", "sideways")).unwrap_err();
    assert!(err.contains("sideways"), "{err}");
}

// ---- the other scalars ------------------------------------------------------

#[test]
fn an_emptied_protocol_is_removed_rather_than_blanked() {
    let (t, mut e) = setup("proto-empty");
    // What `SetField` has always done for an element's scalars, and what this
    // did not do for a relation's until endpoints needed the same path.
    e.apply(set("shop.web", "shop.api", Some("calls"), "protocol", "")).unwrap();
    assert_eq!(read(&t.dir, "model/shop.yaml"), SHOP.replace("    protocol: HTTPS\n", ""));
}

#[test]
fn a_field_the_relation_does_not_have_is_refused() {
    let (_t, mut e) = setup("bad-field");
    let err = e.apply(set("shop.web", "shop.api", Some("calls"), "tech", "Go")).unwrap_err();
    assert!(err.contains("not editable"), "{err}");
}

// ---- nested relations -------------------------------------------------------

#[test]
fn a_container_nested_relation_can_be_edited_at_all() {
    let (t, mut e) = setup("nested");
    // This is the bug the line anchor fixes: the splice used to look for a
    // `relations:` key at the document root, so every relation written under a
    // container or a deployment environment answered "no relations section" —
    // the inspector's label and protocol boxes were inert on all of them.
    e.apply(set("shop.web", "shop.api", Some("renders from"), "protocol", "JSON")).unwrap();
    assert_eq!(
        read(&t.dir, "model/shop.yaml"),
        SHOP.replace(
            "      - to: api\n        label: renders from\n",
            "      - to: api\n        label: renders from\n        protocol: JSON\n"
        )
    );
}

#[test]
fn a_nested_relation_gains_the_from_it_was_implying() {
    let (t, mut e) = setup("nested-from");
    // `from` is implied by the container the block sits under, so re-pointing it
    // means writing a key that was never in the file.
    e.apply(set("shop.web", "shop.api", Some("renders from"), "from", "shop.cache")).unwrap();
    let text = read(&t.dir, "model/shop.yaml");
    assert_eq!(
        text,
        SHOP.replace(
            "      - to: api\n        label: renders from\n",
            "      - to: api\n        label: renders from\n        from: cache\n"
        )
    );
    assert!(SyncEngine::open(&t.dir).diagnostics.is_empty(), "still resolves");
}

// ---- reverse, which is what all of this was for ------------------------------

#[test]
fn reversing_a_relation_keeps_everything_it_used_to_drop() {
    let (t, mut e) = setup("reverse");
    // 0.9.0's *Reverse it* was `delete-relation` + `add-relation` in the UI: it
    // copied label and protocol across by hand and had no way to carry
    // `direction` at all, because `AddRelation` has no such field. Two splices
    // swap the endpoints and touch nothing else.
    // Its own operation, because a swap done one endpoint at a time passes
    // through the state where both name the same element, which is refused:
    e.apply(set("shop.api", "shop.db", Some("reads"), "from", "shop.db")).unwrap_err();
    e.apply(Operation::ReverseRelation {
        from: "shop.api".into(),
        to: "shop.db".into(),
        label: Some("reads".into()),
    })
    .unwrap();
    let text = read(&t.dir, "model/shop.yaml");
    // `db` -> `api` is one character longer, so the padding gives one back and
    // the `#` stays in the column its author put it in.
    assert_eq!(
        text,
        SHOP.replace("  - from: api\n    to: db          #", "  - from: db\n    to: api         #")
    );
    assert!(text.contains("direction: both"), "direction survived the reversal: {text}");
    assert!(text.contains("# keep this comment"), "the comment survived too: {text}");
    assert_eq!(comment_column(&text), comment_column(SHOP), "and it stayed put");
}

/// Column of the one trailing `#` comment in the fixture.
fn comment_column(text: &str) -> usize {
    let line = text.lines().find(|l| l.contains("# keep this comment")).expect("the comment");
    line.chars().position(|c| c == '#').unwrap()
}

#[test]
fn a_shorter_value_pads_out_to_the_same_column() {
    let (t, mut e) = setup("align-shorter");
    e.apply(set("shop.api", "shop.db", Some("reads"), "to", "shop.cache")).unwrap();
    let text = read(&t.dir, "model/shop.yaml");
    assert_eq!(comment_column(&text), comment_column(SHOP), "{text}");
}

#[test]
fn a_value_long_enough_to_reach_the_comment_keeps_one_space() {
    let (t, mut e) = setup("align-overflow");
    // Nothing can hold the column once the value runs past it; the guarantee
    // degrades to the single space that keeps the YAML readable and valid.
    e.apply(set("shop.api", "shop.db", Some("reads"), "to", "warehouse.picker")).unwrap();
    let text = read(&t.dir, "model/shop.yaml");
    assert!(text.contains("to: warehouse.picker # keep this comment"), "{text}");
    assert!(SyncEngine::open(&t.dir).diagnostics.is_empty(), "and it still resolves");
}

#[test]
fn reversing_a_nested_relation_writes_the_from_it_was_implying() {
    let (t, mut e) = setup("reverse-nested");
    // The container-nested item has no `from:` at all — it is implied by the
    // container. Reversing it has to write one, and the second splice has to
    // still find the item after the first inserted a line into it.
    e.apply(Operation::ReverseRelation {
        from: "shop.web".into(),
        to: "shop.api".into(),
        label: Some("renders from".into()),
    })
    .unwrap();
    let text = read(&t.dir, "model/shop.yaml");
    assert_eq!(
        text,
        SHOP.replace(
            "      - to: api\n        label: renders from\n",
            "      - to: web\n        label: renders from\n        from: api\n"
        )
    );
    assert!(SyncEngine::open(&t.dir).diagnostics.is_empty(), "and it resolves: {text}");
}

#[test]
fn one_reversal_is_one_undo() {
    let (t, mut e) = setup("reverse-undo");
    e.apply(Operation::ReverseRelation {
        from: "shop.api".into(),
        to: "shop.db".into(),
        label: Some("reads".into()),
    })
    .unwrap();
    e.undo().unwrap();
    // The delete-and-re-add shape needed both halves in one transaction to get
    // this; a single splice gets it for free.
    assert_eq!(read(&t.dir, "model/shop.yaml"), SHOP);
}
