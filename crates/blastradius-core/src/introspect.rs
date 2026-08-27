//! L4 introspection (ADR-0016, spec/l4-introspection.md): per-language
//! extractors emit a common facts JSON; core validates, canonicalizes, and
//! grafts read-only derived elements beneath the opted-in component.
//!
//! The Rust extractor lives here (`syn` is an ordinary library — the one
//! language where "spawn the extractor" is a function call). TypeScript and
//! C# run out-of-process in their own runtimes; core enforces the
//! determinism contract for all three by canonicalizing before writing.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::diagnostics::Diagnostic;
use crate::model::{DerivedEdge, DerivedElement, DerivedGraph, ElementKind, SourceMapping, Workspace};
use crate::vfs::Vfs;

pub const FACTS_SCHEMA: u64 = 1;
/// Workspace-relative directory holding committed facts files.
pub const DERIVED_DIR: &str = "model/derived";

// ---- facts file ------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize)]
pub struct Facts {
    pub schema: u64,
    pub language: String,
    pub extractor: String,
    pub component: String,
    pub root: String,
    #[serde(rename = "sourceDigest")]
    pub source_digest: String,
    pub elements: Vec<FactElement>,
    pub edges: Vec<FactEdge>,
    /// References that leave this component's mapped corpus but stay inside
    /// the repository — the raw material for drift detection (ADR-0019).
    /// Recorded as the repo-relative file they point at, because which
    /// *component* owns that file is a question only the whole workspace can
    /// answer. Additive: facts written before 0.5.0 simply have none.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub outbound: Vec<FactOutbound>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Clone)]
pub struct FactOutbound {
    /// Element id inside this component that holds the reference.
    pub from: String,
    /// Repo-root-relative path of the file referenced, forward slashes.
    pub path: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FactElement {
    pub id: String,
    /// module | namespace | class | interface | record | enum
    pub kind: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct FactEdge {
    pub from: String,
    pub to: String,
    /// imports | references | extends | implements
    pub kind: String,
}

pub fn facts_rel(component: &str) -> String {
    format!("{DERIVED_DIR}/{component}.l4.json")
}

/// Enforce the determinism contract centrally: sorted, deduped. Extractors
/// may emit in any order; the bytes on disk come from here.
pub fn canonicalize(f: &mut Facts) {
    f.elements.sort_by(|a, b| a.id.cmp(&b.id));
    f.elements.dedup_by(|a, b| a.id == b.id);
    f.edges.sort();
    f.edges.dedup();
    // Self-edges say nothing at L4.
    f.edges.retain(|e| e.from != e.to);
    f.outbound.sort();
    f.outbound.dedup();
}

/// The exact committed bytes: 2-space pretty JSON, LF, trailing newline.
pub fn facts_bytes(f: &Facts) -> String {
    let mut s = serde_json::to_string_pretty(f).expect("facts serialize");
    s.push('\n');
    s
}

// ---- loading committed facts into the workspace ----------------------------

/// Read every `model/derived/*.l4.json`, validate against the model, and
/// graft `ws.derived`. Never fatal: a bad facts file is a warning and the
/// workspace loads without it (spec: committed artifacts may lag).
pub fn load_derived(vfs: &dyn Vfs, ws: &mut Workspace, diags: &mut Vec<Diagnostic>) {
    let mut seen: BTreeSet<String> = BTreeSet::new();
    for (name, is_dir) in vfs.list(DERIVED_DIR) {
        if is_dir || !name.ends_with(".l4.json") {
            continue;
        }
        let rel = format!("{DERIVED_DIR}/{name}");
        let text = match vfs.read(&rel) {
            Ok(t) => t,
            Err(e) => {
                diags.push(Diagnostic::warning(&rel as &str, 1, format!("unreadable facts file: {e}")));
                continue;
            }
        };
        let facts: Facts = match serde_json::from_str(&text) {
            Ok(f) => f,
            Err(e) => {
                diags.push(Diagnostic::warning(&rel as &str, 1, format!("invalid facts JSON: {e}")));
                continue;
            }
        };
        if facts.schema != FACTS_SCHEMA {
            diags.push(Diagnostic::warning(
                &rel as &str,
                1,
                format!("facts schema {} not supported (this build understands {FACTS_SCHEMA})", facts.schema),
            ));
            continue;
        }
        let expected = format!("{}.l4.json", facts.component);
        if name != expected {
            diags.push(Diagnostic::warning(&rel as &str, 1, format!("facts file name should be {expected:?}")));
        }
        let Some(comp) = ws.elements.get(&facts.component) else {
            diags.push(Diagnostic::warning(
                &rel as &str,
                1,
                format!("stale facts: component {:?} does not exist — safe to delete", facts.component),
            ));
            continue;
        };
        let Some(mapping) = &comp.source else {
            diags.push(Diagnostic::warning(
                &rel as &str,
                1,
                format!("stale facts: component {:?} has no `source:` mapping — safe to delete", facts.component),
            ));
            continue;
        };
        if mapping.language != facts.language || mapping.root != facts.root {
            diags.push(Diagnostic::warning(
                &rel as &str,
                1,
                format!(
                    "facts were extracted for {} at {:?} but the mapping now says {} at {:?} — re-run introspect",
                    facts.language, facts.root, mapping.language, mapping.root
                ),
            ));
        }
        seen.insert(facts.component.clone());
        ws.derived.push(graft(&facts));
    }
    // A mapping with no committed facts: gentle nudge, not a problem.
    for el in ws.elements.values() {
        if el.kind == ElementKind::Component && el.source.is_some() && !seen.contains(&el.id) {
            diags.push(Diagnostic::info(
                &el.file as &str,
                el.line,
                format!("component {:?} opts into introspection but has no facts yet — run `blastradius introspect`", el.id),
            ));
        }
    }
    ws.derived.sort_by(|a, b| a.component.cmp(&b.component));
}

/// Fact ids become full dotted ids under the reserved `.src.` segment, which
/// keeps derived and hand-modeled children disjoint forever (spec).
fn graft(f: &Facts) -> DerivedGraph {
    let full = |fact_id: &str| format!("{}.src.{}", f.component, fact_id);
    DerivedGraph {
        component: f.component.clone(),
        language: f.language.clone(),
        source_digest: f.source_digest.clone(),
        stale: false,
        outbound: f
            .outbound
            .iter()
            .map(|o| (full(&o.from), o.path.clone()))
            .collect(),
        elements: f
            .elements
            .iter()
            .map(|e| DerivedElement {
                id: full(&e.id),
                kind: e.kind.clone(),
                name: e.name.clone(),
                parent: e.parent.as_deref().map(full),
                path: e.path.clone(),
                line: e.line,
            })
            .collect(),
        edges: f.edges.iter().map(|e| DerivedEdge { from: full(&e.from), to: full(&e.to), kind: e.kind.clone() }).collect(),
    }
}

// ---- source file collection -------------------------------------------------

/// Directories no extractor ever descends into.
fn skip_dir(name: &str) -> bool {
    name.starts_with('.') || matches!(name, "target" | "node_modules" | "bin" | "obj" | "dist" | "build" | "out" | "vendor")
}

pub fn glob_set(globs: &[String], label: &str) -> Result<Option<globset::GlobSet>, String> {
    if globs.is_empty() {
        return Ok(None);
    }
    let mut b = globset::GlobSetBuilder::new();
    for g in globs {
        b.add(globset::Glob::new(g).map_err(|e| format!("bad {label} glob {g:?}: {e}"))?);
    }
    Ok(Some(b.build().map_err(|e| format!("bad {label} globs: {e}"))?))
}

/// Files under `repo_root/mapping.root` matching the mapping, as sorted
/// (root-relative forward-slash path, content) pairs. `default_include`
/// is the language's extension filter, applied when the mapping has none.
pub fn collect_files(
    repo_root: &Path,
    mapping: &SourceMapping,
    default_include: &[String],
) -> Result<Vec<(String, String)>, String> {
    let root = repo_root.join(&mapping.root);
    if !root.is_dir() {
        return Err(format!("source root {:?} does not exist under the repo root", mapping.root));
    }
    let include = glob_set(if mapping.include.is_empty() { default_include } else { &mapping.include }, "include")?;
    let exclude = glob_set(&mapping.exclude, "exclude")?;
    let mut out = Vec::new();
    let mut stack: Vec<PathBuf> = vec![root.clone()];
    while let Some(dir) = stack.pop() {
        let entries = std::fs::read_dir(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            if path.is_dir() {
                if !skip_dir(&name) {
                    stack.push(path);
                }
                continue;
            }
            let rel = path
                .strip_prefix(&root)
                .expect("under root")
                .to_string_lossy()
                .replace('\\', "/");
            if let Some(inc) = &include {
                if !inc.is_match(&rel) {
                    continue;
                }
            }
            if let Some(exc) = &exclude {
                if exc.is_match(&rel) {
                    continue;
                }
            }
            let text = std::fs::read_to_string(&path).map_err(|e| format!("{rel}: {e}"))?;
            out.push((rel, text));
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(out)
}

/// sha256 over the sorted (path, content-sha256) list — the staleness probe.
pub fn source_digest(files: &[(String, String)]) -> String {
    let mut h = Sha256::new();
    for (rel, text) in files {
        h.update(rel.as_bytes());
        h.update(b"\n");
        h.update(Sha256::digest(text.as_bytes()));
        h.update(b"\n");
    }
    format!("sha256:{:x}", h.finalize())
}

/// Recompute the digest from the working tree and compare — cheap staleness
/// check without running any extractor. Errors (missing root, unreadable
/// files) count as stale: the facts no longer describe reachable reality.
pub fn is_stale(repo_root: &Path, mapping: &SourceMapping, facts_digest: &str) -> bool {
    let defaults = default_include(&mapping.language);
    match collect_files(repo_root, mapping, &defaults) {
        Ok(files) => source_digest(&files) != facts_digest,
        Err(_) => true,
    }
}

/// Mark each derived graph's staleness against the working tree — cheap
/// (hashing only), used by the app so the canvas can badge lagging facts.
pub fn mark_stale(repo_root: &Path, ws: &mut Workspace) {
    let mappings: std::collections::HashMap<String, SourceMapping> = ws
        .elements
        .values()
        .filter_map(|e| e.source.clone().map(|s| (e.id.clone(), s)))
        .collect();
    for g in &mut ws.derived {
        if let Some(m) = mappings.get(&g.component) {
            g.stale = is_stale(repo_root, m, &g.source_digest);
        }
    }
}

pub fn default_include(language: &str) -> Vec<String> {
    let exts: &[&str] = match language {
        "rust" => &["**/*.rs"],
        "typescript" => &["**/*.ts", "**/*.tsx", "**/*.mts", "**/*.js", "**/*.mjs", "**/*.jsx"],
        "csharp" => &["**/*.cs"],
        _ => &[],
    };
    exts.iter().map(|s| s.to_string()).collect()
}

// ---- the built-in Rust extractor -------------------------------------------

const RUST_EXTRACTOR_VERSION: &str = "blastradius-extract-rust 0.3.0";

/// Id prefix for external-dependency rollups — one node per package, shared
/// by every extractor (spec).
pub const DEP_PREFIX: &str = "dep.";
/// Element kind for those rollups.
pub const DEP_KIND: &str = "dependency";

struct RustCorpus {
    /// module id -> (path, name)
    modules: BTreeMap<String, (String, String)>,
    /// type id -> (kind, module id, path, line, name)
    types: BTreeMap<String, TypeInfo>,
    /// unqualified type name -> ids
    by_name: BTreeMap<String, Vec<String>>,
    /// module id -> (name it re-exports -> defining type id), from `pub use`
    exports: BTreeMap<String, BTreeMap<String, String>>,
}

struct TypeInfo {
    kind: &'static str,
    module: String,
    path: String,
    line: u64,
    name: String,
}

/// Module id from a root-relative file path: minus `.rs`, slashes to dots,
/// `foo/mod.rs` folds to `foo` (spec).
fn module_id(rel: &str) -> String {
    let stem = rel.strip_suffix(".rs").unwrap_or(rel);
    let stem = stem.strip_suffix("/mod").unwrap_or(stem);
    stem.replace('/', ".")
}

pub fn extract_rust(repo_root: &Path, component: &str, mapping: &SourceMapping) -> Result<(Facts, Vec<String>), String> {
    let files = collect_files(repo_root, mapping, &default_include("rust"))?;
    let digest = source_digest(&files);
    let mut warnings = Vec::new();

    // Pass 1: modules and types.
    let mut corpus = RustCorpus {
        modules: BTreeMap::new(),
        types: BTreeMap::new(),
        by_name: BTreeMap::new(),
        exports: BTreeMap::new(),
    };
    let mut parsed: Vec<(String, String, syn::File)> = Vec::new(); // (module id, rel, ast)
    for (rel, text) in &files {
        // The module element exists even when the file does not parse — the
        // file is real; only its contents are unknown (warned, never fatal).
        let mid = module_id(rel);
        let file_name = rel.rsplit('/').next().unwrap_or(rel).to_string();
        let repo_rel = format!("{}/{rel}", mapping.root.trim_end_matches('/'));
        corpus.modules.insert(mid.clone(), (repo_rel.clone(), file_name));
        match syn::parse_file(text) {
            Ok(ast) => {
                collect_items(&ast.items, &mid, &repo_rel, &mut corpus);
                parsed.push((mid, repo_rel, ast));
            }
            Err(e) => warnings.push(format!("{rel}: does not parse ({e}) — items skipped")),
        }
    }
    for (id, info) in &corpus.types {
        corpus.by_name.entry(info.name.clone()).or_default().push(id.clone());
    }

    // Pass 2: the re-export table, so pass 3 can see through façade modules.
    build_exports(&parsed, &mut corpus);

    // Pass 3: edges, plus the external crates they reach for.
    let mut edges: BTreeSet<(String, String, String)> = BTreeSet::new();
    let mut deps: BTreeSet<String> = BTreeSet::new();
    let mut outside: BTreeSet<(String, String)> = BTreeSet::new();
    for (mid, _rel, ast) in &parsed {
        collect_edges(&ast.items, mid, &corpus, &mut edges, &mut deps, &mut outside);
    }

    let mut elements: Vec<FactElement> = corpus
        .modules
        .iter()
        .map(|(id, (path, name))| FactElement {
            id: id.clone(),
            kind: "module".into(),
            name: name.clone(),
            parent: parent_module(id, &corpus),
            path: path.clone(),
            line: None,
        })
        .collect();
    elements.extend(corpus.types.iter().map(|(id, t)| FactElement {
        id: id.clone(),
        kind: t.kind.into(),
        name: t.name.clone(),
        parent: Some(t.module.clone()),
        path: t.path.clone(),
        line: Some(t.line),
    }));
    // External dependencies roll up to one parentless node each: they sit
    // beside the modules at the top of the derived scene, and have no path
    // because they are not part of the mapped source tree (spec).
    elements.extend(deps.iter().map(|name| FactElement {
        id: format!("{DEP_PREFIX}{name}"),
        kind: DEP_KIND.into(),
        name: name.clone(),
        parent: None,
        path: String::new(),
        line: None,
    }));

    // Turn each unresolved crate-relative path into the file it names, so the
    // workspace can later ask which component owns that file (ADR-0019).
    let outbound = outside
        .iter()
        .filter_map(|(from, path)| {
            rust_module_file(repo_root, &mapping.root, path).map(|p| FactOutbound { from: from.clone(), path: p })
        })
        .collect();

    let mut facts = Facts {
        schema: FACTS_SCHEMA,
        language: "rust".into(),
        extractor: RUST_EXTRACTOR_VERSION.into(),
        component: component.to_string(),
        root: mapping.root.clone(),
        source_digest: digest,
        elements,
        edges: edges.into_iter().map(|(from, to, kind)| FactEdge { from, to, kind }).collect(),
        outbound,
    };
    canonicalize(&mut facts);
    Ok((facts, warnings))
}

/// The file a crate-relative module path names, if it exists.
///
/// `crate::model::Workspace` arrives here as `model/Workspace`; the trailing
/// segments may be types rather than modules, so prefixes are tried longest
/// first and the first file that exists wins. Returned repo-root-relative
/// with forward slashes — the form the whole model speaks (ADR-0019).
fn rust_module_file(repo_root: &Path, root: &str, path: &str) -> Option<String> {
    let segs: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    let base = root.trim_end_matches('/');
    for cut in (1..=segs.len()).rev() {
        let stem = segs[..cut].join("/");
        for candidate in [format!("{stem}.rs"), format!("{stem}/mod.rs")] {
            let rel = format!("{base}/{candidate}");
            if repo_root.join(rel.replace('/', std::path::MAIN_SEPARATOR_STR)).is_file() {
                return Some(rel);
            }
        }
    }
    None
}

/// Nesting for the canvas: a module whose id extends another module's id by
/// one dotted segment nests under it (e.g. `model.loader` under `model`).
fn parent_module(id: &str, corpus: &RustCorpus) -> Option<String> {
    let (parent, _) = id.rsplit_once('.')?;
    corpus.modules.contains_key(parent).then(|| parent.to_string())
}

fn type_kind(item: &syn::Item) -> Option<(&'static str, &syn::Ident)> {
    match item {
        syn::Item::Struct(s) => Some(("class", &s.ident)),
        syn::Item::Enum(e) => Some(("enum", &e.ident)),
        syn::Item::Trait(t) => Some(("interface", &t.ident)),
        _ => None,
    }
}

fn collect_items(items: &[syn::Item], module: &str, path: &str, corpus: &mut RustCorpus) {
    use syn::spanned::Spanned;
    for item in items {
        if let Some((kind, ident)) = type_kind(item) {
            let id = format!("{module}.{ident}");
            corpus.types.insert(
                id,
                TypeInfo {
                    kind,
                    module: module.to_string(),
                    path: path.to_string(),
                    line: item.span().start().line as u64,
                    name: ident.to_string(),
                },
            );
        } else if let syn::Item::Mod(m) = item {
            // Inline `mod` blocks become nested modules (spec).
            if let Some((_, inner)) = &m.content {
                let mid = format!("{module}.{}", m.ident);
                corpus.modules.insert(mid.clone(), (path.to_string(), m.ident.to_string()));
                collect_items(inner, &mid, path, corpus);
            }
        }
    }
}

/// Resolve a `use` path (or a reference path prefix) against the corpus:
/// `crate::` anchors at the file-tree root, `self::`/`super::` at the current
/// module. Returns the target type id if the path lands on a corpus type,
/// else the target module id if it lands on a module. Ambiguity = None
/// (dropped, not guessed — spec).
fn resolve_path(segments: &[String], module: &str, corpus: &RustCorpus) -> Option<Resolved> {
    let mut segs: &[String] = segments;
    let mut base: Vec<&str> = Vec::new();
    let anchored = matches!(segs.first().map(String::as_str), Some("crate" | "self" | "super"));
    match segs.first().map(String::as_str) {
        Some("crate") => segs = &segs[1..],
        Some("self") => {
            base = module.split('.').collect();
            segs = &segs[1..];
        }
        Some("super") => {
            base = module.split('.').collect();
            while segs.first().map(String::as_str) == Some("super") {
                base.pop()?;
                segs = &segs[1..];
            }
        }
        _ => {
            // Bare path: could be a sibling module of the crate root or an
            // external crate — try the root anchor, otherwise give up.
        }
    }
    let full: Vec<String> = base.iter().map(|s| s.to_string()).chain(segs.iter().cloned()).collect();
    if full.is_empty() {
        return None;
    }
    // Longest prefix that names a corpus module; the next segment may be a type.
    for cut in (1..=full.len()).rev() {
        let mid = full[..cut].join(".");
        if corpus.modules.contains_key(&mid) {
            if cut < full.len() {
                let tid = format!("{mid}.{}", full[cut]);
                if corpus.types.contains_key(&tid) {
                    return Some(Resolved::Type(tid));
                }
                // Not defined here — but the module may re-export it, in which
                // case the dependency is on whoever defines it (spec).
                if let Some(tid) = corpus.exports.get(&mid).and_then(|e| e.get(&full[cut])) {
                    return Some(Resolved::Type(tid.clone()));
                }
            }
            return Some(Resolved::Module(mid));
        }
    }
    // Anchored but unresolved: real code in this repo that this corpus cannot
    // see. An unanchored bare path is an external crate instead (below).
    anchored.then(|| Resolved::Outside(full))
}

enum Resolved {
    Module(String),
    Type(String),
    /// Anchored inside this crate (`crate::`/`self::`/`super::`) but not in
    /// this component's corpus — it belongs to some other part of the repo,
    /// which is exactly what drift detection needs to know (ADR-0019).
    Outside(Vec<String>),
}

/// Flatten a use tree into `(path segments, name it binds locally)` pairs —
/// the binding differs from the last segment under `as` renames.
fn use_paths(tree: &syn::UseTree, prefix: &mut Vec<String>, out: &mut Vec<(Vec<String>, String)>) {
    match tree {
        syn::UseTree::Path(p) => {
            prefix.push(p.ident.to_string());
            use_paths(&p.tree, prefix, out);
            prefix.pop();
        }
        syn::UseTree::Name(n) => {
            let mut full = prefix.clone();
            full.push(n.ident.to_string());
            out.push((full, n.ident.to_string()));
        }
        syn::UseTree::Rename(r) => {
            let mut full = prefix.clone();
            full.push(r.ident.to_string());
            out.push((full, r.rename.to_string()));
        }
        syn::UseTree::Group(g) => {
            for t in &g.items {
                use_paths(t, prefix, out);
            }
        }
        syn::UseTree::Glob(_) => {} // glob imports dropped (spec)
    }
}

/// Every `pub`/`pub(…)` use in a file, as `(module id, bound name, path)` —
/// the raw material for the re-export table. Restricted visibilities count:
/// `pub(crate) use` is visible to every other module in this corpus.
fn collect_pub_uses(items: &[syn::Item], module: &str, out: &mut Vec<(String, String, Vec<String>)>) {
    for item in items {
        match item {
            syn::Item::Use(u) => {
                if matches!(u.vis, syn::Visibility::Public(_) | syn::Visibility::Restricted(_)) {
                    let mut paths = Vec::new();
                    use_paths(&u.tree, &mut Vec::new(), &mut paths);
                    for (segs, binding) in paths {
                        out.push((module.to_string(), binding, segs));
                    }
                }
            }
            syn::Item::Mod(m) => {
                if let Some((_, inner)) = &m.content {
                    collect_pub_uses(inner, &format!("{module}.{}", m.ident), out);
                }
            }
            _ => {}
        }
    }
}

/// Build the re-export table by fixpoint, so a name re-exported through a
/// chain of façades resolves to the module that actually defines it.
///
/// Each round resolves the `pub use` paths that the table can already answer;
/// a round that adds nothing ends it. Additions are monotone over `BTreeMap`s,
/// so the result is deterministic and a re-export cycle simply never resolves
/// (dropped, not guessed — spec).
fn build_exports(parsed: &[(String, String, syn::File)], corpus: &mut RustCorpus) {
    let mut pending: Vec<(String, String, Vec<String>)> = Vec::new();
    for (mid, _rel, ast) in parsed {
        collect_pub_uses(&ast.items, mid, &mut pending);
    }
    let mut done = vec![false; pending.len()];
    loop {
        let mut round: Vec<(usize, String)> = Vec::new();
        for (i, (module, _binding, segs)) in pending.iter().enumerate() {
            if done[i] {
                continue;
            }
            if let Some(Resolved::Type(tid)) = resolve_path(segs, module, corpus) {
                round.push((i, tid));
            }
        }
        if round.is_empty() {
            break;
        }
        for (i, tid) in round {
            done[i] = true;
            let (module, binding, _) = &pending[i];
            corpus.exports.entry(module.clone()).or_default().insert(binding.clone(), tid);
        }
    }
}

/// Crates that ship with the toolchain: present in every program, so they
/// carry no architectural signal and are not rolled up (spec).
const RUST_SYSROOT_CRATES: [&str; 5] = ["std", "core", "alloc", "proc_macro", "test"];

/// The external crate a `use` path names, if it names one: an unresolved
/// path whose first segment is neither a crate anchor nor the sysroot.
fn external_crate(path: &[String]) -> Option<&str> {
    let first = path.first()?.as_str();
    let anchored = matches!(first, "crate" | "self" | "super");
    (!anchored && !RUST_SYSROOT_CRATES.contains(&first)).then_some(first)
}

fn collect_edges(
    items: &[syn::Item],
    module: &str,
    corpus: &RustCorpus,
    edges: &mut BTreeSet<(String, String, String)>,
    deps: &mut BTreeSet<String>,
    outside: &mut BTreeSet<(String, String)>,
) {
    // File-level use map: unqualified name -> corpus type id.
    let mut use_map: BTreeMap<String, String> = BTreeMap::new();
    for item in items {
        if let syn::Item::Use(u) = item {
            let mut out = Vec::new();
            use_paths(&u.tree, &mut Vec::new(), &mut out);
            for (path, binding) in out {
                match resolve_path(&path, module, corpus) {
                    Some(Resolved::Type(tid)) => {
                        let target_module = corpus.types[&tid].module.clone();
                        if target_module != module {
                            edges.insert((module.to_string(), target_module, "imports".into()));
                        }
                        use_map.insert(binding, tid);
                    }
                    Some(Resolved::Outside(full)) => {
                        // Real code in this repo that this corpus cannot see.
                        // Which component owns it is a workspace-level question
                        // (ADR-0019), so the crate-relative path is recorded raw.
                        outside.insert((module.to_string(), full.join("/")));
                    }
                    Some(Resolved::Module(mid)) => {
                        if mid != module {
                            edges.insert((module.to_string(), mid, "imports".into()));
                        }
                    }
                    None => {
                        // Unresolved and not anchored in this crate: an
                        // external dependency, named by its first segment.
                        // A corpus id of the same name wins — the rollup is
                        // skipped rather than colliding (spec).
                        if let Some(name) = external_crate(&path) {
                            let id = format!("{DEP_PREFIX}{name}");
                            if !corpus.modules.contains_key(&id) && !corpus.types.contains_key(&id) {
                                deps.insert(name.to_string());
                                edges.insert((module.to_string(), id, "imports".into()));
                            }
                        }
                    }
                }
            }
        }
    }

    for item in items {
        match item {
            syn::Item::Impl(imp) => {
                let self_id = impl_self_type(imp, module, corpus, &use_map);
                let ctx = self_id.clone().unwrap_or_else(|| module.to_string());
                if let (Some(sid), Some((_, tp, _))) = (&self_id, imp.trait_.as_ref()) {
                    if let Some(tid) = resolve_type_path(tp, module, corpus, &use_map) {
                        if &tid != sid {
                            edges.insert((sid.clone(), tid, "implements".into()));
                        }
                    }
                }
                collect_refs(item, &ctx, module, corpus, &use_map, edges);
            }
            _ => {
                let ctx = type_kind(item)
                    .map(|(_, ident)| format!("{module}.{ident}"))
                    .unwrap_or_else(|| module.to_string());
                if let syn::Item::Mod(m) = item {
                    if let Some((_, inner)) = &m.content {
                        collect_edges(inner, &format!("{module}.{}", m.ident), corpus, edges, deps, outside);
                        continue;
                    }
                }
                collect_refs(item, &ctx, module, corpus, &use_map, edges);
            }
        }
    }
}

fn impl_self_type(
    imp: &syn::ItemImpl,
    module: &str,
    corpus: &RustCorpus,
    use_map: &BTreeMap<String, String>,
) -> Option<String> {
    if let syn::Type::Path(tp) = imp.self_ty.as_ref() {
        return resolve_type_path(&tp.path, module, corpus, use_map);
    }
    None
}

/// Resolve a referenced type path: qualified paths through the corpus,
/// unqualified names via the file's use map, the current module, then a
/// unique corpus-wide name match. Ambiguous or external: None.
fn resolve_type_path(
    path: &syn::Path,
    module: &str,
    corpus: &RustCorpus,
    use_map: &BTreeMap<String, String>,
) -> Option<String> {
    let segs: Vec<String> = path.segments.iter().map(|s| s.ident.to_string()).collect();
    if segs.len() > 1 {
        return match resolve_path(&segs, module, corpus)? {
            Resolved::Type(tid) => Some(tid),
            Resolved::Module(_) | Resolved::Outside(_) => None,
        };
    }
    let name = segs.first()?;
    if !name.chars().next().is_some_and(char::is_uppercase) {
        return None;
    }
    if let Some(tid) = use_map.get(name) {
        return Some(tid.clone());
    }
    let local = format!("{module}.{name}");
    if corpus.types.contains_key(&local) {
        return Some(local);
    }
    match corpus.by_name.get(name).map(Vec::as_slice) {
        Some([only]) => Some(only.clone()),
        _ => None, // absent or ambiguous
    }
}

/// Walk one item's full syntax for type-path references (signatures and
/// bodies — spec), attributed to `ctx` (the containing type, or the module).
fn collect_refs(
    item: &syn::Item,
    ctx: &str,
    module: &str,
    corpus: &RustCorpus,
    use_map: &BTreeMap<String, String>,
    edges: &mut BTreeSet<(String, String, String)>,
) {
    use syn::visit::Visit;
    struct V<'a> {
        ctx: &'a str,
        module: &'a str,
        corpus: &'a RustCorpus,
        use_map: &'a BTreeMap<String, String>,
        edges: &'a mut BTreeSet<(String, String, String)>,
    }
    impl<'a> Visit<'a> for V<'_> {
        fn visit_path(&mut self, p: &'a syn::Path) {
            if let Some(tid) = resolve_type_path(p, self.module, self.corpus, self.use_map) {
                if tid != self.ctx {
                    self.edges.insert((self.ctx.to_string(), tid, "references".into()));
                }
            }
            syn::visit::visit_path(self, p);
        }
        fn visit_item_mod(&mut self, _m: &'a syn::ItemMod) {
            // Inline modules are walked separately with their own module id.
        }
    }
    if matches!(item, syn::Item::Use(_)) {
        return; // already produced imports edges
    }
    V { ctx, module, corpus, use_map, edges }.visit_item(item);
}

// ---- external extractors (typescript, csharp) ------------------------------

/// What core sends an out-of-process extractor on stdin.
#[derive(Serialize)]
struct ExtractorInput<'a> {
    component: &'a str,
    #[serde(rename = "repoRoot")]
    repo_root: &'a str,
    root: &'a str,
    include: &'a [String],
    exclude: &'a [String],
    /// Omitted unless the mapping asks for one, so older extractors and the
    /// TypeScript one see exactly the input they saw before.
    #[serde(skip_serializing_if = "Option::is_none")]
    mode: Option<&'a str>,
}

/// True if we can create a file in `dir` — the proxy for "this install
/// directory is an ordinary place from which things can run".
fn writable(dir: &Path) -> bool {
    let probe = dir.join(".blastradius-write-probe");
    match std::fs::write(&probe, b"") {
        Ok(()) => {
            let _ = std::fs::remove_file(&probe);
            true
        }
        Err(_) => false,
    }
}

fn copy_tree(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let dest = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_tree(&entry.path(), &dest)?;
        } else {
            std::fs::copy(entry.path(), &dest)?;
        }
    }
    Ok(())
}

/// Where a private copy of an extractor lives. Persistent per user where the
/// OS gives us somewhere, else the temp dir (same fallback the journal uses).
fn extractor_cache() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    base.join("Blastradius").join(format!("extractors-{}", env!("CARGO_PKG_VERSION")))
}

/// Make an extractor directory runnable, copying it out of a restricted
/// install location if it is not one already.
///
/// A Store install lives under `C:\Program Files\WindowsApps\...`, whose ACLs
/// let an outside process *read* a file but not load it as an assembly. The
/// C# extractor runs inside `dotnet.exe`, which is not part of our package, so
/// the CLR refuses it outright:
///
/// ```text
/// Could not load file or assembly '...\extractors\dotnet\BlastradiusExtract.dll'.
/// Access is denied.
/// ```
///
/// Publishing the extractor (0.6.1) was necessary but not sufficient — nothing
/// can execute it from in there at all. Node reads and runs the TypeScript
/// `.mjs` from the same directory happily, so this is specific to assembly
/// loading and only the C# extractor needs it.
///
/// The copy happens once per version, on the first C# introspection, and is
/// keyed by version because a package's contents are immutable per version.
fn runnable_dir(dir: &Path) -> PathBuf {
    if writable(dir) {
        return dir.to_path_buf();
    }
    let leaf = dir.file_name().unwrap_or_default();
    let staged = extractor_cache().join(leaf);
    // Trust an existing copy: same version, same immutable package.
    if staged.join(CSHARP_DLL).is_file() {
        return staged;
    }
    match copy_tree(dir, &staged) {
        Ok(()) => staged,
        // Fall back to the original: it may still work (a read-only directory
        // is not automatically a restricted one), and if it does not, the
        // extractor's own error is more useful than one about the copy.
        Err(_) => dir.to_path_buf(),
    }
}

const CSHARP_DLL: &str = "BlastradiusExtract.dll";

/// Locate the default extractor entry: beside the running binary first
/// (installed layout), then in the repo (dev layout) — spec.
fn default_command(language: &str, repo_root: &Path) -> Result<Vec<String>, String> {
    let candidates: Vec<PathBuf> = {
        let mut dirs = Vec::new();
        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                dirs.push(dir.join("extractors"));
            }
        }
        dirs.push(repo_root.join("extractors"));
        dirs
    };
    // Each language lists its entries in preference order. C# ships published
    // in a release (`dotnet <dll>`, runtime only, nothing to build) and lives
    // as a project in a checkout (`dotnet run`, which writes bin/ and obj/ and
    // so cannot work from a read-only install directory) — tools/stage-extractors.mjs.
    let entries: &[&str] = match language {
        "typescript" => &["typescript/extract.mjs"],
        "csharp" => &["dotnet/BlastradiusExtract.dll", "dotnet/BlastradiusExtract.csproj"],
        other => return Err(format!("no extractor for language {other:?}")),
    };
    for dir in &candidates {
        for entry in entries {
            let path = dir.join(entry);
            if !path.exists() {
                continue;
            }
            if language == "typescript" {
                return Ok(vec!["node".into(), path.to_string_lossy().into_owned()]);
            }
            // The published DLL may be somewhere nothing can load it from
            // (see runnable_dir) — resolve to a copy that works.
            if entry.ends_with(".dll") {
                let home = runnable_dir(&path.parent().expect("entry has a parent").to_path_buf());
                return Ok(vec!["dotnet".into(), home.join(CSHARP_DLL).to_string_lossy().into_owned()]);
            }
            return Ok(vec![
                "dotnet".into(), "run".into(), "--project".into(),
                path.to_string_lossy().into_owned(), "-c".into(), "Release".into(),
            ]);
        }
    }
    // Reported as "no csharp extractor found" and nothing else for five
    // releases, which told the first person to hit it nothing actionable.
    let needs = match language {
        "typescript" => "Node on PATH",
        _ => "the .NET runtime on PATH (opt-in semantic mode additionally needs an SDK)",
    };
    Err(format!(
        "no {language} extractor found. {language} introspection runs out of process, from an \
         `extractors/` directory beside the blastradius binary or at the repository root; looked \
         for {} under {}. Rust needs no extractor — it is built in. Install a release that ships \
         extractors/, or run from a checkout. Running it also needs {needs}.",
        entries.join(" or "),
        candidates.iter().map(|d| d.display().to_string()).collect::<Vec<_>>().join(", ")
    ))
}

/// Where to run an extractor from: the directory of the entry the argv points
/// at (`node <path>/extract.mjs`, `dotnet run --project <dir>`). None when the
/// command is a bare override we cannot read a path out of.
fn extractor_dir(prog: &str, args: &[String]) -> Option<PathBuf> {
    let entry = match prog {
        // `dotnet run --project <dir>`, or `dotnet <published dll>`.
        "dotnet" => args
            .iter()
            .position(|a| a == "--project")
            .and_then(|i| args.get(i + 1))
            .or_else(|| args.first().filter(|a| a.ends_with(".dll"))),
        _ => args.first(),
    }?;
    let path = Path::new(entry);
    let dir = if path.is_dir() { path } else { path.parent()? };
    dir.is_dir().then(|| dir.to_path_buf())
}

pub fn run_external(repo_root: &Path, component: &str, mapping: &SourceMapping) -> Result<Facts, String> {
    use std::io::Write;
    use std::process::{Command, Stdio};

    let argv = match &mapping.extractor {
        Some(cmd) => shell_words(cmd),
        None => default_command(&mapping.language, repo_root)?,
    };
    let (prog, args) = argv.split_first().ok_or("empty extractor command")?;
    // Absolute, because the child no longer runs from the repo root (below) —
    // extractors join `repoRoot` with `root`, so a relative one would resolve
    // against the wrong directory.
    // strip_verbatim, not just canonicalize: on Windows canonicalize returns
    // the `\\?\C:\...` verbatim form, which takes no separator normalization —
    // the extractors join it with forward slashes and the result is rejected
    // outright ("The filename, directory name, or volume label syntax is
    // incorrect"). This never showed up in our own gates: the dogfood corpus
    // has no C# mapping, and the fixture suite passes a *relative* repoRoot.
    let abs_root = strip_verbatim(
        repo_root.canonicalize().unwrap_or_else(|_| repo_root.to_path_buf()),
    );
    let input = serde_json::to_string(&ExtractorInput {
        component,
        repo_root: &abs_root.to_string_lossy(),
        root: &mapping.root,
        include: &mapping.include,
        exclude: &mapping.exclude,
        mode: mapping.mode.as_deref(),
    })
    .expect("input serialize");

    // Run the extractor from its own directory, not the target repo. The
    // .NET muxer resolves `global.json` from the working directory upward, so
    // launching from a repo that pins an old SDK would try to build our own
    // net8.0 extractor with it and fail. The extractor receives `repoRoot` on
    // stdin and switches to it itself where that matters — C# semantic mode
    // does, so the target solution still loads under the SDK it pins
    // (spec/l4-introspection.md).
    let cwd = extractor_dir(prog, args).unwrap_or_else(|| repo_root.to_path_buf());
    let mut child = Command::new(prog)
        .args(args)
        .current_dir(cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("failed to start {prog}: {e}"))?;
    child.stdin.take().expect("piped").write_all(input.as_bytes()).map_err(|e| e.to_string())?;
    let out = child.wait_with_output().map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(format!(
            "{prog} exited with {}:\n{}",
            out.status,
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    let mut facts: Facts = serde_json::from_slice(&out.stdout)
        .map_err(|e| format!("{prog} produced invalid facts JSON: {e}"))?;
    if facts.schema != FACTS_SCHEMA {
        return Err(format!("{prog} emitted schema {}, expected {FACTS_SCHEMA}", facts.schema));
    }
    facts.component = component.to_string();
    facts.root = mapping.root.clone();
    canonicalize(&mut facts);
    Ok(facts)
}

/// Minimal whitespace splitter with double-quote support for the
/// `extractor:` override — not a shell.
fn shell_words(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quoted = false;
    for c in s.chars() {
        match c {
            '"' => quoted = !quoted,
            c if c.is_whitespace() && !quoted => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
            }
            c => cur.push(c),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// Extract facts for one opted-in component, dispatching on language.
/// Returns the canonical facts plus non-fatal warnings.
pub fn extract(repo_root: &Path, component: &str, mapping: &SourceMapping) -> Result<(Facts, Vec<String>), String> {
    match mapping.language.as_str() {
        "rust" => extract_rust(repo_root, component, mapping),
        "typescript" | "csharp" => run_external(repo_root, component, mapping).map(|f| (f, Vec::new())),
        other => Err(format!("no extractor for language {other:?}")),
    }
}

/// Walk up from the workspace directory to the repository root (`.git`).
pub fn find_repo_root(workspace_dir: &Path) -> Option<PathBuf> {
    let mut dir = strip_verbatim(workspace_dir.canonicalize().ok()?);
    loop {
        if dir.join(".git").exists() {
            return Some(dir);
        }
        dir = dir.parent()?.to_path_buf();
    }
}

/// Windows canonicalize() yields `\\?\C:\...` verbatim paths, which Node (and
/// humans reading output) reject — strip the prefix.
pub fn strip_verbatim(p: PathBuf) -> PathBuf {
    let s = p.to_string_lossy();
    match s.strip_prefix(r"\\?\") {
        Some(rest) => PathBuf::from(rest),
        None => p,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("br-extractor-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn touch(root: &Path, rel: &str) {
        let path = root.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, "").unwrap();
    }

    /// A release ships the C# extractor published, so it runs on a machine
    /// with only the .NET runtime and — crucially — from a read-only install
    /// directory, where `dotnet run` cannot write bin/ and obj/.
    #[test]
    fn published_csharp_extractor_wins_over_the_project() {
        let dir = temp("published");
        touch(&dir, "extractors/dotnet/BlastradiusExtract.csproj");
        touch(&dir, "extractors/dotnet/BlastradiusExtract.dll");
        let argv = default_command("csharp", &dir).unwrap();
        assert_eq!(argv[0], "dotnet");
        assert!(argv[1].ends_with("BlastradiusExtract.dll"), "{argv:?}");
        assert_eq!(argv.len(), 2, "a published build is run, never built: {argv:?}");
        // And it still runs from its own directory, not the target repo.
        let (prog, args) = argv.split_first().unwrap();
        assert_eq!(extractor_dir(prog, args), Some(dir.join("extractors/dotnet")));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_checkout_falls_back_to_building_the_project() {
        let dir = temp("checkout");
        touch(&dir, "extractors/dotnet/BlastradiusExtract.csproj");
        let argv = default_command("csharp", &dir).unwrap();
        assert!(argv.contains(&"run".to_string()), "{argv:?}");
        assert!(argv.contains(&"--project".to_string()), "{argv:?}");
        let _ = fs::remove_dir_all(&dir);
    }

    /// An ordinary, writable extractor directory is used where it is — the
    /// staging copy is for restricted install locations only.
    #[test]
    fn a_writable_extractor_directory_is_used_in_place() {
        let dir = temp("writable");
        let ext = dir.join("extractors/dotnet");
        fs::create_dir_all(&ext).unwrap();
        fs::write(ext.join(CSHARP_DLL), "").unwrap();
        assert!(writable(&ext), "a temp dir must be writable");
        assert_eq!(runnable_dir(&ext), ext, "no copy should have been made");
        let _ = fs::remove_dir_all(&dir);
    }

    /// The copy that makes a packaged install work has to bring the whole
    /// tree: Roslyn ships satellite folders beside the assembly.
    #[test]
    fn staging_copies_nested_directories() {
        let dir = temp("copytree");
        let from = dir.join("from");
        fs::create_dir_all(from.join("runtimes/win/lib")).unwrap();
        fs::write(from.join(CSHARP_DLL), "assembly").unwrap();
        fs::write(from.join("runtimes/win/lib/native.dll"), "native").unwrap();

        let to = dir.join("to");
        copy_tree(&from, &to).unwrap();
        assert_eq!(fs::read_to_string(to.join(CSHARP_DLL)).unwrap(), "assembly");
        assert_eq!(
            fs::read_to_string(to.join("runtimes/win/lib/native.dll")).unwrap(),
            "native"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    /// Reported as bare "no csharp extractor found" for five releases, which
    /// told the first person to hit it nothing at all (docs/roadmap.md).
    #[test]
    fn a_missing_extractor_says_what_to_do_about_it() {
        let dir = temp("missing");
        let err = default_command("csharp", &dir).unwrap_err();
        assert!(err.contains("BlastradiusExtract.dll"), "{err}");
        assert!(err.contains(&dir.display().to_string()), "{err}");
        assert!(err.contains("Rust needs no extractor"), "{err}");
        assert!(err.contains(".NET runtime"), "{err}");
        let _ = fs::remove_dir_all(&dir);
    }
}
