//! L4 introspection tests (ADR-0016, spec/l4-introspection.md): the built-in
//! Rust extractor, facts determinism, derived grafting, the read-only guard,
//! and stale-artifact diagnostics.

use blastradius_core::introspect::{self, extract_rust, facts_bytes};
use blastradius_core::model::SourceMapping;
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

fn temp(name: &str) -> TempDir {
    let dir = std::env::temp_dir().join(format!("blastradius-introspect-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    TempDir { dir }
}

fn write(root: &Path, rel: &str, content: &str) {
    let path = root.join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}

/// A little crate: two top-level modules, an inline module, a trait impl,
/// use-based imports, and a field reference across modules.
fn rust_fixture(repo: &Path) {
    write(
        repo,
        "src/lib.rs",
        "use crate::engine::Engine;\n\npub struct App {\n    engine: Engine,\n}\n\nmod detail {\n    pub struct Hidden;\n}\n",
    );
    write(
        repo,
        "src/engine.rs",
        // `serde` is an external crate (rolls up); `std` ships with the
        // toolchain and never does.
        "use serde::Serialize;\nuse std::fmt::Debug;\n\npub trait Runner {\n    fn run(&self);\n}\n\npub struct Engine;\n\nimpl Runner for Engine {\n    fn run(&self) {}\n}\n\npub enum Mode {\n    Fast,\n    Careful,\n}\n",
    );
    // A two-hop re-export chain: `deep` re-exports what `facade` re-exports
    // from `engine`, and `facade` also renames one of them.
    write(
        repo,
        "src/facade.rs",
        "pub use crate::engine::Engine;\npub use crate::engine::Mode as Speed;\n",
    );
    write(repo, "src/deep.rs", "pub use crate::facade::Engine;\n");
    write(
        repo,
        "src/user.rs",
        "use crate::deep::Engine;\nuse crate::facade::Speed;\n\npub struct User {\n    engine: Engine,\n    speed: Speed,\n}\n",
    );
    // Unparseable files are skipped with a warning, never fatal.
    write(repo, "src/broken.rs", "fn oops( {\n");
}

fn mapping() -> SourceMapping {
    SourceMapping {
        language: "rust".into(),
        root: "src".into(),
        include: vec![],
        exclude: vec![],
        extractor: None,
        mode: None,
    }
}

#[test]
fn rust_extractor_finds_modules_types_and_edges() {
    let t = temp("extract");
    rust_fixture(&t.dir);
    let (facts, warnings) = extract_rust(&t.dir, "sys.app.comp", &mapping()).unwrap();

    assert_eq!(warnings.len(), 1, "broken.rs should warn: {warnings:?}");
    assert!(warnings[0].contains("broken.rs"));

    let ids: Vec<&str> = facts.elements.iter().map(|e| e.id.as_str()).collect();
    // Modules: files plus the inline `mod detail`; broken.rs still registers
    // as a module (it exists) even though its items were unreadable.
    for expected in ["lib", "engine", "broken", "lib.detail", "lib.detail.Hidden", "lib.App", "engine.Engine", "engine.Runner", "engine.Mode"] {
        assert!(ids.contains(&expected), "missing {expected} in {ids:?}");
    }
    let kind_of = |id: &str| facts.elements.iter().find(|e| e.id == id).unwrap().kind.clone();
    assert_eq!(kind_of("engine.Engine"), "class");
    assert_eq!(kind_of("engine.Runner"), "interface");
    assert_eq!(kind_of("engine.Mode"), "enum");
    assert_eq!(kind_of("lib.detail"), "module");

    // The inline module nests under its file module.
    let detail = facts.elements.iter().find(|e| e.id == "lib.detail").unwrap();
    assert_eq!(detail.parent.as_deref(), Some("lib"));
    // Types carry a declaration line for click-through.
    let engine = facts.elements.iter().find(|e| e.id == "engine.Engine").unwrap();
    assert_eq!(engine.path, "src/engine.rs");
    assert!(engine.line.is_some());

    let edge = |from: &str, to: &str, kind: &str| {
        facts.edges.iter().any(|e| e.from == from && e.to == to && e.kind == kind)
    };
    assert!(edge("lib", "engine", "imports"), "use crate::engine::Engine → imports; edges: {:?}", facts.edges);
    assert!(edge("engine.Engine", "engine.Runner", "implements"));
    assert!(edge("lib.App", "engine.Engine", "references"), "field type reference; edges: {:?}", facts.edges);

    // Re-exports resolve to the defining module, through two façade hops and
    // through an `as` rename — not to the façade that forwarded them.
    assert!(
        edge("user", "engine", "imports"),
        "two-hop `pub use` should import from the defining module; edges: {:?}",
        facts.edges
    );
    assert!(!edge("user", "deep", "imports"), "façade should not be the import target; edges: {:?}", facts.edges);
    assert!(!edge("user", "facade", "imports"), "façade should not be the import target; edges: {:?}", facts.edges);
    assert!(
        edge("user.User", "engine.Engine", "references"),
        "re-exported type reference; edges: {:?}",
        facts.edges
    );
    assert!(
        edge("user.User", "engine.Mode", "references"),
        "renamed re-export `Speed` should resolve to engine.Mode; edges: {:?}",
        facts.edges
    );

    // External crates roll up to one parentless, pathless node each; the
    // sysroot is excluded because it carries no architectural signal.
    let dep = facts.elements.iter().find(|e| e.id == "dep.serde").expect("dep.serde rollup");
    assert_eq!(dep.kind, "dependency");
    assert_eq!(dep.name, "serde");
    assert_eq!(dep.path, "");
    assert_eq!(dep.parent, None);
    assert!(edge("engine", "dep.serde", "imports"), "edges: {:?}", facts.edges);
    assert!(!ids.contains(&"dep.std"), "sysroot crates are not dependencies: {ids:?}");
}

#[test]
fn extraction_is_byte_deterministic() {
    let t = temp("determinism");
    rust_fixture(&t.dir);
    let (a, _) = extract_rust(&t.dir, "sys.app.comp", &mapping()).unwrap();
    let (b, _) = extract_rust(&t.dir, "sys.app.comp", &mapping()).unwrap();
    assert_eq!(facts_bytes(&a), facts_bytes(&b));
    // And the digest actually probes content: touching a file changes it.
    write(&t.dir, "src/lib.rs", "pub struct App;\n");
    let (c, _) = extract_rust(&t.dir, "sys.app.comp", &mapping()).unwrap();
    assert_ne!(a.source_digest, c.source_digest);
}

const MANIFEST: &str = "workspace:\n  name: T\n  version: 1\nmodel:\n  include: [model/*.yaml]\nviews:\n  include: [views/*.yaml]\n";

const MODEL: &str = "\
system: sys
name: Sys

containers:
  app:
    name: App
    components:
      comp:
        name: Comp
        source:
          language: rust
          root: src
";

/// `mode:` is opt-in depth for the C# extractor: valid values parse, bad ones
/// are an error, and asking for semantic on a language without a semantic pass
/// warns rather than pretending.
#[test]
fn source_mode_is_parsed_and_validated() {
    let with_mode = |lang: &str, mode: &str| {
        let t = temp(&format!("mode-{lang}-{mode}"));
        write(&t.dir, "docs/blastradius.yaml", MANIFEST);
        write(
            &t.dir,
            "docs/model/sys.yaml",
            &MODEL.replace("language: rust", &format!("language: {lang}\n          mode: {mode}")),
        );
        let (ws, diags) = blastradius_core::load_workspace(&t.dir.join("docs"));
        let mode = ws
            .elements
            .iter()
            .find(|(id, _)| id.as_str() == "sys.app.comp")
            .and_then(|(_, e)| e.source.as_ref())
            .and_then(|s| s.mode.clone());
        (mode, diags)
    };

    let (mode, diags) = with_mode("csharp", "semantic");
    assert_eq!(mode.as_deref(), Some("semantic"));
    assert!(!blastradius_core::diagnostics::has_errors(&diags), "{diags:?}");

    // Unknown value: an error, listing what is allowed.
    let (_, diags) = with_mode("csharp", "deep");
    assert!(blastradius_core::diagnostics::has_errors(&diags));
    assert!(diags.iter().any(|d| d.message.contains("unknown source mode")), "{diags:?}");

    // Right value, wrong language: a warning, and the workspace still loads.
    let (_, diags) = with_mode("rust", "semantic");
    assert!(!blastradius_core::diagnostics::has_errors(&diags), "{diags:?}");
    assert!(diags.iter().any(|d| d.message.contains("no effect for rust")), "{diags:?}");
}

/// End to end: extract, commit the facts, reload — derived elements are
/// grafted with `.src.` ids and the sync engine refuses to touch them.
#[test]
fn derived_elements_load_and_are_read_only() {
    let t = temp("graft");
    rust_fixture(&t.dir);
    let ws_dir = t.dir.join("docs");
    write(&t.dir, "docs/blastradius.yaml", MANIFEST);
    write(&t.dir, "docs/model/sys.yaml", MODEL);

    let (facts, _) = extract_rust(&t.dir, "sys.app.comp", &mapping()).unwrap();
    write(&t.dir, "docs/model/derived/sys.app.comp.l4.json", &facts_bytes(&facts));

    let (ws, diags) = blastradius_core::load_workspace(&ws_dir);
    assert!(!blastradius_core::diagnostics::has_errors(&diags), "{diags:?}");
    assert_eq!(ws.derived.len(), 1);
    let g = &ws.derived[0];
    assert_eq!(g.component, "sys.app.comp");
    assert!(g.elements.iter().any(|e| e.id == "sys.app.comp.src.engine.Engine"));
    let eng = ws.derived_element("sys.app.comp.src.engine.Engine").unwrap();
    assert_eq!(eng.path, "src/engine.rs");

    // Read-only: every write path refuses with a source-pointing error.
    let mut engine = SyncEngine::open(&ws_dir);
    let err = engine
        .apply(Operation::SetField {
            id: "sys.app.comp.src.engine.Engine".into(),
            field: "description".into(),
            value: "nope".into(),
        })
        .unwrap_err();
    assert!(err.contains("derived from source"), "{err}");
    assert!(err.contains("src/engine.rs"), "{err}");

    // A component with no `source:` mapping is untouched by the feature:
    // hand-modeled editing works exactly as before (spec exit criterion 6).
    engine
        .apply(Operation::Create {
            parent: Some("sys.app".into()),
            id: "manual".into(),
            name: "Manual".into(),
            kind: "component".into(),
        })
        .unwrap();
    engine
        .apply(Operation::SetField { id: "sys.app.manual".into(), field: "description".into(), value: "hand-modeled".into() })
        .unwrap();
}

#[test]
fn stale_and_missing_facts_are_diagnosed_not_fatal() {
    let t = temp("diagnostics");
    rust_fixture(&t.dir);
    let ws_dir = t.dir.join("docs");
    write(&t.dir, "docs/blastradius.yaml", MANIFEST);
    write(&t.dir, "docs/model/sys.yaml", MODEL);
    // Facts for a component that no longer exists: warning, safe to delete.
    write(
        &t.dir,
        "docs/model/derived/sys.app.gone.l4.json",
        "{\n  \"schema\": 1,\n  \"language\": \"rust\",\n  \"extractor\": \"x\",\n  \"component\": \"sys.app.gone\",\n  \"root\": \"src\",\n  \"sourceDigest\": \"sha256:0\",\n  \"elements\": [],\n  \"edges\": []\n}\n",
    );

    let (ws, diags) = blastradius_core::load_workspace(&ws_dir);
    assert!(!blastradius_core::diagnostics::has_errors(&diags), "{diags:?}");
    assert!(ws.derived.is_empty());
    let msgs: Vec<&str> = diags.iter().map(|d| d.message.as_str()).collect();
    assert!(msgs.iter().any(|m| m.contains("does not exist") && m.contains("safe to delete")), "{msgs:?}");
    // The mapped component has no facts yet: info nudge to run introspect.
    assert!(msgs.iter().any(|m| m.contains("no facts yet")), "{msgs:?}");
}

#[test]
fn staleness_probe_tracks_the_working_tree() {
    let t = temp("stale");
    rust_fixture(&t.dir);
    let (facts, _) = extract_rust(&t.dir, "sys.app.comp", &mapping()).unwrap();
    assert!(!introspect::is_stale(&t.dir, &mapping(), &facts.source_digest));
    write(&t.dir, "src/new.rs", "pub struct New;\n");
    assert!(introspect::is_stale(&t.dir, &mapping(), &facts.source_digest));
}

/// A `source:` mapping written on a *container* used to vanish without a
/// word: introspection is component-level, so the key was never read and YAML
/// ignores what it does not know. Someone wrote one, watched nothing happen,
/// and had to work out why (reported 2026-08-26).
#[test]
fn a_source_mapping_on_a_container_says_it_is_ignored() {
    let t = temp("container-source");
    let repo = &t.dir;
    write(repo, "blastradius.yaml", "workspace:\n  name: T\n  version: 1\nmodel:\n  include: [model/*.yaml]\n");
    write(
        repo,
        "model/shop.yaml",
        "system: shop\nname: Shop\ncontainers:\n  backend:\n    name: Backend\n    source:\n      language: csharp\n      root: src\n    components:\n      billing: { name: Billing }\n",
    );

    let (_ws, diags) = blastradius_core::load_workspace(repo);
    let msg = diags.iter().map(|d| d.to_string()).collect::<Vec<_>>().join("\n");
    assert!(msg.contains("shop.backend"), "no diagnostic names the container:\n{msg}");
    assert!(msg.contains("component-level"), "the diagnostic does not explain why:\n{msg}");
    // A warning, not an error: the rest of the model is perfectly good.
    assert!(
        !blastradius_core::diagnostics::has_errors(&diags),
        "an ignored mapping must not invalidate the workspace:\n{msg}"
    );
}
