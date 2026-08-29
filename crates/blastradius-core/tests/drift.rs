//! Architecture drift detection (ADR-0019): the model checked against the
//! code, rather than against itself.

use blastradius_core::drift::{detect, DriftKind};
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
    let dir = std::env::temp_dir().join(format!("br-drift-{tag}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    TempDir { dir }
}
fn write(root: &Path, rel: &str, content: &str) {
    let path = root.join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}

const MANIFEST: &str = "workspace:\n  name: Shop\n  version: 1\nmodel:\n  include: [model/*.yaml]\n";

/// Two components in *different* containers, each owning one Rust file;
/// `store.rs` imports from `ledger.rs`, so the code says store depends on
/// ledger.
fn fixture(tag: &str, relations: &str) -> (TempDir, blastradius_core::Workspace) {
    let t = temp(tag);
    write(&t.dir, "src/ledger.rs", "pub struct Entry;\n");
    write(
        &t.dir,
        "src/store.rs",
        "use crate::ledger::Entry;\n\npub struct Store {\n    last: Entry,\n}\n",
    );
    let model = format!(
        r#"system: shop
name: Shop
containers:
  api:
    name: API
    components:
      store:
        name: Store
        source:
          language: rust
          root: src
          include: [store.rs]
  data:
    name: Data
    components:
      ledger:
        name: Ledger
        source:
          language: rust
          root: src
          include: [ledger.rs]
{relations}"#
    );
    write(&t.dir, "docs/model/shop.yaml", &model);
    write(&t.dir, "docs/blastradius.yaml", MANIFEST);

    // Extract both components the way the CLI would.
    let (ws, _) = blastradius_core::load_workspace(&t.dir.join("docs"));
    for id in ["shop.api.store", "shop.data.ledger"] {
        let mapping = ws.elements[id].source.as_ref().unwrap();
        let (facts, _) = blastradius_core::introspect::extract(&t.dir, id, mapping).unwrap();
        write(
            &t.dir,
            &format!("docs/model/derived/{id}.l4.json"),
            &blastradius_core::introspect::facts_bytes(&facts),
        );
    }
    let (ws, _) = blastradius_core::load_workspace(&t.dir.join("docs"));
    (t, ws)
}

#[test]
fn an_undeclared_code_dependency_is_drift() {
    let (_t, ws) = fixture("undeclared", "");
    let found = detect(&ws);
    let undeclared: Vec<_> = found.iter().filter(|d| d.kind == DriftKind::Undeclared).collect();
    assert_eq!(undeclared.len(), 1, "{found:?}");
    assert_eq!(undeclared[0].from, "shop.api.store");
    assert_eq!(undeclared[0].to, "shop.data.ledger");
    // The finding names the file that proves it — otherwise it is unarguable.
    assert_eq!(undeclared[0].via.as_deref(), Some("src/ledger.rs"));
}

#[test]
fn declaring_the_dependency_clears_it() {
    let (_t, ws) = fixture(
        "declared",
        "relations:\n  - from: api.store\n    to: data.ledger\n    label: reads entries\n",
    );
    assert!(detect(&ws).is_empty(), "{:?}", detect(&ws));
}

#[test]
fn a_relation_higher_up_the_hierarchy_covers_it() {
    // A container-level relation declares the dependency between anything
    // inside them — the same lifting the canvas does when it draws an edge.
    let (_t, ws) = fixture(
        "lifted",
        "relations:\n  - from: api\n    to: data\n    label: reads\n",
    );
    let found = detect(&ws);
    assert!(
        !found.iter().any(|d| d.kind == DriftKind::Undeclared),
        "a relation between the shared ancestors should cover it: {found:?}"
    );
}

#[test]
fn a_declaration_the_code_does_not_support_is_drift() {
    // Declared backwards: ledger -> store, while the code runs store -> ledger.
    let (_t, ws) = fixture(
        "unbacked",
        "relations:\n  - from: data.ledger\n    to: api.store\n    label: pushes entries\n",
    );
    let found = detect(&ws);
    assert!(
        found.iter().any(|d| d.kind == DriftKind::Unbacked
            && d.from == "shop.data.ledger"
            && d.to == "shop.api.store"),
        "{found:?}"
    );
    // And the real dependency is still reported as undeclared.
    assert!(
        found.iter().any(|d| d.kind == DriftKind::Undeclared && d.from == "shop.api.store"),
        "{found:?}"
    );
}

#[test]
fn a_cross_language_relation_is_never_called_unbacked() {
    // A TypeScript UI talking to a Rust engine is a real relation that no
    // static import can evidence, so its silence must not be reported.
    let t = temp("crosslang");
    write(&t.dir, "src/ledger.rs", "pub struct Entry;\n");
    write(&t.dir, "web/app.ts", "export class App {}\n");
    write(
        &t.dir,
        "docs/model/shop.yaml",
        "system: shop\nname: Shop\ncontainers:\n  api:\n    name: API\n    components:\n      \
         ledger:\n        name: Ledger\n        source:\n          language: rust\n          \
         root: src\n          include: [ledger.rs]\n      web:\n        name: Web\n        source:\n          \
         language: typescript\n          root: web\nrelations:\n  - from: api.web\n    to: data.ledger\n    label: calls over IPC\n",
    );
    write(&t.dir, "docs/blastradius.yaml", MANIFEST);
    let (ws, _) = blastradius_core::load_workspace(&t.dir.join("docs"));
    let mapping = ws.elements["shop.api.ledger"].source.as_ref().unwrap();
    let (facts, _) = blastradius_core::introspect::extract(&t.dir, "shop.api.ledger", mapping).unwrap();
    write(
        &t.dir,
        "docs/model/derived/shop.api.ledger.l4.json",
        &blastradius_core::introspect::facts_bytes(&facts),
    );
    let (ws, _) = blastradius_core::load_workspace(&t.dir.join("docs"));
    assert!(
        !detect(&ws).iter().any(|d| d.kind == DriftKind::Unbacked),
        "cross-language relations cannot be evidenced by imports: {:?}",
        detect(&ws)
    );
}

/// The renderer needs the findings whole. `diagnose` flattens each into a
/// warning string, which is why the canvas could only ever have shown a count:
/// the remedy for an undeclared dependency is one `add-relation` call, and a
/// sentence cannot carry its arguments.
#[test]
fn the_snapshot_carries_drift_as_structure_not_prose() {
    let (t, ws) = fixture("snapshot", "");
    let vfs = blastradius_core::vfs::DiskVfs::new(&t.dir.join("docs"));
    let snap = blastradius_core::snapshot::snapshot(&vfs, &ws, &[]);
    assert_eq!(snap.drift.len(), 1, "one seeded finding");
    let d = &snap.drift[0];
    assert_eq!(d.kind, "undeclared");
    assert_eq!(d.from, "shop.api.store");
    assert_eq!(d.to, "shop.data.ledger");
    assert_eq!(d.via.as_deref(), Some("src/ledger.rs"));

    // And a clean model says nothing rather than an empty list — the field is
    // skipped entirely, which keeps every existing snapshot byte-identical.
    let (t2, clean) = fixture(
        "snapshot-clean",
        "relations:\n  - from: api.store\n    to: data.ledger\n    label: reads entries\n",
    );
    let vfs2 = blastradius_core::vfs::DiskVfs::new(&t2.dir.join("docs"));
    let snap2 = blastradius_core::snapshot::snapshot(&vfs2, &clean, &[]);
    assert!(snap2.drift.is_empty());
    let json = serde_json::to_string(&snap2).unwrap();
    assert!(!json.contains("\"drift\""), "an empty finding list is not a field");
}

// --- C# (0.10.0 item 2) ------------------------------------------------------

/// The two committed C# facts files, as the real extractor produced them.
///
/// They are not hand-written: `extractors/dotnet/fixtures/semantic` is a
/// two-project solution where `Beta/Consumer.cs` references a type defined in
/// `Alpha/Widget.cs`, and these are what semantic mode emits for it. That
/// extractor's own gate (`extractors/dotnet/test.sh`) keeps them honest; this
/// test is about what the *workspace* does with them, which is the half a
/// fixture cannot fake.
fn csharp_fixture(tag: &str, relations: &str) -> (TempDir, blastradius_core::Workspace) {
    let t = temp(tag);
    let model = format!(
        r#"system: shop
name: Shop
containers:
  api:
    name: API
    components:
      consumer:
        name: Consumer
        source:
          language: csharp
          root: Beta
          mode: semantic
  data:
    name: Data
    components:
      widgets:
        name: Widgets
        source:
          language: csharp
          root: Alpha
          mode: semantic
{relations}"#
    );
    write(&t.dir, "docs/model/shop.yaml", &model);
    write(&t.dir, "docs/blastradius.yaml", MANIFEST);
    let here = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/csharp-drift");
    for id in ["shop.api.consumer", "shop.data.widgets"] {
        let facts = fs::read_to_string(here.join(format!("{id}.l4.json"))).unwrap();
        write(&t.dir, &format!("docs/model/derived/{id}.l4.json"), &facts);
    }
    let (ws, _) = blastradius_core::load_workspace(&t.dir.join("docs"));
    (t, ws)
}

/// The claim ADR-0019 could not make for C# until now: a cross-project
/// reference is a code dependency between components, and the model has to
/// declare it or hear about it.
///
/// Nothing in `drift.rs` knows what C# is — which is the point. The extractor
/// records the file a reference resolves to, and everything downstream is the
/// same code that reports Rust.
#[test]
fn csharp_cross_project_references_are_drift_like_any_other() {
    let (_t, ws) = csharp_fixture("csharp-undeclared", "");
    let found = detect(&ws);
    assert_eq!(found.len(), 1, "one undeclared dependency, got {found:?}");
    assert_eq!(found[0].from, "shop.api.consumer");
    assert_eq!(found[0].to, "shop.data.widgets");
    assert_eq!(found[0].kind, DriftKind::Undeclared);
    // The evidence is the file that defines the type, which is the thing
    // syntax-level C# cannot name and the reason this was blind until 0.10.0.
    assert_eq!(found[0].via.as_deref(), Some("Alpha/Widget.cs"));
}

#[test]
fn declaring_the_csharp_dependency_clears_it() {
    let (_t, ws) = csharp_fixture(
        "csharp-declared",
        "relations:\n  - from: api.consumer\n    to: data.widgets\n    label: uses\n",
    );
    assert!(detect(&ws).is_empty(), "{:?}", detect(&ws));
}

/// And the other direction, which is the finding this repository's own first
/// drift run needed: a relation written the wrong way round.
#[test]
fn a_backwards_csharp_relation_is_unbacked_and_undeclared_at_once() {
    let (_t, ws) = csharp_fixture(
        "csharp-backwards",
        "relations:\n  - from: data.widgets\n    to: api.consumer\n    label: feeds\n",
    );
    let found = detect(&ws);
    let kinds: Vec<_> = found.iter().map(|d| (d.from.as_str(), d.to.as_str(), d.kind)).collect();
    assert!(
        kinds.contains(&("shop.api.consumer", "shop.data.widgets", DriftKind::Undeclared)),
        "{kinds:?}"
    );
    assert!(
        kinds.contains(&("shop.data.widgets", "shop.api.consumer", DriftKind::Unbacked)),
        "{kinds:?}"
    );
}

/// A mapping rooted at the repository itself — the natural one for a
/// single-project repo — could never own a file, so drift was silently
/// impossible there. Found while wiring C# up, and it was never about C#.
#[test]
fn a_mapping_rooted_at_the_repository_still_owns_its_files() {
    let (_t, ws) = fixture("root-dot", "");
    let mut ws = ws;
    for el in ws.elements.values_mut() {
        if let Some(m) = el.source.as_mut() {
            // Same files, named from the repository root instead of src/.
            m.include = m.include.iter().map(|i| format!("src/{i}")).collect();
            m.root = ".".into();
        }
    }
    // The facts still say `src/ledger.rs`, which is what the outbound entry
    // carries; only the mapping's shape changed.
    let found = detect(&ws);
    assert_eq!(found.len(), 1, "the dependency is still owned, got {found:?}");
    assert_eq!(found[0].to, "shop.data.ledger");
}
