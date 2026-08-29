//! MCP server (ADR-0012): a third head on the core library, next to the CLI
//! commands and the Tauri shell (ADR-0005). Speaks the Model Context Protocol
//! over stdio so coding agents can query the model with task-shaped tools —
//! and edit it through the sync engine, whose CST-preserving splices keep
//! agent edits indistinguishable from hand edits in the diff.
//!
//! Hand-rolled on purpose: MCP's stdio transport is newline-delimited
//! JSON-RPC 2.0 and this server needs three methods (initialize, tools/list,
//! tools/call). An SDK would bring an async runtime for a protocol we can
//! serve with a read-line loop — same reasoning as the vendored-libgit2 and
//! hand-rolled Structurizr choices.

use blastradius_core::diagnostics::Severity;
use blastradius_core::git::GitContext;
use blastradius_core::model::Workspace;
use blastradius_core::sync::{Operation, SyncEngine};
use serde_json::{json, Value};
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};

pub const PROTOCOL_VERSION: &str = "2025-06-18";

pub struct McpServer {
    root: PathBuf,
    engine: SyncEngine,
}

/// Resolve the workspace folder like the desktop app does: the argument (or
/// the current directory), with workspace discovery below it — so a repo
/// root works, and the dogfood `./docs` layout is just the common case.
pub fn resolve_root(arg: Option<&str>) -> Result<PathBuf, String> {
    let base = PathBuf::from(arg.unwrap_or("."));
    let hits = blastradius_core::discover::discover_workspaces(&base);
    match hits.as_slice() {
        // strip_verbatim: canonicalize() yields a `\?\` verbatim path on
        // Windows, which then leaks into every message built from it and
        // every extractor argument derived from it. The extractors learned
        // that the hard way in 0.6.2; no reason for the rest to.
        [one] => Ok(blastradius_core::introspect::strip_verbatim(
            one.canonicalize().unwrap_or_else(|_| one.clone()),
        )),
        [] => Err(format!(
            "{}: no blastradius.yaml here or below — pass a workspace folder (or run `blastradius init`)",
            base.display()
        )),
        many => Err(format!(
            "{}: {} workspaces found — pass one explicitly:\n{}",
            base.display(),
            many.len(),
            many.iter().map(|p| format!("  {}", p.display())).collect::<Vec<_>>().join("\n")
        )),
    }
}

/// Blocking serve loop: one JSON-RPC message per stdin line, responses on
/// stdout, logs on stderr (the MCP stdio contract).
pub fn serve(arg: Option<&str>) -> Result<(), String> {
    let root = resolve_root(arg)?;
    let mut server = McpServer::new(&root);
    eprintln!("blastradius mcp: serving {}", root.display());
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    for line in stdin.lock().lines() {
        let line = line.map_err(|e| e.to_string())?;
        if line.trim().is_empty() {
            continue;
        }
        let msg: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(e) => {
                let err = json!({"jsonrpc": "2.0", "id": null,
                    "error": {"code": -32700, "message": format!("parse error: {e}")}});
                writeln!(stdout, "{err}").map_err(|e| e.to_string())?;
                stdout.flush().map_err(|e| e.to_string())?;
                continue;
            }
        };
        if let Some(resp) = server.handle(&msg) {
            writeln!(stdout, "{resp}").map_err(|e| e.to_string())?;
            stdout.flush().map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

impl McpServer {
    pub fn new(root: &Path) -> Self {
        McpServer { root: root.to_path_buf(), engine: SyncEngine::open(root) }
    }

    /// Dispatch one JSON-RPC message; None for notifications (no response).
    pub fn handle(&mut self, msg: &Value) -> Option<Value> {
        let method = msg.get("method")?.as_str()?;
        let id = msg.get("id").cloned();
        // notifications carry no id and get no response
        if id.is_none() || id == Some(Value::Null) {
            return None;
        }
        let id = id.unwrap();
        let params = msg.get("params").cloned().unwrap_or_else(|| json!({}));
        let result = match method {
            "initialize" => Ok(json!({
                "protocolVersion": params.get("protocolVersion")
                    .and_then(Value::as_str).unwrap_or(PROTOCOL_VERSION),
                "capabilities": { "tools": {} },
                "serverInfo": {
                    "name": "blastradius",
                    "version": env!("CARGO_PKG_VERSION"),
                },
                "instructions": "Query and edit the Blastradius architecture model \
                    (C4, YAML-in-repo). Start with workspace_summary; use blast_radius \
                    before changing an element; edits via apply_operation are \
                    format-preserving splices, and undo reverts the last one. \
                    git_status/git_conflicts read repository state; a merge \
                    conflict resolves per element through resolve_conflicts \
                    (staged via the user's git — the commit stays theirs). \
                    introspect derives read-only L4 code elements for \
                    components with a source: mapping.",
            })),
            "ping" => Ok(json!({})),
            "tools/list" => Ok(json!({ "tools": tool_definitions() })),
            "tools/call" => {
                let name = params.get("name").and_then(Value::as_str).unwrap_or("");
                let args = params.get("arguments").cloned().unwrap_or_else(|| json!({}));
                let outcome = self.call_tool(name, &args);
                let (text, is_error) = match outcome {
                    Ok(v) => (serde_json::to_string_pretty(&v).unwrap_or_default(), false),
                    Err(e) => (e, true),
                };
                Ok(json!({
                    "content": [{ "type": "text", "text": text }],
                    "isError": is_error,
                }))
            }
            other => Err(format!("method not found: {other}")),
        };
        Some(match result {
            Ok(r) => json!({"jsonrpc": "2.0", "id": id, "result": r}),
            Err(m) => json!({"jsonrpc": "2.0", "id": id,
                "error": {"code": -32601, "message": m}}),
        })
    }

    fn call_tool(&mut self, name: &str, args: &Value) -> Result<Value, String> {
        // agents also edit files directly — pick up external changes first,
        // exactly like the app's watcher path (echo suppression included)
        self.engine.external_scan();
        let str_arg = |k: &str| args.get(k).and_then(Value::as_str).map(str::to_string);
        match name {
            "workspace_summary" => Ok(self.workspace_summary()),
            "find_elements" => Ok(self.find_elements(str_arg("query"), str_arg("kind"))),
            "element" => self.element(&str_arg("id").ok_or("id is required")?),
            "blast_radius" => self.blast_radius(&str_arg("id").ok_or("id is required")?),
            "introspect" => self.introspect_tool(str_arg("component")),
            "git_status" => self.git_status_tool(),
            "git_conflicts" => self.git_conflicts_tool(),
            "resolve_conflicts" => {
                let res = args.get("resolution").cloned().ok_or("resolution is required")?;
                self.resolve_conflicts_tool(res)
            }
            "validate" => Ok(self.validate()),
            "model_diff" => self.model_diff(str_arg("base")),
            "doc" => self.doc(&str_arg("id").ok_or("id is required")?),
            "model_format" => Ok(json!({ "format": blastradius_core::format_ref::full_reference() })),
            "apply_operation" => {
                let op = args.get("op").cloned().ok_or("op is required")?;
                let op: Operation = parse_operation(op)?;
                let tx = self.engine.apply(op)?;
                Ok(json!({
                    "applied": tx.label,
                    "files": tx.changes.iter().map(|c| c.rel.clone()).collect::<Vec<_>>(),
                    "diagnostics": self.diagnostics_json(),
                }))
            }
            "apply_operations" => {
                let list = args
                    .get("ops")
                    .and_then(Value::as_array)
                    .ok_or("ops is required and must be an array")?;
                let ops = list
                    .iter()
                    .enumerate()
                    .map(|(i, v)| parse_operation(v.clone()).map_err(|e| format!("ops[{i}]: {e}")))
                    .collect::<Result<Vec<Operation>, String>>()?;
                let count = ops.len();
                let tx = self.engine.apply_batch(ops)?;
                Ok(json!({
                    "applied": tx.label,
                    "operations": count,
                    "files": tx.changes.iter().map(|c| c.rel.clone()).collect::<Vec<_>>(),
                    "diagnostics": self.diagnostics_json(),
                }))
            }
            "undo" => {
                let label = self.engine.undo()?;
                Ok(json!({ "undone": label }))
            }
            "redo" => {
                let label = self.engine.redo()?;
                Ok(json!({ "redone": label }))
            }
            other => Err(format!("unknown tool: {other}")),
        }
    }

    fn ws(&self) -> &Workspace {
        &self.engine.model
    }

    fn diagnostics_json(&self) -> Vec<Value> {
        self.engine
            .diagnostics
            .iter()
            .map(|d| {
                json!({
                    "severity": format!("{:?}", d.severity).to_lowercase(),
                    "file": d.file, "line": d.line, "message": d.message,
                })
            })
            .collect()
    }

    fn element_brief(&self, id: &str) -> Value {
        let el = &self.ws().elements[id];
        let mut v = json!({
            "id": el.id, "kind": el.kind.as_str(), "name": el.name,
            "file": el.file, "line": el.line,
        });
        if let Some(t) = &el.tech {
            v["tech"] = json!(t);
        }
        if let Some(d) = &el.description {
            v["description"] = json!(d);
        }
        v
    }

    fn workspace_summary(&mut self) -> Value {
        let ws = self.ws();
        let mut by_kind = std::collections::BTreeMap::new();
        for el in ws.elements.values() {
            *by_kind.entry(el.kind.as_str()).or_insert(0u64) += 1;
        }
        let systems: Vec<Value> = ws
            .elements
            .values()
            .filter(|e| e.kind == blastradius_core::model::ElementKind::System)
            .map(|e| {
                let children = ws.elements.values().filter(|c| {
                    c.id.strip_prefix(&format!("{}.", e.id))
                        .is_some_and(|rest| !rest.contains('.'))
                });
                json!({
                    "id": e.id, "name": e.name,
                    "containers": children.map(|c| c.id.clone()).collect::<Vec<_>>(),
                })
            })
            .collect();
        json!({
            "workspace": ws.name,
            "root": self.root.display().to_string(),
            "elements": by_kind,
            "systems": systems,
            "views": ws.views.iter().map(|v| json!({
                "id": v.id, "level": v.level, "scope": v.scope, "pins": v.layout.len(),
            })).collect::<Vec<_>>(),
            "docs": ws.docs.iter().map(|d| json!({
                "id": d.id, "type": d.doc_type, "status": d.status, "file": d.file,
            })).collect::<Vec<_>>(),
            "stale_files": self.engine.stale.iter().collect::<Vec<_>>(),
            "errors": self.engine.diagnostics.iter()
                .filter(|d| d.severity == Severity::Error).count(),
        })
    }

    fn find_elements(&self, query: Option<String>, kind: Option<String>) -> Value {
        let q = query.unwrap_or_default().to_lowercase();
        let matches: Vec<&str> = self
            .ws()
            .elements
            .values()
            .filter(|e| {
                // Exact, not prefix: "container" must not also drag in every
                // "container instance" (ADR-0018). "external" still matches
                // the "external system" label.
                let kind_ok = kind.as_deref().is_none_or(|k| {
                    e.kind.as_str() == k || (k == "external" && e.kind.as_str().contains("external"))
                });
                let text_ok = q.is_empty()
                    || e.id.to_lowercase().contains(&q)
                    || e.name.to_lowercase().contains(&q)
                    || e.description.as_deref().unwrap_or("").to_lowercase().contains(&q);
                kind_ok && text_ok
            })
            .map(|e| e.id.as_str())
            .collect();
        let mut capped: Vec<Value> = matches.iter().take(50).map(|id| self.element_brief(id)).collect();
        // Derived (L4) elements search too — marked, so agents know they are
        // read-only code facts, not authored model (spec/l4-introspection.md).
        let derived: Vec<Value> = self
            .ws()
            .derived
            .iter()
            .flat_map(|g| g.elements.iter().map(move |e| (g, e)))
            .filter(|(_, e)| {
                let kind_ok = kind.as_deref().is_none_or(|k| e.kind == k);
                let text_ok = q.is_empty()
                    || e.id.to_lowercase().contains(&q)
                    || e.name.to_lowercase().contains(&q);
                kind_ok && text_ok
            })
            .map(|(g, e)| {
                json!({"id": e.id, "kind": e.kind, "name": e.name, "derived": true,
                       "component": g.component, "path": e.path})
            })
            .collect();
        let total = matches.len() + derived.len();
        capped.extend(derived.into_iter().take(50usize.saturating_sub(capped.len())));
        json!({
            "total": total,
            "shown": capped.len(),
            "elements": capped,
        })
    }

    /// Derived (L4) element detail: code-level, read-only, source-pointing
    /// (spec/l4-introspection.md).
    fn derived_element_json(&self, id: &str) -> Option<Value> {
        let ws = self.ws();
        let graph = ws.derived.iter().find(|g| g.elements.iter().any(|e| e.id == id))?;
        let d = graph.elements.iter().find(|e| e.id == id)?;
        let children: Vec<&str> = graph
            .elements
            .iter()
            .filter(|e| e.parent.as_deref() == Some(id))
            .map(|e| e.id.as_str())
            .collect();
        let edge = |from: &str, to: &str, kind: &str| json!({"from": from, "to": to, "kind": kind});
        let outgoing: Vec<Value> =
            graph.edges.iter().filter(|e| e.from == id).map(|e| edge(&e.from, &e.to, &e.kind)).collect();
        let incoming: Vec<Value> =
            graph.edges.iter().filter(|e| e.to == id).map(|e| edge(&e.from, &e.to, &e.kind)).collect();
        Some(json!({
            "id": d.id, "kind": d.kind, "name": d.name, "derived": true,
            "component": graph.component, "language": graph.language,
            "path": d.path, "line": d.line, "stale": graph.stale,
            "children": children,
            "edges": {"outgoing": outgoing, "incoming": incoming},
            "note": "derived from source — edit the file and re-run introspect; apply_operation refuses derived ids",
        }))
    }

    fn element(&self, id: &str) -> Result<Value, String> {
        if let Some(v) = self.derived_element_json(id) {
            return Ok(v);
        }
        let ws = self.ws();
        let el = ws.elements.get(id).ok_or_else(|| unknown_element(ws, id))?;
        let child_prefix = format!("{id}.");
        let children: Vec<&str> = ws
            .elements
            .keys()
            .filter(|k| k.strip_prefix(&child_prefix).is_some_and(|r| !r.contains('.')))
            .map(String::as_str)
            .collect();
        let mut outgoing = Vec::new();
        let mut incoming = Vec::new();
        for (from, to, r) in ws.resolved_relations() {
            let rel = |peer: &str| {
                json!({
                    "peer": peer, "label": r.label, "protocol": r.protocol,
                    "file": r.file, "line": r.line,
                })
            };
            if from == id || from.starts_with(&child_prefix) {
                outgoing.push(rel(&to));
            }
            if to == id || to.starts_with(&child_prefix) {
                incoming.push(rel(&from));
            }
        }
        let docs: Vec<Value> = ws
            .docs
            .iter()
            .filter(|d| d.elements.iter().any(|e| e == id))
            .map(|d| json!({"id": d.id, "type": d.doc_type, "file": d.file}))
            .collect();
        let views: Vec<&str> = ws
            .views
            .iter()
            .filter(|v| {
                v.scope == id
                    || v.scope.starts_with(&child_prefix)
                    || id.starts_with(&format!("{}.", v.scope))
            })
            .map(|v| v.id.as_str())
            .collect();
        let mut out = self.element_brief(&el.id);
        out["external"] = json!(el.external);
        out["children"] = json!(children);
        out["relations"] = json!({"outgoing": outgoing, "incoming": incoming});
        out["docs"] = json!(docs);
        out["views"] = json!(views);
        Ok(out)
    }

    /// The tool the product is named for: everything implicated when this
    /// element changes — contents, transitive dependents (reverse reachability
    /// over relations), direct dependencies, governing docs, affected views.
    /// Code-level blast radius for a derived (L4) element: fan-in/fan-out over
    /// the facts edges — real dependents in source, not modeled relations.
    fn derived_blast_radius(&self, id: &str) -> Option<Value> {
        let ws = self.ws();
        let graph = ws.derived.iter().find(|g| g.elements.iter().any(|e| e.id == id))?;
        let mut dist: std::collections::BTreeMap<&str, u64> = Default::default();
        let mut frontier = vec![id];
        let mut depth = 0u64;
        while !frontier.is_empty() {
            depth += 1;
            let mut next = Vec::new();
            for e in &graph.edges {
                if frontier.iter().any(|f| e.to == *f) && e.from != id && !dist.contains_key(e.from.as_str()) {
                    dist.insert(&e.from, depth);
                    next.push(e.from.as_str());
                }
            }
            frontier = next;
        }
        let dependents: Vec<Value> =
            dist.iter().map(|(k, d)| json!({"id": k, "distance": d})).collect();
        let dependencies: Vec<&str> =
            graph.edges.iter().filter(|e| e.from == id).map(|e| e.to.as_str()).collect();
        let el = graph.elements.iter().find(|e| e.id == id)?;
        Some(json!({
            "id": id, "derived": true, "component": graph.component, "path": el.path,
            "dependents": dependents, "dependencies": dependencies,
            "note": "code-level radius from the committed facts; the owning component's modeled radius is blast_radius on the component id",
        }))
    }

    fn blast_radius(&self, id: &str) -> Result<Value, String> {
        if let Some(v) = self.derived_blast_radius(id) {
            return Ok(v);
        }
        let ws = self.ws();
        if !ws.elements.contains_key(id) {
            return Err(unknown_element(ws, id));
        }
        let child_prefix = format!("{id}.");
        let in_target = |e: &str| e == id || e.starts_with(&child_prefix);

        let resolved = ws.resolved_relations();
        // reverse BFS: who (transitively) depends on the target subtree
        let mut dist: std::collections::BTreeMap<String, u64> = Default::default();
        let mut frontier: Vec<String> = vec![id.to_string()];
        let mut depth = 0u64;
        while !frontier.is_empty() {
            depth += 1;
            let mut next = Vec::new();
            for (from, to, _) in &resolved {
                let hits = if depth == 1 {
                    in_target(to)
                } else {
                    frontier.iter().any(|f| to == f || to.starts_with(&format!("{f}.")))
                };
                if hits && !in_target(from) && !dist.contains_key(from) {
                    dist.insert(from.clone(), depth);
                    next.push(from.clone());
                }
            }
            frontier = next;
        }
        let dependencies: Vec<Value> = resolved
            .iter()
            .filter(|(from, to, _)| in_target(from) && !in_target(to))
            .map(|(_, to, r)| json!({"id": to, "label": r.label}))
            .collect();
        let ancestors: Vec<&str> = {
            let mut v = Vec::new();
            let mut cur = id;
            while let Some(pos) = cur.rfind('.') {
                cur = &cur[..pos];
                v.push(cur);
            }
            v
        };
        let docs: Vec<Value> = ws
            .docs
            .iter()
            .filter_map(|d| {
                let direct = d.elements.iter().any(|e| in_target(e));
                let inherited = !direct && d.elements.iter().any(|e| ancestors.contains(&e.as_str()));
                (direct || inherited).then(|| {
                    json!({"id": d.id, "type": d.doc_type, "file": d.file,
                        "governs": if direct { "directly" } else { "via parent" }})
                })
            })
            .collect();
        let views: Vec<&str> = ws
            .views
            .iter()
            .filter(|v| {
                in_target(&v.scope)
                    || id.starts_with(&format!("{}.", v.scope))
                    || ancestors.contains(&v.scope.as_str())
            })
            .map(|v| v.id.as_str())
            .collect();
        let mut dependents: Vec<Value> = dist
            .iter()
            .map(|(e, d)| json!({"id": e, "distance": d}))
            .collect();
        dependents.sort_by_key(|v| v["distance"].as_u64());
        Ok(json!({
            "target": id,
            "contains": ws.elements.keys()
                .filter(|k| k.starts_with(&child_prefix)).collect::<Vec<_>>(),
            "dependents": dependents,
            "dependencies": dependencies,
            "docs": docs,
            "views": views,
        }))
    }

    fn validate(&mut self) -> Value {
        // a full fresh parse — identical to `blastradius validate`
        self.engine.reload_all();
        let errors =
            self.engine.diagnostics.iter().filter(|d| d.severity == Severity::Error).count();
        json!({
            "result": if errors == 0 { "PASS" } else { "FAIL" },
            "elements": self.ws().elements.len(),
            "diagnostics": self.diagnostics_json(),
        })
    }

    fn model_diff(&self, base: Option<String>) -> Result<Value, String> {
        let ctx = GitContext::discover(&self.root)
            .ok_or("workspace is not inside a git repository")?;
        let base_label = base
            .or_else(|| ctx.default_base())
            .ok_or("no base ref: pass base explicitly (e.g. \"HEAD\" or a branch)")?;
        let (base_ws, base_diags) = ctx.load_at(&base_label)?;
        if blastradius_core::diagnostics::has_errors(&base_diags) {
            return Err(format!("base revision {base_label} does not parse"));
        }
        let (cur_ws, cur_diags) = blastradius_core::load_workspace(&self.root);
        if blastradius_core::diagnostics::has_errors(&cur_diags) {
            return Err("working tree does not parse".to_string());
        }
        let payload = blastradius_core::diff::diff_payload(&base_label, &base_ws, &cur_ws);
        serde_json::to_value(&payload).map_err(|e| e.to_string())
    }

    fn doc(&self, id: &str) -> Result<Value, String> {
        let ws = self.ws();
        let d = ws.docs.iter().find(|d| d.id == id).ok_or_else(|| {
            format!(
                "unknown doc {id:?} — known: {}",
                ws.docs.iter().map(|d| d.id.as_str()).collect::<Vec<_>>().join(", ")
            )
        })?;
        let raw = std::fs::read_to_string(self.root.join(&d.file)).map_err(|e| e.to_string())?;
        // strip the frontmatter block; the metadata rides along as fields
        let body = match raw.strip_prefix("---\n").and_then(|rest| rest.split_once("\n---\n")) {
            Some((_, b)) => b,
            None => raw.as_str(),
        };
        Ok(json!({
            "id": d.id, "type": d.doc_type, "status": d.status,
            "elements": d.elements, "file": d.file, "body": body,
        }))
    }
}

impl McpServer {
    /// Repository state for agents (ADR-0007 surfaces, read-only): branch,
    /// dirty and conflicted files. No repo is an answer, not an error.
    fn git_status_tool(&self) -> Result<Value, String> {
        let Some(ctx) = GitContext::discover(&self.root) else {
            return Ok(json!({"repository": false}));
        };
        let status = ctx.status()?;
        Ok(json!({
            "repository": true,
            "status": serde_json::to_value(&status).map_err(|e| e.to_string())?,
        }))
    }

    /// The current merge conflict, element-shaped (ADR-0015): conflicted
    /// files plus per-element ours/theirs field values.
    fn git_conflicts_tool(&self) -> Result<Value, String> {
        let Some(ctx) = GitContext::discover(&self.root) else {
            return Ok(json!({"repository": false, "conflicts": null}));
        };
        match ctx.conflicts(&self.root)? {
            None => Ok(json!({"repository": true, "conflicts": null})),
            Some(c) => Ok(json!({
                "repository": true,
                "conflicts": serde_json::to_value(&c).map_err(|e| e.to_string())?,
                "hint": "decide per element and call resolve_conflicts \
                    {resolution: {elements: {\"<id>\": \"ours\"|\"theirs\"}, \
                    files: {\"<rel>\": \"ours\"|\"theirs\"}}} — anything \
                    undecided keeps ours; files: is the whole answer for \
                    conflicted views/docs",
            })),
        }
    }

    /// Apply an ADR-0015 resolution: CST splices onto the chosen side, files
    /// written and staged via the user's own git; the commit stays theirs.
    fn resolve_conflicts_tool(&mut self, resolution: Value) -> Result<Value, String> {
        let ctx = GitContext::discover(&self.root).ok_or("not inside a git repository")?;
        let res: blastradius_core::resolve::Resolution =
            serde_json::from_value(resolution).map_err(|e| format!("bad resolution: {e}"))?;
        let staged = blastradius_core::resolve::resolve(&ctx, &self.root, &res)?;
        self.engine.reload_all();
        Ok(json!({
            "staged": staged,
            "diagnostics": self.diagnostics_json(),
            "note": "resolved files are written and staged — committing (and any merge --continue) is the user's move",
        }))
    }

    /// Run L4 extraction for opted-in components and reload the workspace —
    /// the MCP face of `blastradius introspect` (spec/l4-introspection.md).
    fn introspect_tool(&mut self, component: Option<String>) -> Result<Value, String> {
        use blastradius_core::introspect as intro;
        use blastradius_core::model::ElementKind;

        let repo = intro::find_repo_root(&self.root)
            .ok_or("no repository root above the workspace — `source:` roots are repo-root-relative")?;
        let targets: Vec<(String, blastradius_core::model::SourceMapping)> = self
            .ws()
            .elements
            .values()
            .filter(|e| e.kind == ElementKind::Component && e.source.is_some())
            .filter(|e| component.as_deref().is_none_or(|c| e.id == c))
            .map(|e| (e.id.clone(), e.source.clone().expect("filtered")))
            .collect();
        if targets.is_empty() {
            return Err(match component {
                Some(c) => format!("{c:?} is not a component with a `source:` mapping"),
                None => "no components opt into introspection — add a `source:` mapping first".into(),
            });
        }
        let mut results = Vec::new();
        for (id, mapping) in targets {
            match intro::extract(&repo, &id, &mapping) {
                Ok((facts, warnings)) => {
                    let bytes = intro::facts_bytes(&facts);
                    let path = self.root.join("model").join("derived").join(format!("{id}.l4.json"));
                    let existing = std::fs::read_to_string(&path).ok().map(|t| t.replace("\r\n", "\n"));
                    let changed = existing.as_deref() != Some(bytes.as_str());
                    if changed {
                        std::fs::create_dir_all(path.parent().expect("has parent"))
                            .and_then(|()| std::fs::write(&path, &bytes))
                            .map_err(|e| format!("{id}: {e}"))?;
                    }
                    results.push(json!({
                        "component": id, "written": changed,
                        "elements": facts.elements.len(), "edges": facts.edges.len(),
                        "warnings": warnings,
                    }));
                }
                Err(e) => results.push(json!({"component": id, "error": e})),
            }
        }
        self.engine.reload_all();
        Ok(json!({"results": results, "diagnostics": self.diagnostics_json()}))
    }
}

/// serde's own message ("missing field `kind`") never says which shape was
/// expected, which is most of what makes a malformed call hard to fix.
fn parse_operation(v: Value) -> Result<Operation, String> {
    let op = v.get("op").and_then(Value::as_str).unwrap_or_default().to_string();
    serde_json::from_value(v).map_err(|e| match op.is_empty() {
        true => format!("bad operation: {e}. Every operation needs an `op` field naming its shape — see this tool's input schema, or call model_format."),
        false => format!("bad {op:?} operation: {e}. Check the {op:?} shape in this tool's input schema."),
    })
}

fn unknown_element(ws: &Workspace, id: &str) -> String {
    let near: Vec<&str> = ws
        .elements
        .keys()
        .filter(|k| k.contains(id.rsplit('.').next().unwrap_or(id)))
        .take(5)
        .map(String::as_str)
        .collect();
    if near.is_empty() {
        format!("unknown element {id:?} — try find_elements")
    } else {
        format!("unknown element {id:?} — did you mean: {}", near.join(", "))
    }
}

/// The `Operation` enum (sync.rs) as JSON Schema, one branch per variant.
///
/// It used to be `{"type": "object"}` with the shapes described in prose, so
/// every mistake surfaced as a serde error after the call. A model that can
/// see the shapes mostly cannot make them (docs/roadmap.md, first-user
/// findings). Kept beside the enum by the round-trip test in tests/mcp.rs.
fn operation_schema() -> Value {
    let variant = |op: &str, doc: &str, props: Value, required: Vec<&str>| {
        let mut all = props.as_object().cloned().unwrap_or_default();
        all.insert("op".into(), json!({"const": op}));
        let mut req = vec!["op".to_string()];
        req.extend(required.into_iter().map(str::to_string));
        json!({"type": "object", "title": op, "description": doc,
               "properties": all, "required": req, "additionalProperties": false})
    };
    let id = |doc: &str| json!({"type": "string", "description": doc});
    json!({
        "description": "one model operation",
        "oneOf": [
            variant("create", "Add an element. Omit `parent` for a top-level person, external system, or system.",
                json!({
                    "parent": {"type": ["string", "null"], "description": "id of the containing element; omit for top level"},
                    "id": id("dotted id, unique and immutable — renaming later changes `name`, never this"),
                    "name": id("human-readable label"),
                    // Hyphenated, unlike the space-separated display names
                    // find_elements filters on — these are the strings
                    // compute_create matches.
                    "kind": {"type": "string", "enum": ["person", "external", "system", "container", "component", "environment", "deployment-node", "container-instance"],
                             "description": "must be legal inside the parent: containers in systems, components in containers, deployment-node under an environment or another node"},
                }), vec!["id", "name", "kind"]),
            variant("rename", "Change an element's display name. Ids never change.",
                json!({"id": id("element id"), "name": id("new display name")}), vec!["id", "name"]),
            variant("set-field", "Set one scalar field on an element.",
                json!({
                    "id": id("element id"),
                    "field": {"type": "string", "enum": ["name", "description", "tech", "group", "replicas", "external"]},
                    "value": id("new value; an empty string removes the field. `group` draws a boundary round siblings sharing it; `replicas` is a count on a deployment node or container instance; `external` is true on a system outside your control"),
                }), vec!["id", "field", "value"]),
            variant("set-source", "Point a component at the code it is implemented by, so `introspect` can derive its modules and types — or omit `source` to stop introspecting it. Components only.",
                json!({
                    "id": id("component id"),
                    "source": {
                        "type": ["object", "null"],
                        "description": "omit or null to remove the mapping",
                        "properties": {
                            "language": {"type": "string", "enum": ["typescript", "csharp", "rust"]},
                            "root": id("folder holding the code, relative to the repository root — not to the workspace"),
                            "include": {"type": "array", "items": {"type": "string"},
                                        "description": "globs relative to root; omit for the language's defaults"},
                            "exclude": {"type": "array", "items": {"type": "string"}},
                            "mode": {"type": "string", "enum": ["syntax", "semantic"],
                                     "description": "semantic resolves cross-project references; C# only"},
                            "extractor": id("override the extractor command; omit for the language default"),
                        },
                        "required": ["language", "root"],
                    },
                }), vec!["id"]),
            variant("delete", "Remove an element. Its relations and layout pins go in the same transaction — call blast_radius first.",
                json!({"id": id("element id")}), vec!["id"]),
            variant("add-relation", "Connect two elements. Direction is the dependency: from the element that depends on, to the one depended upon.",
                json!({
                    "from": id("source element id"), "to": id("target element id"),
                    "label": id("what the dependency is, e.g. \"reads\", \"calls\""),
                    "protocol": id("technology or protocol, e.g. \"JSON/HTTPS\" — rendered in brackets"),
                }), vec!["from", "to"]),
            variant("delete-relation", "Remove a relation. Pass `label` only when several relations share the same pair.",
                json!({"from": id("source element id"), "to": id("target element id"), "label": id("disambiguates parallel relations")}),
                vec!["from", "to"]),
            variant("set-relation-field", "Set one field on an existing relation.",
                json!({
                    "from": id("source element id"), "to": id("target element id"),
                    "label": id("identifies which relation, when the pair has several"),
                    "field": {"type": "string", "enum": ["label", "protocol"]},
                    "value": id("new value"),
                }), vec!["from", "to", "field", "value"]),
            variant("pin", "Place an element at fixed coordinates in one view; without a pin the layout engine decides.",
                json!({
                    "view": id("view id; omit for the default view at this level and scope"),
                    "level": {"type": "string", "enum": ["L1", "L2", "L3", "LD"]},
                    "scope": id("id of the element being looked inside; omit at L1"),
                    "id": id("element to place"),
                    "x": {"type": "integer"}, "y": {"type": "integer"},
                }), vec!["level", "id", "x", "y"]),
            variant("unpin", "Release pinned coordinates in one view: one element, or every pin in the view when `id` is omitted — which puts the view back to auto-layout.",
                json!({
                    "view": id("view id; omit for the default view at this level and scope"),
                    "level": {"type": "string", "enum": ["L1", "L2", "L3", "LD"]},
                    "scope": id("id of the element being looked inside; omit at L1"),
                    "id": id("element to release; omit to release every pin in the view"),
                }), vec!["level"]),
            variant("show-description", "Draw an element's description inside its box in one view, or stop drawing it. Per view, not per element: set the description itself with set-field.",
                json!({
                    "view": id("view id; omit for the default view at this level and scope"),
                    "level": {"type": "string", "enum": ["L1", "L2", "L3", "LD"]},
                    "scope": id("id of the element being looked inside; omit at L1"),
                    "id": id("element whose box is affected"),
                    "show": {"type": "boolean", "description": "true draws the description, false removes it from this view"},
                }), vec!["level", "id", "show"]),
        ],
    })
}

fn tool_definitions() -> Vec<Value> {
    let obj = |props: Value, required: Vec<&str>| {
        json!({"type": "object", "properties": props, "required": required})
    };
    vec![
        json!({
            "name": "workspace_summary",
            "description": "Orientation: workspace name, element counts by kind, systems with their containers, views, docs, and current validation state. Call this first.",
            "inputSchema": obj(json!({}), vec![]),
        }),
        json!({
            "name": "find_elements",
            "description": "Search elements by substring (id, name, description) and/or kind. Returns up to 50 briefs with file:line locations.",
            "inputSchema": obj(json!({
                "query": {"type": "string", "description": "case-insensitive substring"},
                "kind": {"type": "string", "enum": ["person", "external", "system", "container", "component", "environment", "deployment node", "container instance"]},
            }), vec![]),
        }),
        json!({
            "name": "element",
            "description": "Full detail for one element by dotted id: fields, children, resolved incoming/outgoing relations, governing docs, views it appears in.",
            "inputSchema": obj(json!({"id": {"type": "string", "description": "dotted element id, e.g. shop.api"}}), vec!["id"]),
        }),
        json!({
            "name": "blast_radius",
            "description": "Impact analysis: everything implicated if this element changes — its contents, transitive dependents (with distance), direct dependencies, governing docs (direct and via parent), and affected views. Use before modifying or deleting anything.",
            "inputSchema": obj(json!({"id": {"type": "string"}}), vec!["id"]),
        }),
        json!({
            "name": "introspect",
            "description": "Run L4 code introspection for components with a `source:` mapping: extracts modules/types/edges from the mapped source tree and (re)writes the committed facts under model/derived/. Derived elements then appear in find_elements, element, and blast_radius, marked derived — they are read-only; edit the source instead. Omit `component` to extract every opted-in component.",
            "inputSchema": obj(json!({"component": {"type": "string", "description": "component id; omit for all opted-in components"}}), vec![]),
        }),
        json!({
            "name": "validate",
            "description": "Re-parse and validate the workspace from disk. Returns PASS/FAIL plus every diagnostic with file and line. Run after editing YAML directly.",
            "inputSchema": obj(json!({}), vec![]),
        }),
        json!({
            "name": "model_diff",
            "description": "Semantic model diff of the working tree against a git ref (default: merge-base with the default branch): elements and relations added/removed/changed, layout-only changes separated.",
            "inputSchema": obj(json!({"base": {"type": "string", "description": "git ref; defaults to the merge-base"}}), vec![]),
        }),
        json!({
            "name": "doc",
            "description": "Read a model-registered document (ADR, spec, PRD...) by doc id: metadata, linked elements, and the markdown body.",
            "inputSchema": obj(json!({"id": {"type": "string", "description": "doc id from workspace_summary"}}), vec!["id"]),
        }),
        json!({
            "name": "git_status",
            "description": "Repository state (read-only, ADR-0007): current branch, dirty workspace files, conflicted files. {repository: false} when the workspace is not in a git repo.",
            "inputSchema": obj(json!({}), vec![]),
        }),
        json!({
            "name": "git_conflicts",
            "description": "The current merge conflict, element-shaped (ADR-0015): conflicted workspace files plus per-element ours/theirs field values where both sides changed the same element. null when there is no conflict. Follow up with resolve_conflicts.",
            "inputSchema": obj(json!({}), vec![]),
        }),
        json!({
            "name": "resolve_conflicts",
            "description": "Resolve the current merge conflict from per-element decisions (ADR-0015): each choice is applied as a format-preserving splice onto the chosen side's text (comments survive), files are validated before writing, written, and staged via the user's own git — the commit stays the user's. Anything undecided keeps ours. `files` picks a whole side for conflicted files without element conflicts (views, docs).",
            "inputSchema": obj(json!({
                "resolution": {"type": "object", "properties": {
                    "elements": {"type": "object", "description": "element id -> \"ours\" | \"theirs\"", "additionalProperties": {"type": "string", "enum": ["ours", "theirs"]}},
                    "files": {"type": "object", "description": "workspace-relative file -> \"ours\" | \"theirs\"", "additionalProperties": {"type": "string", "enum": ["ours", "theirs"]}},
                }},
            }), vec!["resolution"]),
        }),
        json!({
            "name": "apply_operation",
            "description": "Edit the model. Prefer this over writing YAML yourself: it is a targeted, format-preserving splice (comments, key order and formatting survive), it validates before writing and refuses anything that would invalidate the workspace, and it is undoable. Hand-written YAML has none of those properties. `op` is one of the shapes in the schema; `apply_operations` applies several in one transaction.",
            "inputSchema": obj(json!({"op": operation_schema()}), vec!["op"]),
        }),
        json!({
            "name": "apply_operations",
            "description": "Apply a list of operations as one transaction — the way to build a model from scratch without dozens of round trips. Same shapes and same guarantees as apply_operation; order matters (create a parent before its children, elements before the relations between them). If any operation is refused the whole list is rolled back, so the workspace is never left half-built, and one undo reverts the lot.",
            "inputSchema": obj(
                json!({"ops": {"type": "array", "minItems": 1, "items": operation_schema(),
                    "description": "operations in application order"}}),
                vec!["ops"],
            ),
        }),
        json!({
            "name": "model_format",
            "description": "The workspace file format, authoritative for this build: directory layout, every element kind and where it may nest, relations, views, docs frontmatter, deployment and groups, plus a complete minimal example. Read this before writing or repairing any YAML by hand — do not infer the schema from an existing file.",
            "inputSchema": obj(json!({}), vec![]),
        }),
        json!({
            "name": "undo",
            "description": "Undo the last transaction (shared history: canvas, MCP, and external edits alike).",
            "inputSchema": obj(json!({}), vec![]),
        }),
        json!({
            "name": "redo",
            "description": "Redo the last undone transaction.",
            "inputSchema": obj(json!({}), vec![]),
        }),
    ]
}
