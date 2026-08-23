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

fn glob_set(globs: &[String], label: &str) -> Result<Option<globset::GlobSet>, String> {
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

const RUST_EXTRACTOR_VERSION: &str = "blastradius-extract-rust 0.1.0";

struct RustCorpus {
    /// module id -> (path, name)
    modules: BTreeMap<String, (String, String)>,
    /// type id -> (kind, module id, path, line, name)
    types: BTreeMap<String, TypeInfo>,
    /// unqualified type name -> ids
    by_name: BTreeMap<String, Vec<String>>,
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
    let mut corpus = RustCorpus { modules: BTreeMap::new(), types: BTreeMap::new(), by_name: BTreeMap::new() };
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

    // Pass 2: edges.
    let mut edges: BTreeSet<(String, String, String)> = BTreeSet::new();
    for (mid, _rel, ast) in &parsed {
        collect_edges(&ast.items, mid, &corpus, &mut edges);
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

    let mut facts = Facts {
        schema: FACTS_SCHEMA,
        language: "rust".into(),
        extractor: RUST_EXTRACTOR_VERSION.into(),
        component: component.to_string(),
        root: mapping.root.clone(),
        source_digest: digest,
        elements,
        edges: edges.into_iter().map(|(from, to, kind)| FactEdge { from, to, kind }).collect(),
    };
    canonicalize(&mut facts);
    Ok((facts, warnings))
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
            }
            return Some(Resolved::Module(mid));
        }
    }
    None
}

enum Resolved {
    Module(String),
    Type(String),
}

fn use_paths(tree: &syn::UseTree, prefix: &mut Vec<String>, out: &mut Vec<Vec<String>>) {
    match tree {
        syn::UseTree::Path(p) => {
            prefix.push(p.ident.to_string());
            use_paths(&p.tree, prefix, out);
            prefix.pop();
        }
        syn::UseTree::Name(n) => {
            let mut full = prefix.clone();
            full.push(n.ident.to_string());
            out.push(full);
        }
        syn::UseTree::Rename(r) => {
            let mut full = prefix.clone();
            full.push(r.ident.to_string());
            out.push(full);
        }
        syn::UseTree::Group(g) => {
            for t in &g.items {
                use_paths(t, prefix, out);
            }
        }
        syn::UseTree::Glob(_) => {} // glob imports dropped (spec)
    }
}

fn collect_edges(items: &[syn::Item], module: &str, corpus: &RustCorpus, edges: &mut BTreeSet<(String, String, String)>) {
    // File-level use map: unqualified name -> corpus type id.
    let mut use_map: BTreeMap<String, String> = BTreeMap::new();
    for item in items {
        if let syn::Item::Use(u) = item {
            let mut out = Vec::new();
            use_paths(&u.tree, &mut Vec::new(), &mut out);
            for path in out {
                match resolve_path(&path, module, corpus) {
                    Some(Resolved::Type(tid)) => {
                        let target_module = corpus.types[&tid].module.clone();
                        if target_module != module {
                            edges.insert((module.to_string(), target_module, "imports".into()));
                        }
                        use_map.insert(path.last().expect("nonempty").clone(), tid);
                    }
                    Some(Resolved::Module(mid)) => {
                        if mid != module {
                            edges.insert((module.to_string(), mid, "imports".into()));
                        }
                    }
                    None => {}
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
                        collect_edges(inner, &format!("{module}.{}", m.ident), corpus, edges);
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
            Resolved::Module(_) => None,
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
}

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
    let entry = match language {
        "typescript" => "typescript/extract.mjs",
        "csharp" => "dotnet",
        other => return Err(format!("no extractor for language {other:?}")),
    };
    for dir in &candidates {
        let path = dir.join(entry);
        if path.exists() {
            let p = path.to_string_lossy().into_owned();
            return Ok(match language {
                "typescript" => vec!["node".into(), p],
                _ => vec!["dotnet".into(), "run".into(), "--project".into(), p, "-c".into(), "Release".into()],
            });
        }
    }
    Err(format!(
        "no {language} extractor found (looked for {entry:?} under {})",
        candidates.iter().map(|d| d.display().to_string()).collect::<Vec<_>>().join(", ")
    ))
}

pub fn run_external(repo_root: &Path, component: &str, mapping: &SourceMapping) -> Result<Facts, String> {
    use std::io::Write;
    use std::process::{Command, Stdio};

    let argv = match &mapping.extractor {
        Some(cmd) => shell_words(cmd),
        None => default_command(&mapping.language, repo_root)?,
    };
    let (prog, args) = argv.split_first().ok_or("empty extractor command")?;
    let input = serde_json::to_string(&ExtractorInput {
        component,
        repo_root: &repo_root.to_string_lossy(),
        root: &mapping.root,
        include: &mapping.include,
        exclude: &mapping.exclude,
    })
    .expect("input serialize");

    let mut child = Command::new(prog)
        .args(args)
        .current_dir(repo_root)
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
fn strip_verbatim(p: PathBuf) -> PathBuf {
    let s = p.to_string_lossy();
    match s.strip_prefix(r"\\?\") {
        Some(rest) => PathBuf::from(rest),
        None => p,
    }
}
