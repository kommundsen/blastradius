//! The mock/engine contract (0.10.0 item 4).
//!
//! The e2e suite runs against a hand-written mock of the sync engine
//! (ADR-0011), and every operation's semantics are mirrored into it by hand.
//! 0.9.0 alone added four such mirrors — unpin removing the `layout:` key,
//! `external: false` clearing rather than setting, `replicas: 1` clearing, and
//! a view file being authored when none exists. Each is a place the suite can
//! agree with itself while disagreeing with the engine, which is exactly what
//! 0.8.0's settle test did for a whole release.
//!
//! So: one operation list (`ui/tests/contract/operations.json`) runs through
//! the real engine here and through `ui/js/mockops.js` in
//! `ui/tests/contract.test.mjs`, and both compare against the same committed
//! snapshot (`ui/tests/contract/after.json`). A divergence fails a build
//! instead of hiding one.
//!
//! Regenerate the expected snapshot with `UPDATE_CONTRACT=1 cargo test --test
//! contract` — and read the diff before committing it, because this file is
//! the only thing standing between the mock and a lie.

use blastradius_core::sync::{Operation, SyncEngine};
use std::fs;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap()
}

fn copy_tree(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let dest = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &dest);
        } else {
            fs::copy(entry.path(), &dest).unwrap();
        }
    }
}

struct TempWs(PathBuf);
impl Drop for TempWs {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// The operation list, as both sides read it.
fn operations(root: &Path) -> Vec<serde_json::Value> {
    let text = fs::read_to_string(root.join("ui/tests/contract/operations.json")).unwrap();
    serde_json::from_str(&text).unwrap()
}

#[test]
fn the_engine_and_the_mock_agree_on_one_operation_list() {
    let root = repo_root();
    let tmp = TempWs(
        std::env::temp_dir().join(format!("blastradius-contract-{}", std::process::id())),
    );
    let _ = fs::remove_dir_all(&tmp.0);
    copy_tree(&root.join("docs"), &tmp.0);

    let mut engine = SyncEngine::open(&tmp.0);
    assert!(engine.stale.is_empty(), "fixture workspace does not load: {:?}", engine.diagnostics);

    for (i, raw) in operations(&root).iter().enumerate() {
        let op: Operation = serde_json::from_value(raw.clone())
            .unwrap_or_else(|e| panic!("operation {i} does not deserialize: {e}\n{raw}"));
        engine
            .apply(op)
            .unwrap_or_else(|e| panic!("operation {i} refused by the engine: {e}\n{raw}"));
    }

    let vfs = blastradius_core::vfs::DiskVfs::new(&tmp.0);
    let snap = blastradius_core::snapshot::snapshot(&vfs, &engine.model, &engine.diagnostics);
    let mut value = serde_json::to_value(&snap).unwrap();

    // Two fields are out of scope, for stated reasons rather than for
    // convenience. Diagnostics: the mock neither validates nor reads a
    // filesystem, and has never claimed to. Docs: no operation in the
    // vocabulary edits a document (that is carried item 7), so including their
    // bodies would only make every prose edit in docs/ fail this test.
    //
    // Everything the operations *can* reach — elements, relations, views,
    // derived — is compared whole. Which also makes this the gate on
    // ui/mock/snapshot.json being regenerated when the model changes: it has
    // been a comment in ci.yml since Phase 1.
    let obj = value.as_object_mut().unwrap();
    obj.remove("diagnostics");
    obj.remove("docs");

    let expected_path = root.join("ui/tests/contract/after.json");
    let text = serde_json::to_string_pretty(&value).unwrap() + "\n";
    if std::env::var("UPDATE_CONTRACT").is_ok() {
        fs::write(&expected_path, &text).unwrap();
        return;
    }
    let expected: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&expected_path).unwrap()).unwrap();
    assert_eq!(
        value, expected,
        "the engine no longer produces the committed contract snapshot.\n\
         If the change is intended, regenerate with UPDATE_CONTRACT=1 and check \
         that ui/js/mockops.js still agrees (npm run test:contract)."
    );
}

/// Every operation the engine understands appears in the fixture.
///
/// The same shape as the box-menu gate in `ui/tests/menu.test.mjs`: a new
/// variant that no one exercises is the failure this whole test exists to
/// prevent, so adding one without a fixture entry fails the build.
#[test]
fn every_operation_variant_is_in_the_fixture() {
    let root = repo_root();
    let src = fs::read_to_string(root.join("crates/blastradius-core/src/sync.rs")).unwrap();
    let body = src
        .split("pub enum Operation {")
        .nth(1)
        .expect("Operation enum")
        .split("\n}")
        .next()
        .unwrap();

    let variants: Vec<String> = body
        .lines()
        .map(str::trim)
        .filter(|l| !l.starts_with("//") && !l.starts_with("///"))
        .filter_map(|l| l.split_once(|c| c == '{' || c == ',').map(|(name, _)| name.trim()))
        .filter(|n| n.chars().next().is_some_and(char::is_uppercase))
        .map(|n| kebab(n))
        .collect();
    assert!(variants.len() >= 12, "parsed too few variants: {variants:?}");

    let used: Vec<String> = operations(&root)
        .iter()
        .map(|o| o["op"].as_str().unwrap().to_string())
        .collect();
    for v in &variants {
        assert!(
            used.contains(v),
            "operation {v:?} is not in ui/tests/contract/operations.json — \
             an operation only one side implements is the bug this test exists to catch"
        );
    }
}

fn kebab(name: &str) -> String {
    let mut out = String::new();
    for (i, c) in name.chars().enumerate() {
        if c.is_uppercase() && i > 0 {
            out.push('-');
        }
        out.extend(c.to_lowercase());
    }
    out
}
