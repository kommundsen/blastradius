//! The sync engine (ADR-0008): files are the single source of truth; every
//! editing surface proposes transactions against them. This engine arbitrates
//! — targeted CST edits (splice), staleness, atomic writes, race abort, echo
//! suppression, and one shared undo history for all surfaces.

use crate::diagnostics::Diagnostic;
use crate::model::{is_valid_slug, ElementKind, Workspace};
use crate::splice;
use crate::vfs::DiskVfs;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// A canvas operation (spec/sync-engine.md, "Outbound"). Deserialized straight
/// from the WebView's IPC call.
#[derive(Debug, Deserialize)]
#[serde(tag = "op", rename_all = "kebab-case")]
pub enum Operation {
    /// Drag-to-pin: upsert `layout.<id>: [x, y]` in the view's file. Creates
    /// the view file when the level+scope has none yet.
    Pin { view: Option<String>, level: String, scope: Option<String>, id: String, x: i64, y: i64 },
    /// Rename = set `name:` — the id is immutable (ADR-0003).
    Rename { id: String, name: String },
    /// Set a scalar field on an element (whitelisted: name, description,
    /// tech). The MCP surface and future inspector fields route here.
    SetField { id: String, field: String, value: String },
    /// Create an element under a parent (None = context person/external).
    Create { parent: Option<String>, id: String, name: String, kind: String },
    /// Delete an element; relations referencing it and its layout pins are
    /// removed in the same transaction (spec table).
    Delete { id: String },
    AddRelation { from: String, to: String, label: Option<String>, protocol: Option<String> },
    DeleteRelation { from: String, to: String, label: Option<String> },
    SetRelationField { from: String, to: String, label: Option<String>, field: String, value: String },
}

/// Every element id an operation would touch — used by the derived-element
/// write guard in `apply`.
fn op_target_ids(op: &Operation) -> Vec<&str> {
    match op {
        Operation::Pin { id, .. } | Operation::Rename { id, .. } | Operation::SetField { id, .. } | Operation::Delete { id } => vec![id],
        Operation::Create { parent, id, .. } => {
            let mut v = vec![id.as_str()];
            if let Some(p) = parent {
                v.push(p);
            }
            v
        }
        Operation::AddRelation { from, to, .. }
        | Operation::DeleteRelation { from, to, .. }
        | Operation::SetRelationField { from, to, .. } => vec![from, to],
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileChange {
    pub rel: String,
    /// None = file did not exist.
    pub before: Option<String>,
    /// None = file deleted (unused in v1 — no op deletes files).
    pub after: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    pub label: String,
    /// "canvas" | "external"
    pub source: String,
    pub changes: Vec<FileChange>,
}

pub struct SyncEngine {
    root: PathBuf,
    /// Last-known text per workspace-relative file.
    files: BTreeMap<String, String>,
    /// Last valid model + its diagnostics.
    pub model: Workspace,
    pub diagnostics: Vec<Diagnostic>,
    /// Files whose on-disk content currently fails to parse (spec: staleness).
    pub stale: BTreeSet<String>,
    /// Manifest-declared view files — staleness in these disables only pinning
    /// (spec: granular staleness, Phase 5).
    view_files: BTreeSet<String>,
    history: Vec<Transaction>,
    cursor: usize, // transactions [0..cursor) are applied
    journal: Option<PathBuf>,
}

pub const HISTORY_DEPTH: usize = 200;

impl SyncEngine {
    pub fn open(root: &Path) -> Self {
        let mut engine = SyncEngine {
            root: root.to_path_buf(),
            files: BTreeMap::new(),
            model: Workspace::default(),
            diagnostics: Vec::new(),
            stale: BTreeSet::new(),
            view_files: BTreeSet::new(),
            history: Vec::new(),
            cursor: 0,
            journal: journal_path(root),
        };
        engine.recover();
        engine.reload_all();
        engine
    }

    /// Full disk re-read: refresh caches, re-parse, recompute staleness.
    pub fn reload_all(&mut self) {
        let (ws, diags) = crate::load_workspace(&self.root);
        self.refresh_file_cache(&diags);
        self.adopt(ws, diags);
    }

    /// Take a fresh parse. Staleness is granular (spec, Phase 5): errors in a
    /// *model* file keep the last valid model; errors confined to *view* files
    /// still adopt the new semantics — only pinning into those files is
    /// disabled, and their last-known pins are retained so the canvas holds.
    fn adopt(&mut self, ws: Workspace, diags: Vec<Diagnostic>) {
        let error_files: BTreeSet<String> = diags
            .iter()
            .filter(|d| d.severity == crate::diagnostics::Severity::Error)
            .map(|d| d.file.clone())
            .collect();
        let model_broken = error_files
            .iter()
            .any(|f| f.is_empty() || !self.view_files.contains(f));
        if error_files.is_empty() {
            self.model = ws;
            self.stale.clear();
        } else if model_broken {
            // keep last valid model; mark offending files stale
            self.stale = error_files;
        } else {
            let mut ws = ws;
            for v in &self.model.views {
                if error_files.contains(&v.file) && !ws.views.iter().any(|nv| nv.id == v.id) {
                    ws.views.push(v.clone());
                }
            }
            self.model = ws;
            self.stale = error_files;
        }
        self.diagnostics = diags;
    }

    fn refresh_file_cache(&mut self, diags: &[Diagnostic]) {
        // cache every file the manifest resolved, plus the manifest itself
        let vfs = DiskVfs::new(&self.root);
        // seed with both manifest names — the read filter below drops the absent
        // one, and a legacy->new rename is then just another external change
        let mut wanted: BTreeSet<String> = [
            crate::manifest::MANIFEST.to_string(),
            crate::manifest::LEGACY_MANIFEST.to_string(),
        ]
        .into();
        if let Some(m) = crate::manifest::load(&vfs, &mut Vec::new()) {
            wanted.extend(m.model_files.iter().cloned());
            wanted.extend(m.view_files.iter().cloned());
            wanted.extend(m.doc_files.iter().cloned());
            self.view_files = m.view_files.iter().cloned().collect();
        }
        // also keep files mentioned in diagnostics (they may have failed the manifest)
        wanted.extend(diags.iter().map(|d| d.file.clone()).filter(|f| !f.is_empty()));
        self.files = wanted
            .into_iter()
            .filter_map(|rel| {
                std::fs::read_to_string(self.root.join(&rel)).ok().map(|t| (rel, t))
            })
            .collect();
    }

    /// The watcher fired: compare disk to last-known text. Content we wrote
    /// ourselves matches the cache and is dropped — the echo-loop killer.
    /// Returns true when anything actually changed (re-render needed).
    pub fn external_scan(&mut self) -> bool {
        let mut changed: Vec<FileChange> = Vec::new();
        let vfs = DiskVfs::new(&self.root);
        let mut current: BTreeSet<String> = [
            crate::manifest::MANIFEST.to_string(),
            crate::manifest::LEGACY_MANIFEST.to_string(),
        ]
        .into();
        if let Some(m) = crate::manifest::load(&vfs, &mut Vec::new()) {
            current.extend(m.model_files.iter().cloned());
            current.extend(m.view_files.iter().cloned());
            current.extend(m.doc_files.iter().cloned());
        }
        for rel in current.iter() {
            let disk = std::fs::read_to_string(self.root.join(rel)).ok();
            let known = self.files.get(rel);
            if disk.as_ref() != known {
                changed.push(FileChange {
                    rel: rel.clone(),
                    before: known.cloned(),
                    after: disk.clone(),
                });
            }
        }
        // deleted files
        for rel in self.files.keys() {
            if !current.contains(rel) && !self.root.join(rel).is_file() {
                changed.push(FileChange {
                    rel: rel.clone(),
                    before: self.files.get(rel).cloned(),
                    after: None,
                });
            }
        }
        if changed.is_empty() {
            return false;
        }
        // external edits enter history as transactions (spec: undo-past-an-
        // external-edit is well-defined)
        let tx = Transaction {
            label: format!(
                "external change: {}",
                changed.iter().map(|c| c.rel.as_str()).collect::<Vec<_>>().join(", ")
            ),
            source: "external".to_string(),
            changes: changed,
        };
        self.push_transaction(tx.clone());
        self.journal_event(&JournalEvent::Tx { tx });
        self.reload_all();
        true
    }

    /// In-app YAML panel keystrokes: same inbound path as external edits, but
    /// the buffer is written to disk debounced by the caller. Returns parse
    /// success (false = the file is now stale).
    pub fn buffer_update(&mut self, rel: &str, text: &str) -> Result<bool, String> {
        if !self.files.contains_key(rel) {
            return Err(format!("{rel}: not a workspace file"));
        }
        let before = self.files.get(rel).cloned();
        let tx = Transaction {
            label: format!("edit {rel}"),
            source: "canvas".to_string(), // in-app surface: undoable like any op
            changes: vec![FileChange { rel: rel.to_string(), before, after: Some(text.to_string()) }],
        };
        self.journal_event(&JournalEvent::Intent { tx: tx.clone() });
        self.write_atomic(rel, text)?;
        self.push_transaction(tx);
        self.journal_event(&JournalEvent::Commit);
        self.files.insert(rel.to_string(), text.to_string());
        self.reload_model_only();
        Ok(!self.stale.contains(rel))
    }

    fn reload_model_only(&mut self) {
        let (ws, diags) = crate::load_workspace(&self.root);
        self.adopt(ws, diags);
    }

    /// Stale files that break the *model* (not mere view files). Canvas
    /// editing is disabled only while this is non-empty.
    pub fn stale_model(&self) -> Vec<String> {
        self.stale.iter().filter(|f| !self.view_files.contains(*f)).cloned().collect()
    }

    /// Ids of views whose backing file is stale — pinning into them is
    /// disabled until the file parses again.
    pub fn stale_view_ids(&self) -> Vec<String> {
        self.model
            .views
            .iter()
            .filter(|v| self.stale.contains(&v.file))
            .map(|v| v.id.clone())
            .collect()
    }

    pub fn file_text(&self, rel: &str) -> Option<&str> {
        self.files.get(rel).map(String::as_str)
    }

    pub fn editable_files(&self) -> Vec<String> {
        let vfs = DiskVfs::new(&self.root);
        match crate::manifest::load(&vfs, &mut Vec::new()) {
            Some(m) => {
                let mut v = m.model_files;
                v.extend(m.view_files);
                v
            }
            None => Vec::new(),
        }
    }

    // ---- operations ---------------------------------------------------------

    /// Apply one canvas operation as a transaction. Refused while any model
    /// file is stale (spec: canvas read-only while stale).
    pub fn apply(&mut self, op: Operation) -> Result<Transaction, String> {
        // Derived (introspected) elements are read-only by design
        // (spec/l4-introspection.md): the code is the source of truth.
        for id in op_target_ids(&op) {
            if let Some(d) = self.model.derived_element(id) {
                return Err(format!(
                    "{id:?} is derived from source — edit {} instead and re-run `blastradius introspect`",
                    d.path
                ));
            }
        }
        let stale_model = self.stale_model();
        if !stale_model.is_empty() {
            return Err(format!(
                "workspace is stale ({}) — fix the file first",
                stale_model.join(", ")
            ));
        }
        // Granular staleness (spec, Phase 5): a stale *view* file blocks only
        // operations that would write into it — i.e. pinning that view. The
        // target must be resolved before compute (its text does not parse).
        if let Operation::Pin { view, level, scope, .. } = &op {
            let target = self.model.views.iter().find(|v| match view {
                Some(vid) => &v.id == vid,
                None => v.level == *level && (level == "L1" || Some(v.scope.as_str()) == scope.as_deref()),
            });
            if let Some(v) = target {
                if self.stale.contains(&v.file) {
                    return Err(format!(
                        "{}: does not parse — pinning is disabled until it is fixed",
                        v.file
                    ));
                }
            }
        }
        let changes = self.compute_changes(&op)?;
        if let Some(c) = changes.iter().find(|c| self.stale.contains(&c.rel)) {
            return Err(format!(
                "{}: does not parse — pinning is disabled until it is fixed",
                c.rel
            ));
        }
        // Race abort (spec): disk must still match our cache for every file
        // we are about to touch — never merge heuristically.
        for c in &changes {
            let disk = std::fs::read_to_string(self.root.join(&c.rel)).ok();
            if disk.as_ref() != c.before.as_ref() {
                return Err(format!("{}: changed on disk — operation aborted", c.rel));
            }
        }
        // validate the result before committing anything
        let candidate_error = self.validate_candidate(&changes);
        if let Some(e) = candidate_error {
            return Err(format!("operation would invalidate the workspace: {e}"));
        }
        // Write-ahead: journal intent, write files, journal commit. Recovery
        // rolls an uncommitted intent forward if the writes were torn.
        self.journal_event(&JournalEvent::Intent {
            tx: Transaction { label: op_label(&op), source: "canvas".to_string(), changes: changes.clone() },
        });
        for c in &changes {
            match &c.after {
                Some(text) => self.write_atomic(&c.rel, text)?,
                None => {
                    let _ = std::fs::remove_file(self.root.join(&c.rel));
                }
            }
            match &c.after {
                Some(t) => {
                    self.files.insert(c.rel.clone(), t.clone());
                }
                None => {
                    self.files.remove(&c.rel);
                }
            }
        }
        let tx = Transaction { label: op_label(&op), source: "canvas".to_string(), changes };
        self.push_transaction(tx.clone());
        self.journal_event(&JournalEvent::Commit);
        self.reload_model_only();
        Ok(tx)
    }

    /// Parse the workspace as it would look after `changes` — without writing.
    fn validate_candidate(&self, changes: &[FileChange]) -> Option<String> {
        let disk = DiskVfs::new(&self.root);
        let mut overrides = std::collections::HashMap::new();
        for c in changes {
            if let Some(after) = &c.after {
                overrides.insert(c.rel.clone(), after.clone());
            }
        }
        let overlay = crate::vfs::OverlayVfs { base: &disk, overrides };
        let (_, diags) = crate::load_workspace_vfs(&overlay);
        let touched: BTreeSet<&str> = changes.iter().map(|c| c.rel.as_str()).collect();
        diags
            .iter()
            .find(|d| {
                d.severity == crate::diagnostics::Severity::Error
                    // a stale view file we are not touching keeps its errors;
                    // they must not veto unrelated operations
                    && !(self.stale.contains(&d.file) && !touched.contains(d.file.as_str()))
            })
            .map(|d| d.to_string())
    }

    fn change(&self, rel: &str, after: String) -> FileChange {
        FileChange {
            rel: rel.to_string(),
            before: self.files.get(rel).cloned(),
            after: Some(after),
        }
    }

    fn compute_changes(&self, op: &Operation) -> Result<Vec<FileChange>, String> {
        match op {
            Operation::Pin { view, level, scope, id, x, y } => {
                self.compute_pin(view.as_deref(), level, scope.as_deref(), id, *x, *y)
            }
            Operation::Rename { id, name } => {
                let (rel, chain) = self.element_chain(id)?;
                let text = self.files.get(&rel).ok_or("file not cached")?;
                let chain_refs: Vec<&str> = chain.iter().map(String::as_str).collect();
                let after = splice::set_field(text, &chain_refs, "name", name)?;
                Ok(vec![self.change(&rel, after)])
            }
            Operation::SetField { id, field, value } => {
                if !matches!(field.as_str(), "name" | "description" | "tech") {
                    return Err(format!("field {field:?} is not editable on an element"));
                }
                let (rel, chain) = self.element_chain(id)?;
                let text = self.files.get(&rel).ok_or("file not cached")?;
                let chain_refs: Vec<&str> = chain.iter().map(String::as_str).collect();
                let after = splice::set_field(text, &chain_refs, field, value)?;
                Ok(vec![self.change(&rel, after)])
            }
            Operation::Create { parent, id, name, kind } => {
                self.compute_create(parent.as_deref(), id, name, kind)
            }
            Operation::Delete { id } => self.compute_delete(id),
            Operation::AddRelation { from, to, label, protocol } => {
                self.compute_add_relation(from, to, label.as_deref(), protocol.as_deref())
            }
            Operation::DeleteRelation { from, to, label } => {
                self.compute_del_relation(from, to, label.as_deref())
            }
            Operation::SetRelationField { from, to, label, field, value } => {
                self.compute_set_relation_field(from, to, label.as_deref(), field, value)
            }
        }
    }

    /// File + key chain addressing an element's mapping.
    fn element_chain(&self, id: &str) -> Result<(String, Vec<String>), String> {
        let el = self.model.elements.get(id).ok_or_else(|| format!("unknown element {id}"))?;
        let segs: Vec<&str> = id.split('.').collect();
        let chain = match el.kind {
            ElementKind::Person => vec!["people".into(), id.into()],
            ElementKind::External => vec!["external".into(), id.into()],
            ElementKind::System => vec![],
            ElementKind::Container => vec!["containers".into(), segs[1].into()],
            ElementKind::Component => vec![
                "containers".into(),
                segs[1].into(),
                "components".into(),
                segs[2].into(),
            ],
            k if k.is_deployment() => crate::model::deployment_chain(id, k),
            _ => unreachable!("every kind is addressed above"),
        };
        Ok((el.file.clone(), chain))
    }

    fn compute_pin(
        &self,
        view: Option<&str>,
        level: &str,
        scope: Option<&str>,
        id: &str,
        x: i64,
        y: i64,
    ) -> Result<Vec<FileChange>, String> {
        let existing = self.model.views.iter().find(|v| match view {
            Some(vid) => v.id == vid,
            None => v.level == level && (level == "L1" || Some(v.scope.as_str()) == scope),
        });
        // pins are written scope-relative when inside the scope (spec §4 style)
        let pin_key = |scope: &str, id: &str| -> String {
            id.strip_prefix(&format!("{scope}."))
                .map(str::to_string)
                .unwrap_or_else(|| id.to_string())
        };
        let value = format!("[{x}, {y}]");
        match existing {
            Some(v) => {
                let rel = v.file.clone();
                let text = self.files.get(&rel).ok_or("view file not cached")?;
                let key = pin_key(&v.scope, id);
                let after = set_layout_pin(text, &key, &value)?;
                Ok(vec![self.change(&rel, after)])
            }
            None => {
                let scope = scope.ok_or("cannot pin at L1 without a scope element")?;
                let last = scope.rsplit('.').next().unwrap_or(scope);
                let view_id = format!("{last}-{}", level.to_lowercase());
                let rel = format!("views/{view_id}.yaml");
                if self.files.contains_key(&rel) {
                    return Err(format!("{rel} exists but defines no matching view"));
                }
                let key = pin_key(scope, id);
                let text = format!(
                    "view: {view_id}\nscope: {scope}\nlevel: {level}\nlayout:\n  {key}: {value}\n"
                );
                Ok(vec![FileChange { rel, before: None, after: Some(text) }])
            }
        }
    }

    fn compute_create(
        &self,
        parent: Option<&str>,
        id: &str,
        name: &str,
        kind: &str,
    ) -> Result<Vec<FileChange>, String> {
        if !is_valid_slug(id) {
            return Err(format!("bad id {id:?} — lowercase slug required (ADR-0003)"));
        }
        let full_id = match parent {
            Some(p) => format!("{p}.{id}"),
            None => id.to_string(),
        };
        if self.model.elements.contains_key(&full_id) {
            return Err(format!("id {full_id:?} already exists"));
        }
        match (kind, parent) {
            ("person" | "external", None) => {
                // context file: first context file, else create model/context.yaml
                let rel = self
                    .model
                    .elements
                    .values()
                    .find(|e| matches!(e.kind, ElementKind::Person | ElementKind::External))
                    .map(|e| e.file.clone())
                    .unwrap_or_else(|| "model/context.yaml".to_string());
                let section = if kind == "person" { "people" } else { "external" };
                let text = self.files.get(&rel).cloned().unwrap_or_default();
                let after =
                    splice::insert_entry(&text, &[section], Some((&[], 0)), id, &[("name", name)])?;
                let before = self.files.get(&rel).cloned();
                Ok(vec![FileChange { rel, before, after: Some(after) }])
            }
            ("container", Some(system)) => {
                let sys = self
                    .model
                    .elements
                    .get(system)
                    .ok_or_else(|| format!("unknown system {system}"))?;
                let rel = sys.file.clone();
                let text = self.files.get(&rel).ok_or("file not cached")?;
                let after = splice::insert_entry(
                    text,
                    &["containers"],
                    Some((&[], 0)),
                    id,
                    &[("name", name)],
                )?;
                Ok(vec![self.change(&rel, after)])
            }
            ("component", Some(container)) => {
                let (rel, chain) = self.element_chain(container)?;
                let text = self.files.get(&rel).ok_or("file not cached")?;
                let mut chain_refs: Vec<&str> = chain.iter().map(String::as_str).collect();
                chain_refs.push("components");
                let owner: Vec<&str> = chain.iter().map(String::as_str).collect();
                // components section may be absent: create it under the container
                let owner_indent = 4; // containers(0) -> cid(2) -> components(4)
                let after = splice::insert_entry(
                    text,
                    &chain_refs,
                    Some((&owner, owner_indent)),
                    id,
                    &[("name", name)],
                )?;
                Ok(vec![self.change(&rel, after)])
            }
            ("system", None) => {
                let rel = format!("model/{id}.yaml");
                if self.files.contains_key(&rel) {
                    return Err(format!("{rel} already exists"));
                }
                let text = format!("system: {id}\nname: {}\n", splice::yaml_scalar(name));
                Ok(vec![FileChange { rel, before: None, after: Some(text) }])
            }
            // Deployment (ADR-0018). An environment is a top-level entry in
            // deployment.yaml; nodes and instances nest under whatever they run
            // on, at any depth.
            ("environment", None) => {
                let rel = self
                    .model
                    .elements
                    .values()
                    .find(|e| e.kind == ElementKind::Environment)
                    .map(|e| e.file.clone())
                    .unwrap_or_else(|| "model/deployment.yaml".to_string());
                let text = self.files.get(&rel).cloned().unwrap_or_default();
                let after =
                    splice::insert_entry(&text, &["environments"], Some((&[], 0)), id, &[("name", name)])?;
                let before = self.files.get(&rel).cloned();
                Ok(vec![FileChange { rel, before, after: Some(after) }])
            }
            ("deployment-node" | "container-instance", Some(owner)) => {
                let parent_el = self
                    .model
                    .elements
                    .get(owner)
                    .ok_or_else(|| format!("unknown element {owner}"))?;
                if !parent_el.kind.is_deployment() || parent_el.kind == ElementKind::ContainerInstance {
                    return Err(format!(
                        "{kind:?} belongs under an environment or a deployment node, not a {}",
                        parent_el.kind.as_str()
                    ));
                }
                let (rel, chain) = self.element_chain(owner)?;
                let text = self.files.get(&rel).ok_or("file not cached")?;
                let section = if kind == "container-instance" { "instances" } else { "nodes" };
                let mut chain_refs: Vec<&str> = chain.iter().map(String::as_str).collect();
                chain_refs.push(section);
                let owner_chain: Vec<&str> = chain.iter().map(String::as_str).collect();
                // environments(0) -> env(2) -> nodes(4) -> node(6) … two spaces
                // per level, and the chain already counts them.
                let owner_indent = chain.len();
                let after = splice::insert_entry(
                    text,
                    &chain_refs,
                    Some((&owner_chain, owner_indent)),
                    id,
                    &[("name", name)],
                )?;
                Ok(vec![self.change(&rel, after)])
            }
            _ => Err(format!("cannot create kind {kind:?} under {parent:?}")),
        }
    }

    fn compute_delete(&self, id: &str) -> Result<Vec<FileChange>, String> {
        let (rel, chain) = self.element_chain(id)?;
        if chain.is_empty() {
            return Err("deleting a whole system means deleting its file — do that in your editor".into());
        }
        let mut edits: BTreeMap<String, String> = BTreeMap::new();
        let text = self.files.get(&rel).ok_or("file not cached")?.clone();
        let chain_refs: Vec<&str> = chain.iter().map(String::as_str).collect();
        edits.insert(rel.clone(), splice::remove_entry(&text, &chain_refs)?);

        // cascade: relations referencing the element (in any model file)
        for r in &self.model.relations {
            let scope = r.scope.as_deref();
            let from = self.model.resolve(&r.from, scope);
            let to = self.model.resolve(&r.to, scope);
            let touches = from.as_deref() == Some(id) || to.as_deref() == Some(id);
            if !touches {
                continue;
            }
            let file = r.file.clone();
            let base = edits
                .get(&file)
                .cloned()
                .or_else(|| self.files.get(&file).cloned())
                .ok_or("relation file not cached")?;
            let raw_from = r.from.clone();
            let raw_to = r.to.clone();
            let raw_label = r.label.clone();
            let (after, _) = splice::remove_seq_items(&base, &["relations"], |m| {
                let get = |k: &str| -> Option<String> {
                    match m.get_node(k) {
                        Some(marked_yaml::Node::Scalar(s)) => Some(s.as_str().to_string()),
                        _ => None,
                    }
                };
                get("from").as_deref() == Some(raw_from.as_str())
                    && get("to").as_deref() == Some(raw_to.as_str())
                    && get("label") == raw_label
            })?;
            edits.insert(file, after);
        }

        // cascade: layout pins in view files
        for v in &self.model.views {
            let key_scoped = id.strip_prefix(&format!("{}.", v.scope)).unwrap_or(id);
            if v.layout.contains_key(key_scoped) || v.layout.contains_key(id) {
                let file = v.file.clone();
                let base = edits
                    .get(&file)
                    .cloned()
                    .or_else(|| self.files.get(&file).cloned())
                    .ok_or("view file not cached")?;
                let mut after = base;
                for key in [key_scoped, id] {
                    if let Ok(next) = splice::remove_entry(&after, &["layout", key]) {
                        after = next;
                    }
                }
                edits.insert(file, after);
            }
        }

        Ok(edits
            .into_iter()
            .map(|(rel, after)| FileChange {
                before: self.files.get(&rel).cloned(),
                rel,
                after: Some(after),
            })
            .collect())
    }

    /// Relations are written into the file of the `from` element's system,
    /// with endpoints relative to that system where possible (spec §3 style).
    fn compute_add_relation(
        &self,
        from: &str,
        to: &str,
        label: Option<&str>,
        protocol: Option<&str>,
    ) -> Result<Vec<FileChange>, String> {
        let from_el = self.model.elements.get(from).ok_or("unknown from")?;
        let system = from.split('.').next().unwrap_or(from);
        let sys_el = self.model.elements.get(system);
        let (rel, in_system) = match sys_el {
            Some(s) if s.kind == ElementKind::System => (s.file.clone(), Some(system)),
            _ => (from_el.file.clone(), None),
        };
        let text = self.files.get(&rel).ok_or("file not cached")?;
        let relative = |id: &str| -> String {
            match in_system {
                Some(sys) => id
                    .strip_prefix(&format!("{sys}."))
                    .map(str::to_string)
                    .unwrap_or_else(|| id.to_string()),
                None => id.to_string(),
            }
        };
        let mut fields: Vec<(&str, String)> = vec![
            ("from", relative(from)),
            ("to", relative(to)),
        ];
        if let Some(l) = label {
            fields.push(("label", l.to_string()));
        }
        if let Some(p) = protocol {
            fields.push(("protocol", p.to_string()));
        }
        let field_refs: Vec<(&str, &str)> =
            fields.iter().map(|(k, v)| (*k, v.as_str())).collect();
        let after = splice::append_seq_item(text, &["relations"], (&[], 0), &field_refs)?;
        Ok(vec![self.change(&rel, after)])
    }

    fn find_relation(&self, from: &str, to: &str, label: Option<&str>) -> Option<&crate::model::Relation> {
        self.model.relations.iter().find(|r| {
            let scope = r.scope.as_deref();
            let rf = self.model.resolve(&r.from, scope);
            let rt = self.model.resolve(&r.to, scope);
            rf.as_deref() == Some(from)
                && rt.as_deref() == Some(to)
                && (label.is_none() || r.label.as_deref() == label)
        })
    }

    fn compute_del_relation(
        &self,
        from: &str,
        to: &str,
        label: Option<&str>,
    ) -> Result<Vec<FileChange>, String> {
        let r = self.find_relation(from, to, label).ok_or("relation not found")?;
        let file = r.file.clone();
        let raw_from = r.from.clone();
        let raw_to = r.to.clone();
        let raw_label = r.label.clone();
        let text = self.files.get(&file).ok_or("file not cached")?;
        let (after, n) = splice::remove_seq_items(text, &["relations"], |m| {
            let get = |k: &str| -> Option<String> {
                match m.get_node(k) {
                    Some(marked_yaml::Node::Scalar(s)) => Some(s.as_str().to_string()),
                    _ => None,
                }
            };
            get("from").as_deref() == Some(raw_from.as_str())
                && get("to").as_deref() == Some(raw_to.as_str())
                && get("label") == raw_label
        })?;
        if n == 0 {
            return Err("relation not found in file".into());
        }
        Ok(vec![self.change(&file, after)])
    }

    fn compute_set_relation_field(
        &self,
        from: &str,
        to: &str,
        label: Option<&str>,
        field: &str,
        value: &str,
    ) -> Result<Vec<FileChange>, String> {
        if !matches!(field, "label" | "protocol") {
            return Err(format!("field {field:?} is not editable on a relation"));
        }
        let r = self.find_relation(from, to, label).ok_or("relation not found")?;
        let file = r.file.clone();
        let text = self.files.get(&file).ok_or("file not cached")?;
        let after = set_relation_field_text(text, &r.from, &r.to, r.label.as_deref(), field, value)?;
        Ok(vec![self.change(&file, after)])
    }

    // ---- history ------------------------------------------------------------

    fn push_transaction(&mut self, tx: Transaction) {
        self.history.truncate(self.cursor); // drop redo tail
        self.history.push(tx);
        if self.history.len() > HISTORY_DEPTH {
            let drop = self.history.len() - HISTORY_DEPTH;
            self.history.drain(..drop);
        }
        self.cursor = self.history.len();
    }

    pub fn undo(&mut self) -> Result<Option<String>, String> {
        if self.cursor == 0 {
            return Ok(None);
        }
        self.journal_event(&JournalEvent::IntentUndo);
        self.cursor -= 1;
        let tx = self.history[self.cursor].clone();
        for c in tx.changes.iter().rev() {
            match &c.before {
                Some(text) => self.write_atomic(&c.rel, text)?,
                None => {
                    let _ = std::fs::remove_file(self.root.join(&c.rel));
                }
            }
            match &c.before {
                Some(t) => {
                    self.files.insert(c.rel.clone(), t.clone());
                }
                None => {
                    self.files.remove(&c.rel);
                }
            }
        }
        self.journal_event(&JournalEvent::Commit);
        self.reload_model_only();
        Ok(Some(tx.label))
    }

    pub fn redo(&mut self) -> Result<Option<String>, String> {
        if self.cursor >= self.history.len() {
            return Ok(None);
        }
        self.journal_event(&JournalEvent::IntentRedo);
        let tx = self.history[self.cursor].clone();
        self.cursor += 1;
        for c in &tx.changes {
            match &c.after {
                Some(text) => self.write_atomic(&c.rel, text)?,
                None => {
                    let _ = std::fs::remove_file(self.root.join(&c.rel));
                }
            }
            match &c.after {
                Some(t) => {
                    self.files.insert(c.rel.clone(), t.clone());
                }
                None => {
                    self.files.remove(&c.rel);
                }
            }
        }
        self.journal_event(&JournalEvent::Commit);
        self.reload_model_only();
        Ok(Some(tx.label))
    }

    pub fn history_labels(&self) -> (Vec<String>, usize) {
        (self.history.iter().map(|t| t.label.clone()).collect(), self.cursor)
    }

    // ---- io -----------------------------------------------------------------

    /// Atomic write: temp file + rename (std::fs::rename replaces on every
    /// supported platform).
    fn write_atomic(&self, rel: &str, text: &str) -> Result<(), String> {
        let path = self.root.join(rel);
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        let tmp = path.with_extension("blastradius-tmp");
        std::fs::write(&tmp, text).map_err(|e| e.to_string())?;
        std::fs::rename(&tmp, &path).map_err(|e| e.to_string())
    }

    fn journal_event(&self, ev: &JournalEvent) {
        let Some(journal) = &self.journal else { return };
        if let Ok(line) = serde_json::to_string(ev) {
            use std::io::Write;
            if let Ok(mut f) =
                std::fs::OpenOptions::new().create(true).append(true).open(journal)
            {
                let _ = writeln!(f, "{line}");
            }
        }
    }

    // ---- crash recovery (spec: journal replay, Phase 5) ----------------------

    /// Replay the journal from a previous session. Three outcomes:
    /// - the journal's final state matches disk: history (undo depth included)
    ///   is restored across the restart;
    /// - the last event is an uncommitted intent and disk is part-way through
    ///   its writes: the transaction is rolled forward, then history restored;
    /// - anything else (the files changed while we were gone): the journal is
    ///   discarded. Files are the truth; recovery never guesses.
    fn recover(&mut self) {
        let Some(journal) = self.journal.clone() else { return };
        let Ok(text) = std::fs::read_to_string(&journal) else { return };
        if text.trim().is_empty() {
            return;
        }
        let lines: Vec<&str> = text.lines().collect();
        let mut events: Vec<JournalEvent> = Vec::new();
        for (i, line) in lines.iter().enumerate() {
            match serde_json::from_str::<JournalEvent>(line) {
                Ok(ev) => events.push(ev),
                Err(_) => {
                    // a torn final line from a crash mid-append is expected;
                    // corruption anywhere else makes the journal unusable
                    if i + 1 != lines.len() {
                        self.clear_journal();
                        return;
                    }
                }
            }
        }
        let mut history: Vec<Transaction> = Vec::new();
        let mut cursor: usize = 0;
        let mut i = 0;
        let mut torn: Option<JournalEvent> = None;
        while i < events.len() {
            let committed = matches!(events.get(i + 1), Some(JournalEvent::Commit));
            let last = i + 1 == events.len();
            match &events[i] {
                JournalEvent::Tx { tx } => {
                    push_replayed(&mut history, &mut cursor, tx.clone());
                }
                JournalEvent::Intent { .. } | JournalEvent::IntentUndo | JournalEvent::IntentRedo
                    if !committed && !last =>
                {
                    // an abandoned intent mid-journal: writes failed and the
                    // session went on — state after this is not reconstructable
                    self.clear_journal();
                    return;
                }
                JournalEvent::Intent { tx } => {
                    if committed {
                        push_replayed(&mut history, &mut cursor, tx.clone());
                        i += 1;
                    } else {
                        torn = Some(events[i].clone());
                    }
                }
                JournalEvent::IntentUndo => {
                    if committed {
                        cursor = cursor.saturating_sub(1);
                        i += 1;
                    } else {
                        torn = Some(JournalEvent::IntentUndo);
                    }
                }
                JournalEvent::IntentRedo => {
                    if committed {
                        cursor = (cursor + 1).min(history.len());
                        i += 1;
                    } else {
                        torn = Some(JournalEvent::IntentRedo);
                    }
                }
                JournalEvent::Commit => {}
                JournalEvent::Cursor { value } => cursor = (*value).min(history.len()),
            }
            i += 1;
        }
        let disk = |rel: &str| std::fs::read_to_string(self.root.join(rel)).ok();
        match torn {
            Some(JournalEvent::Intent { tx }) => {
                for c in &tx.changes {
                    let d = disk(&c.rel);
                    if d == c.after {
                        continue;
                    }
                    let forward_ok = d == c.before
                        && match &c.after {
                            Some(t) => self.write_atomic(&c.rel, t).is_ok(),
                            None => {
                                let _ = std::fs::remove_file(self.root.join(&c.rel));
                                true
                            }
                        };
                    if !forward_ok {
                        self.clear_journal();
                        return;
                    }
                }
                push_replayed(&mut history, &mut cursor, tx);
            }
            Some(JournalEvent::IntentUndo) if cursor > 0 => {
                let tx = history[cursor - 1].clone();
                for c in tx.changes.iter().rev() {
                    let d = disk(&c.rel);
                    if d == c.before {
                        continue;
                    }
                    let back_ok = d == c.after
                        && match &c.before {
                            Some(t) => self.write_atomic(&c.rel, t).is_ok(),
                            None => {
                                let _ = std::fs::remove_file(self.root.join(&c.rel));
                                true
                            }
                        };
                    if !back_ok {
                        self.clear_journal();
                        return;
                    }
                }
                cursor -= 1;
            }
            Some(JournalEvent::IntentRedo) if cursor < history.len() => {
                let tx = history[cursor].clone();
                for c in &tx.changes {
                    let d = disk(&c.rel);
                    if d == c.after {
                        continue;
                    }
                    let forward_ok = d == c.before
                        && match &c.after {
                            Some(t) => self.write_atomic(&c.rel, t).is_ok(),
                            None => {
                                let _ = std::fs::remove_file(self.root.join(&c.rel));
                                true
                            }
                        };
                    if !forward_ok {
                        self.clear_journal();
                        return;
                    }
                }
                cursor += 1;
            }
            Some(_) => {
                self.clear_journal();
                return;
            }
            None => {}
        }
        // verify: the journal's notion of every touched file must match disk
        let mut expected: BTreeMap<String, Option<String>> = BTreeMap::new();
        for tx in history.iter().take(cursor) {
            for c in &tx.changes {
                expected.insert(c.rel.clone(), c.after.clone());
            }
        }
        for tx in history.iter().skip(cursor) {
            for c in &tx.changes {
                expected.entry(c.rel.clone()).or_insert_with(|| c.before.clone());
            }
        }
        for (rel, want) in &expected {
            if disk(rel) != *want {
                self.clear_journal();
                return;
            }
        }
        self.history = history;
        self.cursor = cursor;
        self.compact_journal();
    }

    fn clear_journal(&mut self) {
        self.history.clear();
        self.cursor = 0;
        if let Some(j) = &self.journal {
            let _ = std::fs::write(j, "");
        }
    }

    /// Rewrite the journal as the adopted history — bounds its size to
    /// HISTORY_DEPTH transactions across restarts.
    fn compact_journal(&self) {
        let Some(journal) = &self.journal else { return };
        let mut out = String::new();
        for tx in &self.history {
            if let Ok(line) = serde_json::to_string(&JournalEvent::Tx { tx: tx.clone() }) {
                out.push_str(&line);
                out.push('\n');
            }
        }
        if self.cursor < self.history.len() {
            if let Ok(line) = serde_json::to_string(&JournalEvent::Cursor { value: self.cursor }) {
                out.push_str(&line);
                out.push('\n');
            }
        }
        let _ = std::fs::write(journal, out);
    }
}

/// Journal line format (JSONL). `Intent`/`Commit` bracket every write batch
/// (write-ahead); `Tx` records an already-on-disk observation (external edits,
/// compacted history); `Cursor` encodes undo depth in a compacted journal.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "kebab-case")]
enum JournalEvent {
    Tx { tx: Transaction },
    Intent { tx: Transaction },
    IntentUndo,
    IntentRedo,
    Commit,
    Cursor { value: usize },
}

/// History push with the same truncate/cap semantics as the live engine.
fn push_replayed(history: &mut Vec<Transaction>, cursor: &mut usize, tx: Transaction) {
    history.truncate(*cursor);
    history.push(tx);
    if history.len() > HISTORY_DEPTH {
        let drop = history.len() - HISTORY_DEPTH;
        history.drain(..drop);
    }
    *cursor = history.len();
}

fn op_label(op: &Operation) -> String {
    match op {
        Operation::Pin { id, x, y, .. } => format!("pin {id} at [{x}, {y}]"),
        Operation::Rename { id, name } => format!("rename {id} to {name:?}"),
        Operation::SetField { id, field, value } => format!("set {field} on {id} to {value:?}"),
        Operation::Create { id, kind, .. } => format!("create {kind} {id}"),
        Operation::Delete { id } => format!("delete {id}"),
        Operation::AddRelation { from, to, .. } => format!("relate {from} -> {to}"),
        Operation::DeleteRelation { from, to, .. } => format!("unrelate {from} -> {to}"),
        Operation::SetRelationField { from, to, field, .. } => {
            format!("set {field} on {from} -> {to}")
        }
    }
}

/// Crash-recovery journal location: per-workspace file under the OS temp dir,
/// keyed by a stable hash of the root path (spec: "journaled to the workspace
/// cache dir"). Public so tests (and support tooling) can find it.
pub fn journal_path(root: &Path) -> Option<PathBuf> {
    let mut hash: u64 = 0xcbf29ce484222325;
    for b in root.to_string_lossy().as_bytes() {
        hash ^= *b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    let dir = std::env::temp_dir().join("blastradius-journal");
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir.join(format!("{hash:016x}.jsonl")))
}

/// Upsert `layout.<key>: <value>`; creates `layout:` at end of file if absent.
fn set_layout_pin(text: &str, key: &str, value: &str) -> Result<String, String> {
    if splice::find_entry(text, &["layout", key])?.is_some() {
        // replace the pin line, preserving comments after the value
        let span = splice::find_entry(text, &["layout", key])?.unwrap();
        let mut out = String::new();
        for (i, line) in text.split_inclusive('\n').enumerate() {
            if i + 1 == span.key_line {
                let eol = line.trim_end_matches(['\r', '\n']);
                let nl = &line[eol.len()..];
                let indent = line.len() - line.trim_start().len();
                let comment = eol.find(" #").map(|p| &eol[p..]).unwrap_or("");
                out.push_str(&format!("{}{key}: {value}{comment}{nl}", " ".repeat(indent)));
            } else {
                out.push_str(line);
            }
        }
        Ok(out)
    } else if splice::find_entry(text, &["layout"])?.is_some() {
        let span = splice::find_entry(text, &["layout"])?.unwrap();
        let mut lines: Vec<String> = text.split_inclusive('\n').map(str::to_string).collect();
        let indent = " ".repeat(span.indent + 2);
        lines.insert(span.end_line, format!("{indent}{key}: {value}\n"));
        Ok(lines.concat())
    } else {
        let mut out = text.to_string();
        if !out.ends_with('\n') {
            out.push('\n');
        }
        out.push_str(&format!("layout:\n  {key}: {value}\n"));
        Ok(out)
    }
}

/// Set label/protocol on the sequence item matching raw endpoints.
fn set_relation_field_text(
    text: &str,
    raw_from: &str,
    raw_to: &str,
    raw_label: Option<&str>,
    field: &str,
    value: &str,
) -> Result<String, String> {
    let root = marked_yaml::parse_yaml(0, text).map_err(|e| e.to_string())?;
    let marked_yaml::Node::Mapping(map) = &root else {
        return Err("not a mapping".into());
    };
    let Some(marked_yaml::Node::Sequence(seq)) = map.get_node("relations") else {
        return Err("no relations section".into());
    };
    for item in seq.iter() {
        let marked_yaml::Node::Mapping(m) = item else { continue };
        let get = |k: &str| -> Option<String> {
            match m.get_node(k) {
                Some(marked_yaml::Node::Scalar(s)) => Some(s.as_str().to_string()),
                _ => None,
            }
        };
        if get("from").as_deref() == Some(raw_from)
            && get("to").as_deref() == Some(raw_to)
            && get("label").as_deref() == raw_label
        {
            let item_start = m.span().start().ok_or("no marker")?.line();
            let item_end = {
                // find deepest line in this item
                let mut max = item_start;
                for (k, v) in m.iter() {
                    if let Some(s) = k.span().start() {
                        max = max.max(s.line());
                    }
                    if let Some(s) = match v {
                        marked_yaml::Node::Scalar(s) => s.span().start(),
                        _ => None,
                    } {
                        max = max.max(s.line());
                    }
                }
                max
            };
            let lines: Vec<&str> = text.split_inclusive('\n').collect();
            // existing field line inside the item?
            let field_pat = format!("{field}:");
            for (i, line) in lines.iter().enumerate().take(item_end).skip(item_start - 1) {
                let eol = line.trim_end_matches(['\r', '\n']);
                let t = eol.trim_start().trim_start_matches("- ");
                if t.starts_with(&field_pat) {
                    let nl = &line[eol.len()..];
                    let kpos = eol.find(&field_pat).unwrap();
                    let comment = eol[kpos..].find(" #").map(|p| &eol[kpos + p..]).unwrap_or("");
                    let mut out: Vec<String> = lines.iter().map(|s| s.to_string()).collect();
                    out[i] = format!(
                        "{}{field}: {}{comment}{nl}",
                        &eol[..kpos],
                        splice::yaml_scalar(value)
                    );
                    return Ok(out.concat());
                }
            }
            // insert after the item's last line, aligned with item fields
            let first = lines[item_start - 1];
            let indent = first.len() - first.trim_start().len() + 2;
            let mut out: Vec<String> = lines.iter().map(|s| s.to_string()).collect();
            out.insert(
                item_end,
                format!("{}{field}: {}\n", " ".repeat(indent), splice::yaml_scalar(value)),
            );
            return Ok(out.concat());
        }
    }
    Err("relation item not found".into())
}
